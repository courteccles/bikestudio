//! The side view as plain shapes with style roles (S-SHELL H7, H12), built
//! from `Solved` alone. Coordinates are the model's: millimetres, origin at
//! the crank centre, +Y up. The theme maps roles to strokes and fills.

use bike_components::drawing;

use crate::panel::Field;
use bike_model::{
    Bend, ComponentName, Degrees, Fork, GeometryVariable as V, LengthUnit, MassProperties,
    Millimeters, Point, ResolvedComponents, Solved, SolvedGeometry, SolvedTubes, Steerer, TubeKind,
    WheelSize, format_angle, format_length,
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
    /// Crank arm and pedal.
    Crank,
    /// The saddle point (G4), marked with a solid dot.
    Saddle,
    /// The effective seat line, crank centre to saddle point: thick and dotted.
    SeatLine,
    /// The crank straight down, drawn faint.
    CrankGhost,
    /// Sag and full-travel states.
    Ghost,
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
    /// Separate straight strokes, unfilled (an outline with parts left out).
    Segments(Vec<(Point, Point)>),
    /// Overlapping polygons drawn as one piece: only the outline of their
    /// union shows, with no seams where they overlap.
    Union(Vec<Vec<Point>>),
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
                Shape::Segments(s) => points.extend(s.iter().flat_map(|(a, b)| [*a, *b])),
                Shape::Union(parts) => points.extend(parts.iter().flatten().copied()),
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
    // The steerer, up from the crown through the head tube and out of its
    // top. Drawn before the frame, so the head tube covers it.
    let (top_d, bottom_d) = match fork.steerer {
        Steerer::Tapered { top, bottom } => (top, bottom),
        Steerer::Straight { diameter } => (diameter, diameter),
    };
    let (top_r, bottom_r) = (top_d * MM_PER_INCH / 2.0, bottom_d * MM_PER_INCH / 2.0);
    let along = |q: Point| (q.x - p.front_axle.x) * axis.x + (q.y - p.front_axle.y) * axis.y;
    let x = -fork.offset.0;
    let crown = fork.axle_to_crown.0;
    let head_top = along(p.head_tube_top);
    let steerer_top = head_top + STEERER_ABOVE_HEAD_TUBE;
    if steerer_top > crown {
        scene.push(
            Role::Fork,
            Shape::Polygon(
                [
                    Point::new(x - bottom_r, crown),
                    Point::new(x + bottom_r, crown),
                    Point::new(x + top_r, head_top),
                    Point::new(x + top_r, steerer_top),
                    Point::new(x - top_r, steerer_top),
                    Point::new(x - top_r, head_top),
                ]
                .into_iter()
                .map(to_frame)
                .collect(),
            ),
        );
    }
    for shape in drawing::fork(&fork, 0.0).shapes {
        scene.push(
            Role::Fork,
            Shape::Polygon(shape.into_iter().map(to_frame).collect()),
        );
    }
    // The front axle, as at the rear.
    scene.push(
        Role::Hub,
        Shape::Circle {
            centre: p.front_axle,
            radius: AXLE_RADIUS,
        },
    );

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
        // The stays and rear dropout are one piece, drawn as one outline.
        let rear = rear_end(t, p.rear_axle);
        // Tubes others are mitred to go first (the BB shell, then the head
        // and seat tubes), so the coped ends drawn over them keep their full
        // outline instead of sitting under their fill.
        let rank = |k: TubeKind| match k {
            TubeKind::BottomBracketShell | TubeKind::HeadTube | TubeKind::SeatTube => 0,
            _ => 1,
        };
        let mut order: Vec<_> = t.tubes.iter().collect();
        order.sort_by_key(|tube| rank(tube.kind));
        for tube in order {
            if rear.is_some() && matches!(tube.kind, TubeKind::Chainstay | TubeKind::Seatstay) {
                continue;
            }
            if tube.kind == TubeKind::BottomBracketShell {
                scene.push(
                    Role::Frame,
                    Shape::Circle {
                        centre: tube.start,
                        radius: tube.outside_diameter / 2.0,
                    },
                );
            } else {
                // The tube's body, shaded, its ends mitred to the tubes they join.
                let (at_start, at_end) = miters(tube.kind, t);
                let outline = match &tube.bend {
                    Some(bend) => {
                        bent_band(bend, tube.start, tube.end, tube.outside_diameter, &at_start)
                    }
                    None => mitred_band(
                        tube.start,
                        tube.end,
                        tube.outside_diameter,
                        &at_start,
                        &at_end,
                    ),
                };
                scene.push(Role::Frame, Shape::Polygon(outline));
            }
        }
        if let Some(parts) = rear {
            scene.push(Role::Frame, Shape::Union(parts));
            scene.push(
                Role::Hub,
                Shape::Circle {
                    centre: p.rear_axle,
                    radius: AXLE_RADIUS,
                },
            );
        }
    }
    // The seat tube axis carried on from the top of the seat tube to the saddle point.
    if let Some(t) = &solved.tubes {
        scene.line(Role::Extension, t.seat_tube_top, p.saddle);
    }
    // Effective seat line, the head tube top carried back to it, and the saddle (G4).
    scene.line(Role::SeatLine, p.crank_centre, p.saddle);
    scene.line(Role::Extension, p.head_tube_top, p.saddle);
    scene.push(
        Role::Saddle,
        Shape::Circle {
            centre: p.saddle,
            radius: SADDLE_MARK,
        },
    );

    // Crank and pedal forward and level, with a ghost straight down: the
    // positions the front tyre and ground clearances are measured at.
    for (role, pedal) in [
        (Role::CrankGhost, p.pedal_down),
        (Role::Crank, p.pedal_forward),
    ] {
        crank(
            &mut scene,
            role,
            p.crank_centre,
            pedal,
            g.pedal_length,
            g.pedal_thickness,
        );
    }

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

