//! The side view as plain shapes with style roles (S-SHELL H7, H12), built
//! from `Solved` alone. Coordinates are the model's: millimetres, origin at
//! the crank centre, +Y up. The theme maps roles to strokes and fills.

use bike_components::drawing;
use bike_model::{
    ComponentName, Degrees, Fork, GeometryVariable as V, LengthUnit, MassProperties, Millimeters,
    Point, ResolvedComponents, Solved, SolvedGeometry, Steerer, TubeKind, WheelSize, format_angle,
    format_length,
};

/// What a shape is, for the theme (H12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Ground,
    Tyre,
    Rim,
    Frame,
    Fork,
    DriveUnit,
    /// Sag and full-travel states.
    Ghost,
    Centreline,
    Dimension,
    Origin,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// `width` in millimetres when the line has a real size (a tube), else a hairline.
    Line {
        a: Point,
        b: Point,
        width: Option<f64>,
    },
    Polygon(Vec<Point>),
    Circle {
        centre: Point,
        radius: f64,
    },
    /// Text anchored at a point; drawn at a fixed screen size.
    Label {
        at: Point,
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub role: Role,
    pub shape: Shape,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    pub items: Vec<Item>,
}

impl Scene {
    fn push(&mut self, role: Role, shape: Shape) {
        self.items.push(Item { role, shape });
    }

    fn line(&mut self, role: Role, a: Point, b: Point) {
        self.push(role, Shape::Line { a, b, width: None });
    }

    fn label(&mut self, at: Point, text: impl Into<String>) {
        self.push(
            Role::Dimension,
            Shape::Label {
                at,
                text: text.into(),
            },
        );
    }

