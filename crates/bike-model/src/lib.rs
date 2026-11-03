//! Data types, unit newtypes and serialization for bikestudio projects.
//!
//! `Project` holds inputs only and is the only thing saved. The output types
//! in `solved` are recomputed every time and never stored.

mod geometry;
mod project;
mod solved;
mod units;

pub use geometry::{
    CrankSetup, ForkOverrides, GeometryInputs, GeometryVariable, Parameter, PedalSize,
};
pub use project::{CURRENT_SCHEMA_VERSION, LoadError, Project, WheelSize};
pub use solved::{
    Diagnostic, FramePoints, GeometryState, Point, Severity, Solved, SolvedGeometry, ValueSource,
};
pub use units::{
    ANGLE_DECIMALS, ANGLE_LABEL, Degrees, LengthUnit, MM_PER_INCH, Millimeters, ParseError,
    TRAVEL_UNIT, commit_angle, commit_length, format_angle, format_length, parse_angle,
    parse_length,
};
