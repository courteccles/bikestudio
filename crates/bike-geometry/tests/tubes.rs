//! S-TUBES "Done when": tubes by frame type, placement, mass by hand, diagnostics, robustness.

use std::f64::consts::PI;

use bike_defaults::Defaults;
use bike_geometry::{solve_geometry, solve_tubes};
use bike_model::{
    Diagnostic, GeometryVariable as V, JunctionOffsets, Material, Millimeters, Parameter, Point,
    Project, Severity, SolvedTubes, TubeKind, TubeSection, ValueSource, WheelSize,
};
use proptest::prelude::*;

const EPS: f64 = 1e-6;

fn defaults() -> Defaults {
    Defaults::builtin().unwrap()
}

fn has(diagnostics: &[Diagnostic], code: &str) -> bool {
    diagnostics.iter().any(|d| d.code == code)
}

/// A rigid 29" frame: stack 620, reach 450, head 70°, chainstay 430, BB drop 70,
/// effective and actual seat angles 74°, seat tube 500, junctions 25 / 35 / 50.
fn rigid() -> Project {
    let mut p = Project {
        front_wheel: Some(WheelSize("29".into())),
        rear_wheel: Some(WheelSize("29".into())),
        ..Project::default()
    };
    let g = &mut p.geometry;
    g.pin(V::Stack, 620.0);
    g.pin(V::Reach, 450.0);
    g.pin(V::HeadTubeAngle, 70.0);
    g.pin(V::ChainstayLength, 430.0);
    g.pin(V::BottomBracketDrop, 70.0);
    g.pin(V::EffectiveSeatTubeAngle, 74.0);
    g.pin(V::ActualSeatTubeAngle, 74.0);
    p.tubes.material = Some(Material::Steel);
    p.tubes.seat_tube_length = Parameter::Pinned(500.0);
    p.tubes.junctions = JunctionOffsets {
        top_tube_below_head_tube_top: Some(Millimeters(25.0)),
        down_tube_above_head_tube_bottom: Some(Millimeters(35.0)),
        seat_tube_overhang: Some(Millimeters(50.0)),
    };
    p
}

fn solve(project: &Project) -> (Option<SolvedTubes>, Vec<Diagnostic>) {
    let d = defaults();
    let (geometry, _) = solve_geometry(project, &d);
    solve_tubes(project, geometry.as_ref(), &d)
}

fn close(a: Point, b: Point, tolerance: f64) -> bool {
    (a.x - b.x).abs() < tolerance && (a.y - b.y).abs() < tolerance
}

fn kinds(t: &SolvedTubes) -> Vec<TubeKind> {
    t.tubes.iter().map(|t| t.kind).collect()
}

#[test]
fn rigid_and_hardtail_get_every_tube_full_suspension_the_front_triangle() {
    let (rigid_tubes, _) = solve(&rigid());
    assert_eq!(kinds(&rigid_tubes.unwrap()), TubeKind::ALL.to_vec());

    let mut hardtail = rigid();
    hardtail.front_travel = Some(Millimeters(120.0));
    assert_eq!(kinds(&solve(&hardtail).0.unwrap()), TubeKind::ALL.to_vec());

    let mut full = hardtail.clone();
    full.rear_travel = Some(Millimeters(140.0));
    let tubes = solve(&full).0.unwrap();
    assert_eq!(
        kinds(&tubes),
        vec![
            TubeKind::HeadTube,
            TubeKind::TopTube,
            TubeKind::DownTube,
            TubeKind::SeatTube,
            TubeKind::BottomBracketShell
        ]
    );
    assert!(tubes.frame_mass > 0.0);
    assert!(tubes.fittings.iter().all(|f| f.name != "Dropouts"));
}

