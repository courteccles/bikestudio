//! The saved project and its `.bikestud` file format.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::units::Millimeters;

/// The schema version this build reads and writes.
///
/// There is no compatibility promise before version 1, so files with any
/// other version are rejected rather than migrated.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// The wheel with its tire: a key into `bike-defaults/wheel_size.toml`, for example `"29"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WheelSize(pub String);

/// A bikestudio project. Every field is optional, so an empty project is valid.
///
/// Later specs add their own fields to this skeleton.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub schema_version: u32,
    pub front_wheel: Option<WheelSize>,
    pub rear_wheel: Option<WheelSize>,
    /// `None` means a rigid fork.
    pub front_travel: Option<Millimeters>,
    /// `None` means a rigid rear.
    pub rear_travel: Option<Millimeters>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            front_wheel: None,
            rear_wheel: None,
            front_travel: None,
            rear_travel: None,
        }
    }
}

/// Why a project couldn't be loaded or saved.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("couldn't read or write the project file: {0}")]
    Io(#[from] std::io::Error),
    #[error("the project file isn't valid: {0}")]
    Invalid(#[from] serde_json::Error),
    #[error("the project file has no schema_version, so it isn't a bikestudio project")]
    MissingSchemaVersion,
    #[error(
        "the project file uses schema version {found}, but this build of bikestudio reads only \
         version {supported}; files from other development builds can't be opened before version 1"
    )]
    UnsupportedSchemaVersion { found: u64, supported: u32 },
}

impl Project {
    /// Serializes the project as pretty-printed JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a Project always serializes")
    }

    /// Parses a project from JSON, checking the schema version first.
    pub fn from_json(json: &str) -> Result<Self, LoadError> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        let found = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or(LoadError::MissingSchemaVersion)?;
        if found != u64::from(CURRENT_SCHEMA_VERSION) {
            return Err(LoadError::UnsupportedSchemaVersion {
                found,
                supported: CURRENT_SCHEMA_VERSION,
            });
        }
        Ok(serde_json::from_value(value)?)
    }

    /// Writes the project to a `.bikestud` file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), LoadError> {
        let mut json = self.to_json();
        json.push('\n');
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Reads a project from a `.bikestud` file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, LoadError> {
        Self::from_json(&std::fs::read_to_string(path)?)
    }
}
