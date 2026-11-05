//! The side view as plain shapes with style roles (S-SHELL H7, H12), built
//! from `Solved` alone. Coordinates are the model's: millimetres, origin at
//! the crank centre, +Y up. The theme maps roles to strokes and fills.

use bike_components::drawing;

use crate::panel::Field;
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
    Spoke,
    Hub,
    Frame,
    Fork,
    DriveUnit,
    /// Sag and full-travel states.
    Ghost,
    Centreline,
    Dimension,
    /// Dotted lines from the measured points out to an offset dimension.
    Extension,
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
    /// A shaded band between two circles, outlined (tyres).
    Ring {
        centre: Point,
        outer: f64,
        inner: f64,
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
                Shape::Ring {
                    centre,
                    outer: radius,
                    ..
                }
                | Shape::Circle { centre, radius } => points.extend([
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

/// Builds the side view. Every lettered value gets its dimension; `focused`
/// also shows that one's value (H7).
pub fn build(
    solved: &Solved,
    components: &ResolvedComponents,
    front_travel: f64,
    focused: Option<Field>,
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
        wheel(&mut scene, axle, diameter / 2.0);
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
                // The tube's body, shaded, with its centreline over it.
                scene.push(
                    Role::Frame,
                    Shape::Polygon(band(tube.start, tube.end, tube.outside_diameter)),
                );
                scene.line(Role::Centreline, tube.start, tube.end);
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
    dimensions(&mut scene, solved, focused, unit);
    Some(scene)
}

const TYRE_DEPTH: f64 = 60.0;
const RIM_DEPTH: f64 = 25.0;
const HUB_RADIUS: f64 = 30.0;
const SPOKES: usize = 32;

/// A wheel: tyre, rim, spokes and hub (mockup 04).
fn wheel(scene: &mut Scene, axle: Point, radius: f64) {
    let at = |r: f64, a: f64| Point::new(axle.x + r * a.cos(), axle.y + r * a.sin());
    let tau = std::f64::consts::TAU;
    scene.push(
        Role::Tyre,
        Shape::Ring {
            centre: axle,
            outer: radius,
            inner: radius - TYRE_DEPTH,
        },
    );
    let rim = radius - TYRE_DEPTH;
    scene.push(
        Role::Rim,
        Shape::Circle {
            centre: axle,
            radius: rim - RIM_DEPTH,
        },
    );
    for i in 0..SPOKES {
        let a = tau * i as f64 / SPOKES as f64;
        // Crossed lacing: each spoke leaves the hub a little ahead of where it meets the rim.
        let lead = if i % 2 == 0 { 0.35 } else { -0.35 };
        scene.line(
            Role::Spoke,
            at(HUB_RADIUS, a + lead),
            at(rim - RIM_DEPTH, a),
        );
    }
    scene.push(
        Role::Hub,
        Shape::Circle {
            centre: axle,
            radius: HUB_RADIUS,
        },
    );
}

/// A rectangle `width` wide along the line from `a` to `b`.
fn band(a: Point, b: Point, width: f64) -> Vec<Point> {
    let d = unit_vector(a, b);
    let n = Point::new(-d.y * width / 2.0, d.x * width / 2.0);
    vec![
        Point::new(a.x + n.x, a.y + n.y),
        Point::new(b.x + n.x, b.y + n.y),
        Point::new(b.x - n.x, b.y - n.y),
        Point::new(a.x - n.x, a.y - n.y),
    ]
}
const DIMENSION_GAP: f64 = 80.0;
const LABEL_GAP: f64 = 15.0;
const CALLOUT_RADIUS: f64 = 80.0;

fn with_unit(mm: f64, unit: LengthUnit) -> String {
    format!("{} {}", format_length(Millimeters(mm), unit), unit.label())
}

/// Standover: the top of the top tube above the ground, at the tube's
/// midpoint.
pub fn standover(solved: &Solved) -> Option<f64> {
    let g = solved.geometry.as_ref()?;
    let top = solved.tubes.as_ref()?.tube(TubeKind::TopTube)?;
    let mid_y = (top.start.y + top.end.y) / 2.0;
    Some(mid_y + top.outside_diameter / 2.0 - g.static_state.points.rear_contact.y)
}

/// Every lettered dimension (the panel shows the same letters). The focused
/// one carries its value too.
fn dimensions(scene: &mut Scene, solved: &Solved, focused: Option<Field>, unit: LengthUnit) {
    let Some(g) = solved.geometry.as_ref() else {
        return;
    };
    let s = &g.static_state;
    let p = &s.points;
    let crank = p.crank_centre;
    let ground = p.rear_contact.y;
    let tube = |k: TubeKind| solved.tubes.as_ref().and_then(|t| t.tube(k));
    let axis = unit_vector(p.head_tube_bottom, p.head_tube_top);
    let forward = Point::new(axis.y, -axis.x);
    let along = |a: Point, d: Point, k: f64| Point::new(a.x + d.x * k, a.y + d.y * k);

    let text = |field: Field, value: f64| {
        let letter = field.letter().unwrap_or(' ');
        if focused != Some(field) {
            return letter.to_string();
        }
        if matches!(field, Field::Geometry(v) if v.is_angle()) {
            format!("{letter}  {}°", format_angle(Degrees(value)))
        } else {
            format!("{letter}  {}", with_unit(value, unit))
        }
    };
    let linear = |scene: &mut Scene, field: Field, a: Point, b: Point, value: f64| {
        scene.line(Role::Dimension, a, b);
        let n = unit_vector(a, b);
        let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        // Labels sit just off the line, on its left.
        scene.label(
            along(mid, Point::new(-n.y, n.x), LABEL_GAP),
            text(field, value),
        );
    };

    // Dotted lines showing where an offset dimension lines up with the bike.
    let ext = |scene: &mut Scene, from: Point, to: Point| {
        if (to.x - from.x).hypot(to.y - from.y) > 1.0 {
            scene.line(Role::Extension, from, to);
        }
    };

    let top = p.head_tube_top;
    let reach_y = top.y + DIMENSION_GAP;
    linear(
        scene,
        Field::Geometry(V::Reach),
        Point::new(crank.x, reach_y),
        Point::new(top.x, reach_y),
        s.value(V::Reach),
    );
    ext(scene, crank, Point::new(crank.x, reach_y));
    ext(scene, top, Point::new(top.x, reach_y));
    let stack_x = top.x + DIMENSION_GAP;
    linear(
        scene,
        Field::Geometry(V::Stack),
        Point::new(stack_x, top.y),
        Point::new(stack_x, crank.y),
        s.value(V::Stack),
    );
    ext(scene, top, Point::new(stack_x, top.y));
    ext(scene, crank, Point::new(stack_x, crank.y));
    for (field, vertex, toward) in [
        (V::HeadTubeAngle, p.head_tube_bottom, top),
        (
            V::ActualSeatTubeAngle,
            p.seat_tube_at_crank_height,
            p.saddle,
        ),
        (V::EffectiveSeatTubeAngle, crank, p.saddle),
    ] {
        let f = Field::Geometry(field);
        arc(scene, vertex, toward, text(f, s.value(field)));
    }
    // Front and rear centre at axle height, wheelbase below the ground.
    linear(
        scene,
        Field::FrontCentre,
        Point::new(crank.x, p.front_axle.y),
        p.front_axle,
        s.front_centre,
    );
    ext(scene, crank, Point::new(crank.x, p.front_axle.y));
    linear(
        scene,
        Field::RearCentre,
        Point::new(crank.x, p.rear_axle.y),
        p.rear_axle,
        s.rear_centre,
    );
    ext(scene, crank, Point::new(crank.x, p.rear_axle.y));
    let wheelbase_y = ground - DIMENSION_GAP;
    linear(
        scene,
        Field::Geometry(V::Wheelbase),
        Point::new(p.front_axle.x, wheelbase_y),
        Point::new(p.rear_axle.x, wheelbase_y),
        s.value(V::Wheelbase),
    );
    for axle in [p.front_axle, p.rear_axle] {
        ext(scene, axle, Point::new(axle.x, wheelbase_y));
    }
    let bb_x = crank.x - DIMENSION_GAP / 2.0;
    linear(
        scene,
        Field::Geometry(V::BottomBracketHeight),
        Point::new(bb_x, ground),
        Point::new(bb_x, crank.y),
        s.value(V::BottomBracketHeight),
    );
    ext(scene, crank, Point::new(bb_x, crank.y));
    let drop_x = crank.x + DIMENSION_GAP / 2.0;
    linear(
        scene,
        Field::Geometry(V::BottomBracketDrop),
        Point::new(drop_x, crank.y),
        Point::new(drop_x, (p.rear_axle.y + p.front_axle.y) / 2.0),
        s.value(V::BottomBracketDrop),
    );
    let axle_line = (p.rear_axle.y + p.front_axle.y) / 2.0;
    ext(scene, p.rear_axle, Point::new(drop_x, axle_line));
    ext(scene, crank, Point::new(drop_x, crank.y));
    linear(
        scene,
        Field::Geometry(V::ChainstayLength),
        p.rear_axle,
        crank,
        s.value(V::ChainstayLength),
    );
    if let (Some(t), Some(height)) = (tube(TubeKind::TopTube), standover(solved)) {
        let x = (t.start.x + t.end.x) / 2.0;
        linear(
            scene,
            Field::Standover,
            Point::new(x, ground + height),
            Point::new(x, ground),
            height,
        );
    }
    if let Some(t) = tube(TubeKind::SeatTube) {
        let n = unit_vector(t.start, t.end);
        let back = Point::new(n.y, -n.x);
        let off = |q: Point| along(q, back, -DIMENSION_GAP / 2.0);
        linear(
            scene,
            Field::SeatTubeLength,
            off(t.end),
            off(t.start),
            t.length,
        );
        ext(scene, t.end, off(t.end));
        ext(scene, t.start, off(t.start));
    }
    // Head tube, offset forward of the steering axis.
    let ahead = |q: Point| along(q, forward, DIMENSION_GAP / 2.0);
    linear(
        scene,
        Field::Geometry(V::HeadTubeLength),
        ahead(p.head_tube_bottom),
        ahead(top),
        s.value(V::HeadTubeLength),
    );
    ext(scene, p.head_tube_bottom, ahead(p.head_tube_bottom));
    ext(scene, top, ahead(top));
    // Fork offset: the steering axis to the front axle, square to the axis.
    let foot = {
        let d = Point::new(
            p.front_axle.x - p.head_tube_bottom.x,
            p.front_axle.y - p.head_tube_bottom.y,
        );
        along(p.head_tube_bottom, axis, d.x * axis.x + d.y * axis.y)
    };
    linear(scene, Field::ForkOffset, foot, p.front_axle, g.fork_offset);
    // The steering axis carried down to the offset's foot.
    ext(scene, p.head_tube_bottom, foot);
    // Axle-to-crown along the axis, ahead of the fork.
    let ahead_more = |q: Point| along(q, forward, DIMENSION_GAP);
    linear(
        scene,
        Field::ForkAxleToCrown,
        ahead_more(p.front_axle),
        ahead_more(along(p.front_axle, axis, g.fork_axle_to_crown)),
        g.fork_axle_to_crown,
    );
    let crown = along(p.front_axle, axis, g.fork_axle_to_crown);
    ext(scene, p.front_axle, ahead_more(p.front_axle));
    ext(scene, crown, ahead_more(crown));
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

    fn scene(doc: &mut Document, focused: Option<Field>) -> Scene {
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
        assert_eq!(count(Role::Spoke), 2 * SPOKES);
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
        let s = scene(&mut doc, Some(Field::Geometry(V::HeadTubeAngle)));
        assert_eq!(s.items.iter().filter(|i| i.role == Role::Ghost).count(), 2);
        assert!(
            s.items
                .iter()
                .any(|i| matches!(&i.shape, Shape::Label { text, .. } if text == "D  64.0°"))
        );
        for v in V::ALL {
            scene(&mut doc, Some(Field::Geometry(v)));
        }
    }

    #[test]
    fn every_lettered_value_is_drawn_once() {
        let s = scene(&mut Document::new(), None);
        assert!(s.items.iter().filter(|i| i.role == Role::Extension).count() >= 15);
        let letters: Vec<String> = s
            .items
            .iter()
            .filter_map(|i| match &i.shape {
                Shape::Label { text, .. } if text.len() == 1 => Some(text.clone()),
                _ => None,
            })
            .collect();
        let mut expected: Vec<String> = "ABDEFGHIJKLMNOQR".chars().map(String::from).collect();
        let mut got = letters.clone();
        got.sort();
        expected.sort();
        assert_eq!(got, expected);
    }
}
