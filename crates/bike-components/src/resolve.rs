//! Resolution (C12): every `ComponentReference` in the project becomes concrete
//! values before the solvers run. What isn't selected stays `None`, and the
//! solvers fall back to `bike-defaults`.

use bike_model::shape::{combined_centroid, rotate_degrees};
use bike_model::{
    ComponentId, ComponentReference, Diagnostic, DriveUnitKind, MassProperties, OutlineAccuracy,
    Point, Project, ResolvedBattery, ResolvedComponents, ResolvedDriveUnit, ResolvedFork,
    ResolvedShock, Severity,
};

use crate::drawing;
use crate::library::Library;
use crate::validate;

/// Fork travel that differs from `front_travel` by more than this warns (C4).
const TRAVEL_TOLERANCE: f64 = 0.5;

/// Turns the project's component selections into concrete values.
pub fn resolve(project: &Project, library: &Library) -> (ResolvedComponents, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let mut resolved = ResolvedComponents::default();

    if let Some(reference) = &project.components.fork
        && let Some((fork, id)) = pick(
            reference,
            |id| library.forks.get(id),
            validate::fork,
            "fork",
            &mut diagnostics,
        )
    {
        let travel = project.front_travel.map_or(0.0, |t| t.0);
        if (fork.travel.0 - travel).abs() > TRAVEL_TOLERANCE {
            diagnostics.push(Diagnostic::new(
                Severity::Warning,
                "fork.travel_mismatch",
                format!(
                    "The {} has {:.0} mm travel but the front travel is {travel:.0} mm. Using the fork's {:.0} mm.",
                    fork.name, fork.travel.0, fork.travel.0
                ),
            ));
        }
        let centre_of_mass = centre_of_mass(&fork.mass, &drawing::fork(&fork, 0.0).shapes);
        resolved.fork = Some(ResolvedFork {
            fork,
            id,
            centre_of_mass,
        });
    }

    if let Some(reference) = &project.components.shock
        && let Some((shock, id)) = pick(
            reference,
            |id| library.shocks.get(id),
            validate::shock,
            "shock",
            &mut diagnostics,
        )
    {
        let centre_of_mass = centre_of_mass(&shock.mass, &drawing::shock(&shock, 0.0).shapes);
        resolved.shock = Some(ResolvedShock {
            shock,
            id,
            centre_of_mass,
        });
    }

    if let Some(placed) = &project.drive.unit
        && let Some((unit, id)) = pick(
            &placed.component,
            |id| library.drive_units.get(id),
            validate::drive_unit,
            "drive unit",
            &mut diagnostics,
        )
    {
        if unit.outline_accuracy == OutlineAccuracy::Approximate {
            diagnostics.push(Diagnostic::new(
                Severity::Info,
                "drive_unit.approximate",
                format!(
                    "The {} outline is approximate; check clearances against the maker's drawing.",
                    unit.name
                ),
            ));
        }
        let angle = placed.rotation.0;
        // The casing alone: the bolt circles would pull the centroid toward the mounts.
        let own_com = centre_of_mass(&unit.mass, std::slice::from_ref(&unit.outline));
        resolved.drive_unit = Some(ResolvedDriveUnit {
            outline: unit
                .outline
                .iter()
                .map(|p| rotate_degrees(*p, angle))
                .collect(),
            mounts: unit
                .mounts
                .iter()
                .map(|m| rotate_degrees(m.position, angle))
                .collect(),
            centre_of_mass: rotate_degrees(own_com, angle),
            rotation: placed.rotation,
            unit,
            id,
        });
    }

    if let Some(setup) = &project.drive.battery
        && let Some((battery, id)) = pick(
            &setup.component,
            |id| library.batteries.get(id),
            validate::battery,
            "battery",
            &mut diagnostics,
        )
    {
        let centre_of_mass = centre_of_mass(&battery.mass, &drawing::battery(&battery).shapes);
        resolved.battery = Some(ResolvedBattery {
            battery,
            id,
            placement: setup.clone(),
            centre_of_mass,
        });
    }

    let motor = resolved
        .drive_unit
        .as_ref()
        .is_some_and(|d| d.unit.kind != DriveUnitKind::Gearbox);
    if motor && resolved.battery.is_none() {
        diagnostics.push(Diagnostic::new(
            Severity::Info,
            "battery.none",
            "A motor is fitted but no battery is selected, so no battery mass is counted.",
        ));
    }

    (resolved, diagnostics)
}

/// The library entry or the custom component, or `None` with a diagnostic.
fn pick<'a, T: Clone + 'a>(
    reference: &'a ComponentReference<T>,
    lookup: impl Fn(&ComponentId) -> Option<&'a T>,
    check: fn(&T) -> Result<(), String>,
    kind: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(T, Option<ComponentId>)> {
    match reference {
        ComponentReference::Library(id) => match lookup(id) {
            Some(c) => Some((c.clone(), Some(id.clone()))),
            None => {
                diagnostics.push(Diagnostic::new(
                    Severity::Warning,
                    "component.unknown",
                    format!("The {kind} {id} isn't in the component library. Using the defaults."),
                ));
                None
            }
        },
        ComponentReference::Custom(c) => match check(c) {
            Ok(()) => Some((c.clone(), None)),
            Err(problems) => {
                diagnostics.push(Diagnostic::new(
                    Severity::Error,
                    "component.invalid",
                    format!("The custom {kind} can't be used: {problems}. Using the defaults."),
                ));
                None
            }
        },
    }
}

/// The given centre of mass, else the drawing's centroid (C10).
fn centre_of_mass(mass: &MassProperties, shapes: &[Vec<Point>]) -> Point {
    mass.centre_of_mass
        .or_else(|| combined_centroid(shapes))
        .unwrap_or_default()
}
