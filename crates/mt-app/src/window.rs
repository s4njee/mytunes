//! Preferences for the desktop library window.

use std::collections::HashSet;

use mt_db::queries::{TrackColumn, TrackSort};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::library::AppError;

const KEY: &str = "desktop.window";

/// Desktop theme preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum WindowTheme {
    /// Use the system's current appearance.
    #[default]
    System,
    /// Always use the light palette.
    Light,
    /// Always use the dark palette.
    Dark,
}

/// A user's override for a library column.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
pub struct ColumnPreference {
    /// Column whose layout is overridden.
    pub column: TrackColumn,
    /// Explicit width in logical pixels, when the user resized it.
    pub width: Option<u32>,
    /// Whether this column is shown.
    pub visible: bool,
}

/// Saved presentation of the desktop library.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default)]
pub struct WindowSettings {
    /// Preferred color scheme.
    pub theme: WindowTheme,
    /// Explicit sidebar width in logical pixels.
    pub sidebar_width: Option<u32>,
    /// Explicit height of the column browser in logical pixels.
    pub browser_height: Option<u32>,
    /// Whether to show the column browser.
    pub browser_visible: bool,
    /// Whether to show the sidebar artwork.
    pub artwork_visible: bool,
    /// Overrides to the default column layout.
    pub columns: Vec<ColumnPreference>,
    /// Sort to use when opening the library.
    pub sort: TrackSort,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            theme: WindowTheme::System,
            sidebar_width: None,
            browser_height: None,
            browser_visible: true,
            artwork_visible: true,
            columns: Vec::new(),
            sort: TrackSort::default(),
        }
    }
}

impl WindowSettings {
    fn normalized(mut self) -> Self {
        self.sidebar_width = self.sidebar_width.map(|size| size.clamp(150, 400));
        self.browser_height = self.browser_height.map(|size| size.clamp(96, 440));
        let mut seen = HashSet::new();
        self.columns.retain_mut(|column| {
            column.width = column.width.map(|width| width.clamp(36, 800));
            seen.insert(column.column)
        });
        self
    }
}

pub(crate) fn load(db: &Connection) -> Result<WindowSettings, AppError> {
    let stored: Option<String> = db
        .query_row("SELECT value FROM settings WHERE key = ?1", [KEY], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|_| AppError::Library)?;
    let settings: Option<WindowSettings> = stored
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| AppError::Library)?;
    Ok(settings.unwrap_or_default().normalized())
}

pub(crate) fn save(db: &Connection, settings: WindowSettings) -> Result<WindowSettings, AppError> {
    let settings = settings.normalized();
    let encoded = serde_json::to_string(&settings).map_err(|_| AppError::Library)?;
    db.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (KEY, encoded),
    )
    .map_err(|_| AppError::Library)?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::DesktopLibrary;
    use mt_db::queries::SortDirection;

    #[test]
    fn window_preferences_round_trip_when_the_library_reopens() {
        let dir = tempfile::tempdir().expect("create test directory");
        let path = dir.path().join("library.sqlite");
        let library = DesktopLibrary::open(&path).expect("open test library");
        assert_eq!(library.window_settings(), Ok(WindowSettings::default()));
        let saved = library
            .save_window_settings(WindowSettings {
                theme: WindowTheme::Dark,
                sidebar_width: Some(240),
                browser_height: Some(230),
                browser_visible: false,
                columns: vec![ColumnPreference {
                    column: TrackColumn::Genre,
                    width: Some(28),
                    visible: false,
                }],
                sort: TrackSort {
                    column: TrackColumn::Album,
                    direction: SortDirection::Descending,
                },
                ..WindowSettings::default()
            })
            .expect("save layout");
        drop(library);
        let reloaded = DesktopLibrary::open(&path).expect("reopen test library");
        assert_eq!(saved.columns[0].width, Some(36));
        assert_eq!(reloaded.window_settings(), Ok(saved));
    }
}
