//! Data types, unit newtypes and serialization for bikestudio projects.
//!
//! This crate holds inputs only. Nothing derived is ever stored here.

mod project;
mod units;

pub use project::{CURRENT_SCHEMA_VERSION, LoadError, Project, WheelSize};
pub use units::{Degrees, Millimeters};
