//! Default values for bikestudio, one TOML file per data structure.
//!
//! Every entry cites its source. The built-in tables are compiled in and
//! checked by the tests, so a malformed file fails the build's tests rather
//! than crashing the app.

// `!(x > 0.0)` is deliberate throughout: unlike `x <= 0.0`, it also rejects NaN.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::collections::BTreeMap;

use serde::Deserialize;

const WHEEL_SIZE_TOML: &str = include_str!("../data/wheel_size.toml");
const FORK_TOML: &str = include_str!("../data/fork.toml");
const GEOMETRY_TOML: &str = include_str!("../data/geometry.toml");

/// All default tables.
#[derive(Debug, Clone, PartialEq)]
pub struct Defaults {
    pub wheel_sizes: WheelSizes,
    pub forks: Forks,
    pub geometry: GeometryDefaults,
}

/// `wheel_size.toml`: outer diameter by wheel size key.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WheelSizes {
    pub sizes: BTreeMap<String, WheelSizeEntry>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WheelSizeEntry {
    pub diameter: f64,
    pub source: String,
}

/// `fork.toml`: default fork dimensions by front wheel size.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Forks {
    pub forks: BTreeMap<String, ForkEntry>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForkEntry {
    pub rigid_axle_to_crown: f64,
    pub at_zero_travel: f64,
    pub per_mm_travel: f64,
    pub offset: f64,
    pub source: String,
}

impl ForkEntry {
    /// Axle-to-crown for `travel` millimetres (0 = rigid).
    pub fn axle_to_crown(&self, travel: f64) -> f64 {
        if travel <= 0.0 {
            self.rigid_axle_to_crown
        } else {
            self.at_zero_travel + self.per_mm_travel * travel
        }
    }
}

/// `geometry.toml`: the starting bike for the default solve, cranks and pedals.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryDefaults {
    pub default_wheel: String,
    pub pedal: PedalDefaults,
    pub cranks: CrankDefaults,
    pub bikes: Vec<DefaultBike>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PedalDefaults {
    pub length: f64,
    pub thickness: f64,
    pub source: String,
}

/// Crank length by rider size preset. Until S-MASS adds the rider, Medium is used.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrankDefaults {
    pub small: f64,
    pub medium: f64,
    pub large: f64,
    pub source: String,
}

/// A starting bike for some wheel sizes and front travel range.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefaultBike {
    pub name: String,
    pub wheels: Vec<String>,
    pub max_front_travel: f64,
    pub head_tube_angle: f64,
    pub effective_seat_tube_angle: f64,
    pub actual_seat_tube_angle: f64,
    pub stack: f64,
    pub reach: f64,
    pub head_tube_length: f64,
    pub chainstay_length: f64,
    pub bottom_bracket_drop: f64,
    pub bottom_bracket_height: f64,
    pub wheelbase: f64,
    pub fork_sag: f64,
    pub source: String,
}

impl GeometryDefaults {
    /// The starting bike for a front wheel size and front travel (0 = rigid).
    pub fn bike_for(&self, wheel: &str, front_travel: f64) -> Option<&DefaultBike> {
        self.bikes
            .iter()
            .filter(|b| b.wheels.iter().any(|w| w == wheel))
            .find(|b| front_travel <= b.max_front_travel)
    }
}

/// A default table that couldn't be read or failed validation.
#[derive(Debug, thiserror::Error)]
pub enum DefaultsError {
    #[error("{file} isn't valid: {source}")]
    Parse {
        file: &'static str,
        source: toml::de::Error,
    },
    #[error("{file}: {message}")]
    Invalid { file: &'static str, message: String },
}

impl Defaults {
    /// The built-in tables, parsed and validated.
    pub fn builtin() -> Result<Self, DefaultsError> {
        Self::from_toml(WHEEL_SIZE_TOML, FORK_TOML, GEOMETRY_TOML)
    }

    /// Parses and validates the three tables.
    pub fn from_toml(
        wheel_sizes: &str,
        forks: &str,
        geometry: &str,
    ) -> Result<Self, DefaultsError> {
        let defaults = Self {
            wheel_sizes: parse("wheel_size.toml", wheel_sizes)?,
            forks: parse("fork.toml", forks)?,
            geometry: parse("geometry.toml", geometry)?,
        };
        defaults.validate()?;
        Ok(defaults)
    }

