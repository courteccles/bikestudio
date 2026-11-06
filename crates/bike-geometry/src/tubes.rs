//! Naive tubes (S-TUBES): centrelines, round sections, mass and centre of mass.
//!
//! Tubes come from the static frame only (T8): they're rigid, so at sag they're
//! the same tubes moved. Rigid frames and hardtails get every tube; a
//! full-suspension frame gets the front triangle, and S-LINK adds the rear
//! members as links (T1).

#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::f64::consts::PI;

use bike_defaults::{Defaults, FittingPlace, MaterialEntry};
use bike_model::shape::first_crossing;
use bike_model::{
    Diagnostic, Material, Point, Project, ResolvedComponents, ResolvedDriveUnit, Severity,
    SolvedGeometry, SolvedTubes, Tube, TubeFitting, TubeKind, ValueSource,
};

use crate::{add, scale, sub};

/// Builds the frame's tubes from the solved geometry, with no drive unit.
/// Returns `None` with diagnostics when there's no geometry or the tube inputs
/// are impossible.
pub fn solve_tubes(
    project: &Project,
    geometry: Option<&SolvedGeometry>,
    defaults: &Defaults,
) -> (Option<SolvedTubes>, Vec<Diagnostic>) {
    solve_tubes_with(project, geometry, defaults, &ResolvedComponents::default())
}

/// Builds the frame's tubes with the resolved components. A drive unit
/// replaces the BB shell, and the down tube and chainstays end on its outline
/// (S-COMPONENTS C8).
pub fn solve_tubes_with(
    project: &Project,
    geometry: Option<&SolvedGeometry>,
    defaults: &Defaults,
    components: &ResolvedComponents,
) -> (Option<SolvedTubes>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let tubes = solve(
        project,
        geometry,
        defaults,
        components.drive_unit.as_ref(),
        &mut diagnostics,
    );
    (tubes, diagnostics)
}

