//! Solver outputs. Never saved, always recomputed.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::geometry::GeometryVariable;
use crate::tubes::{Material, TubeKind};

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Something the designer should know: a missing input, a default in use, a conflict.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// A stable identifier such as `geometry.conflict`.
    pub code: String,
    pub message: String,
}

impl Diagnostic {
    pub fn new(severity: Severity, code: &str, message: impl Into<String>) -> Self {
        Self {
            severity,
            code: code.to_string(),
            message: message.into(),
        }
    }
}

/// A 2D point in millimetres: origin at the crank centre, +X toward the
/// front wheel, +Y up, X along the ground.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Where a geometry value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueSource {
    /// Set by the designer.
    Pinned,
    /// Calculated to satisfy the geometry.
    Solved,
    /// Filled from `bike-defaults/geometry.toml`.
    Default,
}

/// The frame's key points in one state (static, at sag or at full travel).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FramePoints {
    pub crank_centre: Point,
    pub rear_axle: Point,
    pub front_axle: Point,
    pub rear_contact: Point,
    pub front_contact: Point,
    pub head_tube_top: Point,
    pub head_tube_bottom: Point,
    pub saddle: Point,
    /// Where the seat tube axis crosses the crank centre's height in the static frame.
    pub seat_tube_at_crank_height: Point,
    /// Pedal axle with the crank straight down.
    pub pedal_down: Point,
    /// Pedal axle with the crank forward and horizontal.
    pub pedal_forward: Point,
}

/// Every geometry value and point in one state of the bike.
#[derive(Debug, Clone, PartialEq)]
pub struct GeometryState {
    pub values: BTreeMap<GeometryVariable, f64>,
    pub trail: f64,
    /// Crank centre to front axle, horizontal.
    pub front_centre: f64,
    /// Crank centre to rear axle, horizontal.
    pub rear_centre: f64,
    pub points: FramePoints,
    /// Pedal underside to the ground, crank straight down.
    pub pedal_ground_clearance: f64,
    /// Pedal to the front tyre, crank forward and horizontal, wheel straight ahead.
    pub pedal_front_tyre_clearance: f64,
}

impl GeometryState {
    pub fn value(&self, variable: GeometryVariable) -> f64 {
        self.values[&variable]
    }
}

/// The solved geometry (S-GEOM).
#[derive(Debug, Clone, PartialEq)]
pub struct SolvedGeometry {
    pub sources: BTreeMap<GeometryVariable, ValueSource>,
    pub front_wheel_diameter: f64,
    pub rear_wheel_diameter: f64,
    pub fork_axle_to_crown: f64,
    pub fork_offset: f64,
    pub crank_length: f64,
    pub pedal_length: f64,
    pub pedal_thickness: f64,
    /// The frame as built, unweighted.
    pub static_state: GeometryState,
    /// Hardtails only: at fork sag. Full suspension comes with S-KINEMATICS.
    pub at_sag: Option<GeometryState>,
    /// Hardtails only: at full fork travel.
    pub full_travel: Option<GeometryState>,
}

/// Everything the solve pipeline produces.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Solved {
    pub geometry: Option<SolvedGeometry>,
    pub tubes: Option<SolvedTubes>,
    pub diagnostics: Vec<Diagnostic>,
}

/// One tube (or pair) of the frame, in the static frame (S-TUBES).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tube {
    pub kind: TubeKind,
    /// Centreline start and end. The BB shell runs across the frame, so both are the crank centre.
    pub start: Point,
    pub end: Point,
    /// Centreline length in mm; for the BB shell, its width.
    pub length: f64,
    pub outside_diameter: f64,
    pub wall: f64,
    /// 2 for the stays (T5).
    pub count: u32,
    /// Grams, all `count` tubes together.
    pub mass: f64,
    pub centre_of_mass: Point,
    /// `Pinned` when the project overrides it, else `Default`.
    pub diameter_source: ValueSource,
    pub wall_source: ValueSource,
}

/// A fittings allowance (dropouts, bosses, welds) placed at a fixed point (T5).
#[derive(Debug, Clone, PartialEq)]
pub struct TubeFitting {
    pub name: String,
    /// Grams.
    pub mass: f64,
    pub position: Point,
}

/// The naive tube frame (S-TUBES).
#[derive(Debug, Clone, PartialEq)]
pub struct SolvedTubes {
    pub material: Material,
    pub material_source: ValueSource,
    pub seat_tube_length: f64,
    pub seat_tube_length_source: ValueSource,
    pub seat_tube_overhang: f64,
    pub top_tube_below_head_tube_top: f64,
    pub down_tube_above_head_tube_bottom: f64,
    pub seat_tube_top: Point,
    pub tubes: Vec<Tube>,
    pub fittings: Vec<TubeFitting>,
    /// Grams: tubes plus fittings.
    pub frame_mass: f64,
    pub frame_centre_of_mass: Point,
}

impl SolvedTubes {
    pub fn tube(&self, kind: TubeKind) -> Option<&Tube> {
        self.tubes.iter().find(|t| t.kind == kind)
    }
}
