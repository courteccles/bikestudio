//! Checks for library and custom components (C1, C11). Each returns every
//! problem in one message, so a contributor sees them all at once.

// `!(x > 0.0)` is deliberate throughout: unlike `x <= 0.0`, it also rejects NaN.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use bike_model::shape::{contains, is_simple};
use bike_model::{
    Battery, ComponentName, DriveUnitComponent, DriveUnitKind, Fork, MassProperties, Shock,
};

use crate::drawing::{self, Drawing};

pub fn fork(f: &Fork) -> Result<(), String> {
    let mut p = Problems::default();
    p.name(&f.name);
    p.check(f.travel.0 >= 0.0, "travel must be zero or more");
    p.check(f.axle_to_crown.0 > 0.0, "axle-to-crown must be positive");
    p.check(
        f.offset.0.is_finite() && f.offset.0 >= 0.0,
        "offset must be zero or more",
    );
    p.check(
        f.travel.0 < f.axle_to_crown.0,
        "travel must be less than axle-to-crown",
    );
    p.check(
        f.travel.0 == 0.0 || f.stanchion_diameter.0 > 0.0,
        "a suspension fork needs a stanchion diameter",
    );
    for (value, name) in [
        (f.lower_leg_diameter, "lower leg diameter"),
        (f.crown_height, "crown height"),
    ] {
        p.check(
            value.is_none_or(|v| v.0 > 0.0),
            &format!("{name} must be positive"),
        );
    }
    p.check(!f.wheel.0.trim().is_empty(), "wheel size is missing");
    p.mass(&f.mass);
    p.source(&f.source);
    if p.0.is_empty() {
        p.drawing(&drawing::fork(f, 0.0));
        p.drawing(&drawing::fork(f, f.travel.0));
    }
    p.result()
}

pub fn shock(s: &Shock) -> Result<(), String> {
    let mut p = Problems::default();
    p.name(&s.name);
    p.check(s.eye_to_eye.0 > 0.0, "eye-to-eye must be positive");
    p.check(
        s.stroke.0 > 0.0 && s.stroke.0 < s.eye_to_eye.0 / 2.0,
        "stroke must be positive and under half the eye-to-eye",
    );
    for (value, name) in [
        (s.body_diameter, "body diameter"),
        (s.can_diameter, "can diameter"),
    ] {
        p.check(
            value.is_none_or(|v| v.0 > 0.0),
            &format!("{name} must be positive"),
        );
    }
    p.mass(&s.mass);
    p.source(&s.source);
    if p.0.is_empty() {
        p.drawing(&drawing::shock(s, 0.0));
        p.drawing(&drawing::shock(s, s.stroke.0));
    }
    p.result()
}

pub fn drive_unit(u: &DriveUnitComponent) -> Result<(), String> {
    let mut p = Problems::default();
    p.name(&u.name);
    p.check(
        is_simple(&u.outline),
        "the outline must be a closed polygon that doesn't cross itself",
    );
    p.check(!u.mounts.is_empty(), "there must be at least one mount");
    for (i, m) in u.mounts.iter().enumerate() {
        p.check(
            m.diameter.0 > 0.0,
            &format!("mount {} needs a positive diameter", i + 1),
        );
        if is_simple(&u.outline) {
            p.check(
                contains(&u.outline, m.position),
                &format!("mount {} must be inside or on the outline", i + 1),
            );
        }
    }
    match (u.kind, &u.motor) {
        (DriveUnitKind::Gearbox, Some(_)) => p.add("a gearbox has no motor specification"),
        (DriveUnitKind::Motor | DriveUnitKind::MotorGearbox, None) => {
            p.add("a motor needs its rated power and torque")
        }
        (_, Some(m)) => p.check(
            m.rated_power > 0.0 && m.torque > 0.0 && m.peak_power.is_none_or(|w| w > 0.0),
            "motor power and torque must be positive",
        ),
        _ => {}
    }
    p.check(
        u.output_sprocket != Some(0),
        "the output sprocket needs teeth",
    );
    if let Some(ratios) = &u.ratios {
        p.check(
            !ratios.is_empty() && ratios.iter().all(|r| *r > 0.0),
            "ratios must be positive",
        );
    }
    p.check(
        u.frame_mount_mass.is_finite() && u.frame_mount_mass >= 0.0,
        "frame mount mass must be zero or more",
    );
    p.mass(&u.mass);
    p.source(&u.source);
    p.svg(&u.svg_path);
    p.result()
}

pub fn battery(b: &Battery) -> Result<(), String> {
    let mut p = Problems::default();
    p.name(&b.name);
    p.check(
        b.capacity > 0.0 && b.nominal_voltage > 0.0 && b.amp_hours.is_none_or(|a| a > 0.0),
        "capacity, voltage and amp hours must be positive",
    );
    p.check(
        b.length.0 > 0.0 && b.width.0 > 0.0 && b.height.0 > 0.0,
        "length, width and height must be positive",
    );
    p.mass(&b.mass);
    p.source(&b.source);
    if p.0.is_empty() {
        p.drawing(&drawing::battery(b));
    }
    p.result()
}

#[derive(Default)]
struct Problems(Vec<String>);

impl Problems {
    fn add(&mut self, message: &str) {
        self.0.push(message.to_string());
    }

    fn check(&mut self, ok: bool, message: &str) {
        if !ok {
            self.add(message);
        }
    }

    fn name(&mut self, n: &ComponentName) {
        self.check(
            [&n.make, &n.model, &n.variant]
                .iter()
                .all(|s| !s.trim().is_empty() && !s.contains('/')),
            "make, model and variant must be filled in, without '/'",
        );
        self.check(n.year >= 1900, "year is missing");
    }

    fn mass(&mut self, m: &MassProperties) {
        self.check(m.mass > 0.0, "mass must be positive");
        self.check(
            m.centre_of_mass
                .is_none_or(|c| c.x.is_finite() && c.y.is_finite()),
            "centre of mass must be a point",
        );
    }

    fn source(&mut self, source: &str) {
        self.check(!source.trim().is_empty(), "a source is required");
    }

    fn svg(&mut self, svg: &Option<String>) {
        self.check(
            svg.as_ref().is_none_or(|s| !s.trim().is_empty()),
            "svg_path must not be empty",
        );
    }

    fn drawing(&mut self, d: &Drawing) {
        self.check(
            d.shapes.iter().all(|s| is_simple(s)),
            "the drawing must be closed shapes with positive sizes",
        );
        self.svg(&d.svg_path);
    }

    fn result(self) -> Result<(), String> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(self.0.join("; "))
        }
    }
}
