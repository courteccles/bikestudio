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
/// Rigid crown overhang past the steerer or blade, each side.
const RIGID_CROWN_MARGIN: f64 = 6.0;
/// Where a rigid blade starts to bend, as a fraction of the crown's height above the axle.
const RIGID_BEND_START: f64 = 0.45;
const RIGID_BEND_STEPS: usize = 24;
/// The bend's control point height, as a fraction of where the bend starts:
/// lower makes the blade arrive at the axle more upright.
const RIGID_BEND_CONTROL: f64 = 0.3;
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
    // Axle-to-crown is measured to the top of the crown, where the head tube sits.
    let crown_height = fork.crown_height.map_or(CROWN_HEIGHT, |c| c.0);
    let crown_top = a2c - compression;
    let crown_bottom = crown_top - crown_height;
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
        let lower_top = a2c - crown_height - travel - SEAL_CLEARANCE;
        shapes.push(rectangle(-lower / 2.0, -DROPOUT, lower / 2.0, lower_top));
        shapes.push(rectangle(
            -stanchion / 2.0,
            lower_top - STANCHION_OVERLAP + compression,
            stanchion / 2.0,
            crown_bottom,
        ));
    } else {
        // A rigid fork: the crown sits on the steering axis, and the blades
        // leave it along the axis, then curve forward to the axle.
        let blade = fork.lower_leg_diameter.map_or(RIGID_BLADE, |l| l.0);
        shapes.push(rigid_blade(offset, crown_bottom, blade));
        let half = (steerer_bottom / 2.0).max(blade / 2.0) + RIGID_CROWN_MARGIN;
        shapes.push(rectangle(
            -offset - half,
            crown_bottom,
            -offset + half,
            crown_top,
        ));
        shapes.push(rectangle(
            -offset - steerer_bottom / 2.0,
            crown_top,
            -offset + steerer_bottom / 2.0,
            crown_top + STEERER_SHOWN,
        ));
        return Drawing {
            shapes,
            svg_path: fork.svg_path.clone(),
        };
    }
    let leg_half = shapes[0][1].x;
    shapes.push(rectangle(
        -offset - steerer_bottom / 2.0,
        crown_bottom,
        leg_half.max(-offset + steerer_bottom / 2.0),
        crown_top,
    ));
    shapes.push(rectangle(
        -offset - steerer_bottom / 2.0,
        crown_top,
        -offset + steerer_bottom / 2.0,
        crown_top + STEERER_SHOWN,
    ));
    Drawing {
        shapes,
        svg_path: fork.svg_path.clone(),
    }
}

/// A rigid blade `width` wide: straight down the steering axis (`offset`
/// behind the axle) from the crown, then one forward curve (no S bend) into
/// the axle, ending in a rounded dropout centred on the axle.
fn rigid_blade(offset: f64, crown_bottom: f64, width: f64) -> Vec<Point> {
    let bend = crown_bottom * RIGID_BEND_START;
    // A quadratic from (−offset, bend) to the axle: it leaves along the axis
    // and bends forward the whole way.
    let (p0, p1, p2) = (
        Point::new(-offset, bend),
        Point::new(-offset, bend * RIGID_BEND_CONTROL),
        Point::new(0.0, 0.0),
    );
    let mut line = vec![Point::new(-offset, crown_bottom)];
    for i in 0..=RIGID_BEND_STEPS {
        let t = i as f64 / RIGID_BEND_STEPS as f64;
        let u = 1.0 - t;
        let (a, b, c) = (u * u, 2.0 * u * t, t * t);
        line.push(Point::new(
            a * p0.x + b * p1.x + c * p2.x,
            a * p0.y + b * p1.y + c * p2.y,
        ));
    }
    // The blade's direction arriving at the axle; the round end is centred
    // on the axle, concentric with the hub.
    let (ex, ey) = (p2.x - p1.x, p2.y - p1.y);
    let n = ex.hypot(ey);
    let dir = Point::new(ex / n, ey / n);
    let half = width / 2.0;
    let tip = p2;
    // Offset each point square to the line, half the width each side.
    let side = |k: f64| -> Vec<Point> {
        (0..line.len())
            .map(|i| {
                let (a, b) = (line[i.saturating_sub(1)], line[(i + 1).min(line.len() - 1)]);
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let n = dx.hypot(dy).max(f64::EPSILON);
                Point::new(line[i].x - dy / n * k, line[i].y + dx / n * k)
            })
            .collect()
    };
    let mut outline = side(half);
    // A half circle round the tip, from the first side to the second.
    // The first side is on the blade's left; sweep clockwise through `dir`.
    let start = dir.x.atan2(-dir.y);
    for i in 1..RIGID_BEND_STEPS {
        let a = start - std::f64::consts::PI * i as f64 / RIGID_BEND_STEPS as f64;
        outline.push(Point::new(tip.x + half * a.cos(), tip.y + half * a.sin()));
    }
    outline.extend(side(-half).into_iter().rev());
    outline
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
