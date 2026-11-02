//! Unit newtypes. Everything is stored and solved in millimetres and degrees.

use serde::{Deserialize, Serialize};

/// A length in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Millimeters(pub f64);

/// An angle in degrees.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Degrees(pub f64);
