//! S-COMPONENTS: every library entry parses, cites a source and produces valid
//! side-view geometry (C11); resolution handles unknown, custom and invalid
//! components (C3, C12).

use bike_components::{FILES, Library, drawing, resolve, validate};
use bike_model::shape::{contains, is_simple};
use bike_model::{
    Battery, BatterySetup, ComponentId, ComponentName, ComponentReference, Degrees, Diagnostic,
    DriveUnit, DriveUnitComponent, DriveUnitKind, MassProperties, Millimeters, MotorSpecification,
    Mount, OutlineAccuracy, Point, Project, Severity,
};

const LYRIK: &str = "rockshox/lyrik-ultimate/2025/29-160-44";
const SID_SL: &str = "rockshox/sid-sl-select/2025/29-110-44";
const SUPER_DELUXE: &str = "rockshox/super-deluxe-ultimate/2025/205x62.5-trunnion";

fn library() -> Library {
    Library::builtin().unwrap()
}

fn has(diagnostics: &[Diagnostic], code: &str, severity: Severity) -> bool {
    diagnostics
        .iter()
        .any(|d| d.code == code && d.severity == severity)
}

fn unit() -> DriveUnitComponent {
    DriveUnitComponent {
        name: ComponentName {
            make: "Test".into(),
            model: "Synthetic motor".into(),
            year: 2026,
            variant: "a".into(),
        },
        kind: DriveUnitKind::Motor,
        motor: Some(MotorSpecification {
            rated_power: 250.0,
            peak_power: None,
            torque: 85.0,
        }),
        mounts: vec![Mount {
            position: Point::new(50.0, 0.0),
            diameter: Millimeters(8.0),
        }],
        outline: vec![
            Point::new(-80.0, -60.0),
            Point::new(100.0, -60.0),
            Point::new(100.0, 60.0),
            Point::new(-80.0, 60.0),
        ],
        outline_accuracy: OutlineAccuracy::Exact,
        output_sprocket: None,
        ratios: None,
        mass: MassProperties {
            mass: 2900.0,
            centre_of_mass: None,
        },
        frame_mount_mass: 0.0,
        svg_path: None,
        source: "Synthetic test data.".into(),
    }
}

#[test]
fn every_entry_parses_cites_a_source_and_draws() {
    let library = library();
    assert_eq!(library.len(), 3);
    assert_eq!(FILES.len(), 2);
    for fork in library.forks.values() {
        assert!(fork.source.contains("https://"), "{}", fork.name);
        for compression in [0.0, fork.travel.0 / 2.0, fork.travel.0] {
            let d = drawing::fork(fork, compression);
            assert!(d.shapes.iter().all(|s| is_simple(s)), "{}", fork.name);
        }
    }
    for shock in library.shocks.values() {
        assert!(shock.source.contains("https://"), "{}", shock.name);
        for compression in [0.0, shock.stroke.0] {
            let d = drawing::shock(shock, compression);
            assert!(d.shapes.iter().all(|s| is_simple(s)), "{}", shock.name);
        }
    }
    for unit in library.drive_units.values() {
        assert!(is_simple(&unit.outline));
        assert!(
            unit.mounts
                .iter()
                .all(|m| contains(&unit.outline, m.position))
        );
    }
}

#[test]
fn library_values_match_their_sources() {
    let library = library();
    let lyrik = &library.forks[&ComponentId(LYRIK.into())];
    assert_eq!(
        (lyrik.travel.0, lyrik.axle_to_crown.0, lyrik.offset.0),
        (160.0, 571.0, 44.0)
    );
    assert_eq!(
        (lyrik.stanchion_diameter.0, lyrik.mass.mass),
        (35.0, 2028.0)
    );
    let sid = &library.forks[&ComponentId(SID_SL.into())];
    assert_eq!((sid.travel.0, sid.axle_to_crown.0), (110.0, 516.0));
    // The Argon GTB GPI sheet allows stanchions up to 32 mm.
    assert_eq!(sid.stanchion_diameter.0, 32.0);
    let shock = &library.shocks[&ComponentId(SUPER_DELUXE.into())];
    assert_eq!((shock.eye_to_eye.0, shock.stroke.0), (205.0, 62.5));
    assert_eq!(shock.mount, bike_model::ShockMount::Trunnion);
}

