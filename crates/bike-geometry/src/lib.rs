//! The geometry solver (S-GEOM).
//!
//! With the wheel sizes and fork known, eight frame variables are tied together
//! by three equations, leaving five free. The two seat angles are independent:
//!
//! 1. BB height = average wheel radius − BB drop (G8).
//! 2. The fork, hanging from the head tube, puts the front axle at its wheel's
//!    radius above the ground.
//! 3. Wheelbase = rear centre + front centre.
//!
//! For each equation the solver picks one free variable to calculate (G5),
//! fills the other free variables from defaults (G12), solves the three
//! equations together with Newton's method, and checks any equation left with
//! nothing free against the 1 mm tolerance (G6). It never panics (G7).
//!
//! Coordinates: origin at the crank centre, +X toward the front wheel, +Y up.

// `!(x > 0.0)` is deliberate throughout: unlike `x <= 0.0`, it also rejects NaN.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

mod tubes;

use std::collections::BTreeMap;

use bike_defaults::{DefaultBike, Defaults};
use bike_model::{
    Diagnostic, FramePoints, GeometryState, GeometryVariable as V, Point, Project, Severity,
    SolvedGeometry, ValueSource,
};

pub use tubes::{solve_tubes, tube_mass};

/// Pinned values that disagree by more than this are a conflict (G6).
pub const CONFLICT_TOLERANCE_MM: f64 = 1.0;

const EQUATION_TOLERANCE: f64 = 1e-9;
const MAX_ITERATIONS: usize = 100;

/// The variables tied together by the three equations.
const EQUATION_VARIABLES: [V; 8] = [
    V::Stack,
    V::Reach,
    V::HeadTubeAngle,
    V::HeadTubeLength,
    V::BottomBracketDrop,
    V::ChainstayLength,
    V::Wheelbase,
    V::BottomBracketHeight,
];

/// For each equation, the order in which free variables are chosen to solve it:
/// the G5 order, then any other variable in the equation, so a default never
/// causes a false conflict.
const SOLVE_ORDER: [&[V]; 3] = [
    &[V::BottomBracketHeight, V::BottomBracketDrop],
    &[
        V::HeadTubeLength,
        V::Stack,
        V::HeadTubeAngle,
        V::BottomBracketDrop,
    ],
    &[
        V::Wheelbase,
        V::Reach,
        V::Stack,
        V::HeadTubeAngle,
        V::ChainstayLength,
        V::HeadTubeLength,
        V::BottomBracketDrop,
    ],
];

/// Fixed inputs for one solve.
#[derive(Debug, Clone, Copy)]
struct Setup {
    front_radius: f64,
    rear_radius: f64,
    axle_to_crown: f64,
    offset: f64,
    crank_length: f64,
    pedal_length: f64,
    pedal_thickness: f64,
}

type Values = BTreeMap<V, f64>;

/// Solves the project's geometry. Returns `None` only when nothing sensible can
/// be drawn; the diagnostics always say why.
pub fn solve_geometry(
    project: &Project,
    defaults: &Defaults,
) -> (Option<SolvedGeometry>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let solved = solve(project, defaults, &mut diagnostics);
    (solved, diagnostics)
}

