//! Data types, unit newtypes and serialization for bikestudio projects.
//!
//! `Project` holds inputs only and is the only thing saved. The output types
//! in `solved` are recomputed every time and never stored.

mod components;
mod geometry;
mod project;
pub mod shape;
mod solved;
mod tubes;
mod units;

pub use components::{
    Battery, BatteryPosition, BatterySetup, ComponentId, ComponentName, ComponentReference,
    Components, DriveSystem, DriveUnit, DriveUnitComponent, DriveUnitKind, Fork, MassProperties,
    MotorSpecification, Mount, OutlineAccuracy, ResolvedBattery, ResolvedComponents,
    ResolvedDriveUnit, ResolvedFork, ResolvedShock, Shock, ShockMount, SpringType, Steerer,
};
pub use geometry::{
    CrankSetup, ForkOverrides, GeometryInputs, GeometryVariable, Parameter, PedalSize,
};
pub use project::{CURRENT_SCHEMA_VERSION, LoadError, Project, WheelSize};
pub use solved::{
    Bend, Diagnostic, FramePoints, GeometryState, Point, Severity, Solved, SolvedGeometry,
    SolvedTubes, Tube, TubeFitting, ValueSource,
};
pub use tubes::{JunctionOffsets, Material, TubeInputs, TubeKind, TubeSection};
pub use units::{
    ANGLE_DECIMALS, ANGLE_LABEL, Degrees, LengthUnit, MM_PER_INCH, Millimeters, ParseError,
    TRAVEL_UNIT, commit_angle, commit_length, format_angle, format_length, parse_angle,
    parse_length,
};