fn solve(
    project: &Project,
    geometry: Option<&SolvedGeometry>,
    defaults: &Defaults,
    drive_unit: Option<&ResolvedDriveUnit>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<SolvedTubes> {
    let Some(geometry) = geometry else {
        diagnostics.push(Diagnostic::new(
            Severity::Info,
            "tubes.no_geometry",
            "No geometry, so there are no tubes.",
        ));
        return None;
    };
    let t = &defaults.tubes;
    let inputs = &project.tubes;

    // Material (T4).
    let (material, material_source) = match inputs.material {
        Some(m) => (m, ValueSource::Pinned),
        None => {
            let m = Material::ALL
                .into_iter()
                .find(|m| m.key() == t.default_material)
                .unwrap_or(Material::Steel);
            (m, ValueSource::Default)
        }
    };
    let Some(entry) = t.materials.get(material.key()) else {
        impossible(
            diagnostics,
            format!("No tube data for {}.", material.label()),
        );
        return None;
    };

    // Seat tube length (T2) and junctions (T3).
    let seat_tube_length = match inputs.seat_tube_length.pinned() {
        Some(v) => (v, ValueSource::Pinned),
        None => {
            let v = defaults.geometry.seat_tube_length.medium;
            (v, ValueSource::Default)
        }
    };
    let j = &inputs.junctions;
    let junction =
        |value: Option<bike_model::Millimeters>, default: f64| value.map_or(default, |m| m.0);
    let top_below = junction(
        j.top_tube_below_head_tube_top,
        t.junctions.top_tube_below_head_tube_top,
    );
    let down_above = junction(
        j.down_tube_above_head_tube_bottom,
        t.junctions.down_tube_above_head_tube_bottom,
    );
    let mut overhang = junction(j.seat_tube_overhang, t.junctions.seat_tube_overhang);
    let mut overhang_source = if j.seat_tube_overhang.is_some() {
        ValueSource::Pinned
    } else {
        ValueSource::Default
    };

    let points = &geometry.static_state.points;
    let head_vec = sub(points.head_tube_top, points.head_tube_bottom);
    let head_length = norm(head_vec);
    let seat_vec = sub(points.saddle, points.seat_tube_at_crank_height);
    let seat_axis_length = norm(seat_vec);

    let mut bad = Vec::new();

    // A pinned standover moves the top tube's seat tube junction along the
    // seat tube; the front junction and seat tube length stay (S-EDIT-CENTERS E2).
    if let Some(standover) = inputs.standover.pinned() {
        if j.seat_tube_overhang.is_some() {
            diagnostics.push(Diagnostic::new(
                Severity::Error,
                "geometry.conflict",
                "Standover and seat tube overhang are both set. Free one of them.",
            ));
        } else if seat_axis_length > 0.0 && head_length > 0.0 {
            let section = inputs
                .sections
                .get(&TubeKind::TopTube)
                .copied()
                .unwrap_or_default();
            let [default_diameter, _] =
                entry.section(TubeKind::TopTube.key()).unwrap_or([0.0, 0.0]);
            let diameter = section.outside_diameter.map_or(default_diameter, |m| m.0);
            let ground = points.rear_contact.y;
            let top_front_y = points.head_tube_top.y - head_vec.y / head_length * top_below;
            // The midpoint's centreline height, then the junction height that gives it.
            let mid_y = standover + ground - diameter / 2.0;
            let junction_y = 2.0 * mid_y - top_front_y;
            let rise = seat_vec.y / seat_axis_length;
            let up_seat = (junction_y - points.seat_tube_at_crank_height.y) / rise;
            let solved = seat_tube_length.0 - up_seat;
            let min_below = t.junctions.min_seat_tube_below_junction;
            if !(solved.is_finite() && solved >= 0.0) {
                bad.push(format!(
                    "Standover {standover:.1} puts the top tube above the top of the seat tube."
                ));
            } else if !(up_seat >= min_below) {
                bad.push(format!(
                    "Standover {standover:.1} puts the top tube less than {min_below:.1} mm up the seat tube."
                ));
            } else {
                overhang = solved;
                overhang_source = ValueSource::Solved;
            }
        }
    }
    if [top_below, down_above, overhang]
        .iter()
        .any(|v| !(v.is_finite() && *v >= 0.0))
    {
        bad.push("Junction offsets must be zero or more.".to_string());
    }
    if !(top_below + down_above < head_length) {
        bad.push(format!(
            "The top and down tube junctions ({top_below:.1} + {down_above:.1} mm) don't fit on the {head_length:.1} mm head tube."
        ));
    }
    let min_below = t.junctions.min_seat_tube_below_junction;
    if !(seat_tube_length.0 - overhang >= min_below) {
        bad.push(format!(
            "Seat tube length {:.1} must be at least the overhang {overhang:.1} plus {min_below:.1} mm.",
            seat_tube_length.0
        ));
    }
    if !(head_length > 0.0 && seat_axis_length > 0.0) {
        bad.push("The head tube or seat tube axis has no length.".to_string());
    }

    let head_axis = scale(head_vec, 1.0 / head_length);
    let seat_axis = scale(seat_vec, 1.0 / seat_axis_length);
    let seat_base = points.seat_tube_at_crank_height;
    let seat_tube_top = add(seat_base, scale(seat_axis, seat_tube_length.0));
    let seat_junction = add(seat_base, scale(seat_axis, seat_tube_length.0 - overhang));
    let top_front = sub(points.head_tube_top, scale(head_axis, top_below));
    let down_front = add(points.head_tube_bottom, scale(head_axis, down_above));
    let crank = points.crank_centre;
    let full_suspension = project.rear_travel.is_some();

    // With a drive unit, the down tube and chainstays end on its outline (C8).
    let outline: Option<Vec<Point>> =
        drive_unit.map(|d| d.outline.iter().map(|p| add(*p, crank)).collect());
    let mut bad_unit = Vec::new();
    let mut on_outline = |from: Point, tube: &str| match &outline {
        None => crank,
        Some(outline) => first_crossing(outline, from, crank).unwrap_or_else(|| {
            bad_unit.push(format!(
                "The {tube} centreline doesn't meet the drive unit outline."
            ));
            crank
        }),
    };
    let down_end = on_outline(down_front, "down tube");

    // A bent seat tube (B1–B3): with a BB shell, and the seat tube axis
    // passing more than 1 mm from the crank centre.
    let (bend_radius, bend_radius_source) = pick(
        inputs.seat_tube_bend_radius,
        t.junctions.seat_tube_bend_radius,
    );
    let shell_section = inputs
        .sections
        .get(&TubeKind::BottomBracketShell)
        .copied()
        .unwrap_or_default();
    let shell_radius = pick(
        shell_section.outside_diameter,
        entry
            .section(TubeKind::BottomBracketShell.key())
            .unwrap_or([0.0, 0.0])[0],
    )
    .0 / 2.0;
    let mut bend = None;
    let mut seat_start = seat_base;
    let mut reported_radius = bend_radius;
    let mut seat_tube_insertion = seat_tube_length.0;
    if drive_unit.is_none() && seat_axis_length > 0.0 {
        let along = dot(sub(crank, seat_base), seat_axis);
        let foot = add(seat_base, scale(seat_axis, along));
        let off = sub(crank, foot);
        let d = norm(off);
        let top_along = seat_tube_length.0 - along;
        if d <= BEND_TOLERANCE {
            // Straight: the post goes down to the shell's outline.
            let into_shell = (shell_radius * shell_radius - d * d).max(0.0).sqrt();
            seat_tube_insertion = top_along - into_shell;
        } else if !(bend_radius > 0.0) {
            bad.push(format!(
                "Seat tube bend radius {bend_radius:.1} must be positive."
            ));
        } else {
            match seat_bend(
                foot,
                seat_axis,
                scale(off, 1.0 / d),
                d,
                bend_radius,
                shell_radius,
            ) {
                Some((lowest, lowest_start)) => {
                    // B6: past vertical (leaning forward into the shell), the
                    // bend moves up to enter the shell vertically.
                    let vertical = (lowest_start.x > crank.x + 1e-9).then(|| {
                        vertical_bend(seat_base, seat_axis, crank, bend_radius, shell_radius)
                    });
                    let (b, start) = match vertical {
                        None => (lowest, lowest_start),
                        Some(Some(found)) => found,
                        Some(None) => {
                            bad.push(format!(
                                "The seat tube can't turn vertical above the BB shell with a {bend_radius:.1} mm bend radius."
                            ));
                            (lowest, lowest_start)
                        }
                    };
                    let bend_radius = b.radius;
                    let insertion = norm(sub(seat_tube_top, b.tangent));
                    let to_junction = seat_tube_length.0 - overhang;
                    let tangent_along = dot(sub(b.tangent, seat_base), seat_axis);
                    if tangent_along >= to_junction {
                        bad.push(format!(
                            "The seat tube bend ({bend_radius:.1} mm radius) starts above the top tube junction."
                        ));
                    }
                    seat_tube_insertion = insertion;
                    seat_start = start;
                    reported_radius = bend_radius;
                    bend = Some(b);
                }
                None => bad.push(format!(
                    "No seat tube bend of {bend_radius:.1} mm radius reaches the BB shell."
                )),
            }
        }
    }
    let chainstay_end = on_outline(points.rear_axle, "chainstay");

    let mut tubes = Vec::new();
    for kind in TubeKind::ALL {
        if full_suspension && kind.is_rear() {
            continue;
        }
        if drive_unit.is_some() && kind == TubeKind::BottomBracketShell {
            continue;
        }
        let (start, end) = match kind {
            TubeKind::HeadTube => (points.head_tube_bottom, points.head_tube_top),
            TubeKind::TopTube => (top_front, seat_junction),
            TubeKind::DownTube => (down_front, down_end),
            TubeKind::SeatTube => (seat_start, seat_tube_top),
            TubeKind::Chainstay => (chainstay_end, points.rear_axle),
            TubeKind::Seatstay => (points.rear_axle, seat_junction),
            TubeKind::BottomBracketShell => (crank, crank),
        };
        let bent = bend.filter(|_| kind == TubeKind::SeatTube);
        let length = if kind == TubeKind::BottomBracketShell {
            t.bottom_bracket_shell_width
        } else if let Some(b) = bent {
            norm(sub(end, b.tangent)) + b.radius * b.sweep + norm(sub(b.at(b.sweep), start))
        } else {
            norm(sub(end, start))
        };
        let section = inputs.sections.get(&kind).copied().unwrap_or_default();
        let [default_diameter, default_wall] = entry.section(kind.key()).unwrap_or([0.0, 0.0]);
        let (outside_diameter, diameter_source) = pick(section.outside_diameter, default_diameter);
        let (wall, wall_source) = pick(section.wall, default_wall);
        if !(outside_diameter > 0.0 && wall > 0.0 && wall < outside_diameter / 2.0) {
            bad.push(format!(
                "{} wall {wall:.2} must be positive and under half its {outside_diameter:.1} mm diameter.",
                kind.label()
            ));
        }
        if !(length > 0.0) {
            bad.push(format!("{} has no length.", kind.label()));
        }
        let count = kind.count();
        let mass = f64::from(count) * tube_mass(outside_diameter, wall, length, entry.density);
        tubes.push(Tube {
            kind,
            start,
            end,
            length,
            outside_diameter,
            wall,
            count,
            mass,
            centre_of_mass: bent.map_or(scale(add(start, end), 0.5), |b| {
                bent_centroid(&b, start, end)
            }),
            diameter_source,
            wall_source,
            bend: bent,
        });
    }
    if !bad_unit.is_empty() {
        for message in bad_unit {
            diagnostics.push(Diagnostic::new(
                Severity::Error,
                "drive_unit.tube_misses",
                message,
            ));
        }
        return None;
    }
    if !bad.is_empty() {
        for message in bad {
            impossible(diagnostics, message);
        }
        return None;
    }

    let tubes_mass: f64 = tubes.iter().map(|t| t.mass).sum();
    let tubes_com = weighted(tubes.iter().map(|t| (t.mass, t.centre_of_mass)));
    let mut fittings = fittings(entry, full_suspension, points, seat_tube_top, tubes_com);
    // The frame-side plate a drive unit bolts to is frame mass (C8).
    if let Some(unit) = drive_unit.filter(|d| d.unit.frame_mount_mass > 0.0) {
        fittings.push(TubeFitting {
            name: format!("{} mounting plate", unit.unit.name.model),
            mass: unit.unit.frame_mount_mass,
            position: crank,
        });
    }
    let frame_mass = tubes_mass + fittings.iter().map(|f| f.mass).sum::<f64>();
    let frame_centre_of_mass = weighted(
        tubes
            .iter()
            .map(|t| (t.mass, t.centre_of_mass))
            .chain(fittings.iter().map(|f| (f.mass, f.position))),
    );

    Some(SolvedTubes {
        material,
        material_source,
        seat_tube_length: seat_tube_length.0,
        seat_tube_length_source: seat_tube_length.1,
        seat_tube_overhang: overhang,
        seat_tube_overhang_source: overhang_source,
        top_tube_below_head_tube_top: top_below,
        down_tube_above_head_tube_bottom: down_above,
        seat_tube_top,
        seat_tube_bend_radius: reported_radius,
        seat_tube_bend_radius_source: bend_radius_source,
        seat_tube_insertion,
        tubes,
        fittings,
        frame_mass,
        frame_centre_of_mass,
    })
}

/// Grams of one round tube: length × section area × density (g/cm³) (T5).
pub fn tube_mass(outside_diameter: f64, wall: f64, length: f64, density: f64) -> f64 {
    let inside = outside_diameter - 2.0 * wall;
    let area = PI / 4.0 * (outside_diameter * outside_diameter - inside * inside);
    area * length * density / 1000.0
}

fn fittings(
    entry: &MaterialEntry,
    full_suspension: bool,
    points: &bike_model::FramePoints,
    seat_tube_top: Point,
    tubes_com: Point,
) -> Vec<TubeFitting> {
    entry
        .fittings
        .iter()
        // Rear-axle fittings belong to the rear members, which S-LINK adds for full suspension.
        .filter(|f| !(full_suspension && f.at == FittingPlace::RearAxle))
        .map(|f| TubeFitting {
            name: f.name.clone(),
            mass: f.mass,
            position: match f.at {
                FittingPlace::RearAxle => points.rear_axle,
                FittingPlace::CrankCentre => points.crank_centre,
                FittingPlace::HeadTube => {
                    scale(add(points.head_tube_top, points.head_tube_bottom), 0.5)
                }
                FittingPlace::SeatTubeTop => seat_tube_top,
                FittingPlace::Tubes => tubes_com,
            },
        })
        .collect()
}

fn pick(value: Option<bike_model::Millimeters>, default: f64) -> (f64, ValueSource) {
    match value {
        Some(m) => (m.0, ValueSource::Pinned),
        None => (default, ValueSource::Default),
    }
}

fn weighted(items: impl Iterator<Item = (f64, Point)>) -> Point {
    let (mut total, mut x, mut y) = (0.0, 0.0, 0.0);
    for (m, p) in items {
        total += m;
        x += m * p.x;
        y += m * p.y;
    }
    if total > 0.0 {
        Point::new(x / total, y / total)
    } else {
        Point::default()
    }
}

/// Within this distance of the crank centre, the seat tube axis counts as
/// passing through it and the tube stays straight (B3).
const BEND_TOLERANCE: f64 = 1.0;

/// The bend that takes a seat tube from its axis to the BB shell (B1, B2):
/// an arc of `radius`, tangent to the axis, ending on the shell (radius
/// `shell`) with the tube aimed at the crank centre, placed as low as it can
/// go. `foot` is the crank centre's foot on the axis, `up` the axis direction,
/// `toward` the unit normal from the axis to the crank centre and `d` the
/// distance. Returns the bend and the tube's start on the shell.
///
/// With the arc's centre O at `radius` from the axis on the crank's side, the
/// tangent from the shell point through the crank centre C makes a right
/// angle at the shell point, so |OC|² = radius² + shell². That fixes O's
/// height above the foot: a² = shell² + 2·radius·d − d².
fn seat_bend(
    foot: Point,
    up: Point,
    toward: Point,
    d: f64,
    radius: f64,
    shell: f64,
) -> Option<(bike_model::Bend, Point)> {
    let a2 = shell * shell + 2.0 * radius * d - d * d;
    if !(a2 >= 0.0) {
        return None;
    }
    let a = a2.sqrt();
    let tangent = add(foot, scale(up, a));
    let centre = add(tangent, scale(toward, radius));
    // From the tangent point, the arc turns by φ where
    // cos φ (radius − d) + sin φ a = radius.
    let hyp = (radius * radius + shell * shell).sqrt();
    let sweep = (a).atan2(radius - d) - (radius / hyp).acos();
    if !(sweep > 0.0 && sweep < std::f64::consts::PI) {
        return None;
    }
    let bend = bike_model::Bend {
        tangent,
        centre,
        radius,
        sweep,
        down: scale(up, -1.0),
    };
    let start = bend.at(sweep);
    Some((bend, start))
}

/// B6: the bend that turns the seat tube from its axis to the vertical
/// through the crank centre, then runs straight down to the top of the shell.
/// The arc is the fillet of `radius` between the two lines, so its place is
/// fixed; `None` when it would end below the top of the shell (a larger
/// radius only reaches lower). Returns the bend and the tube's start (the top
/// of the shell).
fn vertical_bend(
    base: Point,
    up: Point,
    crank: Point,
    radius: f64,
    shell: f64,
) -> Option<(bike_model::Bend, Point)> {
    // Where the axis crosses the vertical through the crank.
    let cross = add(base, scale(up, (crank.x - base.x) / up.x));
    let down = scale(up, -1.0);
    // The turn from `down` to straight down.
    let turn = (-down.y).clamp(-1.0, 1.0).acos();
    let top_of_shell = Point::new(crank.x, crank.y + shell);
    let tangent_length = radius * (turn / 2.0).tan();
    let tangent = add(cross, scale(up, tangent_length));
    let low = Point::new(crank.x, cross.y - tangent_length);
    if low.y < top_of_shell.y - 1e-9 {
        return None;
    }
    // The centre is `radius` from the axis, on the vertical's side.
    let mut n = Point::new(-up.y, up.x);
    if dot(sub(low, tangent), n) < 0.0 {
        n = scale(n, -1.0);
    }
    let centre = add(tangent, scale(n, radius));
    let bend = bike_model::Bend {
        tangent,
        centre,
        radius,
        sweep: turn,
        down,
    };
    Some((bend, top_of_shell))
}

/// The centre of mass of a bent tube's centreline: the straight section and
/// the arc, weighted by length.
fn bent_centroid(b: &bike_model::Bend, bottom: Point, top: Point) -> Point {
    let straight = norm(sub(top, b.tangent));
    let low = b.at(b.sweep);
    let stub = norm(sub(low, bottom));
    let arc = b.radius * b.sweep;
    let half = b.sweep / 2.0;
    // An arc's centroid lies on its bisector at R sin(h)/h from the centre.
    let mid = b.at(half);
    let k = if half > 1e-12 { half.sin() / half } else { 1.0 };
    let arc_c = add(b.centre, scale(sub(mid, b.centre), k));
    let total = straight + arc + stub;
    let weighted = add(
        add(
            scale(scale(add(top, b.tangent), 0.5), straight),
            scale(arc_c, arc),
        ),
        scale(scale(add(low, bottom), 0.5), stub),
    );
    scale(weighted, 1.0 / total)
}

fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y
}

fn norm(p: Point) -> f64 {
    p.x.hypot(p.y)
}

fn impossible(diagnostics: &mut Vec<Diagnostic>, message: String) {
    diagnostics.push(Diagnostic::new(
        Severity::Error,
        "tubes.impossible",
        message,
    ));
}
