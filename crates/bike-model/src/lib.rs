//! Data types, unit newtypes and serialization for bikestudio projects.
//!
//! This crate holds inputs only. Nothing derived is ever stored here.

mod project;
mod units;

pub use project::{CURRENT_SCHEMA_VERSION, LoadError, Project, WheelSize};
pub use units::{
    ANGLE_DECIMALS, ANGLE_LABEL, Degrees, LengthUnit, MM_PER_INCH, Millimeters, ParseError,
    TRAVEL_UNIT, commit_angle, commit_length, format_angle, format_length, parse_angle,
    parse_length,
};
