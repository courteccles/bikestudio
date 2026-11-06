//! The open project: commands, the reducer, the solve pipeline and files (S-SHELL H2, H10).

use std::path::{Path, PathBuf};

use bike_components::{Library, resolve};
use bike_defaults::Defaults;
use bike_geometry::{solve_geometry_with, solve_tubes_with};
use bike_model::{
    CrankSetup, Diagnostic, ForkOverrides, GeometryVariable, LoadError, Millimeters, Parameter,
    Project, ResolvedComponents, Severity, Solved, TubeInputs, WheelSize,
};

/// Front or rear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum End {
    Front,
    Rear,
}

/// An edit from the interface (H2). The reducer applies it to the project.
/// `PickComponent` arrives with the Components tab; undo with S-LINK-EDITOR.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    SetWheel(End, Option<WheelSize>),
    /// `None` is rigid.
    SetTravel(End, Option<Millimeters>),
    PinGeometry(GeometryVariable, f64),
    FreeGeometry(GeometryVariable),
    SetFork(ForkOverrides),
    SetCrank(CrankSetup),
    SetTubes(TubeInputs),
}

/// The built-in tables every solve uses.
pub struct Data {
    pub defaults: Defaults,
    pub library: Library,
}

impl Data {
    pub fn builtin() -> Self {
        Self {
            defaults: Defaults::builtin().expect("the built-in defaults are tested"),
            library: Library::builtin().expect("the built-in library is tested"),
        }
    }
}

/// The open project and everything solved from it.
pub struct Document {
    pub project: Project,
    pub path: Option<PathBuf>,
    /// Unsaved changes.
    pub dirty: bool,
    /// Raised by every applied command; the solve runs when it changes.
    pub generation: u64,
    solved_generation: Option<u64>,
    pub solved: Solved,
    pub components: ResolvedComponents,
    /// Why the last command's value was held at a limit; shown with the
    /// solve's diagnostics until the next command.
    limit: Option<Diagnostic>,
}

impl Document {
    /// A new, empty project (D20).
    pub fn new() -> Self {
        Self::with_project(Project::default(), None)
    }

    pub fn with_project(project: Project, path: Option<PathBuf>) -> Self {
        Self {
            project,
            path,
            dirty: false,
            generation: 0,
            solved_generation: None,
            solved: Solved::default(),
            components: ResolvedComponents::default(),
            limit: None,
        }
    }

    /// The name shown in the top bar.
    pub fn name(&self) -> String {
        self.path
            .as_deref()
            .and_then(Path::file_stem)
            .map_or_else(|| "untitled".into(), |s| s.to_string_lossy().into_owned())
    }

    /// Applies a command (the reducer). A command that changes nothing leaves
    /// the project clean and the generation where it was.
    pub fn apply(&mut self, command: Command) {
        let command = self.hold_seat_angles(command);
        // An effective angle below the seat tube angle takes the seat tube
        // angle down with it (S-TUBES B6).
        let follow = match command {
            Command::PinGeometry(GeometryVariable::EffectiveSeatTubeAngle, value) => self
                .current(GeometryVariable::ActualSeatTubeAngle)
                .filter(|actual| *actual > value + 1e-9)
                .map(|_| value),
            _ => None,
        };
        let before = self.project.clone();
        let p = &mut self.project;
        match command {
            Command::SetWheel(End::Front, size) => p.front_wheel = size,
            Command::SetWheel(End::Rear, size) => p.rear_wheel = size,
            Command::SetTravel(End::Front, travel) => p.front_travel = travel,
            Command::SetTravel(End::Rear, travel) => p.rear_travel = travel,
            Command::PinGeometry(variable, value) => p.geometry.pin(variable, value),
            Command::FreeGeometry(variable) => p.geometry.free(variable),
            Command::SetFork(fork) => p.fork = fork,
            Command::SetCrank(crank) => p.crank = crank,
            Command::SetTubes(tubes) => p.tubes = tubes,
        }
        if let Some(value) = follow {
            p.geometry.pin(GeometryVariable::ActualSeatTubeAngle, value);
        }
        if self.project != before {
            self.dirty = true;
            self.generation += 1;
        }
    }