/// Tube ends worked out by hand from the pinned angles, not from the solver's points.
#[test]
fn tube_ends_match_hand_calculation() {
    let project = rigid();
    let d = defaults();
    let (geometry, _) = solve_geometry(&project, &d);
    let geometry = geometry.unwrap();
    let tubes = solve_tubes(&project, Some(&geometry), &d).0.unwrap();
    let rad = f64::to_radians;

    // Seat tube (S-GEOM G4): saddle at stack height on the 74° effective line,
    // seat tube axis through it at 74° actual, so it passes through the crank centre.
    let saddle = Point::new(-620.0 / rad(74.0).tan(), 620.0);
    let base = Point::new(saddle.x + 620.0 / rad(74.0).tan(), 0.0);
    assert!(close(base, Point::new(0.0, 0.0), 1e-9));
    let up = Point::new(-rad(74.0).cos(), rad(74.0).sin());
    let seat_top = Point::new(500.0 * up.x, 500.0 * up.y);
    let junction = Point::new(450.0 * up.x, 450.0 * up.y);

    // Head tube: top at (reach, stack), axis at 70°, length as solved.
    let htl = geometry.static_state.value(V::HeadTubeLength);
    let head_up = Point::new(-rad(70.0).cos(), rad(70.0).sin());
    let ht_top = Point::new(450.0, 620.0);
    let ht_bottom = Point::new(450.0 - htl * head_up.x, 620.0 - htl * head_up.y);
    let tt_front = Point::new(450.0 - 25.0 * head_up.x, 620.0 - 25.0 * head_up.y);
    let dt_front = Point::new(
        ht_bottom.x + 35.0 * head_up.x,
        ht_bottom.y + 35.0 * head_up.y,
    );

    // Rear axle: 70 mm above the crank centre (equal wheels), 430 mm away.
    let axle = Point::new(-(430.0f64.powi(2) - 70.0f64.powi(2)).sqrt(), 70.0);

    let check = |kind, start: Point, end: Point| {
        let t = tubes.tube(kind).unwrap();
        assert!(
            close(t.start, start, 0.001),
            "{kind:?} start {:?} vs {start:?}",
            t.start
        );
        assert!(
            close(t.end, end, 0.001),
            "{kind:?} end {:?} vs {end:?}",
            t.end
        );
    };
    check(TubeKind::SeatTube, base, seat_top);
    check(TubeKind::HeadTube, ht_bottom, ht_top);
    check(TubeKind::TopTube, tt_front, junction);
    check(TubeKind::DownTube, dt_front, Point::new(0.0, 0.0));
    check(TubeKind::Chainstay, Point::new(0.0, 0.0), axle);
    check(TubeKind::Seatstay, axle, junction);
    assert!(close(tubes.seat_tube_top, seat_top, 0.001));
    assert!((tubes.tube(TubeKind::SeatTube).unwrap().length - 500.0).abs() < 0.001);
    assert!((tubes.tube(TubeKind::Chainstay).unwrap().length - 430.0).abs() < 0.001);
}

/// Steel by hand: area = π/4 (D² − d²), mass = area × length × 7.85 / 1000 grams.
#[test]
fn steel_mass_matches_hand_calculation() {
    let tubes = solve(&rigid()).0.unwrap();
    let ring = |od: f64, wall: f64| PI / 4.0 * (od * od - (od - 2.0 * wall).powi(2));
    let grams = |od, wall, length: f64, count: f64| count * ring(od, wall) * length * 7.85 / 1000.0;

    // Seat tube 31.8 × 0.9, 500 mm: π/4 (31.8² − 30.0²) = 87.37 mm², × 500 × 7.85e-3 = 342.9 g.
    let seat = tubes.tube(TubeKind::SeatTube).unwrap();
    assert!((seat.mass - 342.94).abs() < 0.05, "{}", seat.mass);
    // Chainstays 22.2 × 0.8, 430 mm, a pair: π/4 (22.2² − 20.6²) = 53.79 mm² → 2 × 181.6 g.
    let cs = tubes.tube(TubeKind::Chainstay).unwrap();
    assert!((cs.mass - 363.13).abs() < 0.05, "{}", cs.mass);
    // BB shell 40 × 2.5, 73 mm wide: π/4 (40² − 35²) = 294.52 mm² → 168.8 g.
    let bb = tubes.tube(TubeKind::BottomBracketShell).unwrap();
    assert!((bb.mass - 168.77).abs() < 0.05, "{}", bb.mass);

    let sections = [
        (TubeKind::HeadTube, 44.5, 1.1, 1.0),
        (TubeKind::TopTube, 31.8, 0.8, 1.0),
        (TubeKind::DownTube, 38.1, 0.9, 1.0),
        (TubeKind::SeatTube, 31.8, 0.9, 1.0),
        (TubeKind::Chainstay, 22.2, 0.8, 2.0),
        (TubeKind::Seatstay, 16.0, 0.8, 2.0),
        (TubeKind::BottomBracketShell, 40.0, 2.5, 1.0),
    ];
    let mut total = 0.0;
    let (mut mx, mut my) = (0.0, 0.0);
    for (kind, od, wall, count) in sections {
        let t = tubes.tube(kind).unwrap();
        let m = grams(od, wall, t.length, count);
        assert!((t.mass - m).abs() < 1e-6, "{kind:?}");
        let mid = Point::new((t.start.x + t.end.x) / 2.0, (t.start.y + t.end.y) / 2.0);
        assert!(close(t.centre_of_mass, mid, EPS));
        total += m;
        mx += m * mid.x;
        my += m * mid.y;
    }
    // Fittings: dropouts 220 g at the rear axle, 60 + 60 g at the tubes' centre of mass.
    let tubes_com = Point::new(mx / total, my / total);
    let axle = tubes.tube(TubeKind::Chainstay).unwrap().end;
    let frame = total + 340.0;
    let com = Point::new(
        (mx + 220.0 * axle.x + 120.0 * tubes_com.x) / frame,
        (my + 220.0 * axle.y + 120.0 * tubes_com.y) / frame,
    );
    assert!((tubes.frame_mass - frame).abs() < 1e-6);
    assert!(close(tubes.frame_centre_of_mass, com, EPS));
    // A plausible steel frame.
    assert!(
        (1500.0..3000.0).contains(&tubes.frame_mass),
        "{}",
        tubes.frame_mass
    );
}

