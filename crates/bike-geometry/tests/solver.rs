//! S-GEOM "Done when": round trips, sag by hand, diagnostics, pedal clearance, robustness.

use bike_defaults::Defaults;
use bike_geometry::solve_geometry;
use bike_model::{
    CrankSetup, Diagnostic, ForkOverrides, GeometryVariable as V, Millimeters, PedalSize, Project,
    Severity, ValueSource, WheelSize,
};
use proptest::prelude::*;

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

fn defaults() -> Defaults {
    Defaults::builtin().unwrap()
}

fn has(diagnostics: &[Diagnostic], code: &str) -> bool {
    diagnostics.iter().any(|d| d.code == code)
}

/// A 29" hardtail with a known fork, pinned by stack, reach, head angle, chainstay and BB drop.
fn hardtail() -> Project {
    let mut p = Project {
        front_wheel: Some(WheelSize("29".into())),
        rear_wheel: Some(WheelSize("29".into())),
        front_travel: Some(Millimeters(120.0)),
        fork: ForkOverrides {
            axle_to_crown: Some(Millimeters(526.0)),
            offset: Some(Millimeters(44.0)),
        },
        crank: CrankSetup {
            crank_length: Some(Millimeters(170.0)),
            pedal: Some(PedalSize {
                length: Millimeters(100.0),
                thickness: Millimeters(20.0),
            }),
        },
        ..Project::default()
    };
    let g = &mut p.geometry;
    g.pin(V::Stack, 630.0);
    g.pin(V::Reach, 455.0);
    g.pin(V::HeadTubeAngle, 67.5);
    g.pin(V::ChainstayLength, 448.0);
    g.pin(V::BottomBracketDrop, 50.0);
    p
}

#[test]
fn empty_project_shows_a_default_bike() {
    let (solved, diagnostics) = solve_geometry(&Project::default(), &defaults());
    let solved = solved.expect("a default bike");
    for code in [
        "geometry.unpinned",
        "wheel.missing",
        "fork.default",
        "geometry.default",
    ] {
        assert!(has(&diagnostics, code), "missing {code}: {diagnostics:?}");
    }
    assert!(
        !diagnostics.iter().any(|d| d.severity == Severity::Error),
        "{diagnostics:?}"
    );
    assert_eq!(solved.front_wheel_diameter, 745.0);
    assert_eq!(solved.sources[&V::Wheelbase], ValueSource::Solved);
    assert_eq!(solved.sources[&V::HeadTubeAngle], ValueSource::Default);
    assert!(solved.at_sag.is_none(), "an empty project is rigid");
}

#[test]
fn wheels_only_project_solves() {
    let p = Project {
        front_wheel: Some(WheelSize("27.5".into())),
        rear_wheel: Some(WheelSize("27.5".into())),
        ..Project::default()
    };
    let (solved, diagnostics) = solve_geometry(&p, &defaults());
    assert!(solved.is_some());
    assert!(!has(&diagnostics, "wheel.missing"));
}

#[test]
fn unknown_wheel_size_is_an_error_not_a_panic() {
    let p = Project {
        front_wheel: Some(WheelSize("31".into())),
        ..Project::default()
    };
    let (solved, diagnostics) = solve_geometry(&p, &defaults());
    assert!(solved.is_none());
    assert!(has(&diagnostics, "wheel.unknown"));
}

#[test]
fn pinned_values_hold_and_equations_close() {
    let p = hardtail();
    let (solved, diagnostics) = solve_geometry(&p, &defaults());
    let s = solved.unwrap().static_state;
    assert!(!has(&diagnostics, "geometry.conflict"));
    for (v, value) in [
        (V::Stack, 630.0),
        (V::Reach, 455.0),
        (V::HeadTubeAngle, 67.5),
        (V::ChainstayLength, 448.0),
        (V::BottomBracketDrop, 50.0),
    ] {
        assert!((s.value(v) - value).abs() < 1e-6, "{v:?}");
    }
    // Equation 1 with equal wheels: BB height = radius − drop.
    assert!((s.value(V::BottomBracketHeight) - (372.5 - 50.0)).abs() < 1e-6);
    // Both tyres touch the ground.
    assert!((s.points.rear_axle.y - s.points.rear_contact.y - 372.5).abs() < 1e-6);
    assert!((s.points.front_axle.y - s.points.front_contact.y - 372.5).abs() < 1e-6);
    // Trail, by the textbook formula: (R cos HA − offset) / sin HA.
    let h = 67.5_f64.to_radians();
    assert!((s.trail - (372.5 * h.cos() - 44.0) / h.sin()).abs() < 1e-6);
}

