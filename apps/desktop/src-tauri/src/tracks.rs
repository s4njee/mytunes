use std::sync::Arc;

use mt_app::{
    TrackRow,
    library::{AppError, DesktopLibrary, RemovalOutcome},
};
use mt_types::TrackId;
use tauri::{AppHandle, State, async_runtime};
use tauri_plugin_opener::OpenerExt;
use tauri_specta::Event;

use crate::events::LibraryChanged;

/// Get the current tags for a track, including a track outside the viewport.
#[tauri::command]
#[specta::specta]
pub async fn track_get(
    library: State<'_, Arc<DesktopLibrary>>,
    id: TrackId,
) -> Result<TrackRow, AppError> {
    let library = library.inner().clone();
    async_runtime::spawn_blocking(move || library.track(&id))
        .await
        .map_err(|_| AppError::Unavailable)?
}

/// Change whether the track is included in sync.
#[tauri::command]
#[specta::specta]
pub async fn track_set_checked(
    app: AppHandle,
    library: State<'_, Arc<DesktopLibrary>>,
    id: TrackId,
    checked: bool,
) -> Result<(), AppError> {
    let library = library.inner().clone();
    async_runtime::spawn_blocking(move || {
        let revision = library.set_checked(&id, checked)?;
        let _ = LibraryChanged { revision }.emit(&app);
        Ok(())
    })
    .await
    .map_err(|_| AppError::Unavailable)?
}

/// Reveal a known track in the platform file manager.
#[tauri::command]
#[specta::specta]
pub async fn track_reveal(
    app: AppHandle,
    library: State<'_, Arc<DesktopLibrary>>,
    id: TrackId,
) -> Result<(), AppError> {
    let library = library.inner().clone();
    async_runtime::spawn_blocking(move || {
        let path = library.file_to_reveal(&id)?;
        app.opener().reveal_item_in_dir(path).map_err(|error| {
            eprintln!("could not reveal track: {error}");
            AppError::Unavailable
        })
    })
    .await
    .map_err(|_| AppError::Unavailable)?
}

/// Remove selected positions from a current library view.
#[tauri::command]
#[specta::specta]
pub async fn tracks_remove(
    app: AppHandle,
    library: State<'_, Arc<DesktopLibrary>>,
    view_id: String,
    indexes: Vec<u32>,
    trash_files: bool,
) -> Result<RemovalOutcome, AppError> {
    let library = library.inner().clone();
    async_runtime::spawn_blocking(move || {
        let (revision, outcome) = library.remove_tracks(&view_id, &indexes, trash_files)?;
        let _ = LibraryChanged { revision }.emit(&app);
        Ok(outcome)
    })
    .await
    .map_err(|_| AppError::Unavailable)?
}
