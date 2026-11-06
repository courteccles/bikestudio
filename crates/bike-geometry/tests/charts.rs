//! S-GEOM "Done when": the Nicolai charts reproduce within ±3 mm and ±0.3° (S-REF R6).
//! S-TUBES T7: the charts' seat tube length and overhang place the tubes.
//! S-COMPONENTS: the Pinion charts solve with a drive unit and a library fork.

use std::path::PathBuf;

use bike_components::{Library, resolve};
use bike_defaults::{Defaults, WheelSizeEntry};
use bike_geometry::{solve_geometry, solve_geometry_with, solve_tubes, solve_tubes_with};
use bike_model::{
    ComponentId, ComponentName, ComponentReference, Degrees, DriveUnit, DriveUnitComponent,
    DriveUnitKind, ForkOverrides, GeometryVariable as V, MassProperties, Material, Millimeters,
    Mount, OutlineAccuracy, Parameter, Point, Project, Severity, SolvedGeometry, TubeKind,
    WheelSize,
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
    frame_material: Option<String>,
    /// Kilograms.
    frame_weight: Option<f64>,
    notes: Vec<String>,
    sizes: Vec<Size>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Size {
    size: String,
    seat_tube_length: Option<f64>,
    seat_tube_overhang: Option<f64>,
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

/// S-EDIT-CENTERS: pin front and rear center instead of reach and chainstay;
/// reach, chainstay and the chart values still come out.
#[test]
fn charts_reproduce_with_front_and_rear_center_pinned() {
    let mut failures = Vec::new();
    for chart in charts() {
        for size in &chart.sizes {
            let mut project = project_for(&chart, size);
            project
                .geometry
                .pin(V::HeadTubeLength, head_tube_length(&chart, size));
            let base = solve(&chart, size, &project).static_state;
            let g = &mut project.geometry;
            g.free(V::Reach);
            g.free(V::ChainstayLength);
            g.pin(V::FrontCentre, base.value(V::FrontCentre));
            g.pin(V::RearCentre, base.value(V::RearCentre));
            let s = solve(&chart, size, &project).static_state;
            let mut c = Checker {
                test: "head_tube_length_pinned",
                chart: &chart.name,
                size,
                failures: &mut failures,
            };
            c.check("reach", s.value(V::Reach), size.reach);
            c.check(
                "chainstay",
                s.value(V::ChainstayLength),
                size.chainstay_length,
            );
            if let Some(wb) = size.wheelbase {
                c.check("wheelbase", s.value(V::Wheelbase), wb);
            }
            if let Some(drop) = size.bottom_bracket_drop {
                c.check("BB drop", s.value(V::BottomBracketDrop), drop);
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

/// S-TUBES T7: pinning each chart's seat tube length and overhang puts the seat
/// tube top and the top tube junction on the seat tube axis at those distances.
#[test]
fn chart_seat_tubes_place_the_tubes() {
    let mut checked = 0;
    for chart in charts() {
        for size in &chart.sizes {
            let (Some(length), Some(overhang)) = (size.seat_tube_length, size.seat_tube_overhang)
            else {
                continue;
            };
            let mut project = project_for(&chart, size);
            project.tubes.seat_tube_length = Parameter::Pinned(length);
            project.tubes.junctions.seat_tube_overhang = Some(Millimeters(overhang));
            let geometry = solve(&chart, size, &project);
            let defaults = Defaults::builtin().unwrap();
            let (tubes, diagnostics) = solve_tubes(&project, Some(&geometry), &defaults);
            let label = format!("{} {}", chart.name, size.size);
            let tubes = tubes.unwrap_or_else(|| panic!("{label}: {diagnostics:?}"));

            let points = &geometry.static_state.points;
            let base = points.seat_tube_at_crank_height;
            let axis = (points.saddle.x - base.x, points.saddle.y - base.y);
            let along = |p: bike_model::Point| {
                let (dx, dy) = (p.x - base.x, p.y - base.y);
                let n = axis.0.hypot(axis.1);
                // Distance along the axis, and distance off it.
                (
                    (dx * axis.0 + dy * axis.1) / n,
                    (dx * axis.1 - dy * axis.0).abs() / n,
                )
            };
            let (top_along, top_off) = along(tubes.seat_tube_top);
            let junction = tubes.tube(TubeKind::TopTube).unwrap().end;
            let (junction_along, junction_off) = along(junction);
            assert!(
                (top_along - length).abs() < 0.01 && top_off < 0.01,
                "{label}: seat tube top"
            );
            assert!(
                (junction_along - (length - overhang)).abs() < 0.01 && junction_off < 0.01,
                "{label}: top tube junction"
            );
            if let Some(seatstay) = tubes.tube(TubeKind::Seatstay) {
                let (a, off) = along(seatstay.end);
                assert!(
                    (a - (length - overhang)).abs() < 0.01 && off < 0.01,
                    "{label}: seatstay"
                );
            }
            checked += 1;
        }
    }
    assert_eq!(
        checked, 27,
        "every size of every chart gives a seat tube length and overhang"
    );
}

/// S-TUBES T7: published frame weights are reported against the estimate, not tested.
#[test]
fn chart_frame_weights_are_reported() {
    for chart in charts() {
        let Some(weight) = chart.frame_weight else {
            continue;
        };
        let material = match chart.frame_material.as_deref() {
            Some("steel") => Material::Steel,
            Some("aluminium") => Material::Aluminium,
            Some("titanium") => Material::Titanium,
            Some("carbon") => Material::Carbon,
            other => panic!("{}: frame_material {other:?}", chart.name),
        };
        for size in &chart.sizes {
            let mut project = project_for(&chart, size);
            project.tubes.material = Some(material);
            if let Some(l) = size.seat_tube_length {
                project.tubes.seat_tube_length = Parameter::Pinned(l);
            }
            project.tubes.junctions.seat_tube_overhang = size.seat_tube_overhang.map(Millimeters);
            let geometry = solve(&chart, size, &project);
            let (tubes, _) = solve_tubes(&project, Some(&geometry), &Defaults::builtin().unwrap());
            let estimate = tubes.unwrap().frame_mass / 1000.0;
            println!(
                "{} {}: estimate {estimate:.2} kg, published {weight:.2} kg ({:+.0}%)",
                chart.name,
                size.size,
                (estimate / weight - 1.0) * 100.0
            );
        }
    }
}

/// A made-up drive unit for the tests only: not Pinion's dimensions, which
/// aren't published (S-COMPONENTS C10). Roughly gearbox-sized around the crank.
fn synthetic_unit(kind: DriveUnitKind) -> DriveUnitComponent {
    DriveUnitComponent {
        name: ComponentName {
            make: "Test".into(),
            model: "Synthetic unit".into(),
            year: 2026,
            variant: "not-real-data".into(),
        },
        kind,
        motor: None,
        mounts: vec![
            Mount {
                position: Point::new(70.0, 40.0),
                diameter: Millimeters(8.0),
            },
            Mount {
                position: Point::new(-60.0, -50.0),
                diameter: Millimeters(8.0),
            },
        ],
        outline: vec![
            Point::new(-90.0, -70.0),
            Point::new(100.0, -70.0),
            Point::new(115.0, 30.0),
            Point::new(80.0, 75.0),
            Point::new(-90.0, 60.0),
        ],
        outline_accuracy: OutlineAccuracy::Exact,
        output_sprocket: None,
        ratios: None,
        mass: MassProperties {
            mass: 2700.0,
            centre_of_mass: None,
        },
        frame_mount_mass: 120.0,
        svg_path: None,
        source: "Synthetic test data, not a real unit.".into(),
    }
}

fn pinion_charts() -> Vec<Chart> {
    let charts: Vec<Chart> = charts()
        .into_iter()
        .filter(|c| c.name.contains("Argon GTB GPI") || c.name.contains("S16 MGU"))
        .collect();
    assert_eq!(charts.len(), 2);
    charts
}

/// The Pinion charts with a drive unit: no BB shell, the down tube and
/// chainstays end on the outline, and the chart geometry doesn't move.
#[test]
fn pinion_charts_solve_with_a_drive_unit() {
    let library = Library::builtin().unwrap();
    for chart in pinion_charts() {
        for size in &chart.sizes {
            let label = format!("{} {}", chart.name, size.size);
            let mut project = project_for(&chart, size);
            project
                .geometry
                .pin(V::HeadTubeLength, head_tube_length(&chart, size));
            let bare = solve(&chart, size, &project);
            let defaults = defaults_for(
                size.front_tyre_diameter
                    .unwrap_or(chart.front_tyre_diameter),
                size.rear_tyre_diameter.unwrap_or(chart.rear_tyre_diameter),
            );

            project.drive.unit = Some(DriveUnit {
                component: ComponentReference::Custom(synthetic_unit(DriveUnitKind::Gearbox)),
                rotation: Degrees(-5.0),
            });
            let (resolved, notes) = resolve(&project, &library);
            assert!(
                notes.iter().all(|d| d.severity != Severity::Error),
                "{label}: {notes:?}"
            );
            let (geometry, _) = solve_geometry_with(&project, &defaults, &resolved);
            let geometry = geometry.unwrap();
            assert_eq!(geometry, bare, "{label}: geometry moved");

            let (tubes, diagnostics) =
                solve_tubes_with(&project, Some(&geometry), &defaults, &resolved);
            let tubes = tubes.unwrap_or_else(|| panic!("{label}: {diagnostics:?}"));
            assert!(
                tubes.tube(TubeKind::BottomBracketShell).is_none(),
                "{label}"
            );
            let outline = &resolved.drive_unit.as_ref().unwrap().outline;
            let on_outline = |p: Point| {
                outline
                    .iter()
                    .zip(outline.iter().cycle().skip(1))
                    .any(|(a, b)| {
                        let (ab, ap) = ((b.x - a.x, b.y - a.y), (p.x - a.x, p.y - a.y));
                        let len = ab.0.hypot(ab.1);
                        let t = (ap.0 * ab.0 + ap.1 * ab.1) / (len * len);
                        (ap.0 * ab.1 - ap.1 * ab.0).abs() / len < 1e-6 && (0.0..=1.0).contains(&t)
                    })
            };
            assert!(
                on_outline(tubes.tube(TubeKind::DownTube).unwrap().end),
                "{label}"
            );
            match tubes.tube(TubeKind::Chainstay) {
                Some(stay) => assert!(on_outline(stay.start), "{label}"),
                None => assert!(chart.rear_travel.is_some(), "{label}"),
            }
            assert!(
                tubes.fittings.iter().any(|f| f.mass == 120.0),
                "{label}: mounting plate"
            );
        }
    }
}

/// The library fork selected with the sheet's axle-to-crown kept as an
/// override: the chart geometry doesn't move (S-COMPONENTS Done when).
#[test]
fn pinion_charts_solve_with_a_library_fork() {
    let library = Library::builtin().unwrap();
    for (chart, fork) in pinion_charts().into_iter().zip([
        "rockshox/sid-sl-select/2025/29-110-44",
        "rockshox/lyrik-ultimate/2025/29-160-44",
    ]) {
        for size in &chart.sizes {
            let mut project = project_for(&chart, size);
            project
                .geometry
                .pin(V::HeadTubeLength, head_tube_length(&chart, size));
            let bare = solve(&chart, size, &project);
            project.components.fork = Some(ComponentReference::Library(ComponentId(fork.into())));
            // Front travel set to the fork's, so there's no travel mismatch.
            project.front_travel = library
                .forks
                .get(&ComponentId(fork.into()))
                .map(|f| f.travel);
            let (resolved, _) = resolve(&project, &library);
            assert!(resolved.fork.is_some(), "{fork}");
            let defaults = defaults_for(
                size.front_tyre_diameter
                    .unwrap_or(chart.front_tyre_diameter),
                size.rear_tyre_diameter.unwrap_or(chart.rear_tyre_diameter),
            );
            let (geometry, _) = solve_geometry_with(&project, &defaults, &resolved);
            let geometry = geometry.unwrap();
            assert_eq!(
                geometry.static_state, bare.static_state,
                "{} {}",
                chart.name, size.size
            );
            assert_eq!(geometry.fork_axle_to_crown, chart.fork_axle_to_crown);
        }
    }
}