#[test]
fn mullet_bb_height_uses_the_average_radius() {
    let mut p = hardtail();
    p.rear_wheel = Some(WheelSize("27.5".into()));
    let s = solve_geometry(&p, &defaults()).0.unwrap().static_state;
    let average = (745.0 + 715.0) / 4.0;
    assert!((s.value(V::BottomBracketHeight) - (average - 50.0)).abs() < 1e-6);
    let midpoint = (s.points.rear_axle.y + s.points.front_axle.y) / 2.0;
    assert!((midpoint - 50.0).abs() < 1e-6);
}

/// Hand calculation for a hardtail at fork sag and at full travel (equal wheels).
///
/// Statically the axles are level, `wheelbase` apart. Compressing the fork by `c`
/// moves the front axle up the steering axis: by `c·cos HA` back and `c·sin HA` up.
/// The bike then pitches about the rear axle by θ = −atan(c·sin HA / (wheelbase − c·cos HA)),
/// so the head angle becomes HA − θ, and the BB, which sits `rear centre` ahead of
/// and `drop` below the rear axle, ends up at height R + rear centre·sin θ − drop·cos θ.
#[test]
fn fork_sag_and_full_travel_match_hand_calculation() {
    let p = hardtail();
    let solved = solve_geometry(&p, &defaults()).0.unwrap();
    let s0 = &solved.static_state;
    let wheelbase = s0.value(V::Wheelbase);
    let rear_centre = s0.rear_centre;
    let ha = 67.5_f64.to_radians();
    let (r, drop, crank, half_pedal) = (372.5, 50.0, 170.0, 10.0);
    let sag = defaults().geometry.bike_for("29", 120.0).unwrap().fork_sag;

    for (compression, state) in [
        (sag * 120.0, solved.at_sag.unwrap()),
        (120.0, solved.full_travel.unwrap()),
    ] {
        let theta = -(compression * ha.sin()).atan2(wheelbase - compression * ha.cos());
        let head_angle = 67.5 - theta.to_degrees();
        let bb_height = r + rear_centre * theta.sin() - drop * theta.cos();
        let new_wheelbase = ((wheelbase - compression * ha.cos()).powi(2)
            + (compression * ha.sin()).powi(2))
        .sqrt();
        assert!((state.value(V::HeadTubeAngle) - head_angle).abs() < 1e-9);
        assert!((state.value(V::BottomBracketHeight) - bb_height).abs() < 1e-9);
        assert!((state.value(V::Wheelbase) - new_wheelbase).abs() < 1e-9);
        assert!((state.pedal_ground_clearance - (bb_height - crank - half_pedal)).abs() < 1e-9);
        assert!(
            state.value(V::HeadTubeAngle) > 67.5,
            "the head angle steepens"
        );
    }
}

#[test]
fn rigid_and_full_suspension_have_no_fork_sag_state() {
    let mut rigid = hardtail();
    rigid.front_travel = None;
    let solved = solve_geometry(&rigid, &defaults()).0.unwrap();
    assert!(solved.at_sag.is_none() && solved.full_travel.is_none());

    let mut full = hardtail();
    full.rear_travel = Some(Millimeters(150.0));
    let solved = solve_geometry(&full, &defaults()).0.unwrap();
    assert!(
        solved.at_sag.is_none(),
        "full suspension at sag comes with S-KINEMATICS"
    );
}

#[test]
fn pedal_clearance_by_hand() {
    let s = solve_geometry(&hardtail(), &defaults())
        .0
        .unwrap()
        .static_state;
    // Straight down: BB height − crank − half the pedal thickness.
    assert!((s.pedal_ground_clearance - (322.5 - 170.0 - 10.0)).abs() < 1e-6);
    // Forward: the pedal's front edge is at x = 170 + 50 = 220, its top 10 mm up;
    // the front axle is far ahead and above, so the gap is the distance from the
    // axle to that corner, less the tyre radius.
    let f = s.points.front_axle;
    let gap = ((f.x - 220.0).powi(2) + (f.y - 10.0).powi(2)).sqrt() - 372.5;
    assert!((s.pedal_front_tyre_clearance - gap).abs() < 1e-6);
}

#[test]
fn pedal_strikes_are_warnings() {
    let mut p = hardtail();
    p.crank.crank_length = Some(Millimeters(320.0));
    p.crank.pedal = Some(PedalSize {
        length: Millimeters(700.0),
        thickness: Millimeters(20.0),
    });
    let (solved, diagnostics) = solve_geometry(&p, &defaults());
    assert!(solved.is_some());
    for code in ["pedal.ground", "pedal.front_tyre"] {
        let d = diagnostics
            .iter()
            .find(|d| d.code == code)
            .unwrap_or_else(|| panic!("{code}: {diagnostics:?}"));
        assert_eq!(d.severity, Severity::Warning);
    }
}