fn solve(
    project: &Project,
    defaults: &Defaults,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<SolvedGeometry> {
    // Wheels: a missing size uses the default (G12).
    let default_wheel = &defaults.geometry.default_wheel;
    let wheel_key =
        |wheel: &Option<bike_model::WheelSize>, end: &str, diagnostics: &mut Vec<Diagnostic>| {
            match wheel {
                Some(w) => w.0.clone(),
                None => {
                    diagnostics.push(Diagnostic::new(
                        Severity::Info,
                        "wheel.missing",
                        format!("No {end} wheel size set. Using the default {default_wheel}\"."),
                    ));
                    default_wheel.clone()
                }
            }
        };
    let front_key = wheel_key(&project.front_wheel, "front", diagnostics);
    let rear_key = wheel_key(&project.rear_wheel, "rear", diagnostics);
    let diameter =
        |key: &str, diagnostics: &mut Vec<Diagnostic>| match defaults.wheel_sizes.sizes.get(key) {
            Some(entry) => Some(entry.diameter),
            None => {
                diagnostics.push(Diagnostic::new(
                    Severity::Error,
                    "wheel.unknown",
                    format!("Wheel size {key} isn't in the wheel size table."),
                ));
                None
            }
        };
    let front_diameter = diameter(&front_key, diagnostics);
    let rear_diameter = diameter(&rear_key, diagnostics);
    let (front_diameter, rear_diameter) = (front_diameter?, rear_diameter?);

    // Travel: none means rigid.
    let front_travel = project.front_travel.map_or(0.0, |t| t.0);
    if !(front_travel.is_finite() && front_travel >= 0.0) {
        impossible(diagnostics, "Front travel must be zero or more.");
        return None;
    }
    let is_hardtail = front_travel > 0.0 && project.rear_travel.is_none();

    // Fork: overrides, else defaults by front wheel and travel (G3).
    let fork_entry = defaults.forks.forks.get(&front_key);
    let default_a2c = fork_entry.map(|f| f.axle_to_crown(front_travel));
    let default_offset = fork_entry.map(|f| f.offset);
    let axle_to_crown = project.fork.axle_to_crown.map(|m| m.0).or(default_a2c);
    let offset = project.fork.offset.map(|m| m.0).or(default_offset);
    let (Some(axle_to_crown), Some(offset)) = (axle_to_crown, offset) else {
        impossible(
            diagnostics,
            &format!("No default fork for {front_key}\" wheels; set axle-to-crown and offset."),
        );
        return None;
    };
    if !(axle_to_crown > 0.0 && offset.is_finite()) {
        impossible(diagnostics, "Fork axle-to-crown must be positive.");
        return None;
    }
    if project.fork.axle_to_crown.is_none() || project.fork.offset.is_none() {
        let fork = if front_travel > 0.0 {
            format!("{front_travel:.0} mm")
        } else {
            "rigid".to_string()
        };
        diagnostics.push(Diagnostic::new(
            Severity::Info,
            "fork.default",
            format!("No fork selected. Using default {fork} {front_key}\" fork."),
        ));
    }

    // Crank and pedal (G13). Medium rider until S-MASS adds the rider.
    let crank_length = project
        .crank
        .crank_length
        .map_or(defaults.geometry.cranks.medium, |c| c.0);
    let (pedal_length, pedal_thickness) = project.crank.pedal.map_or(
        (
            defaults.geometry.pedal.length,
            defaults.geometry.pedal.thickness,
        ),
        |p| (p.length.0, p.thickness.0),
    );
    if [crank_length, pedal_length, pedal_thickness]
        .iter()
        .any(|v| !(*v > 0.0))
    {
        impossible(diagnostics, "Crank length and pedal size must be positive.");
        return None;
    }

    let Some(bike) = defaults.geometry.bike_for(&front_key, front_travel) else {
        impossible(
            diagnostics,
            &format!("No default geometry for {front_key}\" wheels."),
        );
        return None;
    };

    let setup = Setup {
        front_radius: front_diameter / 2.0,
        rear_radius: rear_diameter / 2.0,
        axle_to_crown,
        offset,
        crank_length,
        pedal_length,
        pedal_thickness,
    };

    // Pins, rejecting any that can't be right.
    let mut pinned: Values = BTreeMap::new();
    for variable in V::ALL {
        if let Some(value) = project.geometry.pinned(variable) {
            if valid_pin(variable, value) {
                pinned.insert(variable, value);
            } else {
                impossible(
                    diagnostics,
                    &format!(
                        "{} {} is out of range and is ignored.",
                        variable.label(),
                        format_value(variable, value)
                    ),
                );
            }
        }
    }
    if pinned.is_empty() {
        diagnostics.push(Diagnostic::new(
            Severity::Info,
            "geometry.unpinned",
            format!("No geometry pinned yet. Showing a default bike for {front_key}\" wheels."),
        ));
    }

    // Choose what each equation solves for (G5).
    let mut unknowns: Vec<(usize, V)> = Vec::new();
    let mut checks: Vec<usize> = Vec::new();
    for (equation, order) in SOLVE_ORDER.iter().enumerate() {
        let choice = order
            .iter()
            .copied()
            .find(|v| !pinned.contains_key(v) && !unknowns.iter().any(|(_, u)| u == v));
        match choice {
            Some(v) => unknowns.push((equation, v)),
            None => checks.push(equation),
        }
    }

    // Everything else that's free comes from the defaults (G12); unknowns start there too.
    let mut values: Values = BTreeMap::new();
    let mut sources = BTreeMap::new();
    for variable in V::ALL {
        if let Some(value) = pinned.get(&variable) {
            values.insert(variable, *value);
            sources.insert(variable, ValueSource::Pinned);
        } else {
            values.insert(variable, default_value(bike, variable));
            let solved = unknowns.iter().any(|(_, u)| *u == variable);
            sources.insert(
                variable,
                if solved {
                    ValueSource::Solved
                } else {
                    ValueSource::Default
                },
            );
        }
    }

    if !newton(&mut values, &unknowns, &setup) {
        impossible(
            diagnostics,
            "These values can't make a bike: no frame satisfies them. Check the chainstay against the BB drop, and the head tube against the fork.",
        );
        return None;
    }

    // Equations with nothing free: the pinned values must agree (G6).
    if let Some(r) = residuals(&values, &setup) {
        for equation in checks {
            if r[equation].abs() > CONFLICT_TOLERANCE_MM {
                let names: Vec<&str> = EQUATION_VARIABLES
                    .iter()
                    .filter(|v| in_equation(equation, **v) && sources[*v] == ValueSource::Pinned)
                    .map(|v| v.label())
                    .collect();
                diagnostics.push(Diagnostic::new(
                    Severity::Error,
                    "geometry.conflict",
                    format!(
                        "{} disagree by {:.1} mm. Free one of them.",
                        join(&names),
                        r[equation].abs()
                    ),
                ));
            }
        }
    }

    for variable in V::ALL {
        if sources[&variable] == ValueSource::Default {
            diagnostics.push(Diagnostic::new(
                Severity::Info,
                "geometry.default",
                format!(
                    "{} {} is a default.",
                    variable.label(),
                    format_value(variable, values[&variable])
                ),
            ));
        }
    }

    // Solved values must make physical sense.
    let bad: Vec<&str> = [
        V::HeadTubeLength,
        V::ChainstayLength,
        V::Wheelbase,
        V::BottomBracketHeight,
        V::Stack,
    ]
    .iter()
    .filter(|v| !(values[*v] > 0.0))
    .map(|v| v.label())
    .chain(
        (!(values[&V::HeadTubeAngle] > 0.0 && values[&V::HeadTubeAngle] < 90.0))
            .then_some("Head angle"),
    )
    .collect();
    if !bad.is_empty() {
        impossible(
            diagnostics,
            &format!(
                "These values would need an impossible {}.",
                bad.iter()
                    .map(|n| in_sentence(n))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
        return None;
    }

    let Some(static_state) = state(&values, &setup, 0.0) else {
        impossible(
            diagnostics,
            "The chainstay is too short to reach the rear axle.",
        );
        return None;
    };
    let (at_sag, full_travel) = if is_hardtail {
        (
            state(&values, &setup, bike.fork_sag * front_travel),
            state(&values, &setup, front_travel),
        )
    } else {
        (None, None)
    };

    for (name, s) in [
        ("static", Some(&static_state)),
        ("at sag", at_sag.as_ref()),
        ("at full travel", full_travel.as_ref()),
    ] {
        let Some(s) = s else { continue };
        if s.pedal_ground_clearance <= 0.0 {
            diagnostics.push(Diagnostic::new(
                Severity::Warning,
                "pedal.ground",
                format!(
                    "The pedal hits the ground {name} ({:.1} mm).",
                    s.pedal_ground_clearance
                ),
            ));
        }
        if s.pedal_front_tyre_clearance <= 0.0 {
            diagnostics.push(Diagnostic::new(
                Severity::Warning,
                "pedal.front_tyre",
                format!(
                    "The pedal hits the front tyre {name} ({:.1} mm).",
                    s.pedal_front_tyre_clearance
                ),
            ));
        }
    }

    Some(SolvedGeometry {
        sources,
        front_wheel_diameter: front_diameter,
        rear_wheel_diameter: rear_diameter,
        fork_axle_to_crown: axle_to_crown,
        fork_offset: offset,
        crank_length,
        pedal_length,
        pedal_thickness,
        static_state,
        at_sag,
        full_travel,
    })
}

fn impossible(diagnostics: &mut Vec<Diagnostic>, message: &str) {
    diagnostics.push(Diagnostic::new(
        Severity::Error,
        "geometry.impossible",
        message,
    ));
}

fn valid_pin(variable: V, value: f64) -> bool {
    if !value.is_finite() {
        return false;
    }
    match variable {
        _ if variable.is_angle() => value > 0.0 && value < 90.0,
        V::BottomBracketDrop => value.abs() < 1000.0,
        _ => value > 0.0 && value < 10_000.0,
    }
}

fn default_value(bike: &DefaultBike, variable: V) -> f64 {
    match variable {
        V::Stack => bike.stack,
        V::Reach => bike.reach,
        V::HeadTubeAngle => bike.head_tube_angle,
        V::HeadTubeLength => bike.head_tube_length,
        V::EffectiveSeatTubeAngle => bike.effective_seat_tube_angle,
        V::ActualSeatTubeAngle => bike.actual_seat_tube_angle,
        V::BottomBracketDrop => bike.bottom_bracket_drop,
        V::ChainstayLength => bike.chainstay_length,
        V::Wheelbase => bike.wheelbase,
        V::BottomBracketHeight => bike.bottom_bracket_height,
    }
}

fn in_equation(equation: usize, variable: V) -> bool {
    SOLVE_ORDER[equation].contains(&variable)
}

/// The three equation residuals, in millimetres. `None` when the chainstay
/// can't reach the rear axle or the head angle is out of range.
fn residuals(v: &Values, s: &Setup) -> Option<[f64; 3]> {
    let hta = v[&V::HeadTubeAngle];
    if !(hta > 0.0 && hta < 90.0) {
        return None;
    }
    let front = front_axle(v, s, 0.0);
    let drop = v[&V::BottomBracketDrop];
    let bb_height = v[&V::BottomBracketHeight];
    let rear_axle_height = s.rear_radius - bb_height;
    let cs = v[&V::ChainstayLength];
    let rear_centre_sq = cs * cs - rear_axle_height * rear_axle_height;
    if !(rear_centre_sq > 0.0) {
        return None;
    }
    Some([
        bb_height - ((s.front_radius + s.rear_radius) / 2.0 - drop),
        front.y - ((s.front_radius - s.rear_radius) / 2.0 + drop),
        v[&V::Wheelbase] - (front.x + rear_centre_sq.sqrt()),
    ])
}

/// The front axle in the frame's static coordinates, with the fork compressed by `compression`.
fn front_axle(v: &Values, s: &Setup, compression: f64) -> Point {
    let h = v[&V::HeadTubeAngle].to_radians();
    let along = v[&V::HeadTubeLength] + s.axle_to_crown - compression;
    Point::new(
        v[&V::Reach] + along * h.cos() + s.offset * h.sin(),
        v[&V::Stack] - along * h.sin() + s.offset * h.cos(),
    )
}

/// Solves the chosen unknowns with damped Newton steps. Returns false if it can't.
fn newton(values: &mut Values, unknowns: &[(usize, V)], s: &Setup) -> bool {
    let norm = |v: &Values| -> Option<f64> {
        let r = residuals(v, s)?;
        Some(
            unknowns
                .iter()
                .map(|(e, _)| r[*e] * r[*e])
                .sum::<f64>()
                .sqrt(),
        )
    };
    if unknowns.is_empty() {
        return residuals(values, s).is_some();
    }
    let Some(mut current) = norm(values) else {
        return false;
    };
    for _ in 0..MAX_ITERATIONS {
        if current < EQUATION_TOLERANCE {
            return true;
        }
        let n = unknowns.len();
        let r = residuals(values, s).expect("checked by norm");
        let mut jacobian = vec![vec![0.0; n]; n];
        for (col, (_, var)) in unknowns.iter().enumerate() {
            let x = values[var];
            let h = 1e-6 * x.abs().max(1.0);
            let mut probe = values.clone();
            probe.insert(*var, x + h);
            let Some(rp) = residuals(&probe, s) else {
                return false;
            };
            for (row, (eq, _)) in unknowns.iter().enumerate() {
                jacobian[row][col] = (rp[*eq] - r[*eq]) / h;
            }
        }
        let rhs: Vec<f64> = unknowns.iter().map(|(eq, _)| -r[*eq]).collect();
        let Some(step) = solve_linear(jacobian, rhs) else {
            return false;
        };
        let mut t = 1.0;
        loop {
            let mut trial = values.clone();
            for ((_, var), dx) in unknowns.iter().zip(&step) {
                trial.insert(*var, values[var] + t * dx);
            }
            if let Some(next) = norm(&trial)
                && next < current
            {
                *values = trial;
                current = next;
                break;
            }
            t /= 2.0;
            if t < 1e-6 {
                return current < 1e-6;
            }
        }
    }
    current < 1e-6
}

/// Gaussian elimination with partial pivoting. `None` if singular.
fn solve_linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n).max_by(|i, j| a[*i][col].abs().total_cmp(&a[*j][col].abs()))?;
        if !(a[pivot][col].abs() > 1e-12) {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        let (upper, lower) = a.split_at_mut(col + 1);
        let pivot_row = &upper[col];
        for (offset, row) in lower.iter_mut().enumerate() {
            let factor = row[col] / pivot_row[col];
            for (value, pivot_value) in row.iter_mut().zip(pivot_row).skip(col) {
                *value -= factor * pivot_value;
            }
            b[col + 1 + offset] -= factor * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let sum: f64 = (row + 1..n).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - sum) / a[row][row];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

/// The bike with the fork compressed by `compression` (0 = static), pitched so
/// both tyres touch the ground, re-expressed with the crank centre at the origin.
fn state(v: &Values, s: &Setup, compression: f64) -> Option<GeometryState> {
    let h = v[&V::HeadTubeAngle].to_radians();
    let axis = Point::new(h.cos(), -h.sin()); // down the steering axis
    let stack = v[&V::Stack];
    let reach = v[&V::Reach];
    let head_top = Point::new(reach, stack);
    let head_bottom = add(head_top, scale(axis, v[&V::HeadTubeLength]));

    let rear_axle_height = s.rear_radius - v[&V::BottomBracketHeight];
    let cs = v[&V::ChainstayLength];
    let rear_centre_sq = cs * cs - rear_axle_height * rear_axle_height;
    if !(rear_centre_sq > 0.0) {
        return None;
    }
    let rear = Point::new(-rear_centre_sq.sqrt(), rear_axle_height);
    let front = front_axle(v, s, compression);

    // Seat (G4): saddle on the effective line at stack height; seat tube through it.
    let effective = v[&V::EffectiveSeatTubeAngle].to_radians();
    let actual = v[&V::ActualSeatTubeAngle].to_radians();
    let saddle = Point::new(-stack / effective.tan(), stack);
    let seat_axis = Point::new(actual.cos(), -actual.sin()); // down the seat tube
    let seat_at_crank = Point::new(saddle.x + stack / actual.tan(), 0.0);

    // Pitch about the rear axle until the ground touches both tyres.
    let to_front = sub(front, rear);
    let length = (to_front.x * to_front.x + to_front.y * to_front.y).sqrt();
    let rise = (s.front_radius - s.rear_radius) / length;
    if !(rise.abs() < 1.0) {
        return None;
    }
    let theta = rise.asin() - to_front.y.atan2(to_front.x);
    let place = |p: Point| rotate_about(p, rear, theta);
    let crank = place(Point::default());
    let at = |p: Point| sub(place(p), crank);
    let turn = |d: Point| rotate(d, theta);

    let rear = at(rear);
    let front = at(front);
    let head_top = at(head_top);
    let head_bottom = at(head_bottom);
    let saddle = at(saddle);
    let seat_at_crank = at(seat_at_crank);
    let axis = turn(axis);
    let seat_axis = turn(seat_axis);
    let ground = rear.y - s.rear_radius;

    let bb_height = -ground;
    let head_angle = (-axis.y).atan2(axis.x).to_degrees();
    let to_ground = (ground - head_top.y) / axis.y;
    let trail = head_top.x + to_ground * axis.x - front.x;

    let pedal_down = Point::new(0.0, -s.crank_length);
    let pedal_forward = Point::new(s.crank_length, 0.0);
    let pedal_ground_clearance = bb_height - s.crank_length - s.pedal_thickness / 2.0;
    let pedal_front_tyre_clearance = distance_to_box(
        front,
        pedal_forward,
        s.pedal_length / 2.0,
        s.pedal_thickness / 2.0,
    ) - s.front_radius;

    let values = BTreeMap::from([
        (V::Stack, head_top.y),
        (V::Reach, head_top.x),
        (V::HeadTubeAngle, head_angle),
        (V::HeadTubeLength, v[&V::HeadTubeLength]),
        (
            V::EffectiveSeatTubeAngle,
            saddle.y.atan2(-saddle.x).to_degrees(),
        ),
        (
            V::ActualSeatTubeAngle,
            (-seat_axis.y).atan2(seat_axis.x).to_degrees(),
        ),
        (V::BottomBracketDrop, (rear.y + front.y) / 2.0),
        (V::ChainstayLength, cs),
        (V::Wheelbase, front.x - rear.x),
        (V::BottomBracketHeight, bb_height),
    ]);

    let state = GeometryState {
        values,
        trail,
        front_centre: front.x,
        rear_centre: -rear.x,
        points: FramePoints {
            crank_centre: Point::default(),
            rear_axle: rear,
            front_axle: front,
            rear_contact: Point::new(rear.x, ground),
            front_contact: Point::new(front.x, ground),
            head_tube_top: head_top,
            head_tube_bottom: head_bottom,
            saddle,
            seat_tube_at_crank_height: seat_at_crank,
            pedal_down,
            pedal_forward,
        },
        pedal_ground_clearance,
        pedal_front_tyre_clearance,
    };
    let finite = state.values.values().all(|x| x.is_finite())
        && state.trail.is_finite()
        && state.pedal_front_tyre_clearance.is_finite();
    finite.then_some(state)
}

fn add(a: Point, b: Point) -> Point {
    Point::new(a.x + b.x, a.y + b.y)
}

fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}

fn scale(a: Point, k: f64) -> Point {
    Point::new(a.x * k, a.y * k)
}

fn rotate(p: Point, theta: f64) -> Point {
    let (sin, cos) = theta.sin_cos();
    Point::new(p.x * cos - p.y * sin, p.x * sin + p.y * cos)
}

fn rotate_about(p: Point, centre: Point, theta: f64) -> Point {
    add(centre, rotate(sub(p, centre), theta))
}

/// Distance from `p` to an axis-aligned box centred on `centre`.
fn distance_to_box(p: Point, centre: Point, half_width: f64, half_height: f64) -> f64 {
    let dx = ((p.x - centre.x).abs() - half_width).max(0.0);
    let dy = ((p.y - centre.y).abs() - half_height).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

fn format_value(variable: V, value: f64) -> String {
    if variable.is_angle() {
        format!("{value:.1}°")
    } else {
        format!("{value:.1} mm")
    }
}

/// "Stack, reach and BB drop": the first name capitalised, the rest in running text.
fn join(names: &[&str]) -> String {
    let lower: Vec<String> = names.iter().map(|n| in_sentence(n)).collect();
    let text = match lower.as_slice() {
        [] => "the pinned values".to_string(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    };
    let mut chars = text.chars();
    chars
        .next()
        .map_or(text.clone(), |c| c.to_uppercase().chain(chars).collect())
}

/// A label as it reads mid-sentence; abbreviations like "BB" keep their capitals.
fn in_sentence(label: &str) -> String {
    if label.starts_with("BB") {
        label.to_string()
    } else {
        label.to_lowercase()
    }
}
