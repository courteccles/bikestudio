//! Components (S-COMPONENTS): forks, shocks, drive units and batteries.
//!
//! A design refers to a component either by its library id or by carrying a
//! custom one (C3). Every component has a mass, a centre of mass in its own
//! coordinates and a side-view drawing (C1, C13). Nothing requires a
//! component: with none selected the defaults stand in (design principle 7).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::project::WheelSize;
use crate::solved::Point;
use crate::units::{Degrees, Millimeters};

/// A library component's stable id, `make/model/year/variant`, for example
/// `rockshox/lyrik-ultimate/2025/29-160-44` (C3).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ComponentId(pub String);

impl fmt::Display for ComponentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A library component by id, or a custom one with the same fields (C3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentReference<T> {
    Library(ComponentId),
    Custom(T),
}

/// Who made a component and which one it is (C1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentName {
    pub make: String,
    pub model: String,
    pub year: u16,
    /// The size or option within the model, for example `29-160-44`.
    pub variant: String,
}

impl ComponentName {
    /// The library id this name gives: lower case, spaces as hyphens.
    pub fn id(&self) -> ComponentId {
        let slug = |s: &str| s.trim().to_lowercase().replace(' ', "-");
        ComponentId(format!(
            "{}/{}/{}/{}",
            slug(&self.make),
            slug(&self.model),
            self.year,
            slug(&self.variant)
        ))
    }
}

impl fmt::Display for ComponentName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} ({})",
            self.make, self.model, self.year, self.variant
        )
    }
}

/// Mass in grams and where it sits, in the component's own coordinates (C1).
/// `centre_of_mass: None` means the source doesn't give one, so the centroid of
/// the component's drawing is used (C10).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MassProperties {
    pub mass: f64,
    #[serde(default)]
    pub centre_of_mass: Option<Point>,
}

/// The fork's steerer tube.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Steerer {
    /// Inches, for example 1.125 at the top and 1.5 at the crown.
    Tapered {
        top: f64,
        bottom: f64,
    },
    Straight {
        diameter: f64,
    },
}

/// A suspension or rigid fork (C4). Own coordinates: origin at the axle, +Y up
/// the steering axis, +X forward; the steering axis is the line x = -offset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fork {
    pub name: ComponentName,
    pub wheel: WheelSize,
    /// 0 for a rigid fork.
    pub travel: Millimeters,
    pub axle_to_crown: Millimeters,
    /// Perpendicular distance from the steering axis to the axle.
    pub offset: Millimeters,
    pub steerer: Steerer,
    /// Upper tube (stanchion) diameter; 0 for a rigid fork.
    pub stanchion_diameter: Millimeters,
    /// Drawing dimensions the source may not give (C13); `None` uses drawing proportions.
    #[serde(default)]
    pub lower_leg_diameter: Option<Millimeters>,
    #[serde(default)]
    pub crown_height: Option<Millimeters>,
    pub mass: MassProperties,
    #[serde(default)]
    pub svg_path: Option<String>,
    pub source: String,
}

/// How a shock mounts at its body end (C5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShockMount {
    Eyelet,
    Trunnion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpringType {
    Air,
    Coil,
}

/// A rear shock (C5). Own coordinates: origin at the body eye (or trunnion
/// axis), +X toward the shaft eye.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shock {
    pub name: ComponentName,
    pub eye_to_eye: Millimeters,
    pub stroke: Millimeters,
    pub mount: ShockMount,
    pub spring: SpringType,
    /// Drawing dimensions the source may not give (C13); `None` uses drawing proportions.
    #[serde(default)]
    pub body_diameter: Option<Millimeters>,
    #[serde(default)]
    pub can_diameter: Option<Millimeters>,
    #[serde(default)]
    pub reservoir: bool,
    pub mass: MassProperties,
    #[serde(default)]
    pub svg_path: Option<String>,
    pub source: String,
}

/// What a drive unit does (C6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveUnitKind {
    Gearbox,
    Motor,
    /// A motor and gearbox in one body, such as the Pinion MGU.
    MotorGearbox,
}

/// A motor's published output (C6).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotorSpecification {
    /// Watts.
    pub rated_power: f64,
    /// Watts, where given.
    #[serde(default)]
    pub peak_power: Option<f64>,
    /// Newton metres.
    pub torque: f64,
}

/// A bolt the frame must provide (C6).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mount {
    pub position: Point,
    pub diameter: Millimeters,
}

