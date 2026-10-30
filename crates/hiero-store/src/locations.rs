//! Which database the app uses, and the ones used recently. They're kept in a small JSON file
//! next to the app's settings, so the MCP server follows the database the app last opened.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::default_path;

/// How many recent databases are remembered.
pub const MAX_RECENT: usize = 8;

/// The database in use and the ones used before it, most recent first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Locations {
    /// Unset means the default database.
    pub current: Option<PathBuf>,
    pub recent: Vec<PathBuf>,
}

impl Locations {
    /// Where the locations are kept: the platform's config directory, such as
    /// `~/Library/Application Support/ArchHiero/locations.json` on macOS.
    pub fn file() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "ArchHiero")
            .map(|dirs| dirs.config_dir().join("locations.json"))
    }

    /// The saved locations. A missing or unreadable file gives none, so the default is used.
    pub fn load() -> Self {
        Self::file()
            .map(|f| Self::load_from(&f))
            .unwrap_or_default()
    }

    pub fn load_from(file: &Path) -> Self {
        std::fs::read_to_string(file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        match Self::file() {
            Some(file) => self.save_to(&file),
            None => Ok(()),
        }
    }

    pub fn save_to(&self, file: &Path) -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).expect("locations serialise");
        std::fs::write(file, text)
    }

    /// The database to open: the current one, or else the default.
    pub fn current_or_default(&self) -> Option<PathBuf> {
        self.current.clone().or_else(default_path)
    }

    /// Makes `path` the current database and puts it first in the recent ones.
    pub fn use_path(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_owned());
        self.recent.truncate(MAX_RECENT);
        self.current = (Some(path) != default_path().as_deref()).then(|| path.to_owned());
    }

    /// Takes `path` off the recent list. The current database stays current.
    pub fn forget(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_most_recent_first_without_repeats() {
        let mut locations = Locations::default();
        for name in ["a", "b", "a"] {
            locations.use_path(Path::new(name));
        }
        assert_eq!(locations.current.as_deref(), Some(Path::new("a")));
        assert_eq!(locations.recent, [PathBuf::from("a"), PathBuf::from("b")]);
        for i in 0..20 {
            locations.use_path(&PathBuf::from(i.to_string()));
        }
        assert_eq!(locations.recent.len(), MAX_RECENT);
        assert_eq!(locations.recent[0], PathBuf::from("19"));
        locations.forget(Path::new("19"));
        assert_eq!(locations.recent[0], PathBuf::from("18"));
        assert_eq!(locations.current.as_deref(), Some(Path::new("19")));
    }

    #[test]
    fn the_default_database_is_current_when_none_is_set() {
        let mut locations = Locations::default();
        locations.use_path(Path::new("elsewhere.sqlite3"));
        let default = default_path().unwrap();
        locations.use_path(&default);
        assert_eq!(locations.current, None);
        assert_eq!(locations.current_or_default(), Some(default.clone()));
        assert_eq!(locations.recent[0], default);
    }

    #[test]
    fn round_trips_through_its_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("nested").join("locations.json");
        assert_eq!(Locations::load_from(&file), Locations::default());
        let mut locations = Locations::default();
        locations.use_path(Path::new("/tmp/one.sqlite3"));
        locations.save_to(&file).unwrap();
        assert_eq!(Locations::load_from(&file), locations);
        std::fs::write(&file, "not json").unwrap();
        assert_eq!(Locations::load_from(&file), Locations::default());
    }
}