/// Every material: frame mass = Σ tubes by the hand formula + its fittings.
#[test]
fn every_material_mass_matches_hand_calculation() {
    let d = defaults();
    for material in Material::ALL {
        let mut p = rigid();
        p.tubes.material = Some(material);
        let tubes = solve(&p).0.unwrap();
        let entry = &d.tubes.materials[material.key()];
        let mut expected = entry.fittings.iter().map(|f| f.mass).sum::<f64>();
        for t in &tubes.tubes {
            let [od, wall] = entry.section(t.kind.key()).unwrap();
            let area = PI / 4.0 * (od * od - (od - 2.0 * wall).powi(2));
            expected += f64::from(t.kind.count()) * area * t.length * entry.density / 1000.0;
        }
        assert!((tubes.frame_mass - expected).abs() < 1e-6, "{material:?}");
        assert!(
            (800.0..3500.0).contains(&tubes.frame_mass),
            "{material:?} {}",
            tubes.frame_mass
        );
    }
}

#[test]
fn overrides_replace_defaults_and_are_marked_pinned() {
    let mut p = rigid();
    p.tubes.sections.insert(
        TubeKind::DownTube,
        TubeSection {
            outside_diameter: Some(Millimeters(44.5)),
            wall: None,
        },
    );
    let tubes = solve(&p).0.unwrap();
    let dt = tubes.tube(TubeKind::DownTube).unwrap();
    assert_eq!(dt.outside_diameter, 44.5);
    assert_eq!(dt.diameter_source, ValueSource::Pinned);
    assert_eq!(dt.wall, 0.9);
    assert_eq!(dt.wall_source, ValueSource::Default);
}

#[test]
fn defaults_are_labelled() {
    let mut p = rigid();
    p.tubes = Default::default();
    let (tubes, diagnostics) = solve(&p);
    let tubes = tubes.unwrap();
    assert_eq!(tubes.material, Material::Steel);
    assert_eq!(tubes.material_source, ValueSource::Default);
    assert_eq!(tubes.seat_tube_length, 450.0);
    assert_eq!(tubes.seat_tube_length_source, ValueSource::Default);
    assert_eq!(tubes.seat_tube_overhang, 60.0);
    let count = diagnostics
        .iter()
        .filter(|d| d.code == "tubes.default")
        .count();
    assert_eq!(count, 0, "defaults aren't reported: {diagnostics:?}");
}

#[test]
fn empty_project_and_no_geometry() {
    let (tubes, diagnostics) = solve(&Project::default());
    assert!(tubes.is_some(), "{diagnostics:?}");
    let (tubes, diagnostics) = solve_tubes(&Project::default(), None, &defaults());
    assert!(tubes.is_none());
    assert!(has(&diagnostics, "tubes.no_geometry"));
}

