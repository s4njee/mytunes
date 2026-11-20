//! Background import and its persistent library preferences.

use std::path::{Path, PathBuf};

use mt_db::{FolderImport, ImportError, ManagedMedia, import_library_paths, open_desktop};
use mt_meta::scan::ScanProgress;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{library::AppError, settings};

const KEY: &str = "desktop.import";

/// Preferences shared by manual imports and watched folders.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default)]
pub struct ImportSettings {
    /// Copy newly imported files into MyTunes Media.
    pub copy_files: bool,
    /// Canonical folders to monitor recursively.
    pub watched_folders: Vec<PathBuf>,
}

/// The import worker's connection, separate from the webview's reading connection.
pub struct LibraryImporter {
    db: Connection,
    media: PathBuf,
    settings: ImportSettings,
}

impl LibraryImporter {
    /// Load import preferences and open a connection to the desktop library.
    ///
    /// # Errors
    /// Returns a library error if the connection or preferences cannot be read.
    pub fn open(db_path: &Path, media: PathBuf) -> Result<Self, AppError> {
        let db = open_desktop(db_path).map_err(|_| AppError::Library)?;
        let settings = settings::load(&db, KEY)?;
        Ok(Self {
            db,
            media,
            settings,
        })
    }

    /// Current persisted preferences.
    pub fn settings(&self) -> &ImportSettings {
        &self.settings
    }

    /// Persist new preferences before applying them to imports.
    ///
    /// # Errors
    /// Returns a library error if the preferences cannot be saved.
    pub fn save(&mut self, settings: ImportSettings) -> Result<(), AppError> {
        settings::save(&self.db, KEY, &settings)?;
        self.settings = settings;
        Ok(())
    }

    /// Import the selection using the current preferences.
    ///
    /// # Errors
    /// Returns an error if the scan cannot be committed to the library.
    pub fn scan(
        &mut self,
        paths: &[PathBuf],
        on_progress: impl FnMut(ScanProgress),
    ) -> Result<FolderImport, ImportError> {
        let media = ManagedMedia {
            folder: &self.media,
            copy_files: self.settings.copy_files,
        };
        import_library_paths(&mut self.db, paths, &media, on_progress)
    }
}
