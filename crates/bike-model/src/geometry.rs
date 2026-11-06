//! Geometry inputs: what the designer pins (S-GEOM).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::units::Millimeters;

/// A frame geometry value the designer can pin or leave free.
///
/// Lengths are millimetres and angles degrees. Pinned values are static: the
/// frame as built, unweighted, as geometry charts publish it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryVariable {
    /// Head tube top centre, height above the crank centre.
    Stack,
    /// Head tube top centre, horizontal distance ahead of the crank centre.
    Reach,
    /// Steering axis angle from the horizontal.
    HeadTubeAngle,
    /// Includes the lower headset stack (S-GEOM G3).
    HeadTubeLength,
    /// Crank centre to the saddle point, at stack height (G4).
    EffectiveSeatTubeAngle,
    /// The seat tube axis through the saddle point (G4).
    ActualSeatTubeAngle,
    /// From the midpoint of the line between the axles, vertically down to the crank centre (G8).
    BottomBracketDrop,
    /// Crank centre to rear axle, straight line.
    ChainstayLength,
    /// Rear axle to front axle, horizontal.
    Wheelbase,
    /// Crank centre above the ground.
    BottomBracketHeight,
    /// Crank centre to front axle, horizontal (S-EDIT-CENTERS E1).
    FrontCentre,
    /// Crank centre to rear axle, horizontal (E1).
    RearCentre,
}

impl GeometryVariable {
    /// Every variable, in display order.
    pub const ALL: [GeometryVariable; 12] = [
        GeometryVariable::HeadTubeAngle,
        GeometryVariable::EffectiveSeatTubeAngle,
        GeometryVariable::ActualSeatTubeAngle,
        GeometryVariable::Stack,
        GeometryVariable::Reach,
        GeometryVariable::HeadTubeLength,
        GeometryVariable::ChainstayLength,
        GeometryVariable::BottomBracketDrop,
        GeometryVariable::BottomBracketHeight,
        GeometryVariable::Wheelbase,
        GeometryVariable::FrontCentre,
        GeometryVariable::RearCentre,
    ];

    /// True for angles (degrees), false for lengths (millimetres).
    pub fn is_angle(self) -> bool {
        matches!(
            self,
            GeometryVariable::HeadTubeAngle
                | GeometryVariable::EffectiveSeatTubeAngle
                | GeometryVariable::ActualSeatTubeAngle
        )
    }

    /// The name shown to the designer.
    pub fn label(self) -> &'static str {
        match self {
            GeometryVariable::Stack => "Stack",
            GeometryVariable::Reach => "Reach",
            GeometryVariable::HeadTubeAngle => "Head angle",
            GeometryVariable::HeadTubeLength => "Head tube length",
            GeometryVariable::EffectiveSeatTubeAngle => "Effective seat angle",
            GeometryVariable::ActualSeatTubeAngle => "Seat tube angle",
            GeometryVariable::BottomBracketDrop => "BB drop",
            GeometryVariable::ChainstayLength => "Chainstay",
            GeometryVariable::Wheelbase => "Wheelbase",
            GeometryVariable::BottomBracketHeight => "BB height",
            GeometryVariable::FrontCentre => "Front center",
            GeometryVariable::RearCentre => "Rear center",
        }
    }
}

/// A geometry value pinned by the designer, or left free for the solver.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Parameter {
    Pinned(f64),
    #[default]
    Free,
}

impl Parameter {
    /// The pinned value, if any.
    pub fn pinned(self) -> Option<f64> {
        match self {
            Parameter::Pinned(value) => Some(value),
            Parameter::Free => None,
        }
    }
}

/// The designer's geometry pins. Variables not in the map are free.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GeometryInputs {
    pub variables: BTreeMap<GeometryVariable, Parameter>,
}

impl GeometryInputs {
    /// The pinned value, if any.
    pub fn pinned(&self, variable: GeometryVariable) -> Option<f64> {
        match self.variables.get(&variable) {
            Some(Parameter::Pinned(value)) => Some(*value),
            _ => None,
        }
    }

    /// Pins `variable` to `value`.
    pub fn pin(&mut self, variable: GeometryVariable, value: f64) {
        self.variables.insert(variable, Parameter::Pinned(value));
    }

    /// Frees `variable`.
    pub fn free(&mut self, variable: GeometryVariable) {
        self.variables.remove(&variable);
    }
}

/// Fork dimensions that override the selected fork, or the defaults in
/// `bike-defaults/fork.toml` when none is selected (S-GEOM G3, S-COMPONENTS C4).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForkOverrides {
    pub axle_to_crown: Option<Millimeters>,
    pub offset: Option<Millimeters>,
}

/// A pedal platform, centred on the pedal axle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PedalSize {
    pub length: Millimeters,
    pub thickness: Millimeters,
}

/// Crank and pedal, for clearance (S-GEOM G13). `None` uses the defaults.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrankSetup {
    pub crank_length: Option<Millimeters>,
    pub pedal: Option<PedalSize>,
}