    /// A geometry value as the model has it: pinned, else last solved.
    fn current(&self, v: GeometryVariable) -> Option<f64> {
        self.project.geometry.pinned(v).or_else(|| {
            self.solved
                .geometry
                .as_ref()
                .map(|g| g.static_state.value(v))
        })
    }

    /// The limit a seat angle pin would break, and the rule, or `None` when
    /// it's allowed: the seat tube angle can't be steeper than the effective
    /// (S-TUBES B6). The effective angle has no limit: the seat tube angle
    /// follows it down (see `apply`).
    pub fn seat_angle_limit(
        &self,
        variable: GeometryVariable,
        value: f64,
    ) -> Option<(f64, &'static str)> {
        use GeometryVariable as V;
        match variable {
            V::ActualSeatTubeAngle => self
                .current(V::EffectiveSeatTubeAngle)
                .filter(|limit| value > *limit + 1e-9)
                .map(|limit| (limit, "cannot exceed the effective seat angle")),
            _ => None,
        }
    }

    /// Shows a blocked value's reason as an error diagnostic, until the next
    /// command (the panel blocks seat angles past their limit).
    pub fn flag_limit(&mut self, message: String) {
        self.flag("geometry.seat_angle_limit", message);
    }

    /// Shows why an edit was refused as an error diagnostic, until the next
    /// command.
    pub fn flag(&mut self, code: &'static str, message: String) {
        self.limit = Some(Diagnostic::new(Severity::Error, code, message));
        self.generation += 1;
    }

    /// "Invalid geometry" when `command` would break a model that solves now
    /// (no geometry or no tubes after it); `None` when it's fine.
    /// Clearing or freeing a value can do this when its default clashes
    /// with the other values.
    pub fn would_break(&self, command: &Command, data: &Data) -> Option<String> {
        let (had_geometry, had_tubes) =
            (self.solved.geometry.is_some(), self.solved.tubes.is_some());
        if !had_geometry && !had_tubes {
            return None;
        }
        let mut trial = Document::with_project(self.project.clone(), None);
        trial.solved = self.solved.clone();
        trial.apply(command.clone());
        trial.limit = None;
        trial.solve(data);
        let lost = (had_geometry && trial.solved.geometry.is_none())
            || (had_tubes && trial.solved.tubes.is_none());
        lost.then(|| "Invalid geometry".to_string())
    }

    /// A pin past a seat angle limit (the panel blocks these; this catches
    /// any other way in) is held at the limit, with an error saying so, so
    /// the model keeps solving.
    fn hold_seat_angles(&mut self, command: Command) -> Command {
        let had_limit = self.limit.take().is_some();
        let held = match command {
            Command::PinGeometry(v, value) => self
                .seat_angle_limit(v, value)
                .map(|(limit, rule)| (v, limit, rule)),
            _ => None,
        };
        let Some((variable, limit, rule)) = held else {
            if had_limit {
                // The error goes away even if nothing else changes.
                self.generation += 1;
            }
            return command;
        };
        self.limit = Some(Diagnostic::new(
            Severity::Error,
            "geometry.seat_angle_limit",
            format!(
                "The {} {rule} ({limit:.1}°), so it is held there.",
                variable.label().to_lowercase()
            ),
        ));
        // Re-solve so the error shows, even when the value was already at the limit.
        self.generation += 1;
        Command::PinGeometry(variable, limit)
    }

    /// Solves if anything changed since the last solve.
    pub fn solve(&mut self, data: &Data) {
        if self.solved_generation == Some(self.generation) {
            return;
        }
        let (components, mut diagnostics) = resolve(&self.project, &data.library);
        let (geometry, d) = solve_geometry_with(&self.project, &data.defaults, &components);
        diagnostics.extend(d);
        let (tubes, d) = solve_tubes_with(
            &self.project,
            geometry.as_ref(),
            &data.defaults,
            &components,
        );
        diagnostics.extend(d);
        diagnostics.extend(self.limit.clone());
        self.solved = Solved {
            geometry,
            tubes,
            diagnostics,
        };
        self.components = components;
        self.solved_generation = Some(self.generation);
    }

