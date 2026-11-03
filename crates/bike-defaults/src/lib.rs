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
const TUBES_TOML: &str = include_str!("../data/tubes.toml");

/// All default tables.
#[derive(Debug, Clone, PartialEq)]
pub struct Defaults {
    pub wheel_sizes: WheelSizes,
    pub forks: Forks,
    pub geometry: GeometryDefaults,
    pub tubes: TubeDefaults,
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
    pub seat_tube_length: SeatTubeLengthDefaults,
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

/// Seat tube length by rider size preset (S-TUBES T2). Until S-MASS adds the rider, Medium is used.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeatTubeLengthDefaults {
    pub small: f64,
    pub medium: f64,
    pub large: f64,
    pub source: String,
}

/// `tubes.toml`: materials, default sections, junctions and fittings (S-TUBES).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TubeDefaults {
    /// A key in `materials`.
    pub default_material: String,
    pub bottom_bracket_shell_width: f64,
    pub bottom_bracket_shell_source: String,
    pub junctions: JunctionDefaults,
    pub materials: BTreeMap<String, MaterialEntry>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JunctionDefaults {
    pub top_tube_below_head_tube_top: f64,
    pub down_tube_above_head_tube_bottom: f64,
    pub seat_tube_overhang: f64,
    /// The seat tube must reach at least this far below its junction (T2).
    pub min_seat_tube_below_junction: f64,
    pub source: String,
}

/// One material: density (g/cm3), default `[outside diameter, wall]` per tube, fittings.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialEntry {
    pub density: f64,
    pub density_source: String,
    pub sections_source: String,
    pub head_tube: [f64; 2],
    pub top_tube: [f64; 2],
    pub down_tube: [f64; 2],
    pub seat_tube: [f64; 2],
    pub chainstay: [f64; 2],
    pub seatstay: [f64; 2],
    pub bottom_bracket_shell: [f64; 2],
    pub fittings: Vec<FittingEntry>,
    pub fittings_source: String,
}

impl MaterialEntry {
    /// The default `[outside diameter, wall]` for a tube, by its `tubes.toml` key.
    pub fn section(&self, tube_key: &str) -> Option<[f64; 2]> {
        Some(match tube_key {
            "head_tube" => self.head_tube,
            "top_tube" => self.top_tube,
            "down_tube" => self.down_tube,
            "seat_tube" => self.seat_tube,
            "chainstay" => self.chainstay,
            "seatstay" => self.seatstay,
            "bottom_bracket_shell" => self.bottom_bracket_shell,
            _ => return None,
        })
    }

    fn sections(&self) -> [[f64; 2]; 7] {
        [
            self.head_tube,
            self.top_tube,
            self.down_tube,
            self.seat_tube,
            self.chainstay,
            self.seatstay,
            self.bottom_bracket_shell,
        ]
    }
}

/// Where a fitting's mass sits (T5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FittingPlace {
    RearAxle,
    CrankCentre,
    HeadTube,
    SeatTubeTop,
    /// At the tubes' combined centre of mass (welds, bosses spread over the frame).
    Tubes,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FittingEntry {
    pub name: String,
    /// Grams.
    pub mass: f64,
    pub at: FittingPlace,
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
        Self::from_toml(WHEEL_SIZE_TOML, FORK_TOML, GEOMETRY_TOML, TUBES_TOML)
    }

    /// Parses and validates the four tables.
    pub fn from_toml(
        wheel_sizes: &str,
        forks: &str,
        geometry: &str,
        tubes: &str,
    ) -> Result<Self, DefaultsError> {
        let defaults = Self {
            wheel_sizes: parse("wheel_size.toml", wheel_sizes)?,
            forks: parse("fork.toml", forks)?,
            geometry: parse("geometry.toml", geometry)?,
            tubes: parse("tubes.toml", tubes)?,
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
        let seat = &g.seat_tube_length;
        if [seat.small, seat.medium, seat.large]
            .iter()
            .any(|v| !(*v > 0.0))
            || seat.source.trim().is_empty()
        {
            return invalid(
                "geometry.toml",
                "seat tube lengths need positive values and a source".into(),
            );
        }
        self.validate_tubes()
    }

    fn validate_tubes(&self) -> Result<(), DefaultsError> {
        let invalid = |message: String| {
            Err(DefaultsError::Invalid {
                file: "tubes.toml",
                message,
            })
        };
        let t = &self.tubes;
        if !t.materials.contains_key(&t.default_material) {
            return invalid(format!(
                "default_material {} isn't a material",
                t.default_material
            ));
        }
        for key in ["steel", "aluminium", "titanium", "carbon"] {
            if !t.materials.contains_key(key) {
                return invalid(format!("no material {key}"));
            }
        }
        let j = &t.junctions;
        if [
            j.top_tube_below_head_tube_top,
            j.down_tube_above_head_tube_bottom,
            j.seat_tube_overhang,
            j.min_seat_tube_below_junction,
        ]
        .iter()
        .any(|v| !(*v >= 0.0))
            || j.source.trim().is_empty()
            || !(t.bottom_bracket_shell_width > 0.0)
            || t.bottom_bracket_shell_source.trim().is_empty()
        {
            return invalid("junctions and BB shell need valid values and sources".into());
        }
        for (key, m) in &t.materials {
            let sections_ok = m
                .sections()
                .iter()
                .all(|[d, w]| *d > 0.0 && *w > 0.0 && *w < d / 2.0);
            let fittings_ok = m
                .fittings
                .iter()
                .all(|f| f.mass >= 0.0 && !f.name.trim().is_empty());
            if !(m.density > 0.0)
                || !sections_ok
                || !fittings_ok
                || [&m.density_source, &m.sections_source, &m.fittings_source]
                    .iter()
                    .any(|s| s.trim().is_empty())
            {
                return invalid(format!(
                    "material {key} needs a positive density, walls under half the diameter, valid fittings and sources"
                ));
            }
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
        let err =
            Defaults::from_toml("sizes = 3", FORK_TOML, GEOMETRY_TOML, TUBES_TOML).unwrap_err();
        assert!(err.to_string().starts_with("wheel_size.toml"), "{err}");
    }

    #[test]
    fn a_missing_source_is_an_error() {
        let wheels = WHEEL_SIZE_TOML.replace(
            "source = \"Nicolai S16 MGU 2026 tech sheet: tyre diameter 745 mm (29 x 2.4).\"",
            "source = \"\"",
        );
        assert!(Defaults::from_toml(&wheels, FORK_TOML, GEOMETRY_TOML, TUBES_TOML).is_err());
    }

    #[test]
    fn tube_defaults_cover_every_material() {
        let defaults = Defaults::builtin().unwrap();
        assert_eq!(defaults.tubes.materials.len(), 4);
        assert_eq!(defaults.tubes.junctions.seat_tube_overhang, 60.0);
        assert_eq!(defaults.geometry.seat_tube_length.medium, 450.0);
    }

    #[test]
    fn a_wall_too_thick_for_its_tube_is_an_error() {
        let tubes = TUBES_TOML.replace("head_tube = [44.5, 1.1]", "head_tube = [44.5, 30.0]");
        let err =
            Defaults::from_toml(WHEEL_SIZE_TOML, FORK_TOML, GEOMETRY_TOML, &tubes).unwrap_err();
        assert!(err.to_string().starts_with("tubes.toml"), "{err}");
    }
}