#[test]
fn impossible_tubes_are_errors() {
    type Case = Box<dyn Fn(&mut Project)>;
    let cases: Vec<Case> = vec![
        Box::new(|p| {
            p.tubes.sections.insert(
                TubeKind::TopTube,
                TubeSection {
                    outside_diameter: Some(Millimeters(30.0)),
                    wall: Some(Millimeters(15.0)),
                },
            );
        }),
        Box::new(|p| {
            p.tubes.sections.insert(
                TubeKind::Seatstay,
                TubeSection {
                    outside_diameter: Some(Millimeters(-1.0)),
                    wall: None,
                },
            );
        }),
        Box::new(|p| p.tubes.seat_tube_length = Parameter::Pinned(80.0)),
        Box::new(|p| p.tubes.seat_tube_length = Parameter::Pinned(f64::NAN)),
        Box::new(|p| p.tubes.junctions.seat_tube_overhang = Some(Millimeters(-5.0))),
        Box::new(|p| p.tubes.junctions.top_tube_below_head_tube_top = Some(Millimeters(400.0))),
    ];
    for (i, case) in cases.iter().enumerate() {
        let mut p = rigid();
        case(&mut p);
        let (tubes, diagnostics) = solve(&p);
        assert!(tubes.is_none(), "case {i}");
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == "tubes.impossible" && d.severity == Severity::Error),
            "case {i}: {diagnostics:?}"
        );
    }
}

fn arbitrary_kind() -> impl Strategy<Value = TubeKind> {
    proptest::sample::select(TubeKind::ALL.to_vec())
}

fn arbitrary_variable() -> impl Strategy<Value = V> {
    proptest::sample::select(V::ALL.to_vec())
}

proptest! {
    #[test]
    fn random_tube_inputs_never_panic(
        pins in proptest::collection::vec((arbitrary_variable(), -2000.0f64..3000.0), 0..10),
        seat in proptest::option::of(-500.0f64..1500.0),
        overhang in proptest::option::of(-100.0f64..600.0),
        top in proptest::option::of(-100.0f64..400.0),
        sections in proptest::collection::vec(
            (arbitrary_kind(), proptest::option::of(-10.0f64..80.0), proptest::option::of(-1.0f64..40.0)),
            0..7,
        ),
        rear in proptest::option::of(0.0f64..250.0),
    ) {
        let mut p = Project { rear_travel: rear.map(Millimeters), ..Project::default() };
        for (v, value) in pins {
            p.geometry.pin(v, value);
        }
        if let Some(s) = seat {
            p.tubes.seat_tube_length = Parameter::Pinned(s);
        }
        p.tubes.junctions.seat_tube_overhang = overhang.map(Millimeters);
        p.tubes.junctions.top_tube_below_head_tube_top = top.map(Millimeters);
        for (kind, od, wall) in sections {
            p.tubes.sections.insert(kind, TubeSection {
                outside_diameter: od.map(Millimeters),
                wall: wall.map(Millimeters),
            });
        }
        let (tubes, diagnostics) = solve(&p);
        if let Some(tubes) = tubes {
            prop_assert!(tubes.frame_mass.is_finite() && tubes.frame_mass > 0.0);
            prop_assert!(tubes.tubes.iter().all(|t| t.length.is_finite() && t.mass > 0.0));
        } else {
            prop_assert!(diagnostics.iter().any(|d| d.severity == Severity::Error
                || d.code == "tubes.no_geometry"));
        }
    }
}

fn standover(t: &SolvedTubes, ground: f64) -> f64 {
    let top = t.tube(TubeKind::TopTube).unwrap();
    (top.start.y + top.end.y) / 2.0 + top.outside_diameter / 2.0 - ground
}