    /// The smallest box holding every shape, for Fit (H7).
    pub fn bounds(&self) -> Option<(Point, Point)> {
        let mut points = Vec::new();
        for item in &self.items {
            match &item.shape {
                Shape::Line { a, b, .. } => points.extend([*a, *b]),
                Shape::Polygon(p) => points.extend(p.iter().copied()),
                Shape::Circle { centre, radius } => points.extend([
                    Point::new(centre.x - radius, centre.y - radius),
                    Point::new(centre.x + radius, centre.y + radius),
                ]),
                Shape::Label { at, .. } => points.push(*at),
            }
        }
        let first = *points.first()?;
        Some(points.iter().fold((first, first), |(lo, hi), p| {
            (
                Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                Point::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        }))
    }
}

/// Builds the side view. `focused` adds that value's dimension callout (H7).
pub fn build(
    solved: &Solved,
    components: &ResolvedComponents,
    front_travel: f64,
    focused: Option<V>,
    unit: LengthUnit,
) -> Option<Scene> {
    let g = solved.geometry.as_ref()?;
    let s = &g.static_state;
    let p = &s.points;
    let mut scene = Scene::default();

    // Ground and wheels.
    let ground = p.rear_contact.y;
    scene.line(
        Role::Ground,
        Point::new(p.rear_axle.x - g.rear_wheel_diameter, ground),
        Point::new(p.front_axle.x + g.front_wheel_diameter, ground),
    );
    for (axle, diameter) in [
        (p.rear_axle, g.rear_wheel_diameter),
        (p.front_axle, g.front_wheel_diameter),
    ] {
        scene.push(
            Role::Tyre,
            Shape::Circle {
                centre: axle,
                radius: diameter / 2.0,
            },
        );
        scene.push(
            Role::Rim,
            Shape::Circle {
                centre: axle,
                radius: diameter / 2.0 - TYRE_DEPTH,
            },
        );
    }

    // Ghost states for a hardtail (H7).
    for state in [g.at_sag.as_ref(), g.full_travel.as_ref()]
        .into_iter()
        .flatten()
    {
        scene.push(
            Role::Ghost,
            Shape::Circle {
                centre: state.points.front_axle,
                radius: g.front_wheel_diameter / 2.0,
            },
        );
    }

    // Fork, from its drawing in fork coordinates (C13).
    let fork = components
        .fork
        .as_ref()
        .map(|f| f.fork.clone())
        .unwrap_or_else(|| default_fork(g, front_travel));
    let axis = unit_vector(p.head_tube_bottom, p.head_tube_top);
    let forward = Point::new(axis.y, -axis.x);
    let to_frame = |q: Point| {
        Point::new(
            p.front_axle.x + q.x * forward.x + q.y * axis.x,
            p.front_axle.y + q.x * forward.y + q.y * axis.y,
        )
    };
    for shape in drawing::fork(&fork, 0.0).shapes {
        scene.push(
            Role::Fork,
            Shape::Polygon(shape.into_iter().map(to_frame).collect()),
        );
    }

    // Drive unit outline (C8).
    if let Some(unit) = &components.drive_unit {
        scene.push(
            Role::DriveUnit,
            Shape::Polygon(
                unit.outline
                    .iter()
                    .map(|q| Point::new(q.x + p.crank_centre.x, q.y + p.crank_centre.y))
                    .collect(),
            ),
        );
    }

    // Tubes, with their diameters; the steering axis as a centreline.
    if let Some(t) = &solved.tubes {
        for tube in &t.tubes {
            if tube.kind == TubeKind::BottomBracketShell {
                scene.push(
                    Role::Frame,
                    Shape::Circle {
                        centre: tube.start,
                        radius: tube.outside_diameter / 2.0,
                    },
                );
            } else {
                scene.push(
                    Role::Frame,
                    Shape::Line {
                        a: tube.start,
                        b: tube.end,
                        width: Some(tube.outside_diameter),
                    },
                );
            }
        }
    }
    scene.line(Role::Centreline, p.seat_tube_at_crank_height, p.saddle);

    // Origin and the wheelbase dimension (mockup 04).
    scene.push(
        Role::Origin,
        Shape::Circle {
            centre: p.crank_centre,
            radius: 6.0,
        },
    );
    scene.label(
        Point::new(p.crank_centre.x - 60.0, p.crank_centre.y - 40.0),
        "0,0 crank axis",
    );
    let dim_y = ground + DIMENSION_GAP;
    let wheelbase = Point::new(p.rear_axle.x, dim_y);
    let front = Point::new(p.front_axle.x, dim_y);
    scene.line(Role::Dimension, wheelbase, front);
    scene.label(
        Point::new((wheelbase.x + front.x) / 2.0, dim_y + LABEL_GAP),
        format!("({})", with_unit(s.value(V::Wheelbase), unit)),
    );

    if let Some(v) = focused {
        callout(&mut scene, g, v, unit);
    }
    Some(scene)
}

const TYRE_DEPTH: f64 = 60.0;
const DIMENSION_GAP: f64 = 80.0;
const LABEL_GAP: f64 = 15.0;
const CALLOUT_RADIUS: f64 = 80.0;

fn with_unit(mm: f64, unit: LengthUnit) -> String {
    format!("{} {}", format_length(Millimeters(mm), unit), unit.label())
}

/// The dimension for a focused geometry field (mockup 07).
fn callout(scene: &mut Scene, g: &SolvedGeometry, v: V, unit: LengthUnit) {
    let s = &g.static_state;
    let p = &s.points;
    let value = s.value(v);
    let text = if v.is_angle() {
        format!("{}°", format_angle(Degrees(value)))
    } else {
        with_unit(value, unit)
    };
    let mid = |a: Point, b: Point| Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let crank = p.crank_centre;
    let (a, b) = match v {
        V::Stack => (crank, Point::new(crank.x, p.head_tube_top.y)),
        V::Reach => (Point::new(crank.x, p.head_tube_top.y), p.head_tube_top),
        V::HeadTubeLength => (p.head_tube_bottom, p.head_tube_top),
        V::ChainstayLength => (crank, p.rear_axle),
        V::Wheelbase => (p.rear_axle, p.front_axle),
        V::BottomBracketHeight => (crank, Point::new(crank.x, p.rear_contact.y)),
        V::BottomBracketDrop => (
            crank,
            Point::new(crank.x, (p.rear_axle.y + p.front_axle.y) / 2.0),
        ),
        V::HeadTubeAngle => {
            arc(scene, p.head_tube_bottom, p.head_tube_top, text);
            return;
        }
        V::EffectiveSeatTubeAngle => {
            arc(scene, crank, p.saddle, text);
            return;
        }
        V::ActualSeatTubeAngle => {
            arc(scene, p.seat_tube_at_crank_height, p.saddle, text);
            return;
        }
    };
    scene.line(Role::Dimension, a, b);
    scene.label(mid(a, b), text);
}

/// An angle from the horizontal at `vertex`, toward `toward`.
fn arc(scene: &mut Scene, vertex: Point, toward: Point, text: String) {
    let d = unit_vector(vertex, toward);
    let reference = Point::new(vertex.x + d.x.signum() * CALLOUT_RADIUS, vertex.y);
    scene.line(Role::Dimension, vertex, reference);
    scene.line(
        Role::Dimension,
        vertex,
        Point::new(
            vertex.x + d.x * CALLOUT_RADIUS,
            vertex.y + d.y * CALLOUT_RADIUS,
        ),
    );
    scene.label(
        Point::new(
            vertex.x + d.x * CALLOUT_RADIUS * 1.3,
            vertex.y + d.y * CALLOUT_RADIUS * 0.6,
        ),
        text,
    );
}

fn unit_vector(a: Point, b: Point) -> Point {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let n = dx.hypot(dy);
    if n > 0.0 {
        Point::new(dx / n, dy / n)
    } else {
        Point::new(0.0, 1.0)
    }
}

/// A stand-in fork for drawing when none is selected: the solved dimensions
/// with a generic 35 mm chassis. Visual only.
fn default_fork(g: &SolvedGeometry, travel: f64) -> Fork {
    Fork {
        name: ComponentName {
            make: "Default".into(),
            model: "Fork".into(),
            year: 2026,
            variant: "default".into(),
        },
        wheel: WheelSize(String::new()),
        travel: Millimeters(travel.clamp(0.0, g.fork_axle_to_crown * 0.5)),
        axle_to_crown: Millimeters(g.fork_axle_to_crown),
        offset: Millimeters(g.fork_offset),
        steerer: Steerer::Tapered {
            top: 1.125,
            bottom: 1.5,
        },
        stanchion_diameter: Millimeters(if travel > 0.0 { 35.0 } else { 0.0 }),
        lower_leg_diameter: None,
        crown_height: None,
        mass: MassProperties {
            mass: 1.0,
            centre_of_mass: None,
        },
        svg_path: None,
        source: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Command, Data, Document};

    fn scene(doc: &mut Document, focused: Option<V>) -> Scene {
        doc.solve(&Data::builtin());
        build(
            &doc.solved,
            &doc.components,
            doc.project.front_travel.map_or(0.0, |t| t.0),
            focused,
            LengthUnit::Millimeters,
        )
        .unwrap()
    }

    #[test]
    fn an_empty_project_draws_wheels_frame_fork_and_origin() {
        let s = scene(&mut Document::new(), None);
        let count = |role| s.items.iter().filter(|i| i.role == role).count();
        assert_eq!(count(Role::Tyre), 2);
        assert!(count(Role::Frame) >= 6);
        assert!(count(Role::Fork) >= 3);
        assert_eq!(count(Role::Ghost), 0, "rigid: no sag states");
        assert!(
            s.items
                .iter()
                .any(|i| matches!(&i.shape, Shape::Label { text, .. } if text == "0,0 crank axis"))
        );
        let (lo, hi) = s.bounds().unwrap();
        assert!(hi.x - lo.x > 1500.0 && hi.y - lo.y > 700.0);
    }

    #[test]
    fn the_fork_hangs_from_the_head_tube() {
        let mut doc = Document::new();
        let s = scene(&mut doc, None);
        let top = doc
            .solved
            .geometry
            .as_ref()
            .unwrap()
            .static_state
            .points
            .head_tube_bottom;
        // Some fork corner sits near the head tube bottom (the crown).
        let near = s
            .items
            .iter()
            .filter(|i| i.role == Role::Fork)
            .flat_map(|i| match &i.shape {
                Shape::Polygon(p) => p.clone(),
                _ => vec![],
            })
            .map(|q| (q.x - top.x).hypot(q.y - top.y))
            .fold(f64::MAX, f64::min);
        assert!(near < 40.0, "{near}");
    }

    #[test]
    fn a_hardtail_shows_ghosts_and_a_focused_field_a_callout() {
        let mut doc = Document::new();
        doc.apply(Command::SetTravel(
            crate::document::End::Front,
            Some(Millimeters(150.0)),
        ));
        doc.apply(Command::PinGeometry(V::HeadTubeAngle, 64.0));
        let s = scene(&mut doc, Some(V::HeadTubeAngle));
        assert_eq!(s.items.iter().filter(|i| i.role == Role::Ghost).count(), 2);
        assert!(
            s.items
                .iter()
                .any(|i| matches!(&i.shape, Shape::Label { text, .. } if text == "64.0°"))
        );
        for v in V::ALL {
            scene(&mut doc, Some(v));
        }
    }
}
