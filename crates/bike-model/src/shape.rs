//! Plane polygon helpers for component outlines (S-COMPONENTS C8, C11).
//!
//! A polygon is a list of corners, closed implicitly (the first point isn't repeated).

// `!(x > 0.0)` is deliberate throughout: unlike `x <= 0.0`, it also rejects NaN.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::solved::Point;

/// Signed area: positive when the corners run counter-clockwise.
pub fn signed_area(polygon: &[Point]) -> f64 {
    edges(polygon)
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum::<f64>()
        / 2.0
}

/// The area centroid, or `None` for a polygon with no area.
pub fn centroid(polygon: &[Point]) -> Option<Point> {
    let area = signed_area(polygon);
    if !(area.abs() > 1e-12) {
        return None;
    }
    let (mut x, mut y) = (0.0, 0.0);
    for (a, b) in edges(polygon) {
        let cross = a.x * b.y - b.x * a.y;
        x += (a.x + b.x) * cross;
        y += (a.y + b.y) * cross;
    }
    Some(Point::new(x / (6.0 * area), y / (6.0 * area)))
}

/// The area-weighted centroid of several polygons (overlaps count twice).
pub fn combined_centroid(polygons: &[Vec<Point>]) -> Option<Point> {
    let (mut total, mut x, mut y) = (0.0, 0.0, 0.0);
    for polygon in polygons {
        let area = signed_area(polygon).abs();
        if let Some(c) = centroid(polygon) {
            total += area;
            x += area * c.x;
            y += area * c.y;
        }
    }
    (total > 0.0).then(|| Point::new(x / total, y / total))
}

/// True when the polygon has at least three finite corners, an area, and no
/// two non-adjacent edges touching (C11).
pub fn is_simple(polygon: &[Point]) -> bool {
    let n = polygon.len();
    if n < 3
        || polygon
            .iter()
            .any(|p| !(p.x.is_finite() && p.y.is_finite()))
    {
        return false;
    }
    if !(signed_area(polygon).abs() > 1e-9) {
        return false;
    }
    let edges: Vec<_> = edges(polygon).collect();
    for i in 0..n {
        for j in i + 1..n {
            let adjacent = j == i + 1 || (i == 0 && j == n - 1);
            if adjacent {
                // Adjacent edges may only share their common corner: reject a fold-back.
                let (a, b) = edges[i];
                let (c, d) = edges[j];
                let shared = if j == i + 1 { b } else { a };
                let (p, q) = if j == i + 1 { (a, d) } else { (b, c) };
                if collinear_overlap(shared, p, q) {
                    return false;
                }
                continue;
            }
            if segments_touch(edges[i].0, edges[i].1, edges[j].0, edges[j].1) {
                return false;
            }
        }
    }
    true
}

/// True when `p` is inside the polygon or on its boundary.
pub fn contains(polygon: &[Point], p: Point) -> bool {
    const ON_EDGE: f64 = 1e-6;
    let mut inside = false;
    for (a, b) in edges(polygon) {
        if distance_to_segment(p, a, b) <= ON_EDGE {
            return true;
        }
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if p.x < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// The first point where the segment from `from` to `to` meets the polygon's
/// boundary, measured from `from`; `None` if it never does.
pub fn first_crossing(polygon: &[Point], from: Point, to: Point) -> Option<Point> {
    let d = Point::new(to.x - from.x, to.y - from.y);
    let mut best: Option<f64> = None;
    for (a, b) in edges(polygon) {
        let e = Point::new(b.x - a.x, b.y - a.y);
        let denom = cross(d, e);
        if denom.abs() < 1e-12 {
            continue;
        }
        let w = Point::new(a.x - from.x, a.y - from.y);
        let t = cross(w, e) / denom;
        let u = cross(w, d) / denom;
        if (-1e-12..=1.0 + 1e-12).contains(&t)
            && (-1e-12..=1.0 + 1e-12).contains(&u)
            && best.is_none_or(|b| t < b)
        {
            best = Some(t);
        }
    }
    best.map(|t| Point::new(from.x + t * d.x, from.y + t * d.y))
}

/// Rotates a point about the origin by `degrees`, counter-clockwise.
pub fn rotate_degrees(p: Point, degrees: f64) -> Point {
    let (s, c) = degrees.to_radians().sin_cos();
    Point::new(c * p.x - s * p.y, s * p.x + c * p.y)
}

/// A regular polygon approximating a circle, for drawing eyes and bolts.
pub fn circle(centre: Point, radius: f64, sides: usize) -> Vec<Point> {
    (0..sides)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / sides as f64;
            Point::new(centre.x + radius * a.cos(), centre.y + radius * a.sin())
        })
        .collect()
}