/// How closely an outline follows its source (C7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutlineAccuracy {
    #[default]
    Exact,
    Approximate,
}

/// A gearbox or mid-drive motor (C6). Own coordinates: origin at the crank
/// centre, which is also the output axis; +X forward, +Y up when unrotated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriveUnitComponent {
    pub name: ComponentName,
    pub kind: DriveUnitKind,
    #[serde(default)]
    pub motor: Option<MotorSpecification>,
    pub mounts: Vec<Mount>,
    /// The casing, a closed polygon (first point not repeated).
    pub outline: Vec<Point>,
    #[serde(default)]
    pub outline_accuracy: OutlineAccuracy,
    /// Teeth on a fixed output sprocket.
    #[serde(default)]
    pub output_sprocket: Option<u32>,
    #[serde(default)]
    pub ratios: Option<Vec<f64>>,
    pub mass: MassProperties,
    /// Grams of frame-side mounting plate, counted as frame mass (C8).
    #[serde(default)]
    pub frame_mount_mass: f64,
    #[serde(default)]
    pub svg_path: Option<String>,
    pub source: String,
}

/// An e-bike battery (C9). Own coordinates: origin at the battery's lower
/// end on its long axis, +X along its length toward the head tube.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Battery {
    pub name: ComponentName,
    /// Watt hours.
    pub capacity: f64,
    /// Volts.
    pub nominal_voltage: f64,
    #[serde(default)]
    pub amp_hours: Option<f64>,
    pub length: Millimeters,
    pub width: Millimeters,
    pub height: Millimeters,
    /// Side view, a closed polygon; empty draws the length × height rectangle.
    #[serde(default)]
    pub outline: Vec<Point>,
    pub mass: MassProperties,
    #[serde(default)]
    pub svg_path: Option<String>,
    pub source: String,
}

/// The selected fork and shock (C3). `None` uses the defaults.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Components {
    #[serde(default)]
    pub fork: Option<ComponentReference<Fork>>,
    #[serde(default)]
    pub shock: Option<ComponentReference<Shock>>,
}

/// A drive unit as placed in this design (C6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriveUnit {
    pub component: ComponentReference<DriveUnitComponent>,
    /// About the crank axis, counter-clockwise in the side view.
    #[serde(default)]
    pub rotation: Degrees,
}

/// Which side of the down tube the battery sits (C9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatteryPosition {
    #[default]
    Inside,
    Under,
}

/// A battery as placed in this design (C9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatterySetup {
    pub component: ComponentReference<Battery>,
    /// From the down tube's lower end to the battery's near end, along the tube.
    pub distance_from_down_tube_end: Millimeters,
    #[serde(default)]
    pub position: BatteryPosition,
}

/// The drive unit and battery. Rear drive and idlers wait for S-FORCES (C12).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriveSystem {
    /// `None` is a bare bottom bracket.
    #[serde(default)]
    pub unit: Option<DriveUnit>,
    #[serde(default)]
    pub battery: Option<BatterySetup>,
}

/// A fork after resolution (C12): concrete values for the geometry solver.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedFork {
    pub fork: Fork,
    /// `Some` when it came from the library.
    pub id: Option<ComponentId>,
    /// In the fork's own coordinates; the drawing centroid when the source gives none.
    pub centre_of_mass: Point,
}

/// A shock after resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedShock {
    pub shock: Shock,
    pub id: Option<ComponentId>,
    pub centre_of_mass: Point,
}

/// A drive unit after resolution, rotated into the frame's coordinates (origin
/// at the crank centre).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedDriveUnit {
    pub unit: DriveUnitComponent,
    pub id: Option<ComponentId>,
    pub rotation: Degrees,
    pub outline: Vec<Point>,
    pub mounts: Vec<Point>,
    pub centre_of_mass: Point,
}

/// A battery after resolution. Its frame position needs the down tube, so
/// S-MASS places it.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedBattery {
    pub battery: Battery,
    pub id: Option<ComponentId>,
    pub placement: BatterySetup,
    /// In the battery's own coordinates.
    pub centre_of_mass: Point,
}

/// Every selected component turned into concrete values (C12). Empty when
/// nothing is selected, so the solvers use the defaults.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResolvedComponents {
    pub fork: Option<ResolvedFork>,
    pub shock: Option<ResolvedShock>,
    pub drive_unit: Option<ResolvedDriveUnit>,
    pub battery: Option<ResolvedBattery>,
}
