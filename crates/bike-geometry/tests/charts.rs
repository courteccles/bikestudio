//! S-GEOM "Done when": the Nicolai charts reproduce within ±3 mm and ±0.3° (S-REF R6).

use std::path::PathBuf;

use bike_defaults::{Defaults, WheelSizeEntry};
use bike_geometry::solve_geometry;
use bike_model::{
    ForkOverrides, GeometryVariable as V, Millimeters, Project, Severity, SolvedGeometry, WheelSize,
};
use serde::Deserialize;

const LENGTH_TOLERANCE: f64 = 3.0;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Chart {
    name: String,
    frame: String,
    source: String,
    retrieved: String,
    front_tyre_diameter: f64,
    rear_tyre_diameter: f64,
    fork_axle_to_crown: f64,
    fork_offset: f64,
    lower_headset_stack: f64,
    front_travel: Option<f64>,
    rear_travel: Option<f64>,
    notes: Vec<String>,
    sizes: Vec<Size>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Size {
    size: String,
    front_tyre_diameter: Option<f64>,
    rear_tyre_diameter: Option<f64>,
    lower_headset_stack: Option<f64>,
    stack: f64,
    reach: f64,
    head_tube_angle: f64,
    effective_seat_tube_angle: Option<f64>,
    actual_seat_tube_angle: Option<f64>,
    head_tube_length: f64,
    chainstay_length: f64,
    bottom_bracket_drop: Option<f64>,
    wheelbase: Option<f64>,
    front_centre: Option<f64>,
    #[serde(default)]
    corrections: Vec<Correction>,
    #[serde(default)]
    known_gaps: Vec<KnownGap>,
}

/// A chart value that doesn't agree with the chart's own other values by more
/// than the tolerance. It is excluded from the pass/fail check, but its gap is
/// recorded so that any change in the solver's result is still caught.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KnownGap {
    /// `head_tube_length_pinned` or `bb_drop_pinned`.
    test: String,
    /// `wheelbase`, `front centre`, `BB drop` or `head tube length`.
    value: String,
    /// Solved minus chart, in mm.
    gap: f64,
    reason: String,
}

/// How far a recorded gap may move before the test reports it.
const GAP_TOLERANCE: f64 = 0.2;

/// An S-REF R7 correction: the chart contradicts itself, so a value is changed.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // read for documentation; the corrected value is what the size holds
struct Correction {
    field: String,
    original: f64,
    corrected: f64,
    reason: String,
}

fn charts() -> Vec<Chart> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/geometry");
    let mut charts: Vec<Chart> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .map(|p| {
            toml::from_str(&std::fs::read_to_string(&p).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        })
        .collect();
    charts.sort_by(|a, b| a.name.cmp(&b.name));
    charts
}

/// Defaults with this size's tyres as wheel sizes "chart-front" and "chart-rear".
fn defaults_for(front: f64, rear: f64) -> Defaults {
    let mut d = Defaults::builtin().unwrap();
    for (key, diameter) in [("chart-front", front), ("chart-rear", rear)] {
        d.wheel_sizes.sizes.insert(
            key.into(),
            WheelSizeEntry {
                diameter,
                source: "chart".into(),
            },
        );
    }
    for bike in &mut d.geometry.bikes {
        bike.wheels.push("chart-front".into());
    }
    d
}

fn project_for(chart: &Chart, size: &Size) -> Project {
    let mut project = Project {
        front_wheel: Some(WheelSize("chart-front".into())),
        rear_wheel: Some(WheelSize("chart-rear".into())),
        front_travel: chart.front_travel.map(Millimeters),
        rear_travel: chart.rear_travel.map(Millimeters),
        fork: ForkOverrides {
            axle_to_crown: Some(Millimeters(chart.fork_axle_to_crown)),
            offset: Some(Millimeters(chart.fork_offset)),
        },
        ..Project::default()
    };
    let g = &mut project.geometry;
    g.pin(V::Stack, size.stack);
    g.pin(V::Reach, size.reach);
    g.pin(V::HeadTubeAngle, size.head_tube_angle);
    g.pin(V::ChainstayLength, size.chainstay_length);
    if let Some(a) = size.effective_seat_tube_angle {
        g.pin(V::EffectiveSeatTubeAngle, a);
    }
    if let Some(a) = size.actual_seat_tube_angle {
        g.pin(V::ActualSeatTubeAngle, a);
    }
    project
}

fn head_tube_length(chart: &Chart, size: &Size) -> f64 {
    size.head_tube_length
        + size
            .lower_headset_stack
            .unwrap_or(chart.lower_headset_stack)
}

