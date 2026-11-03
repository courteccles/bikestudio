//! Naive tube inputs (S-TUBES): material, seat tube length, junctions, sections.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::geometry::Parameter;
use crate::units::Millimeters;

/// The frame material (S-TUBES T4). One per frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Material {
    /// Chromoly 4130.
    Steel,
    /// 6061.
    Aluminium,
    /// 3Al-2.5V.
    Titanium,
    /// Generic carbon; the wall is an equivalent wall.
    Carbon,
}

impl Material {
    pub const ALL: [Material; 4] = [
        Material::Steel,
        Material::Aluminium,
        Material::Titanium,
        Material::Carbon,
    ];

    /// The key used in `bike-defaults/tubes.toml`.
    pub fn key(self) -> &'static str {
        match self {
            Material::Steel => "steel",
            Material::Aluminium => "aluminium",
            Material::Titanium => "titanium",
            Material::Carbon => "carbon",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Material::Steel => "Steel",
            Material::Aluminium => "Aluminium",
            Material::Titanium => "Titanium",
            Material::Carbon => "Carbon",
        }
    }
}

/// A tube of the frame (S-TUBES T1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TubeKind {
    HeadTube,
    TopTube,
    DownTube,
    SeatTube,
    /// A pair.
    Chainstay,
    /// A pair.
    Seatstay,
    BottomBracketShell,
}

impl TubeKind {
    pub const ALL: [TubeKind; 7] = [
        TubeKind::HeadTube,
        TubeKind::TopTube,
        TubeKind::DownTube,
        TubeKind::SeatTube,
        TubeKind::Chainstay,
        TubeKind::Seatstay,
        TubeKind::BottomBracketShell,
    ];

    /// The key used in `bike-defaults/tubes.toml`.
    pub fn key(self) -> &'static str {
        match self {
            TubeKind::HeadTube => "head_tube",
            TubeKind::TopTube => "top_tube",
            TubeKind::DownTube => "down_tube",
            TubeKind::SeatTube => "seat_tube",
            TubeKind::Chainstay => "chainstay",
            TubeKind::Seatstay => "seatstay",
            TubeKind::BottomBracketShell => "bottom_bracket_shell",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TubeKind::HeadTube => "Head tube",
            TubeKind::TopTube => "Top tube",
            TubeKind::DownTube => "Down tube",
            TubeKind::SeatTube => "Seat tube",
            TubeKind::Chainstay => "Chainstays",
            TubeKind::Seatstay => "Seatstays",
            TubeKind::BottomBracketShell => "BB shell",
        }
    }

    /// Stays come in pairs (T5).
    pub fn count(self) -> u32 {
        match self {
            TubeKind::Chainstay | TubeKind::Seatstay => 2,
            _ => 1,
        }
    }

    /// Rear members: on a full-suspension frame these are links (S-LINK), not S-TUBES tubes.
    pub fn is_rear(self) -> bool {
        matches!(self, TubeKind::Chainstay | TubeKind::Seatstay)
    }
}

/// A tube's round section; `None` uses the material's default (T4).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TubeSection {
    pub outside_diameter: Option<Millimeters>,
    pub wall: Option<Millimeters>,
}

/// Where tubes join the head tube and seat tube (T3); `None` uses the default.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JunctionOffsets {
    /// Top tube centreline below the head tube top, along the head tube axis.
    pub top_tube_below_head_tube_top: Option<Millimeters>,
    /// Down tube centreline above the head tube bottom, along the head tube axis.
    pub down_tube_above_head_tube_bottom: Option<Millimeters>,
    /// Top tube and seatstays below the seat tube top, along the seat tube axis
    /// (Nicolai's seat tube overhang).
    pub seat_tube_overhang: Option<Millimeters>,
}

/// The designer's tube choices (S-TUBES).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TubeInputs {
    /// `None` uses the default material.
    pub material: Option<Material>,
    /// Along the actual seat tube axis, from crank height to the top (T2).
    #[serde(default)]
    pub seat_tube_length: Parameter,
    #[serde(default)]
    pub junctions: JunctionOffsets,
    /// Per-tube overrides; tubes not listed use the material's defaults.
    #[serde(default)]
    pub sections: BTreeMap<TubeKind, TubeSection>,
}
