//! The built-in component library: TOML files compiled in, parsed and validated (C2).

use std::collections::BTreeMap;

use bike_model::{
    Battery, ComponentId, ComponentName, DriveUnitComponent, Fork, MassProperties, Millimeters,
    Shock, ShockMount, SpringType, Steerer, WheelSize,
};
use serde::Deserialize;

use crate::validate;

/// Every library file, by path under `data/`. A new file is listed here.
pub const FILES: &[(&str, &str)] = &[
    (
        "forks/rockshox.toml",
        include_str!("../data/forks/rockshox.toml"),
    ),
    (
        "shocks/rockshox.toml",
        include_str!("../data/shocks/rockshox.toml"),
    ),
];

/// Every library component by id.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Library {
    pub forks: BTreeMap<ComponentId, Fork>,
    pub shocks: BTreeMap<ComponentId, Shock>,
    pub drive_units: BTreeMap<ComponentId, DriveUnitComponent>,
    pub batteries: BTreeMap<ComponentId, Battery>,
}

/// A library file that couldn't be read or failed validation.
#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("{file} isn't valid: {source}")]
    Parse {
        file: String,
        source: toml::de::Error,
    },
    #[error("{file}: {message}")]
    Invalid { file: String, message: String },
}

impl Library {
    /// The built-in library, parsed and validated.
    pub fn builtin() -> Result<Self, LibraryError> {
        Self::from_files(FILES.iter().copied())
    }

    /// Parses and validates library files given as `(path, text)`. The path's
    /// first directory (`forks`, `shocks`, `drive_units`, `batteries`) says the kind.
    pub fn from_files<'a>(
        files: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<Self, LibraryError> {
        let mut library = Library::default();
        for (path, text) in files {
            library.add_file(path, text)?;
        }
        Ok(library)
    }