    fn validate(&self) -> Result<(), DefaultsError> {
        let invalid = |file, message: String| Err(DefaultsError::Invalid { file, message });
        for (key, entry) in &self.wheel_sizes.sizes {
            if !(entry.diameter > 0.0) || entry.source.trim().is_empty() {
                return invalid(
                    "wheel_size.toml",
                    format!("size {key} needs a positive diameter and a source"),
                );
            }
        }
        for (key, fork) in &self.forks.forks {
            let positive = [fork.rigid_axle_to_crown, fork.at_zero_travel, fork.offset];
            if positive.iter().any(|v| !(*v > 0.0))
                || !(fork.per_mm_travel >= 0.0)
                || fork.source.trim().is_empty()
            {
                return invalid(
                    "fork.toml",
                    format!("fork {key} needs positive dimensions and a source"),
                );
            }
        }
        let g = &self.geometry;
        if !self.wheel_sizes.sizes.contains_key(&g.default_wheel) {
            return invalid(
                "geometry.toml",
                format!("default_wheel {} isn't in wheel_size.toml", g.default_wheel),
            );
        }
        for bike in &g.bikes {
            let lengths = [
                bike.stack,
                bike.reach,
                bike.head_tube_length,
                bike.chainstay_length,
                bike.bottom_bracket_height,
                bike.wheelbase,
            ];
            let angles = [
                bike.head_tube_angle,
                bike.effective_seat_tube_angle,
                bike.actual_seat_tube_angle,
            ];
            if lengths.iter().any(|v| !(*v > 0.0))
                || angles.iter().any(|a| !(*a > 0.0 && *a < 90.0))
                || !bike.bottom_bracket_drop.is_finite()
                || !(0.0..1.0).contains(&bike.fork_sag)
                || bike.source.trim().is_empty()
            {
                return invalid(
                    "geometry.toml",
                    format!("bike {} has an out-of-range value or no source", bike.name),
                );
            }
        }
        // Every wheel size needs a fork and a starting bike for every travel.
        for key in self.wheel_sizes.sizes.keys() {
            if !self.forks.forks.contains_key(key) {
                return invalid("fork.toml", format!("no fork for wheel size {key}"));
            }
            for travel in [0.0, 100.0, 150.0, 200.0, 300.0] {
                if g.bike_for(key, travel).is_none() {
                    return invalid(
                        "geometry.toml",
                        format!("no bike for wheel size {key} at {travel} mm travel"),
                    );
                }
            }
        }
        let cranks = &g.cranks;
        if [
            cranks.small,
            cranks.medium,
            cranks.large,
            g.pedal.length,
            g.pedal.thickness,
        ]
        .iter()
        .any(|v| !(*v > 0.0))
        {
            return invalid(
                "geometry.toml",
                "cranks and pedal need positive sizes".into(),
            );
        }
        Ok(())
    }
}

fn parse<T: for<'de> Deserialize<'de>>(file: &'static str, text: &str) -> Result<T, DefaultsError> {
    toml::from_str(text).map_err(|source| DefaultsError::Parse { file, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_tables_parse_and_validate() {
        let defaults = Defaults::builtin().unwrap();
        assert_eq!(defaults.geometry.default_wheel, "29");
        assert!(defaults.wheel_sizes.sizes.len() >= 5);
    }

    #[test]
    fn fork_defaults_match_the_nicolai_sheets() {
        let defaults = Defaults::builtin().unwrap();
        let fork = &defaults.forks.forks["29"];
        assert!((fork.axle_to_crown(170.0) - 582.0).abs() < 1.0);
        assert!((fork.axle_to_crown(160.0) - 573.7).abs() < 3.0);
        assert_eq!(fork.axle_to_crown(0.0), fork.rigid_axle_to_crown);
    }

    #[test]
    fn a_malformed_table_is_an_error() {
        let err = Defaults::from_toml("sizes = 3", FORK_TOML, GEOMETRY_TOML).unwrap_err();
        assert!(err.to_string().starts_with("wheel_size.toml"), "{err}");
    }

    #[test]
    fn a_missing_source_is_an_error() {
        let wheels = WHEEL_SIZE_TOML.replace(
            "source = \"Nicolai S16 MGU 2026 tech sheet: tyre diameter 745 mm (29 x 2.4).\"",
            "source = \"\"",
        );
        assert!(Defaults::from_toml(&wheels, FORK_TOML, GEOMETRY_TOML).is_err());
    }
}