/// An axis-aligned rectangle from its corners.
pub fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
    vec![
        Point::new(x0, y0),
        Point::new(x1, y0),
        Point::new(x1, y1),
        Point::new(x0, y1),
    ]
}

fn edges(polygon: &[Point]) -> impl Iterator<Item = (Point, Point)> + '_ {
    let n = polygon.len();
    (0..n).map(move |i| (polygon[i], polygon[(i + 1) % n]))
}

fn cross(a: Point, b: Point) -> f64 {
    a.x * b.y - a.y * b.x
}

fn orientation(a: Point, b: Point, c: Point) -> f64 {
    cross(
        Point::new(b.x - a.x, b.y - a.y),
        Point::new(c.x - a.x, c.y - a.y),
    )
}

fn segments_touch(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (o1, o2) = (orientation(a, b, c), orientation(a, b, d));
    let (o3, o4) = (orientation(c, d, a), orientation(c, d, b));
    if o1 * o2 < 0.0 && o3 * o4 < 0.0 {
        return true;
    }
    let eps = 1e-9;
    (o1.abs() < eps && on_segment(a, b, c))
        || (o2.abs() < eps && on_segment(a, b, d))
        || (o3.abs() < eps && on_segment(c, d, a))
        || (o4.abs() < eps && on_segment(c, d, b))
}

fn on_segment(a: Point, b: Point, p: Point) -> bool {
    p.x >= a.x.min(b.x) - 1e-9
        && p.x <= a.x.max(b.x) + 1e-9
        && p.y >= a.y.min(b.y) - 1e-9
        && p.y <= a.y.max(b.y) + 1e-9
}

/// Two edges from `shared` to `p` and `q` that point the same way overlap.
fn collinear_overlap(shared: Point, p: Point, q: Point) -> bool {
    let u = Point::new(p.x - shared.x, p.y - shared.y);
    let v = Point::new(q.x - shared.x, q.y - shared.y);
    cross(u, v).abs() < 1e-9 && u.x * v.x + u.y * v.y > 0.0
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let ab = Point::new(b.x - a.x, b.y - a.y);
    let len2 = ab.x * ab.x + ab.y * ab.y;
    let t = if len2 > 0.0 {
        (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.x - a.x - t * ab.x).hypot(p.y - a.y - t * ab.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<Point> {
        rectangle(-1.0, -1.0, 1.0, 1.0)
    }

    #[test]
    fn square_area_centroid_and_containment() {
        assert_eq!(signed_area(&square()), 4.0);
        assert_eq!(centroid(&square()), Some(Point::new(0.0, 0.0)));
        assert!(contains(&square(), Point::new(0.5, 0.5)));
        assert!(contains(&square(), Point::new(1.0, 0.0)));
        assert!(!contains(&square(), Point::new(1.5, 0.0)));
        assert!(is_simple(&square()));
    }

    #[test]
    fn a_bow_tie_is_not_simple() {
        let bow = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(1.0, 0.0),
            Point::new(0.0, 1.0),
        ];
        assert!(!is_simple(&bow));
        assert!(!is_simple(&[Point::new(0.0, 0.0), Point::new(1.0, 0.0)]));
    }

    #[test]
    fn first_crossing_is_nearest_the_start() {
        let hit = first_crossing(&square(), Point::new(5.0, 0.0), Point::new(0.0, 0.0)).unwrap();
        assert!((hit.x - 1.0).abs() < 1e-12 && hit.y.abs() < 1e-12);
        assert!(first_crossing(&square(), Point::new(5.0, 5.0), Point::new(3.0, 5.0)).is_none());
    }

    #[test]
    fn rotation_is_counter_clockwise() {
        let p = rotate_degrees(Point::new(1.0, 0.0), 90.0);
        assert!(p.x.abs() < 1e-12 && (p.y - 1.0).abs() < 1e-12);
    }
}