#[test]
fn a_fork_drawing_follows_travel() {
    let lyrik = &library().forks[&ComponentId(LYRIK.into())];
    let top = |c: f64| {
        drawing::fork(lyrik, c)
            .shapes
            .iter()
            .flatten()
            .map(|p| p.y)
            .fold(f64::MIN, f64::max)
    };
    assert!((top(0.0) - top(160.0) - 160.0).abs() < 1e-9);
}

#[test]
fn malformed_or_unsourced_files_are_errors() {
    let fork = FILES[0].1;
    let err = Library::from_files([("forks/x.toml", "make = 3")]).unwrap_err();
    assert!(err.to_string().starts_with("forks/x.toml"), "{err}");

    let unsourced = fork.replacen("source = \"SRAM", "source = \"\" # \"", 1);
    assert!(Library::from_files([("forks/x.toml", unsourced.as_str())]).is_err());

    let negative = fork.replacen("axle_to_crown = 571.0", "axle_to_crown = -1.0", 1);
    let err = Library::from_files([("forks/x.toml", negative.as_str())]).unwrap_err();
    assert!(err.to_string().contains("axle-to-crown"), "{err}");

    let err = Library::from_files([("forks/a.toml", fork), ("forks/b.toml", fork)]).unwrap_err();
    assert!(err.to_string().contains("twice"), "{err}");

    assert!(Library::from_files([("tyres/x.toml", "make = \"A\"")]).is_err());
}

#[test]
fn drive_unit_and_battery_files_load() {
    let units = r#"
make = "Test"
[[units]]
name = { make = "Test", model = "Box", year = 2026, variant = "a" }
kind = "gearbox"
mounts = [{ position = { x = 10.0, y = 10.0 }, diameter = 8.0 }]
outline = [{ x = -50.0, y = -50.0 }, { x = 50.0, y = -50.0 }, { x = 0.0, y = 60.0 }]
outline_accuracy = "approximate"
mass = { mass = 2000.0 }
source = "test"
"#;
    let batteries = r#"
make = "Test"
[[units]]
name = { make = "Test", model = "Pack", year = 2026, variant = "720" }
capacity = 720.0
nominal_voltage = 36.0
length = 400.0
width = 70.0
height = 80.0
mass = { mass = 3800.0 }
source = "test"
"#;
    let library = Library::from_files([
        ("drive_units/test.toml", units),
        ("batteries/test.toml", batteries),
    ])
    .unwrap();
    assert!(
        library
            .drive_units
            .contains_key(&ComponentId("test/box/2026/a".into()))
    );
    assert_eq!(library.batteries.len(), 1);

    let wrong_make = units.replacen("make = \"Test\"", "make = \"Other\"", 1);
    assert!(Library::from_files([("drive_units/x.toml", wrong_make.as_str())]).is_err());
}

#[test]
fn bad_outlines_and_mounts_are_rejected() {
    let mut bow_tie = unit();
    bow_tie.outline.swap(1, 2);
    assert!(
        validate::drive_unit(&bow_tie)
            .unwrap_err()
            .contains("outline")
    );

    let mut outside = unit();
    outside.mounts[0].position = Point::new(500.0, 0.0);
    assert!(
        validate::drive_unit(&outside)
            .unwrap_err()
            .contains("mount 1")
    );

    let mut no_motor = unit();
    no_motor.motor = None;
    assert!(validate::drive_unit(&no_motor).is_err());
    assert!(validate::drive_unit(&unit()).is_ok());
}

