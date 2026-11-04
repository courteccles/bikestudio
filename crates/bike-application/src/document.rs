//! The open project: commands, the reducer, the solve pipeline and files (S-SHELL H2, H10).

use std::path::{Path, PathBuf};

use bike_components::{Library, resolve};
use bike_defaults::Defaults;
use bike_geometry::{solve_geometry_with, solve_tubes_with};
use bike_model::{
    CrankSetup, ForkOverrides, GeometryVariable, LoadError, Millimeters, Parameter, Project,
    ResolvedComponents, Solved, TubeInputs, WheelSize,
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
        if self.project != before {
            self.dirty = true;
            self.generation += 1;
        }
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