#[test]
fn conflicting_pins_are_an_error_and_the_rest_still_solves() {
    let mut p = hardtail();
    let s = solve_geometry(&p, &defaults()).0.unwrap().static_state;
    p.geometry
        .pin(V::HeadTubeLength, s.value(V::HeadTubeLength));
    p.geometry
        .pin(V::BottomBracketHeight, s.value(V::BottomBracketHeight));
    p.geometry.pin(V::Wheelbase, s.value(V::Wheelbase) + 4.2);
    let (solved, diagnostics) = solve_geometry(&p, &defaults());
    assert!(solved.is_some());
    let conflict = diagnostics
        .iter()
        .find(|d| d.code == "geometry.conflict")
        .expect("a conflict");
    assert_eq!(conflict.severity, Severity::Error);
    assert!(conflict.message.contains("4.2 mm"), "{}", conflict.message);
    assert!(
        conflict.message.starts_with("Stack, reach"),
        "{}",
        conflict.message
    );

    // Within 1 mm isn't a conflict (G6).
    p.geometry.pin(V::Wheelbase, s.value(V::Wheelbase) + 0.9);
    assert!(!has(
        &solve_geometry(&p, &defaults()).1,
        "geometry.conflict"
    ));
}

#[test]
fn impossible_values_are_an_error() {
    let mut p = hardtail();
    p.geometry.pin(V::ChainstayLength, 30.0); // shorter than the 50 mm BB drop
    let (solved, diagnostics) = solve_geometry(&p, &defaults());
    assert!(solved.is_none());
    assert!(has(&diagnostics, "geometry.impossible"));

    let mut p = hardtail();
    p.geometry.pin(V::HeadTubeAngle, 95.0);
    let (solved, diagnostics) = solve_geometry(&p, &defaults());
    assert!(
        solved.is_some(),
        "an out-of-range pin is ignored, the rest solves"
    );
    assert!(has(&diagnostics, "geometry.impossible"));
}

/// Solve with one set of pins, pin the solved values in a different set, get the same frame.
#[test]
fn round_trip_between_pin_sets() {
    let sets: [&[V]; 4] = [
        &[
            V::Stack,
            V::Reach,
            V::HeadTubeAngle,
            V::ChainstayLength,
            V::BottomBracketDrop,
        ],
        &[
            V::Stack,
            V::Reach,
            V::HeadTubeAngle,
            V::HeadTubeLength,
            V::ChainstayLength,
        ],
        &[
            V::Wheelbase,
            V::BottomBracketHeight,
            V::HeadTubeLength,
            V::HeadTubeAngle,
            V::ChainstayLength,
        ],
        &[
            V::Wheelbase,
            V::Stack,
            V::HeadTubeLength,
            V::BottomBracketDrop,
            V::Reach,
        ],
    ];
    let first = solve_geometry(&hardtail(), &defaults())
        .0
        .unwrap()
        .static_state;
    for set in sets {
        let mut p = hardtail();
        p.geometry = Default::default();
        for v in set {
            p.geometry.pin(*v, first.value(*v));
        }
        let (solved, diagnostics) = solve_geometry(&p, &defaults());
        let s = solved
            .unwrap_or_else(|| panic!("{set:?}: {diagnostics:?}"))
            .static_state;
        for v in EQUATION_VARIABLES {
            let tolerance = if v.is_angle() { 0.001 } else { 0.01 };
            // Variables outside the set and outside the equations come from defaults.
            let expected = first.value(v);
            assert!(
                (s.value(v) - expected).abs() < tolerance,
                "{set:?} {v:?}: {} vs {expected}",
                s.value(v)
            );
        }
    }
}

fn arbitrary_variable() -> impl Strategy<Value = V> {
    proptest::sample::select(V::ALL.to_vec())
}

proptest! {
    #[test]
    fn random_pins_never_panic(
        pins in proptest::collection::vec((arbitrary_variable(), -2000.0f64..3000.0), 0..10),
        travel in proptest::option::of(-50.0f64..300.0),
        rear in proptest::option::of(0.0f64..250.0),
        a2c in proptest::option::of(-100.0f64..900.0),
    ) {
        let mut p = Project {
            front_travel: travel.map(Millimeters),
            rear_travel: rear.map(Millimeters),
            fork: ForkOverrides { axle_to_crown: a2c.map(Millimeters), offset: None },
            ..Project::default()
        };
        for (v, value) in pins {
            p.geometry.pin(v, value);
        }
        let (solved, diagnostics) = solve_geometry(&p, &defaults());
        if let Some(solved) = solved {
            prop_assert!(solved.static_state.values.values().all(|x| x.is_finite()));
        } else {
            prop_assert!(diagnostics.iter().any(|d| d.severity == Severity::Error));
        }
    }
}