fn solve(chart: &Chart, size: &Size, project: &Project) -> SolvedGeometry {
    let defaults = defaults_for(
        size.front_tyre_diameter
            .unwrap_or(chart.front_tyre_diameter),
        size.rear_tyre_diameter.unwrap_or(chart.rear_tyre_diameter),
    );
    let (solved, diagnostics) = solve_geometry(project, &defaults);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "{} {}: {errors:?}",
        chart.name,
        size.size
    );
    solved.unwrap()
}

struct Checker<'a> {
    test: &'static str,
    chart: &'a str,
    size: &'a Size,
    failures: &'a mut Vec<String>,
}

impl Checker<'_> {
    fn check(&mut self, value: &str, solved: f64, chart: f64) {
        let diff = solved - chart;
        let label = format!("{} {} {value}", self.chart, self.size.size);
        let known = self
            .size
            .known_gaps
            .iter()
            .find(|g| g.test == self.test && g.value == value);
        match known {
            Some(g) if (diff - g.gap).abs() > GAP_TOLERANCE => self.failures.push(format!(
                "{label}: known gap was {:+.1} mm, now {diff:+.1} mm",
                g.gap
            )),
            Some(_) => {}
            None if diff.abs() > LENGTH_TOLERANCE => self.failures.push(format!(
                "{label}: solved {solved:.1}, chart {chart:.1}, off by {diff:+.1} mm"
            )),
            None => {}
        }
    }
}

#[test]
fn every_chart_is_documented() {
    let charts = charts();
    assert_eq!(charts.len(), 5);
    for chart in &charts {
        assert!(chart.source.starts_with("https://"), "{}", chart.name);
        assert!(
            !chart.retrieved.is_empty() && !chart.notes.is_empty(),
            "{}",
            chart.name
        );
        assert!(["rigid", "hardtail", "full_suspension"].contains(&chart.frame.as_str()));
        for size in &chart.sizes {
            for c in &size.corrections {
                assert!(!c.reason.is_empty() && !c.field.is_empty() && c.original != c.corrected);
            }
            for g in &size.known_gaps {
                assert!(["head_tube_length_pinned", "bb_drop_pinned"].contains(&g.test.as_str()));
                assert!(!g.reason.is_empty() && g.gap.abs() > LENGTH_TOLERANCE);
            }
        }
    }
}

/// Pin stack, reach, head angle, head tube length and chainstay; check the rest.
#[test]
fn charts_reproduce_with_head_tube_length_pinned() {
    let mut failures = Vec::new();
    for chart in charts() {
        for size in &chart.sizes {
            let mut project = project_for(&chart, size);
            project
                .geometry
                .pin(V::HeadTubeLength, head_tube_length(&chart, size));
            let solved = solve(&chart, size, &project);
            let s = &solved.static_state;
            let mut c = Checker {
                test: "head_tube_length_pinned",
                chart: &chart.name,
                size,
                failures: &mut failures,
            };
            if let Some(wb) = size.wheelbase {
                c.check("wheelbase", s.value(V::Wheelbase), wb);
            }
            if let Some(drop) = size.bottom_bracket_drop {
                c.check("BB drop", s.value(V::BottomBracketDrop), drop);
            }
            if let Some(fc) = size.front_centre {
                c.check("front centre", s.front_centre, fc);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} chart values off:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Pin stack, reach, head angle, chainstay and BB drop; check head tube length and wheelbase.
#[test]
fn charts_reproduce_with_bb_drop_pinned() {
    let mut failures = Vec::new();
    for chart in charts() {
        for size in chart
            .sizes
            .iter()
            .filter(|s| s.bottom_bracket_drop.is_some())
        {
            let mut project = project_for(&chart, size);
            project
                .geometry
                .pin(V::BottomBracketDrop, size.bottom_bracket_drop.unwrap());
            let solved = solve(&chart, size, &project);
            let s = &solved.static_state;
            let mut c = Checker {
                test: "bb_drop_pinned",
                chart: &chart.name,
                size,
                failures: &mut failures,
            };
            c.check(
                "head tube length",
                s.value(V::HeadTubeLength),
                head_tube_length(&chart, size),
            );
            if let Some(wb) = size.wheelbase {
                c.check("wheelbase", s.value(V::Wheelbase), wb);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} chart values off:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Pinned seat angles come back unchanged in the static state.
#[test]
fn chart_seat_angles_hold() {
    for chart in charts() {
        for size in &chart.sizes {
            let mut project = project_for(&chart, size);
            project
                .geometry
                .pin(V::HeadTubeLength, head_tube_length(&chart, size));
            let s = solve(&chart, size, &project).static_state;
            if let Some(a) = size.effective_seat_tube_angle {
                assert!((s.value(V::EffectiveSeatTubeAngle) - a).abs() < 1e-9);
            }
            if let Some(a) = size.actual_seat_tube_angle {
                assert!((s.value(V::ActualSeatTubeAngle) - a).abs() < 1e-9);
            }
        }
    }
}