/// The saddle point's mark, for the drawing only.
const SADDLE_MARK: f64 = 8.0;

/// Crank arm ends, for the drawing only: rounded about the crank centre and
/// the pedal axle, wider at the spindle.
const CRANK_SPINDLE_RADIUS: f64 = 18.0;
const CRANK_PEDAL_RADIUS: f64 = 12.0;

/// A crank arm from the crank centre to the pedal axle, and the pedal as a
/// level box `length` long and `thickness` deep centred on the axle.
fn crank(scene: &mut Scene, role: Role, centre: Point, pedal: Point, length: f64, thickness: f64) {
    let arm = crank_arm(centre, pedal);
    let (hx, hy) = (length / 2.0, thickness / 2.0);
    // Anticlockwise, like the arm.
    let box_ = vec![
        Point::new(pedal.x - hx, pedal.y - hy),
        Point::new(pedal.x + hx, pedal.y - hy),
        Point::new(pedal.x + hx, pedal.y + hy),
        Point::new(pedal.x - hx, pedal.y + hy),
    ];
    if role == Role::CrankGhost {
        // The ghost isn't filled: the pedal is drawn whole, and the arm only
        // where it's outside the pedal.
        let mut segments = outside(&arm, &box_);
        segments.extend((0..box_.len()).map(|i| (box_[i], box_[(i + 1) % box_.len()])));
        scene.push(role, Shape::Segments(segments));
    } else {
        scene.push(role, Shape::Polygon(arm));
        scene.push(role, Shape::Polygon(box_));
    }
    // The pedal axle, on top.
    scene.push(
        role,
        Shape::Circle {
            centre: pedal,
            radius: PEDAL_AXLE_RADIUS,
        },
    );
}

/// The pedal axle (a 9/16 inch spindle), for the drawing only.
const PEDAL_AXLE_RADIUS: f64 = 7.0;

/// The parts of `shape`'s edges outside the convex, anticlockwise `other`.
fn outside(shape: &[Point], other: &[Point]) -> Vec<(Point, Point)> {
    let n = shape.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (a, b) = (shape[i], shape[(i + 1) % n]);
        let at = |t: f64| Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        match inside_span(a, b, other) {
            None => out.push((a, b)),
            Some((enter, leave)) => {
                if enter > 1e-9 {
                    out.push((a, at(enter)));
                }
                if leave < 1.0 - 1e-9 {
                    out.push((at(leave), b));
                }
            }
        }
    }
    out
}

