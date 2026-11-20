use std::sync::{Arc, Mutex};

use mt_app::{
    TrackRow,
    library::{AppError, DesktopLibrary, RemovalOutcome},
};
use mt_audio::{Player, PlayerError};
use mt_types::TrackId;
use tauri::{AppHandle, Manager, State, async_runtime};
use tauri_plugin_opener::OpenerExt;
use tauri_specta::Event;

use crate::events::{DesktopChannels, LibraryChanged};

#[derive(Default)]
pub struct DesktopPlayer(Mutex<Option<Player>>);

impl DesktopPlayer {
    fn with_player(
        app: &AppHandle,
        action: impl FnOnce(&Player) -> Result<(), PlayerError>,
    ) -> Result<(), AppError> {
        let state = app.state::<Self>();
        let mut slot = state.0.lock().map_err(|_| AppError::Unavailable)?;
        if slot.is_none() {
            let player = Player::new().map_err(playback_error)?;
            let updates = player.events().map_err(playback_error)?;
            let handle = app.clone();
            std::thread::Builder::new()
                .name("desktop-player-events".into())
                .spawn(move || {
                    let channels = handle.state::<DesktopChannels>();
                    for event in updates {
                        let _ = channels.player.publish(event);
                    }
                })
                .map_err(|_| AppError::Unavailable)?;
            *slot = Some(player);
        }
        action(slot.as_ref().ok_or(AppError::Unavailable)?).map_err(playback_error)
    }
}

fn playback_error(error: PlayerError) -> AppError {
    eprintln!("desktop player: {error}");
    AppError::Playback
}

/// Start playing the selected track in the current view order.
#[tauri::command]
#[specta::specta]
pub async fn play_view(
    app: AppHandle,
    library: State<'_, Arc<DesktopLibrary>>,
    view_id: String,
    index: u32,
) -> Result<(), AppError> {
    let library = library.inner().clone();
    async_runtime::spawn_blocking(move || {
        let (queue, start) = library.playback_queue(&view_id, index)?;
        DesktopPlayer::with_player(&app, |player| {
            player.load_queue(queue, start)?;
            player.play()
        })
    })
    .await
    .map_err(|_| AppError::Unavailable)?
}

/// Toggle play and pause in the current queue.
#[tauri::command]
#[specta::specta]
pub async fn player_toggle(app: AppHandle) -> Result<(), AppError> {
    async_runtime::spawn_blocking(move || DesktopPlayer::with_player(&app, Player::toggle))
        .await
        .map_err(|_| AppError::Unavailable)?
}

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
