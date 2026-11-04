//! Side-view drawings from component data (C13), in each component's own
//! coordinates. S-SHELL draws them; S-COMPONENTS checks they're valid (C11)
//! and takes a centre of mass from them when the source gives none (C10).
//!
//! Forks and shocks are drawn from a few dimensions. Where an entry leaves a
//! drawing dimension out, the proportions below stand in: they shape the
//! picture only and never feed a solver.

use bike_model::shape::{circle, rectangle};
use bike_model::{Battery, DriveUnitComponent, Fork, Point, Shock, ShockMount, SpringType};

/// Lower leg outside diameter over the stanchion's, when not given.
const LOWER_LEG_EXTRA: f64 = 8.0;
/// A rigid fork blade's diameter, when not given.
const RIGID_BLADE: f64 = 28.0;
const CROWN_HEIGHT: f64 = 40.0;
/// Dropout below the axle.
const DROPOUT: f64 = 20.0;
/// Stanchion showing above the lower leg at full extension, beyond the travel.
const SEAL_CLEARANCE: f64 = 20.0;
/// Stanchion inside the lower leg at full extension.
const STANCHION_OVERLAP: f64 = 100.0;
/// Steerer shown above the crown.
const STEERER_SHOWN: f64 = 40.0;
const MM_PER_INCH: f64 = 25.4;

const SHOCK_EYE_RADIUS: f64 = 11.0;
const TRUNNION_HALF_LENGTH: f64 = 15.0;
const TRUNNION_HALF_HEIGHT: f64 = 20.0;
const AIR_CAN: f64 = 50.0;
const COIL_SPRING: f64 = 60.0;
const SHOCK_BODY: f64 = 32.0;
const SHOCK_SHAFT: f64 = 12.0;
const RESERVOIR: f64 = 30.0;
/// Where the can or coil starts along the body, as a fraction of its length.
const SLEEVE_START: f64 = 0.4;
const CIRCLE_SIDES: usize = 24;

/// A side view: closed polygons, and an optional SVG path override (C13).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Drawing {
    pub shapes: Vec<Vec<Point>>,
    pub svg_path: Option<String>,
}

/// The fork with its stanchions `compression` mm into the travel (0 = static).
pub fn fork(fork: &Fork, compression: f64) -> Drawing {
    let a2c = fork.axle_to_crown.0;
    let travel = fork.travel.0;
    let offset = fork.offset.0;
    let compression = compression.clamp(0.0, travel.max(0.0));
    let crown_bottom = a2c - compression;
    let crown_height = fork.crown_height.map_or(CROWN_HEIGHT, |c| c.0);
    let steerer_bottom = match fork.steerer {
        bike_model::Steerer::Tapered { bottom, .. } => bottom,
        bike_model::Steerer::Straight { diameter } => diameter,
    } * MM_PER_INCH;

    let mut shapes = Vec::new();
    if travel > 0.0 {
        let stanchion = fork.stanchion_diameter.0;
        let lower = fork
            .lower_leg_diameter
            .map_or(stanchion + LOWER_LEG_EXTRA, |l| l.0);
        let lower_top = a2c - travel - SEAL_CLEARANCE;
        shapes.push(rectangle(-lower / 2.0, -DROPOUT, lower / 2.0, lower_top));
        shapes.push(rectangle(
            -stanchion / 2.0,
            lower_top - STANCHION_OVERLAP + compression,
            stanchion / 2.0,
            crown_bottom,
        ));
    } else {
        let blade = fork.lower_leg_diameter.map_or(RIGID_BLADE, |l| l.0);
        shapes.push(rectangle(-blade / 2.0, -DROPOUT, blade / 2.0, crown_bottom));
    }
    let leg_half = shapes[0][1].x;
    shapes.push(rectangle(
        -offset - steerer_bottom / 2.0,
        crown_bottom,
        leg_half.max(-offset + steerer_bottom / 2.0),
        crown_bottom + crown_height,
    ));
    shapes.push(rectangle(
        -offset - steerer_bottom / 2.0,
        crown_bottom + crown_height,
        -offset + steerer_bottom / 2.0,
        crown_bottom + crown_height + STEERER_SHOWN,
    ));
    Drawing {
        shapes,
        svg_path: fork.svg_path.clone(),
    }
}

/// The shock `compression` mm into its stroke (0 = fully extended).
pub fn shock(shock: &Shock, compression: f64) -> Drawing {
    let e2e = shock.eye_to_eye.0;
    let stroke = shock.stroke.0;
    let compression = compression.clamp(0.0, stroke.max(0.0));
    let shaft_eye = e2e - compression;
    let body_end = e2e - stroke - SHOCK_EYE_RADIUS;
    let body = shock.body_diameter.map_or(SHOCK_BODY, |d| d.0);
    let sleeve = match shock.spring {
        SpringType::Air => shock.can_diameter.map_or(AIR_CAN, |d| d.0),
        SpringType::Coil => shock.can_diameter.map_or(COIL_SPRING, |d| d.0),
    };

    let mut shapes = vec![match shock.mount {
        ShockMount::Eyelet => circle(Point::new(0.0, 0.0), SHOCK_EYE_RADIUS, CIRCLE_SIDES),
        ShockMount::Trunnion => rectangle(
            -TRUNNION_HALF_LENGTH,
            -TRUNNION_HALF_HEIGHT,
            TRUNNION_HALF_LENGTH,
            TRUNNION_HALF_HEIGHT,
        ),
    }];
    shapes.push(rectangle(0.0, -body / 2.0, body_end, body / 2.0));
    // The air can or coil, over the outer part of the body.
    shapes.push(rectangle(
        body_end * SLEEVE_START,
        -sleeve / 2.0,
        body_end,
        sleeve / 2.0,
    ));
    shapes.push(rectangle(
        body_end,
        -SHOCK_SHAFT / 2.0,
        shaft_eye,
        SHOCK_SHAFT / 2.0,
    ));
    shapes.push(circle(
        Point::new(shaft_eye, 0.0),
        SHOCK_EYE_RADIUS,
        CIRCLE_SIDES,
    ));
    if shock.reservoir {
        shapes.push(rectangle(0.0, body / 2.0, body_end, body / 2.0 + RESERVOIR));
    }
    Drawing {
        shapes,
        svg_path: shock.svg_path.clone(),
    }
}

/// The drive unit's outline and its mounting bolts.
pub fn drive_unit(unit: &DriveUnitComponent) -> Drawing {
    let mut shapes = vec![unit.outline.clone()];
    shapes.extend(
        unit.mounts
            .iter()
            .map(|m| circle(m.position, m.diameter.0 / 2.0, CIRCLE_SIDES)),
    );
    Drawing {
        shapes,
        svg_path: unit.svg_path.clone(),
    }
}

/// The battery's outline, or its length × height rectangle.
pub fn battery(battery: &Battery) -> Drawing {
    let outline = if battery.outline.is_empty() {
        let h = battery.height.0 / 2.0;
        rectangle(0.0, -h, battery.length.0, h)
    } else {
        battery.outline.clone()
    };
    Drawing {
        shapes: vec![outline],
        svg_path: battery.svg_path.clone(),
    }
}
