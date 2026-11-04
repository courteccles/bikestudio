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
            note_default(
                diagnostics,
                format!("Frame material {} is a default.", m.label()),
            );
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
            note_default(
                diagnostics,
                format!("Seat tube length {v:.1} is a default."),
            );
            (v, ValueSource::Default)
        }
    };
    let j = &inputs.junctions;
    let junction = |value: Option<bike_model::Millimeters>,
                    default: f64,
                    name: &str,
                    diagnostics: &mut Vec<Diagnostic>| {
        value.map(|m| m.0).unwrap_or_else(|| {
            note_default(diagnostics, format!("{name} {default:.1} is a default."));
            default
        })
    };
    let top_below = junction(
        j.top_tube_below_head_tube_top,
        t.junctions.top_tube_below_head_tube_top,
        "Top tube junction below the head tube top",
        diagnostics,
    );
    let down_above = junction(
        j.down_tube_above_head_tube_bottom,
        t.junctions.down_tube_above_head_tube_bottom,
        "Down tube junction above the head tube bottom",
        diagnostics,
    );
    let overhang = junction(
        j.seat_tube_overhang,
        t.junctions.seat_tube_overhang,
        "Seat tube overhang",
        diagnostics,
    );

    let points = &geometry.static_state.points;
    let head_vec = sub(points.head_tube_top, points.head_tube_bottom);
    let head_length = norm(head_vec);
    let seat_vec = sub(points.saddle, points.seat_tube_at_crank_height);
    let seat_axis_length = norm(seat_vec);

    let mut bad = Vec::new();
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
            TubeKind::SeatTube => (seat_base, seat_tube_top),
            TubeKind::Chainstay => (chainstay_end, points.rear_axle),
            TubeKind::Seatstay => (points.rear_axle, seat_junction),
            TubeKind::BottomBracketShell => (crank, crank),
        };
        let length = if kind == TubeKind::BottomBracketShell {
            t.bottom_bracket_shell_width
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
            centre_of_mass: scale(add(start, end), 0.5),
            diameter_source,
            wall_source,
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
        top_tube_below_head_tube_top: top_below,
        down_tube_above_head_tube_bottom: down_above,
        seat_tube_top,
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

fn norm(p: Point) -> f64 {
    p.x.hypot(p.y)
}

fn note_default(diagnostics: &mut Vec<Diagnostic>, message: String) {
    diagnostics.push(Diagnostic::new(Severity::Info, "tubes.default", message));
}

fn impossible(diagnostics: &mut Vec<Diagnostic>, message: String) {
    diagnostics.push(Diagnostic::new(
        Severity::Error,
        "tubes.impossible",
        message,
    ));
}
