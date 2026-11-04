//! App settings (S-SHELL H11), stored in the platform config directory and
//! never in a project: theme, projects folder, display unit and recent files.

use std::path::{Path, PathBuf};

use bike_model::LengthUnit;
use serde::{Deserialize, Serialize};

/// How many recent projects the Bikes menu lists (H10).
pub const RECENT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Dark,
    /// Comes in a later spec (H12).
    Drafting,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub projects_folder: PathBuf,
    /// S-UNITS U3, U13: switched in the status bar.
    pub display_unit: LengthUnit,
    /// Most recent first.
    pub recent: Vec<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            projects_folder: home().join("Bikes"),
            display_unit: LengthUnit::Millimeters,
            recent: Vec::new(),
        }
    }
}

impl Settings {
    /// Reads the settings file; anything missing or unreadable is the default.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self).expect("settings always serialize");
        std::fs::write(path, json + "\n")
    }

    /// Puts `path` first in the recent list.
    pub fn opened(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(RECENT);
    }
}

/// `settings.json` in the platform config directory.
pub fn default_path() -> PathBuf {
    config_dir().join("bikestudio").join("settings.json")
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from)
}

fn config_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        home().join("Library/Application Support")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map_or_else(|| home().join(".config"), PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_keeps_five_recent() {
        let path = std::env::temp_dir()
            .join(format!("bikestudio-settings-{}", std::process::id()))
            .join("settings.json");
        let mut s = Settings::default();
        for i in 0..7 {
            s.opened(Path::new(&format!("/tmp/{i}.bikestud")));
        }
        s.opened(Path::new("/tmp/3.bikestud"));
        assert_eq!(s.recent.len(), RECENT);
        assert_eq!(s.recent[0], Path::new("/tmp/3.bikestud"));
        s.display_unit = LengthUnit::Inches;
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_missing_or_bad_file_is_the_default() {
        assert_eq!(
            Settings::load(Path::new("/nonexistent/x.json")),
            Settings::default()
        );
    }
}