/// S-EDIT-CENTERS E2: a pinned standover moves the seat tube junction only.
#[test]
fn pinned_standover_moves_the_seat_tube_junction() {
    let d = defaults();
    let base = rigid();
    let ground = solve_geometry(&base, &d)
        .0
        .unwrap()
        .static_state
        .points
        .rear_contact
        .y;
    let before = solve(&base).0.unwrap();
    let mut p = base.clone();
    p.tubes.junctions.seat_tube_overhang = None;
    p.tubes.standover = Parameter::Pinned(standover(&before, ground) - 20.0);
    let (after, diagnostics) = solve(&p);
    let after = after.unwrap_or_else(|| panic!("{diagnostics:?}"));
    assert!((standover(&after, ground) - (standover(&before, ground) - 20.0)).abs() < 0.1);
    assert_eq!(after.seat_tube_length, 500.0);
    assert_eq!(after.seat_tube_overhang_source, ValueSource::Solved);
    assert!(after.seat_tube_overhang > 50.0);
    let (b, a) = (
        before.tube(TubeKind::TopTube).unwrap(),
        after.tube(TubeKind::TopTube).unwrap(),
    );
    assert!(close(a.start, b.start, EPS), "front junction stays");
    assert!(a.length != b.length);
    assert_eq!(
        after.tube(TubeKind::SeatTube).unwrap().length,
        before.tube(TubeKind::SeatTube).unwrap().length
    );

    // With the overhang set too: a conflict.
    let mut both = p.clone();
    both.tubes.junctions.seat_tube_overhang = Some(Millimeters(50.0));
    assert!(has(&solve(&both).1, "geometry.conflict"));

    // Above the seat tube, or too low on it: impossible.
    for delta in [400.0, -600.0] {
        let mut out = p.clone();
        out.tubes.standover = Parameter::Pinned(standover(&before, ground) + delta);
        let (t, diagnostics) = solve(&out);
        assert!(
            t.is_none() && has(&diagnostics, "tubes.impossible"),
            "{delta} {diagnostics:?}"
        );
    }
}

/// The rigid frame with its actual seat angle slacker than the effective, so
/// the seat tube axis passes in front of the crank centre. A gentle bend, so
/// the lowest bend doesn't lean past vertical (B6).
fn offset_seat() -> Project {
    let mut p = rigid();
    p.geometry.pin(V::EffectiveSeatTubeAngle, 75.0);
    p.geometry.pin(V::ActualSeatTubeAngle, 74.0);
    p.tubes.seat_tube_bend_radius = Some(Millimeters(OFFSET_RADIUS));
    p
}

const OFFSET_RADIUS: f64 = 400.0;

/// S-TUBES B1–B4: a bent seat tube leaves its axis tangentially, ends on the
/// BB shell aimed at the crank centre, and keeps its chart length.
#[test]
fn offset_seat_tube_bends_to_the_shell() {
    let (t, diagnostics) = solve(&offset_seat());
    let t = t.unwrap_or_else(|| panic!("{diagnostics:?}"));
    let seat = t.tube(TubeKind::SeatTube).unwrap();
    let shell = t.tube(TubeKind::BottomBracketShell).unwrap();
    let b = seat.bend.expect("bent");
    let r = shell.outside_diameter / 2.0;
    let crank = shell.start;
    let dist = |a: Point, c: Point| (a.x - c.x).hypot(a.y - c.y);

    // B1: on the arc, starting on the shell, aimed at the crank centre.
    assert!((dist(seat.start, b.centre) - b.radius).abs() < EPS);
    assert!((dist(seat.start, crank) - r).abs() < EPS);
    let radial = (seat.start.x - b.centre.x, seat.start.y - b.centre.y);
    let to_crank = (crank.x - seat.start.x, crank.y - seat.start.y);
    assert!(
        (radial.0 * to_crank.0 + radial.1 * to_crank.1).abs() < 1e-6,
        "tangent hits the crank"
    );
    // Tangent to the axis at the bend: the straight section runs along it.
    let up = (
        t.seat_tube_top.x - b.tangent.x,
        t.seat_tube_top.y - b.tangent.y,
    );
    let rad_t = (b.tangent.x - b.centre.x, b.tangent.y - b.centre.y);
    assert!((up.0 * rad_t.0 + up.1 * rad_t.1).abs() < 1e-6);
    assert!((dist(b.tangent, b.centre) - b.radius).abs() < EPS);

    // Not past vertical, so it isn't moved up (B6).
    assert!(seat.start.x <= crank.x, "{:?}", seat.start);
    // B2: the radius asked for, and insertion is the straight section.
    assert_eq!(b.radius, OFFSET_RADIUS);
    assert_eq!(t.seat_tube_bend_radius_source, ValueSource::Pinned);
    assert!((t.seat_tube_insertion - dist(t.seat_tube_top, b.tangent)).abs() < EPS);
    // B4: chart length unchanged; mass length runs along the arc.
    assert_eq!(t.seat_tube_length, 500.0);
    let expected = dist(t.seat_tube_top, b.tangent) + b.radius * b.sweep;
    assert!((seat.length - expected).abs() < EPS);

    // A gentler bend starts higher, leaving less insertion.
    let mut gentle = offset_seat();
    gentle.tubes.seat_tube_bend_radius = Some(Millimeters(2.0 * OFFSET_RADIUS));
    let g = solve(&gentle).0.unwrap();
    assert!(g.seat_tube_insertion < t.seat_tube_insertion);
}

