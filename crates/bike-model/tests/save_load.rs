use bike_model::{CURRENT_SCHEMA_VERSION, LoadError, Millimeters, Project, WheelSize};

fn filled() -> Project {
    Project {
        schema_version: CURRENT_SCHEMA_VERSION,
        front_wheel: Some(WheelSize("29".into())),
        rear_wheel: Some(WheelSize("27.5".into())),
        front_travel: Some(Millimeters(170.0)),
        rear_travel: Some(Millimeters(160.0)),
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