#[test]
fn an_unknown_component_warns_and_falls_back() {
    let mut project = Project::default();
    project.components.fork = Some(ComponentReference::Library(ComponentId(
        "nobody/nothing/1999/x".into(),
    )));
    let project = Project::from_json(&project.to_json()).unwrap();
    let (resolved, diagnostics) = resolve(&project, &library());
    assert!(resolved.fork.is_none());
    assert!(has(&diagnostics, "component.unknown", Severity::Warning));
}

#[test]
fn fork_travel_mismatch_warns() {
    let mut project = Project {
        front_travel: Some(Millimeters(150.0)),
        ..Project::default()
    };
    project.components.fork = Some(ComponentReference::Library(ComponentId(LYRIK.into())));
    let (resolved, diagnostics) = resolve(&project, &library());
    assert!(resolved.fork.is_some());
    assert!(has(&diagnostics, "fork.travel_mismatch", Severity::Warning));

    project.front_travel = Some(Millimeters(160.0));
    let (_, diagnostics) = resolve(&project, &library());
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn an_invalid_custom_component_is_an_error() {
    let mut bad = unit();
    bad.mass.mass = 0.0;
    let mut project = Project::default();
    project.drive.unit = Some(DriveUnit {
        component: ComponentReference::Custom(bad),
        rotation: Degrees(0.0),
    });
    let (resolved, diagnostics) = resolve(&project, &library());
    assert!(resolved.drive_unit.is_none());
    assert!(has(&diagnostics, "component.invalid", Severity::Error));
}

#[test]
fn a_drive_unit_is_rotated_and_reports_approximate_and_battery() {
    let mut approximate = unit();
    approximate.outline_accuracy = OutlineAccuracy::Approximate;
    let mut project = Project::default();
    project.drive.unit = Some(DriveUnit {
        component: ComponentReference::Custom(approximate),
        rotation: Degrees(90.0),
    });
    let (resolved, diagnostics) = resolve(&project, &library());
    assert!(has(&diagnostics, "drive_unit.approximate", Severity::Info));
    assert!(has(&diagnostics, "battery.none", Severity::Info));
    let unit = resolved.drive_unit.unwrap();
    // The mount at (50, 0) turns to (0, 50); the outline centroid (10, 0) to (0, 10).
    assert!(unit.mounts[0].x.abs() < 1e-9 && (unit.mounts[0].y - 50.0).abs() < 1e-9);
    assert!(unit.centre_of_mass.x.abs() < 1e-9 && (unit.centre_of_mass.y - 10.0).abs() < 1e-9);

    project.drive.battery = Some(BatterySetup {
        component: ComponentReference::Custom(Battery {
            name: ComponentName {
                make: "Test".into(),
                model: "Pack".into(),
                year: 2026,
                variant: "720".into(),
            },
            capacity: 720.0,
            nominal_voltage: 36.0,
            amp_hours: None,
            length: Millimeters(400.0),
            width: Millimeters(70.0),
            height: Millimeters(80.0),
            outline: vec![],
            mass: MassProperties {
                mass: 3800.0,
                centre_of_mass: None,
            },
            svg_path: None,
            source: "test".into(),
        }),
        distance_from_down_tube_end: Millimeters(50.0),
        position: Default::default(),
    });
    let (resolved, diagnostics) = resolve(&project, &library());
    assert!(!has(&diagnostics, "battery.none", Severity::Info));
    let battery = resolved.battery.unwrap();
    assert_eq!(battery.centre_of_mass, Point::new(200.0, 0.0));
}

#[test]
fn a_given_centre_of_mass_wins_over_the_drawing() {
    let mut given = unit();
    given.mass.centre_of_mass = Some(Point::new(-5.0, 7.0));
    let mut project = Project::default();
    project.drive.unit = Some(DriveUnit {
        component: ComponentReference::Custom(given),
        rotation: Degrees(0.0),
    });
    let (resolved, _) = resolve(&project, &library());
    assert_eq!(
        resolved.drive_unit.unwrap().centre_of_mass,
        Point::new(-5.0, 7.0)
    );
}
