//! S-COMPONENTS in the solvers: a selected fork (C4), an unknown component
//! (C3) and a drive unit the tubes miss (C8).

use bike_components::{Library, resolve};
use bike_defaults::Defaults;
use bike_geometry::{solve_geometry, solve_geometry_with, solve_tubes_with};
use bike_model::{
    ComponentId, ComponentName, ComponentReference, Degrees, Diagnostic, DriveUnit,
    DriveUnitComponent, DriveUnitKind, ForkOverrides, MassProperties, Millimeters, Mount,
    OutlineAccuracy, Point, Project, Severity, WheelSize,
};

const LYRIK: &str = "rockshox/lyrik-ultimate/2025/29-160-44";

fn has(diagnostics: &[Diagnostic], code: &str) -> bool {
    diagnostics.iter().any(|d| d.code == code)
}

fn hardtail() -> Project {
    Project {
        front_wheel: Some(WheelSize("29".into())),
        rear_wheel: Some(WheelSize("29".into())),
        front_travel: Some(Millimeters(160.0)),
        ..Project::default()
    }
}

#[test]
fn a_library_fork_solves_like_its_typed_dimensions() {
    let defaults = Defaults::builtin().unwrap();
    let library = Library::builtin().unwrap();

    let mut selected = hardtail();
    selected.components.fork = Some(ComponentReference::Library(ComponentId(LYRIK.into())));
    let (resolved, notes) = resolve(&selected, &library);
    assert!(notes.is_empty(), "{notes:?}");
    let (from_library, _) = solve_geometry_with(&selected, &defaults, &resolved);

    let mut typed = hardtail();
    typed.fork = ForkOverrides {
        axle_to_crown: Some(Millimeters(571.0)),
        offset: Some(Millimeters(44.0)),
    };
    let (from_overrides, _) = solve_geometry(&typed, &defaults);
    assert_eq!(from_library.unwrap(), from_overrides.unwrap());
}

#[test]
fn a_fork_override_adjusts_the_selected_fork() {
    let defaults = Defaults::builtin().unwrap();
    let mut project = hardtail();
    project.components.fork = Some(ComponentReference::Library(ComponentId(LYRIK.into())));
    project.fork.axle_to_crown = Some(Millimeters(581.0));
    let (resolved, _) = resolve(&project, &Library::builtin().unwrap());
    let geometry = solve_geometry_with(&project, &defaults, &resolved)
        .0
        .unwrap();
    assert_eq!(geometry.fork_axle_to_crown, 581.0);
    assert_eq!(geometry.fork_offset, 44.0);
}

#[test]
fn the_selected_forks_travel_is_used() {
    let defaults = Defaults::builtin().unwrap();
    let mut project = hardtail();
    project.front_travel = Some(Millimeters(120.0));
    project.components.fork = Some(ComponentReference::Library(ComponentId(LYRIK.into())));
    let (resolved, notes) = resolve(&project, &Library::builtin().unwrap());
    assert!(has(&notes, "fork.travel_mismatch"));
    let mut at_160 = project.clone();
    at_160.front_travel = Some(Millimeters(160.0));
    let geometry = solve_geometry_with(&project, &defaults, &resolved).0;
    assert_eq!(
        geometry,
        solve_geometry_with(&at_160, &defaults, &resolved).0
    );
}

#[test]
fn an_unknown_component_loads_warns_and_solves_from_defaults() {
    let defaults = Defaults::builtin().unwrap();
    let mut project = hardtail();
    project.components.fork = Some(ComponentReference::Library(ComponentId(
        "rockshox/gone/2019/29-160-51".into(),
    )));
    let project = Project::from_json(&project.to_json()).unwrap();
    let (resolved, notes) = resolve(&project, &Library::builtin().unwrap());
    assert!(has(&notes, "component.unknown"));
    let (geometry, _) = solve_geometry_with(&project, &defaults, &resolved);
    assert_eq!(geometry, solve_geometry(&hardtail(), &defaults).0);
}

#[test]
fn a_tube_that_misses_the_drive_unit_is_an_error() {
    let defaults = Defaults::builtin().unwrap();
    let mut project = hardtail();
    // A synthetic unit drawn nowhere near the crank centre: a bad custom entry.
    project.drive.unit = Some(DriveUnit {
        component: ComponentReference::Custom(DriveUnitComponent {
            name: ComponentName {
                make: "Test".into(),
                model: "Misplaced".into(),
                year: 2026,
                variant: "a".into(),
            },
            kind: DriveUnitKind::Gearbox,
            motor: None,
            mounts: vec![Mount {
                position: Point::new(0.0, -450.0),
                diameter: Millimeters(8.0),
            }],
            outline: vec![
                Point::new(-50.0, -500.0),
                Point::new(50.0, -500.0),
                Point::new(50.0, -400.0),
                Point::new(-50.0, -400.0),
            ],
            outline_accuracy: OutlineAccuracy::Exact,
            output_sprocket: None,
            ratios: None,
            mass: MassProperties {
                mass: 2000.0,
                centre_of_mass: None,
            },
            frame_mount_mass: 0.0,
            svg_path: None,
            source: "Synthetic test data.".into(),
        }),
        rotation: Degrees(0.0),
    });
    let (resolved, _) = resolve(&project, &Library::builtin().unwrap());
    let geometry = solve_geometry_with(&project, &defaults, &resolved).0;
    let (tubes, diagnostics) = solve_tubes_with(&project, geometry.as_ref(), &defaults, &resolved);
    assert!(tubes.is_none());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "drive_unit.tube_misses" && d.severity == Severity::Error)
    );
}

proptest::proptest! {
    /// Any custom fork or drive unit resolves and solves without panicking (S-GEOM G7).
    #[test]
    fn random_custom_components_never_panic(
        a2c in -100.0f64..900.0,
        offset in -20.0f64..80.0,
        travel in -10.0f64..250.0,
        corners in proptest::collection::vec((-300.0f64..300.0, -300.0f64..300.0), 0..7),
        rotation in -360.0f64..360.0,
    ) {
        let library = Library::builtin().unwrap();
        let defaults = Defaults::builtin().unwrap();
        let mut project = hardtail();
        let mut fork = library.forks[&ComponentId(LYRIK.into())].clone();
        fork.axle_to_crown = Millimeters(a2c);
        fork.offset = Millimeters(offset);
        fork.travel = Millimeters(travel);
        project.components.fork = Some(ComponentReference::Custom(fork));
        let outline: Vec<Point> = corners.iter().map(|&(x, y)| Point::new(x, y)).collect();
        project.drive.unit = Some(DriveUnit {
            component: ComponentReference::Custom(DriveUnitComponent {
                name: ComponentName {
                    make: "Test".into(),
                    model: "Random".into(),
                    year: 2026,
                    variant: "a".into(),
                },
                kind: DriveUnitKind::Gearbox,
                motor: None,
                mounts: vec![Mount {
                    position: outline.first().copied().unwrap_or_default(),
                    diameter: Millimeters(8.0),
                }],
                outline,
                outline_accuracy: OutlineAccuracy::Exact,
                output_sprocket: None,
                ratios: None,
                mass: MassProperties { mass: 2000.0, centre_of_mass: None },
                frame_mount_mass: 0.0,
                svg_path: None,
                source: "random".into(),
            }),
            rotation: Degrees(rotation),
        });
        let (resolved, _) = resolve(&project, &library);
        let geometry = solve_geometry_with(&project, &defaults, &resolved).0;
        let _ = solve_tubes_with(&project, geometry.as_ref(), &defaults, &resolved);
    }
}