/// Where the segment `a`–`b` is inside the convex, anticlockwise `polygon`,
/// as a span of 0..1 along it (Cyrus–Beck), or `None` if it never is.
fn inside_span(a: Point, b: Point, polygon: &[Point]) -> Option<(f64, f64)> {
    let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
    let d = Point::new(b.x - a.x, b.y - a.y);
    let m = polygon.len();
    for i in 0..m {
        let (v, w) = (polygon[i], polygon[(i + 1) % m]);
        let e = Point::new(w.x - v.x, w.y - v.y);
        // Inside is to the left of each edge: cross(e, p - v) >= 0.
        let start = e.x * (a.y - v.y) - e.y * (a.x - v.x);
        let rate = e.x * d.y - e.y * d.x;
        if rate.abs() < 1e-12 {
            if start < 0.0 {
                return None;
            }
            continue;
        }
        let t = -start / rate;
        if rate > 0.0 {
            enter = enter.max(t);
        } else {
            leave = leave.min(t);
        }
    }
    (enter < leave - 1e-12).then_some((enter, leave))
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

/// Rear dropout, for the drawing only: rounded around the axle (concentric
/// with the hub) and running up each stay far enough to join it.
const DROPOUT_RADIUS: f64 = 20.0;
/// How far up each stay the dropout reaches, from the axle.
const DROPOUT_REACH: f64 = 50.0;
/// The steerer above the head tube top (spacers and stem), for the drawing only.
const STEERER_ABOVE_HEAD_TUBE: f64 = 30.0;
const MM_PER_INCH: f64 = 25.4;
/// The thru-axle, 12 mm.
const AXLE_RADIUS: f64 = 6.0;

/// The rear dropout's outline: the convex hull of a circle round the axle
/// and a tab along each stay, so its back is an arc concentric with the hub
/// and it joins the chainstay and seatstay. `None` without both stays (full
/// suspension: the rear is S-LINK's).
fn dropout(tubes: &SolvedTubes, axle: Point) -> Option<Vec<Point>> {
    let chainstay = tubes.tube(TubeKind::Chainstay)?;
    let seatstay = tubes.tube(TubeKind::Seatstay)?;
    let mut points: Vec<Point> = (0..72)
        .map(|i| {
            let a = std::f64::consts::TAU * f64::from(i) / 72.0;
            Point::new(
                axle.x + DROPOUT_RADIUS * a.cos(),
                axle.y + DROPOUT_RADIUS * a.sin(),
            )
        })
        .collect();
    // Each stay's far end, seen from the axle.
    for (stay, far) in [(chainstay, chainstay.start), (seatstay, seatstay.end)] {
        let d = unit_vector(axle, far);
        let n = Point::new(-d.y, d.x);
        // As wide as the stay, so the two outlines run on into each other.
        let half = stay.outside_diameter / 2.0;
        let reach = DROPOUT_REACH.min(distance(axle, far));
        for side in [half, -half] {
            for along in [0.0, reach] {
                points.push(Point::new(
                    axle.x + d.x * along + n.x * side,
                    axle.y + d.y * along + n.y * side,
                ));
            }
        }
    }
    Some(convex_hull(points))
}

/// The stays and rear dropout as one piece (S-SHELL): the dropout and both
/// stays (mitred at their far ends, as usual, and running into the dropout),
/// drawn as a union so there's no seam at any angle between the stays.
/// `None` without both stays.
fn rear_end(tubes: &SolvedTubes, axle: Point) -> Option<Vec<Vec<Point>>> {
    let hull = dropout(tubes, axle)?;
    let chainstay = tubes.tube(TubeKind::Chainstay)?;
    let seatstay = tubes.tube(TubeKind::Seatstay)?;
    let (cs_start, _) = miters(TubeKind::Chainstay, tubes);
    let (_, ss_end) = miters(TubeKind::Seatstay, tubes);
    Some(vec![
        hull,
        mitred_band(
            chainstay.start,
            chainstay.end,
            chainstay.outside_diameter,
            &cs_start,
            &[],
        ),
        mitred_band(
            seatstay.start,
            seatstay.end,
            seatstay.outside_diameter,
            &[],
            &ss_end,
        ),
    ])
}

/// The convex hull of `points`, anticlockwise (monotone chain).
fn convex_hull(mut points: Vec<Point>) -> Vec<Point> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let cross =
        |o: Point, a: Point, b: Point| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut hull: Vec<Point> = Vec::new();
    for pass in 0..2 {
        let start = hull.len();
        let ordered: Box<dyn Iterator<Item = &Point>> = if pass == 0 {
            Box::new(points.iter())
        } else {
            Box::new(points.iter().rev())
        };
        for &p in ordered {
            while hull.len() >= start + 2
                && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0
            {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

/// A crank arm's outline: round ends concentric with the crank centre and
/// the pedal axle, joined by straight tangent sides (the hull of the two
/// circles).
fn crank_arm(centre: Point, pedal: Point) -> Vec<Point> {
    let circle = |c: Point, r: f64| {
        (0..48).map(move |i| {
            let a = std::f64::consts::TAU * f64::from(i) / 48.0;
            Point::new(c.x + r * a.cos(), c.y + r * a.sin())
        })
    };
    convex_hull(
        circle(centre, CRANK_SPINDLE_RADIUS)
            .chain(circle(pedal, CRANK_PEDAL_RADIUS))
            .collect(),
    )
}

/// What a tube end is cut to, so it looks mitred in side view.
#[derive(Debug, Clone, Copy)]
enum Cut {
    /// A round tube: a point on its axis, the axis direction, the unit
    /// normal toward the joining tube, and its radius.
    Tube {
        at: Point,
        along: Point,
        side: Point,
        radius: f64,
        /// Its length along the axis from `at`: hits beyond its ends miss.
        length: f64,
    },
    /// The BB shell, seen end on: a circle.
    Shell { centre: Point, radius: f64 },
}

/// The cuts at a tube's start and end: the top and down tubes meet the head
/// tube, the top tube and seatstays the seat tube, and the seat tube, down
/// tube and chainstays the BB shell (the down tube meets the seat tube when
/// there's no shell). Ends with no cuts stay square.
fn miters(kind: TubeKind, tubes: &SolvedTubes) -> (Vec<Cut>, Vec<Cut>) {
    let joined = |joined: TubeKind, far: Point| -> Option<Cut> {
        let j = tubes.tube(joined)?;
        // A bent seat tube's joints are on its straight upper section.
        let from = j.bend.map_or(j.start, |b| b.tangent);
        let d = unit_vector(from, j.end);
        let mut n = Point::new(-d.y, d.x);
        // The side of the joined tube facing this tube's other end.
        if n.x * (far.x - from.x) + n.y * (far.y - from.y) < 0.0 {
            n = Point::new(-n.x, -n.y);
        }
        Some(Cut::Tube {
            at: from,
            along: d,
            side: n,
            radius: j.outside_diameter / 2.0,
            length: distance(from, j.end),
        })
    };
    let shell = || {
        tubes
            .tube(TubeKind::BottomBracketShell)
            .map(|s| Cut::Shell {
                centre: s.start,
                radius: s.outside_diameter / 2.0,
            })
    };
    let Some(t) = tubes.tube(kind) else {
        return (Vec::new(), Vec::new());
    };
    let (start, end) = match kind {
        TubeKind::TopTube => (
            vec![joined(TubeKind::HeadTube, t.end)],
            vec![joined(TubeKind::SeatTube, t.start)],
        ),
        // At the BB the down tube is cut to the shell; with no shell (a
        // drive unit) it meets the seat tube instead.
        TubeKind::DownTube => (
            vec![joined(TubeKind::HeadTube, t.end)],
            vec![shell().or_else(|| joined(TubeKind::SeatTube, t.start))],
        ),
        TubeKind::SeatTube => (vec![shell()], Vec::new()),
        TubeKind::Seatstay => (Vec::new(), vec![joined(TubeKind::SeatTube, t.start)]),
        TubeKind::Chainstay => (vec![shell()], Vec::new()),
        _ => (Vec::new(), Vec::new()),
    };
    let flat = |cuts: Vec<Option<Cut>>| cuts.into_iter().flatten().collect();
    (flat(start), flat(end))
}

/// Where the line from `from` along `dir` first meets `cut`, as a distance.
/// `offset` is how far the line is across the joining tube from its axis and
/// `half` that tube's radius: a round tube meeting a round tube of radius R
/// is seen side on at sqrt(R² − half² + offset²) from R's axis, which is R at
/// the edges and dips in toward the axis in the middle (a coped joint).
fn first_hit(from: Point, dir: Point, cut: Cut, offset: f64, half: f64) -> Option<f64> {
    match cut {
        Cut::Tube {
            at,
            along,
            side,
            radius,
            length,
        } => {
            // A joining tube as large as this one can't be coped round it;
            // scale the cope so it stays a smooth curve, meeting the edge at
            // the sides as before.
            let c = half.min(radius * COPE_LIMIT);
            let s = offset * c / half;
            let u = (radius * radius - c * c + s * s).max(0.0).sqrt();
            let on = Point::new(at.x + side.x * u, at.y + side.y * u);
            let cross = dir.x * along.y - dir.y * along.x;
            if cross.abs() < 1e-9 {
                return None;
            }
            let (wx, wy) = (on.x - from.x, on.y - from.y);
            let t = (wx * along.y - wy * along.x) / cross;
            // Only where the joined tube really is.
            let hit = Point::new(from.x + dir.x * t, from.y + dir.y * t);
            let along_axis = (hit.x - at.x) * along.x + (hit.y - at.y) * along.y;
            (0.0..=length).contains(&along_axis).then_some(t)
        }
        Cut::Shell { centre, radius } => {
            let (fx, fy) = (from.x - centre.x, from.y - centre.y);
            let b = fx * dir.x + fy * dir.y;
            let c = fx * fx + fy * fy - radius * radius;
            let disc = b * b - c;
            (disc >= 0.0).then(|| -b - disc.sqrt())
        }
    }
}

/// The deepest a cope goes, as a fraction of the joined tube's radius.
const COPE_LIMIT: f64 = 0.9;

/// Points across a cut tube end, from the left edge to the right.
const MITRE_STEPS: usize = 12;

/// A tube's outline from `a` to `b`, `width` wide, with each end cut to its
/// joint (a curve across the tube), or square where there's none. Where a
/// line across the tube misses its cut, that point stays square.
fn mitred_band(a: Point, b: Point, width: f64, at_a: &[Cut], at_b: &[Cut]) -> Vec<Point> {
    let d = unit_vector(a, b);
    let half = width / 2.0;
    let n = Point::new(-d.y, d.x);
    let shift = |p: Point, s: f64| Point::new(p.x + n.x * s, p.y + n.y * s);
    let length = distance(a, b);
    // An end's points, left edge (+half) to right (−half).
    let end = |near: Point, far: Point, cuts: &[Cut]| -> Vec<Point> {
        let dir = unit_vector(far, near);
        let steps = if cuts.is_empty() { 1 } else { MITRE_STEPS };
        (0..=steps)
            .map(|i| {
                let s = half - width * i as f64 / steps as f64;
                let from = shift(far, s);
                // The first cut this line across the tube reaches.
                let hit = cuts
                    .iter()
                    .filter_map(|c| first_hit(from, dir, *c, s, half))
                    .filter(|t| *t > 0.0 && *t < length * 1.5)
                    .min_by(f64::total_cmp);
                match hit {
                    Some(t) => Point::new(from.x + dir.x * t, from.y + dir.y * t),
                    _ => shift(near, s),
                }
            })
            .collect()
    };
    let start = end(a, b, at_a);
    let finish = end(b, a, at_b);
    // Up the left side to `b`, across it, back down the right, across `a`.
    let mut out = vec![start[0]];
    out.extend(finish.iter().copied());
    out.extend(start.iter().rev().take(start.len() - 1));
    out
}

/// Points along a bent tube's arc, for its outline.
const BEND_STEPS: usize = 24;

/// A bent seat tube's outline (S-TUBES B1, B6): from the shell, any straight
/// below the arc, the arc up to the straight section, then straight to `top`, `width` wide. The lower end is
/// cut to `at_start` (the shell); the top is square.
fn bent_band(bend: &Bend, bottom: Point, top: Point, width: f64, at_start: &[Cut]) -> Vec<Point> {
    let half = width / 2.0;
    // Centreline from the shell end up to the top, with each point's
    // direction of travel (upward).
    let mut path: Vec<(Point, Point)> = (0..=BEND_STEPS)
        .rev()
        .map(|i| {
            let angle = bend.sweep * i as f64 / BEND_STEPS as f64;
            let p = bend.at(angle);
            // Moving toward the tangent point is turning back along the arc.
            let ahead = bend.at(angle - 1e-6);
            (p, unit_vector(p, ahead))
        })
        .collect();
    path.push((top, unit_vector(bend.tangent, top)));
    // A straight below the arc (S-TUBES B6), down to the shell.
    let low = path[0].0;
    if distance(bottom, low) > 1e-6 {
        path.insert(0, (bottom, unit_vector(bottom, low)));
    }
    let left = |(p, d): (Point, Point), s: f64| Point::new(p.x - d.y * s, p.y + d.x * s);
    // The lower end: lines down the tube at each offset, cut where they meet the shell.
    let (p0, d0) = path[0];
    let down = Point::new(-d0.x, -d0.y);
    let lower: Vec<Point> = (0..=MITRE_STEPS)
        .map(|i| {
            let s = half - width * i as f64 / MITRE_STEPS as f64;
            // Start back up the tube, so the line reaches the shell going down.
            let from = left(
                (Point::new(p0.x - down.x * width, p0.y - down.y * width), d0),
                s,
            );
            at_start
                .iter()
                .filter_map(|c| first_hit(from, down, *c, s, half))
                .filter(|t| *t > 0.0)
                .min_by(f64::total_cmp)
                .map_or(left((p0, d0), s), |t| {
                    Point::new(from.x + down.x * t, from.y + down.y * t)
                })
        })
        .collect();
    // Up the left side, across the top, down the right, across the lower end.
    let mut out: Vec<Point> = vec![lower[0]];
    out.extend(path.iter().skip(1).map(|q| left(*q, half)));
    out.extend(path.iter().skip(1).rev().map(|q| left(*q, -half)));
    out.extend(lower.iter().rev().take(lower.len() - 1));
    out
}

const DIMENSION_GAP: f64 = 80.0;
const LABEL_GAP: f64 = 15.0;

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

    // Pedal ground clearance: dotted from the down pedal's underside to the
    // ground, with the measurement (S-GEOM G13).
    let pedal_bottom = Point::new(p.pedal_down.x, p.pedal_down.y - g.pedal_thickness / 2.0);
    ext(scene, pedal_bottom, Point::new(pedal_bottom.x, ground));
    scene.label(
        Point::new(pedal_bottom.x + LABEL_GAP, (pedal_bottom.y + ground) / 2.0),
        with_unit(s.pedal_ground_clearance, unit),
    );

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
    // Clear of the fork and front wheel.
    let stack_x = top.x.max(p.front_axle.x + g.front_wheel_diameter / 2.0) + DIMENSION_GAP / 2.0;
    linear(
        scene,
        Field::Geometry(V::Stack),
        Point::new(stack_x, top.y),
        Point::new(stack_x, crank.y),
        s.value(V::Stack),
    );
    ext(scene, top, Point::new(stack_x, top.y));
    ext(scene, crank, Point::new(stack_x, crank.y));
    // D: on the same stack-height horizontal as E and F, an arc at the head
    // tube top from the steering axis (down, ahead) up to the horizontal.
    let f = Field::Geometry(V::HeadTubeAngle);
    swept_arc(
        scene,
        top,
        unit_vector(top, p.head_tube_bottom),
        // E's radius: clear of the head tube length (N) beside the tube.
        SEAT_ARC_RADIUS * 1.6,
        text(f, s.value(V::HeadTubeAngle)),
    );
    // E and F: arcs at the saddle point, from each seat line up to the
    // horizontal carried back from the head tube top (G4).
    for (field, from, radius) in [
        (V::EffectiveSeatTubeAngle, crank, SEAT_ARC_RADIUS),
        (
            V::ActualSeatTubeAngle,
            p.seat_tube_at_crank_height,
            SEAT_ARC_RADIUS * 1.6,
        ),
    ] {
        let f = Field::Geometry(field);
        let down = unit_vector(p.saddle, from);
        swept_arc(scene, p.saddle, down, radius, text(f, s.value(field)));
    }
    // Below the ground: front and rear centre on the first row, split at the
    // crank, then wheelbase under them.
    let centre_y = ground - DIMENSION_GAP;
    linear(
        scene,
        Field::Geometry(V::FrontCentre),
        Point::new(crank.x, centre_y),
        Point::new(p.front_axle.x, centre_y),
        s.front_centre,
    );
    linear(
        scene,
        Field::Geometry(V::RearCentre),
        Point::new(p.rear_axle.x, centre_y),
        Point::new(crank.x, centre_y),
        s.rear_centre,
    );
    let wheelbase_y = ground - 2.0 * DIMENSION_GAP;
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
    ext(scene, crank, Point::new(crank.x, centre_y));
    // BB height behind the down pedal, clear of the cranks.
    let bb_x = crank.x - g.pedal_length / 2.0 - DIMENSION_GAP / 2.0;
    linear(
        scene,
        Field::Geometry(V::BottomBracketHeight),
        Point::new(bb_x, ground),
        Point::new(bb_x, crank.y),
        s.value(V::BottomBracketHeight),
    );
    ext(scene, crank, Point::new(bb_x, crank.y));
    // BB drop: from the middle of the line between the axles straight down
    // to crank height (G8).
    ext(scene, p.rear_axle, p.front_axle);
    let mid = Point::new(
        (p.rear_axle.x + p.front_axle.x) / 2.0,
        (p.rear_axle.y + p.front_axle.y) / 2.0,
    );
    linear(
        scene,
        Field::Geometry(V::BottomBracketDrop),
        // Downward, so the label sits ahead of the line.
        mid,
        Point::new(mid.x, crank.y),
        s.value(V::BottomBracketDrop),
    );
    ext(scene, crank, Point::new(mid.x, crank.y));
    // Chainstay below the stay, square to it.
    let stay = unit_vector(p.rear_axle, crank);
    let below = |q: Point| along(q, Point::new(stay.y, -stay.x), DIMENSION_GAP / 2.0);
    linear(
        scene,
        Field::Geometry(V::ChainstayLength),
        below(p.rear_axle),
        below(crank),
        s.value(V::ChainstayLength),
    );
    ext(scene, p.rear_axle, below(p.rear_axle));
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
        // Overhang: the top tube junction to the seat tube top, on the other side.
        if let Some(top) = tube(TubeKind::TopTube) {
            let on = |q: Point| along(q, back, DIMENSION_GAP / 2.0);
            let junction = top.end;
            linear(
                scene,
                Field::SeatTubeOverhang,
                on(t.end),
                on(junction),
                distance(junction, t.end),
            );
            ext(scene, t.end, on(t.end));
            ext(scene, junction, on(junction));
        }
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

/// Arcs for the seat angles at the saddle point.
const SEAT_ARC_RADIUS: f64 = 110.0;
const ARC_STEPS: usize = 16;

/// An arc of `radius` about `vertex`, from direction `from` up to the
/// horizontal on the same side, labelled just inside its middle.
fn swept_arc(scene: &mut Scene, vertex: Point, from: Point, radius: f64, text: String) {
    let start = from.y.atan2(from.x);
    let end = if from.x >= 0.0 {
        0.0
    } else {
        std::f64::consts::PI
    };
    let at = |a: f64, r: f64| Point::new(vertex.x + r * a.cos(), vertex.y + r * a.sin());
    for i in 0..ARC_STEPS {
        let t = |k: usize| start + (end - start) * k as f64 / ARC_STEPS as f64;
        scene.line(Role::Dimension, at(t(i), radius), at(t(i + 1), radius));
    }
    // Inside the arc, the same gap off it as the linear dimensions' labels,
    // which keeps E off the reach extension line.
    scene.label(at((start + end) / 2.0, radius - LABEL_GAP), text);
}

fn distance(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
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
        assert_eq!(count(Role::Crank), 3, "forward arm, pedal and its axle");
        assert_eq!(
            count(Role::CrankGhost),
            2,
            "down arm and pedal outline, and its axle"
        );
        assert_eq!(count(Role::Saddle), 1);
        assert_eq!(count(Role::SeatLine), 1, "effective seat line");
        // Pedal ground clearance: labelled, with a dotted line down to the ground.
        let mut doc = Document::new();
        doc.solve(&Data::builtin());
        let g = doc.solved.geometry.as_ref().unwrap();
        let shown = with_unit(
            g.static_state.pedal_ground_clearance,
            LengthUnit::Millimeters,
        );
        assert!(
            s.items
                .iter()
                .any(|i| matches!(&i.shape, Shape::Label { text, .. } if *text == shown)),
            "{shown}"
        );
        let ground = g.static_state.points.rear_contact.y;
        assert!(s.items.iter().any(|i| i.role == Role::Extension
            && matches!(i.shape, Shape::Line { b, .. } if b.y == ground && b.x == 0.0)));
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
        let mut expected: Vec<String> = "ABDEFGHIJKLMNOQRS".chars().map(String::from).collect();
        let mut got = letters.clone();
        got.sort();
        expected.sort();
        assert_eq!(got, expected);
    }

    /// The seat tube axis shows as a dashed line from the seat tube top to
    /// the saddle point, not a centreline down through the frame.
    #[test]
    fn seat_axis_is_dashed_from_the_seat_tube_top_to_the_saddle() {
        let mut doc = Document::new();
        doc.solve(&Data::builtin());
        let scene = build(
            &doc.solved,
            &doc.components,
            0.0,
            None,
            LengthUnit::Millimeters,
        )
        .unwrap();
        let p = &doc.solved.geometry.as_ref().unwrap().static_state.points;
        let top = doc.solved.tubes.as_ref().unwrap().seat_tube_top;
        let lines = |a: Point, b: Point| {
            scene
                .items
                .iter()
                .filter(|i| matches!(i.shape, Shape::Line { a: x, b: y, .. } if x == a && y == b))
                .map(|i| i.role)
                .collect::<Vec<_>>()
        };
        assert_eq!(lines(top, p.saddle), [Role::Extension]);
        assert!(lines(p.seat_tube_at_crank_height, p.saddle).is_empty());
    }

    /// The front axle is drawn like the rear, and the steerer runs from the
    /// crown up past the head tube top.
    #[test]
    fn front_axle_and_steerer_through_the_head_tube() {
        let mut doc = Document::new();
        doc.solve(&Data::builtin());
        let scene = build(
            &doc.solved,
            &doc.components,
            0.0,
            None,
            LengthUnit::Millimeters,
        )
        .unwrap();
        let p = &doc.solved.geometry.as_ref().unwrap().static_state.points;
        let axles = scene
            .items
            .iter()
            .filter(|i| {
                matches!(i.shape, Shape::Circle { centre, radius }
                    if radius == AXLE_RADIUS && (centre == p.front_axle || centre == p.rear_axle))
            })
            .count();
        assert_eq!(axles, 2);
        // Some fork shape reaches past the head tube top along the steering axis.
        let axis = unit_vector(p.head_tube_bottom, p.head_tube_top);
        let along =
            |q: Point| (q.x - p.head_tube_top.x) * axis.x + (q.y - p.head_tube_top.y) * axis.y;
        let highest = scene
            .items
            .iter()
            .filter(|i| i.role == Role::Fork)
            .flat_map(|i| match &i.shape {
                Shape::Polygon(q) => q.clone(),
                _ => vec![],
            })
            .map(along)
            .fold(f64::MIN, f64::max);
        assert!(
            (highest - STEERER_ABOVE_HEAD_TUBE).abs() < 1e-6,
            "{highest}"
        );
    }

    /// The ghost crank's pedal is drawn whole, and its arm isn't drawn
    /// inside the pedal.
    #[test]
    fn the_ghost_arm_stops_at_its_pedal() {
        let mut scene = Scene::default();
        let (centre, pedal) = (Point::new(0.0, 0.0), Point::new(0.0, -170.0));
        crank(&mut scene, Role::CrankGhost, centre, pedal, 105.0, 17.5);
        let [
            Item {
                shape: Shape::Segments(segments),
                ..
            },
            Item {
                shape:
                    Shape::Circle {
                        centre: axle,
                        radius,
                    },
                ..
            },
        ] = scene.items.as_slice()
        else {
            panic!("one outline and the pedal axle");
        };
        assert_eq!((*axle, *radius), (pedal, PEDAL_AXLE_RADIUS));
        let inside_box =
            |p: Point| (p.x - pedal.x).abs() < 52.5 - 1e-6 && (p.y - pedal.y).abs() < 8.75 - 1e-6;
        for (a, b) in segments {
            let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            assert!(!inside_box(mid), "an arm edge inside the pedal at {mid:?}");
        }
        // All four pedal edges, whole.
        let (hx, hy) = (52.5, 8.75);
        let corners = [
            Point::new(pedal.x - hx, pedal.y - hy),
            Point::new(pedal.x + hx, pedal.y - hy),
            Point::new(pedal.x + hx, pedal.y + hy),
            Point::new(pedal.x - hx, pedal.y + hy),
        ];
        for i in 0..4 {
            let edge = (corners[i], corners[(i + 1) % 4]);
            assert!(segments.contains(&edge), "pedal edge {i}");
        }
    }

    /// A crank arm is round at both ends, about the crank centre and the
    /// pedal axle.
    #[test]
    fn crank_arms_are_round_about_the_spindle_and_pedal() {
        let (centre, pedal) = (Point::new(0.0, 0.0), Point::new(170.0, 0.0));
        let arm = crank_arm(centre, pedal);
        let on = |c: Point, r: f64| {
            arm.iter()
                .filter(|p| (distance(**p, c) - r).abs() < 1e-9)
                .count()
        };
        assert!(on(centre, CRANK_SPINDLE_RADIUS) > 10);
        assert!(on(pedal, CRANK_PEDAL_RADIUS) > 10);
        // Nothing reaches past either end's circle.
        assert!(
            arm.iter()
                .all(|p| distance(*p, centre) <= 170.0 + CRANK_PEDAL_RADIUS + 1e-9)
        );
        assert!(
            arm.iter()
                .all(|p| distance(*p, pedal) <= 170.0 + CRANK_SPINDLE_RADIUS + 1e-9)
        );
    }

    /// The rear dropout joins both stays and is round about the axle.
    #[test]
    fn the_dropout_joins_the_stays_round_the_axle() {
        let mut doc = Document::new();
        doc.solve(&Data::builtin());
        let t = doc.solved.tubes.as_ref().unwrap();
        let axle = doc
            .solved
            .geometry
            .as_ref()
            .unwrap()
            .static_state
            .points
            .rear_axle;
        let outline = dropout(t, axle).expect("a hardtail has a dropout");
        // Every point is at least the dropout radius from the axle.
        assert!(
            outline
                .iter()
                .all(|p| distance(*p, axle) >= DROPOUT_RADIUS - 1e-9)
        );
        // Away from the stays, the outline is the arc: some points sit on it.
        let on_arc = outline
            .iter()
            .filter(|p| (distance(**p, axle) - DROPOUT_RADIUS).abs() < 1e-9)
            .count();
        assert!(on_arc > 10, "{on_arc}");
        // It reaches up each stay.
        let cs = t.tube(TubeKind::Chainstay).unwrap();
        let ss = t.tube(TubeKind::Seatstay).unwrap();
        for far in [cs.start, ss.end] {
            let d = unit_vector(axle, far);
            let reach = outline
                .iter()
                .map(|p| (p.x - axle.x) * d.x + (p.y - axle.y) * d.y)
                .fold(f64::MIN, f64::max);
            assert!((reach - DROPOUT_REACH).abs() < 1e-6, "{reach}");
        }
    }

    /// The stays and dropout are one piece: drawn once, as a union of the
    /// dropout and both stays, at any angle between the stays.
    #[test]
    fn stays_and_dropout_are_one_piece() {
        for drop in [60.0, -29.0, -120.0] {
            let mut doc = Document::new();
            doc.apply(Command::PinGeometry(V::BottomBracketDrop, drop));
            doc.solve(&Data::builtin());
            let t = doc.solved.tubes.as_ref().unwrap();
            let axle = doc
                .solved
                .geometry
                .as_ref()
                .unwrap()
                .static_state
                .points
                .rear_axle;
            let parts = rear_end(t, axle).expect("joined");
            assert_eq!(parts.len(), 3);
            let scene = build(
                &doc.solved,
                &doc.components,
                0.0,
                None,
                LengthUnit::Millimeters,
            )
            .unwrap();
            let unions = scene
                .items
                .iter()
                .filter(|i| matches!(&i.shape, Shape::Union(p) if p == &parts))
                .count();
            assert_eq!(unions, 1, "drop {drop}");
            // The stays aren't drawn again on their own: only the other tubes
            // and the dropout's axle are frame shapes besides the union.
            let polygons = scene
                .items
                .iter()
                .filter(|i| i.role == Role::Frame && matches!(i.shape, Shape::Polygon(_)))
                .count();
            assert_eq!(polygons, 4, "head, top, down and seat tubes");
        }
    }

    /// Tube ends meet the edge of the head and seat tubes, and the BB shell's outline.
    /// The seat tube and down tube are both cut to the shell.
    #[test]
    fn tube_ends_are_mitred_to_the_tubes_they_join() {
        let mut doc = Document::new();
        doc.solve(&Data::builtin());
        let t = doc.solved.tubes.as_ref().unwrap();
        let on_edge = |p: Point, joined: TubeKind| {
            let j = t.tube(joined).unwrap();
            let from = j.bend.map_or(j.start, |b| b.tangent);
            let d = unit_vector(from, j.end);
            let off = ((p.x - from.x) * -d.y + (p.y - from.y) * d.x).abs();
            (off - j.outside_diameter / 2.0).abs() < 1e-6
        };
        let shell = t.tube(TubeKind::BottomBracketShell).unwrap();
        let on_shell =
            |p: Point| (distance(p, shell.start) - shell.outside_diameter / 2.0).abs() < 1e-6;
        for (kind, start_ok, end_ok) in [
            (
                TubeKind::TopTube,
                Some(TubeKind::HeadTube),
                Some(TubeKind::SeatTube),
            ),
            (TubeKind::DownTube, Some(TubeKind::HeadTube), None),
            (TubeKind::Seatstay, None, Some(TubeKind::SeatTube)),
        ] {
            let tube = t.tube(kind).unwrap();
            let (a, b) = miters(kind, t);
            let outline = mitred_band(tube.start, tube.end, tube.outside_diameter, &a, &b);
            // Starts at the start's left corner, then the end's, left first.
            let (sl, el) = (outline[0], outline[1]);
            if let Some(j) = start_ok {
                assert!(on_edge(sl, j), "{kind:?} start");
            }
            if let Some(j) = end_ok {
                assert!(on_edge(el, j), "{kind:?} end");
            }
        }
        // At the BB the down tube's end and the seat tube's start are cut to
        // the shell's outline.
        let down = t.tube(TubeKind::DownTube).unwrap();
        let (a, b) = miters(TubeKind::DownTube, t);
        let outline = mitred_band(down.start, down.end, down.outside_diameter, &a, &b);
        assert!(outline[1..=1 + MITRE_STEPS].iter().all(|p| on_shell(*p)));
        // The default bike's seat tube is bent (S-TUBES B1); its whole lower
        // end meets the shell.
        let seat = t.tube(TubeKind::SeatTube).unwrap();
        let bend = seat.bend.expect("default seat tube is bent");
        let (a, _) = miters(TubeKind::SeatTube, t);
        let outline = bent_band(&bend, seat.start, seat.end, seat.outside_diameter, &a);
        let lower = std::iter::once(outline[0])
            .chain(outline[outline.len() - MITRE_STEPS..].iter().copied());
        assert!(lower.clone().count() == MITRE_STEPS + 1);
        for p in lower {
            assert!(
                on_shell(p),
                "seat tube meets the shell: {}",
                distance(p, shell.start)
            );
        }
        let down = t.tube(TubeKind::DownTube).unwrap();
        let (a, b) = miters(TubeKind::DownTube, t);
        let outline = mitred_band(down.start, down.end, down.outside_diameter, &a, &b);
        assert!(outline.len() > 4, "the shell end is an arc");
        // A coped end dips into the joined tube in the middle (rounded).
        let top = t.tube(TubeKind::TopTube).unwrap();
        let (a, b) = miters(TubeKind::TopTube, t);
        let outline = mitred_band(top.start, top.end, top.outside_diameter, &a, &b);
        let head = t.tube(TubeKind::HeadTube).unwrap();
        let d = unit_vector(head.start, head.end);
        let from_axis = |p: Point| ((p.x - head.start.x) * -d.y + (p.y - head.start.y) * d.x).abs();
        let middle = outline[outline.len() - MITRE_STEPS / 2];
        assert!(
            from_axis(middle) < head.outside_diameter / 2.0 - 1.0,
            "{}",
            from_axis(middle)
        );
    }
}