    /// Every component's id, for listing.
    pub fn len(&self) -> usize {
        self.forks.len() + self.shocks.len() + self.drive_units.len() + self.batteries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn add_file(&mut self, path: &str, text: &str) -> Result<(), LibraryError> {
        let invalid = |message: String| LibraryError::Invalid {
            file: path.to_string(),
            message,
        };
        let kind = path.split('/').next().unwrap_or_default();
        match kind {
            "forks" => {
                let file: ForkFile = parse(path, text)?;
                for fork in file.forks().map_err(invalid)? {
                    insert(&mut self.forks, fork.name.id(), fork, validate::fork)
                        .map_err(invalid)?;
                }
            }
            "shocks" => {
                let file: ShockFile = parse(path, text)?;
                for shock in file.shocks().map_err(invalid)? {
                    insert(&mut self.shocks, shock.name.id(), shock, validate::shock)
                        .map_err(invalid)?;
                }
            }
            "drive_units" => {
                let file: UnitFile<DriveUnitComponent> = parse(path, text)?;
                for unit in file.units {
                    check_make(&file.make, &unit.name).map_err(invalid)?;
                    insert(
                        &mut self.drive_units,
                        unit.name.id(),
                        unit,
                        validate::drive_unit,
                    )
                    .map_err(invalid)?;
                }
            }
            "batteries" => {
                let file: UnitFile<Battery> = parse(path, text)?;
                for battery in file.units {
                    check_make(&file.make, &battery.name).map_err(invalid)?;
                    insert(
                        &mut self.batteries,
                        battery.name.id(),
                        battery,
                        validate::battery,
                    )
                    .map_err(invalid)?;
                }
            }
            other => return Err(invalid(format!("unknown component kind {other:?}"))),
        }
        Ok(())
    }
}

fn insert<T>(
    map: &mut BTreeMap<ComponentId, T>,
    id: ComponentId,
    component: T,
    check: fn(&T) -> Result<(), String>,
) -> Result<(), String> {
    check(&component).map_err(|e| format!("{id}: {e}"))?;
    if map.insert(id.clone(), component).is_some() {
        return Err(format!("{id} is listed twice"));
    }
    Ok(())
}

fn check_make(make: &str, name: &ComponentName) -> Result<(), String> {
    if name.make != make {
        return Err(format!(
            "{} is in the {make} file but made by {}",
            name.model, name.make
        ));
    }
    Ok(())
}

fn parse<T: for<'de> Deserialize<'de>>(path: &str, text: &str) -> Result<T, LibraryError> {
    toml::from_str(text).map_err(|source| LibraryError::Parse {
        file: path.to_string(),
        source,
    })
}

fn sources(model: &str, mass: &str) -> Result<(), String> {
    if model.trim().is_empty() || mass.trim().is_empty() {
        return Err("every model needs a source and every variant a mass_source".into());
    }
    Ok(())
}

/// A fork file: one make, models with their variants (C4).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ForkFile {
    make: String,
    models: Vec<ForkModel>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ForkModel {
    model: String,
    year: u16,
    steerer: Steerer,
    stanchion_diameter: f64,
    lower_leg_diameter: Option<f64>,
    crown_height: Option<f64>,
    svg_path: Option<String>,
    source: String,
    variants: Vec<ForkVariant>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ForkVariant {
    variant: String,
    wheel: String,
    travel: f64,
    axle_to_crown: f64,
    offset: f64,
    mass: MassProperties,
    mass_source: String,
}

impl ForkFile {
    fn forks(&self) -> Result<impl Iterator<Item = Fork> + '_, String> {
        for m in &self.models {
            for v in &m.variants {
                sources(&m.source, &v.mass_source)?;
            }
        }
        Ok(self.models.iter().flat_map(move |m| {
            m.variants.iter().map(move |v| Fork {
                name: ComponentName {
                    make: self.make.clone(),
                    model: m.model.clone(),
                    year: m.year,
                    variant: v.variant.clone(),
                },
                wheel: WheelSize(v.wheel.clone()),
                travel: Millimeters(v.travel),
                axle_to_crown: Millimeters(v.axle_to_crown),
                offset: Millimeters(v.offset),
                steerer: m.steerer,
                stanchion_diameter: Millimeters(m.stanchion_diameter),
                lower_leg_diameter: m.lower_leg_diameter.map(Millimeters),
                crown_height: m.crown_height.map(Millimeters),
                mass: v.mass,
                svg_path: m.svg_path.clone(),
                source: format!("{} Mass: {}", m.source, v.mass_source),
            })
        }))
    }
}

/// A shock file: one make, models with a variant per size (C5).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShockFile {
    make: String,
    models: Vec<ShockModel>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShockModel {
    model: String,
    year: u16,
    spring: SpringType,
    body_diameter: Option<f64>,
    can_diameter: Option<f64>,
    #[serde(default)]
    reservoir: bool,
    svg_path: Option<String>,
    source: String,
    variants: Vec<ShockVariant>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShockVariant {
    variant: String,
    mount: ShockMount,
    eye_to_eye: f64,
    stroke: f64,
    mass: MassProperties,
    mass_source: String,
}

impl ShockFile {
    fn shocks(&self) -> Result<impl Iterator<Item = Shock> + '_, String> {
        for m in &self.models {
            for v in &m.variants {
                sources(&m.source, &v.mass_source)?;
            }
        }
        Ok(self.models.iter().flat_map(move |m| {
            m.variants.iter().map(move |v| Shock {
                name: ComponentName {
                    make: self.make.clone(),
                    model: m.model.clone(),
                    year: m.year,
                    variant: v.variant.clone(),
                },
                eye_to_eye: Millimeters(v.eye_to_eye),
                stroke: Millimeters(v.stroke),
                mount: v.mount,
                spring: m.spring,
                body_diameter: m.body_diameter.map(Millimeters),
                can_diameter: m.can_diameter.map(Millimeters),
                reservoir: m.reservoir,
                mass: v.mass,
                svg_path: m.svg_path.clone(),
                source: format!("{} Mass: {}", m.source, v.mass_source),
            })
        }))
    }
}

/// A drive unit or battery file: one make, one entry per physical unit (C6, C9).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitFile<T> {
    make: String,
    units: Vec<T>,
}
