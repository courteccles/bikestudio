use bike_model::{
    Battery, BatteryPosition, BatterySetup, CURRENT_SCHEMA_VERSION, ComponentId, ComponentName,
    ComponentReference, Components, CrankSetup, Degrees, DriveSystem, DriveUnit,
    DriveUnitComponent, DriveUnitKind, ForkOverrides, GeometryInputs, GeometryVariable,
    JunctionOffsets, LoadError, MassProperties, Material, Millimeters, MotorSpecification, Mount,
    OutlineAccuracy, Parameter, PedalSize, Point, Project, Shock, ShockMount, SpringType,
    TubeInputs, TubeKind, TubeSection, WheelSize,
};

fn filled() -> Project {
    let mut geometry = GeometryInputs::default();
    geometry.pin(GeometryVariable::HeadTubeAngle, 64.0);
    geometry.pin(GeometryVariable::Reach, 480.0);
    geometry.pin(GeometryVariable::Stack, 630.0);
    geometry.pin(GeometryVariable::EffectiveSeatTubeAngle, 77.0);
    geometry.pin(GeometryVariable::FrontCentre, 800.0);
    geometry.pin(GeometryVariable::RearCentre, 440.0);
    Project {
        schema_version: CURRENT_SCHEMA_VERSION,
        front_wheel: Some(WheelSize("29".into())),
        rear_wheel: Some(WheelSize("27.5".into())),
        front_travel: Some(Millimeters(170.0)),
        rear_travel: Some(Millimeters(160.0)),
        geometry,
        fork: ForkOverrides {
            axle_to_crown: Some(Millimeters(582.0)),
            offset: Some(Millimeters(44.0)),
        },
        crank: CrankSetup {
            crank_length: Some(Millimeters(165.0)),
            pedal: Some(PedalSize {
                length: Millimeters(105.0),
                thickness: Millimeters(17.5),
            }),
        },
        tubes: TubeInputs {
            material: Some(Material::Titanium),
            seat_tube_length: Parameter::Pinned(440.0),
            junctions: JunctionOffsets {
                top_tube_below_head_tube_top: Some(Millimeters(30.0)),
                down_tube_above_head_tube_bottom: None,
                seat_tube_overhang: Some(Millimeters(103.0)),
            },
            standover: Parameter::Pinned(780.0),
            seat_tube_bend_radius: Some(Millimeters(90.0)),
            sections: [(
                TubeKind::DownTube,
                TubeSection {
                    outside_diameter: Some(Millimeters(44.5)),
                    wall: Some(Millimeters(1.0)),
                },
            )]
            .into(),
        },
        components: Components {
            fork: Some(ComponentReference::Library(ComponentId(
                "rockshox/lyrik-ultimate/2025/29-160-44".into(),
            ))),
            shock: Some(ComponentReference::Custom(Shock {
                name: name("Builder", "Prototype", "205x62.5"),
                eye_to_eye: Millimeters(205.0),
                stroke: Millimeters(62.5),
                mount: ShockMount::Trunnion,
                spring: SpringType::Coil,
                body_diameter: Some(Millimeters(30.0)),
                can_diameter: None,
                reservoir: true,
                mass: MassProperties {
                    mass: 700.0,
                    centre_of_mass: Some(Point::new(80.0, 0.0)),
                },
                svg_path: None,
                source: "builder's drawing".into(),
            })),
        },
        drive: DriveSystem {
            unit: Some(DriveUnit {
                component: ComponentReference::Custom(DriveUnitComponent {
                    name: name("Builder", "Test unit", "a"),
                    kind: DriveUnitKind::MotorGearbox,
                    motor: Some(MotorSpecification {
                        rated_power: 250.0,
                        peak_power: Some(600.0),
                        torque: 85.0,
                    }),
                    mounts: vec![Mount {
                        position: Point::new(50.0, 40.0),
                        diameter: Millimeters(8.0),
                    }],
                    outline: vec![
                        Point::new(-80.0, -60.0),
                        Point::new(90.0, -60.0),
                        Point::new(90.0, 70.0),
                        Point::new(-80.0, 70.0),
                    ],
                    outline_accuracy: OutlineAccuracy::Approximate,
                    output_sprocket: Some(30),
                    ratios: Some(vec![0.5, 1.0, 3.0]),
                    mass: MassProperties {
                        mass: 4000.0,
                        centre_of_mass: None,
                    },
                    frame_mount_mass: 150.0,
                    svg_path: Some("M 0 0 L 10 0".into()),
                    source: "builder's drawing".into(),
                }),
                rotation: Degrees(-10.0),
            }),
            battery: Some(BatterySetup {
                component: ComponentReference::Custom(Battery {
                    name: name("Builder", "Pack", "720"),
                    capacity: 720.0,
                    nominal_voltage: 36.0,
                    amp_hours: Some(20.0),
                    length: Millimeters(400.0),
                    width: Millimeters(70.0),
                    height: Millimeters(80.0),
                    outline: vec![],
                    mass: MassProperties {
                        mass: 3800.0,
                        centre_of_mass: None,
                    },
                    svg_path: None,
                    source: "builder's scale".into(),
                }),
                distance_from_down_tube_end: Millimeters(60.0),
                position: BatteryPosition::Under,
            }),
        },
    }
}

fn name(make: &str, model: &str, variant: &str) -> ComponentName {
    ComponentName {
        make: make.into(),
        model: model.into(),
        year: 2026,
        variant: variant.into(),
    }
}

fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "bikestudio-test-{}-{name}.bikestud",
        std::process::id()
    ))
}

#[test]
fn empty_project_round_trips() {
    let project = Project::default();
    assert_eq!(Project::from_json(&project.to_json()).unwrap(), project);
}

#[test]
fn filled_project_round_trips() {
    let project = filled();
    assert_eq!(Project::from_json(&project.to_json()).unwrap(), project);
}

#[test]
fn round_trips_through_a_file() {
    let path = temp_path("file");
    for project in [Project::default(), filled()] {
        project.save(&path).unwrap();
        assert_eq!(Project::load(&path).unwrap(), project);
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn saves_pretty_printed_json() {
    let json = filled().to_json();
    assert!(json.contains("\n  \"front_travel\": 170.0"), "{json}");
}

#[test]
fn rejects_other_schema_versions() {
    let json = filled()
        .to_json()
        .replace("\"schema_version\": 1", "\"schema_version\": 2");
    assert!(matches!(
        Project::from_json(&json),
        Err(LoadError::UnsupportedSchemaVersion {
            found: 2,
            supported: 1
        })
    ));
}

#[test]
fn rejects_missing_schema_version() {
    assert!(matches!(
        Project::from_json("{}"),
        Err(LoadError::MissingSchemaVersion)
    ));
}

#[test]
fn rejects_invalid_json_without_panicking() {
    assert!(matches!(
        Project::from_json("not json"),
        Err(LoadError::Invalid(_))
    ));
    assert!(matches!(
        Project::from_json(r#"{"schema_version": 1, "front_travel": "lots"}"#),
        Err(LoadError::Invalid(_))
    ));
}

#[test]
fn missing_file_is_an_error() {
    assert!(matches!(
        Project::load(temp_path("does-not-exist")),
        Err(LoadError::Io(_))
    ));
}