/// B3: aligned within 1 mm, the seat tube stays straight; a bend that would
/// start above the top tube junction is impossible.
#[test]
fn aligned_seat_tube_stays_straight_and_huge_bends_are_impossible() {
    let t = solve(&rigid()).0.unwrap();
    assert!(t.tube(TubeKind::SeatTube).unwrap().bend.is_none());

    let mut huge = offset_seat();
    huge.tubes.seat_tube_bend_radius = Some(Millimeters(100_000.0));
    let (t, diagnostics) = solve(&huge);
    assert!(
        t.is_none() && has(&diagnostics, "tubes.impossible"),
        "{diagnostics:?}"
    );
}

/// B6: actual seat angle slacker than the effective puts the axis in front of
/// the crank; the lowest bend would lean past vertical into the shell, so the
/// bend moves up and the tube enters the shell vertically, at its top.
#[test]
fn seat_tube_never_enters_the_shell_past_vertical() {
    let mut p = rigid();
    p.geometry.pin(V::EffectiveSeatTubeAngle, 78.0);
    p.geometry.pin(V::ActualSeatTubeAngle, 74.0);
    let (t, diagnostics) = solve(&p);
    let t = t.unwrap_or_else(|| panic!("{diagnostics:?}"));
    let seat = t.tube(TubeKind::SeatTube).unwrap();
    let shell = t.tube(TubeKind::BottomBracketShell).unwrap();
    let b = seat.bend.expect("bent");
    let crank = shell.start;
    let top_of_shell = Point::new(crank.x, crank.y + shell.outside_diameter / 2.0);
    assert!(close(seat.start, top_of_shell, EPS), "{:?}", seat.start);
    // The arc ends on the vertical through the crank, heading straight down.
    let low = b.at(b.sweep);
    assert!((low.x - crank.x).abs() < EPS && low.y >= top_of_shell.y);
    let radial = (low.x - b.centre.x, low.y - b.centre.y);
    assert!(radial.1.abs() < 1e-6, "vertical at the arc's end");
    assert_eq!(b.radius, t.seat_tube_bend_radius);
    // Length includes the vertical straight.
    let dist = |a: Point, c: Point| (a.x - c.x).hypot(a.y - c.y);
    let expected = dist(t.seat_tube_top, b.tangent) + b.radius * b.sweep + dist(low, seat.start);
    assert!((seat.length - expected).abs() < EPS);

    // A radius whose fillet would reach below the shell: impossible.
    p.tubes.seat_tube_bend_radius = Some(Millimeters(5000.0));
    let (t, diagnostics) = solve(&p);
    assert!(
        t.is_none() && has(&diagnostics, "tubes.impossible"),
        "{diagnostics:?}"
    );
}

/// The seat tube can only bend back to the BB: an actual seat angle steeper
/// than the effective is a geometry error.
#[test]
fn actual_seat_angle_cant_be_steeper_than_effective() {
    let mut p = rigid();
    p.geometry.pin(V::ActualSeatTubeAngle, 74.5);
    let d = defaults();
    let (geometry, diagnostics) = solve_geometry(&p, &d);
    assert!(geometry.is_none() && has(&diagnostics, "geometry.impossible"));
    p.geometry.pin(V::ActualSeatTubeAngle, 74.0);
    assert!(solve_geometry(&p, &d).0.is_some(), "equal is fine");
}