    /// True when nothing is pinned, so the view is a preview from defaults (H8).
    pub fn is_preview(&self) -> bool {
        !self
            .project
            .geometry
            .variables
            .values()
            .any(|p| matches!(p, Parameter::Pinned(_)))
    }

    pub fn open(path: &Path) -> Result<Self, LoadError> {
        Ok(Self::with_project(
            Project::load(path)?,
            Some(path.to_path_buf()),
        ))
    }

    /// Saves to the current path. Returns `false` when there is none (use `save_as`).
    pub fn save(&mut self) -> Result<bool, LoadError> {
        let Some(path) = self.path.clone() else {
            return Ok(false);
        };
        self.project.save(&path)?;
        self.dirty = false;
        Ok(true)
    }

    /// Saves to `path`, adding `.bikestud` if it has no extension (D25).
    pub fn save_as(&mut self, path: &Path) -> Result<(), LoadError> {
        let path = with_extension(path);
        self.project.save(&path)?;
        self.path = Some(path);
        self.dirty = false;
        Ok(())
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

/// The project file extension (D25).
pub const EXTENSION: &str = "bikestud";

fn with_extension(path: &Path) -> PathBuf {
    if path.extension().is_some() {
        path.to_path_buf()
    } else {
        path.with_extension(EXTENSION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bike_model::GeometryVariable as V;

    /// Freeing a value whose default would break the model is caught before
    /// it's applied.
    #[test]
    fn an_edit_that_would_break_the_model_is_caught() {
        let data = Data::builtin();
        let mut doc = Document::new();
        doc.apply(Command::PinGeometry(V::HeadTubeAngle, 64.0));
        doc.solve(&data);
        assert!(
            doc.would_break(&Command::PinGeometry(V::Reach, 470.0), &data)
                .is_none()
        );
        // A chainstay too short to reach the axle.
        let why = doc.would_break(&Command::PinGeometry(V::ChainstayLength, 10.0), &data);
        assert_eq!(why.as_deref(), Some("Invalid geometry"));
        // The document itself is untouched.
        assert_eq!(doc.project.geometry.pinned(V::ChainstayLength), None);
    }

    /// A blocked value's reason shows as an error diagnostic until the next
    /// command, and the model still solves.
    #[test]
    fn a_blocked_value_shows_as_a_diagnostic() {
        let data = Data::builtin();
        let mut doc = Document::new();
        doc.solve(&data);
        doc.flag_limit("Seat tube angle cannot exceed the effective seat angle (75.0°).".into());
        doc.solve(&data);
        let shown = |d: &Document| {
            d.solved
                .diagnostics
                .iter()
                .any(|d| d.code == "geometry.seat_angle_limit" && d.severity == Severity::Error)
        };
        assert!(shown(&doc) && doc.solved.geometry.is_some());
        assert!(!doc.dirty, "nothing in the project changed");
        doc.apply(Command::PinGeometry(V::Reach, 470.0));
        doc.solve(&data);
        assert!(!shown(&doc));
    }

    /// Past the limit, the seat angle is held there with an error, and the
    /// model still solves (S-TUBES B6).
    #[test]
    fn seat_angle_is_held_at_the_effective_angle() {
        let data = Data::builtin();
        let mut doc = Document::new();
        doc.apply(Command::PinGeometry(V::EffectiveSeatTubeAngle, 75.0));
        doc.apply(Command::PinGeometry(V::ActualSeatTubeAngle, 74.0));
        doc.solve(&data);
        doc.apply(Command::PinGeometry(V::ActualSeatTubeAngle, 75.1));
        doc.solve(&data);
        assert_eq!(
            doc.project.geometry.pinned(V::ActualSeatTubeAngle),
            Some(75.0)
        );
        assert!(doc.solved.geometry.is_some() && doc.solved.tubes.is_some());
        let limit = |d: &Document| {
            d.solved
                .diagnostics
                .iter()
                .any(|d| d.code == "geometry.seat_angle_limit")
        };
        assert!(limit(&doc));
        // Again at the limit: still shown. Then gone after the next command.
        doc.apply(Command::PinGeometry(V::ActualSeatTubeAngle, 75.2));
        doc.solve(&data);
        assert!(limit(&doc));
        doc.apply(Command::PinGeometry(V::ActualSeatTubeAngle, 74.0));
        doc.solve(&data);
        assert!(!limit(&doc));
        // The effective angle isn't limited: below the seat tube angle, the
        // seat tube angle follows it down, with no diagnostic.
        doc.apply(Command::PinGeometry(V::EffectiveSeatTubeAngle, 73.0));
        doc.solve(&data);
        let g = &doc.project.geometry;
        assert_eq!(g.pinned(V::EffectiveSeatTubeAngle), Some(73.0));
        assert_eq!(g.pinned(V::ActualSeatTubeAngle), Some(73.0));
        assert!(!limit(&doc) && doc.solved.geometry.is_some());
        assert!(
            doc.solved
                .diagnostics
                .iter()
                .all(|d| d.severity != Severity::Error)
        );
        // Above it, the seat tube angle stays.
        doc.apply(Command::PinGeometry(V::EffectiveSeatTubeAngle, 76.0));
        assert_eq!(
            doc.project.geometry.pinned(V::ActualSeatTubeAngle),
            Some(73.0)
        );
    }

    #[test]
    fn commands_mark_dirty_and_raise_the_generation() {
        let data = Data::builtin();
        let mut doc = Document::new();
        doc.solve(&data);
        assert!(doc.solved.geometry.is_some(), "an empty project draws");
        assert!(doc.is_preview() && !doc.dirty);

        doc.apply(Command::PinGeometry(V::HeadTubeAngle, 64.0));
        assert!(doc.dirty && doc.generation == 1 && !doc.is_preview());
        doc.solve(&data);
        let angle = &doc.solved.geometry.as_ref().unwrap().static_state;
        assert!((angle.value(V::HeadTubeAngle) - 64.0).abs() < 1e-9);

        // The same value again changes nothing.
        doc.apply(Command::PinGeometry(V::HeadTubeAngle, 64.0));
        assert_eq!(doc.generation, 1);

        doc.apply(Command::FreeGeometry(V::HeadTubeAngle));
        assert!(doc.is_preview());
    }

    #[test]
    fn travel_and_wheels_reach_the_solve() {
        let data = Data::builtin();
        let mut doc = Document::new();
        doc.apply(Command::SetTravel(End::Front, Some(Millimeters(150.0))));
        doc.apply(Command::SetWheel(End::Rear, Some(WheelSize("27.5".into()))));
        doc.solve(&data);
        let g = doc.solved.geometry.as_ref().unwrap();
        assert!(g.at_sag.is_some(), "a hardtail has a sag state");
        assert!(g.rear_wheel_diameter < g.front_wheel_diameter);
    }

    #[test]
    fn save_as_open_round_trips() {
        let dir = std::env::temp_dir().join(format!("bikestudio-doc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut doc = Document::new();
        assert_eq!(doc.name(), "untitled");
        doc.apply(Command::PinGeometry(V::Reach, 480.0));
        assert!(!doc.save().unwrap(), "no path yet");
        doc.save_as(&dir.join("enduro")).unwrap();
        assert!(!doc.dirty);
        assert_eq!(doc.name(), "enduro");
        let path = doc.path.clone().unwrap();
        assert_eq!(path.extension().unwrap(), EXTENSION);
        let opened = Document::open(&path).unwrap();
        assert_eq!(opened.project, doc.project);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_bad_file_is_an_error() {
        let path =
            std::env::temp_dir().join(format!("bikestudio-bad-{}.bikestud", std::process::id()));
        std::fs::write(&path, "{ not json").unwrap();
        assert!(Document::open(&path).is_err());
        std::fs::remove_file(&path).unwrap();
    }
}
