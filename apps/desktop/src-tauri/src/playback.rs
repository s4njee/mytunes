use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use mt_app::{
    library::{AppError, DesktopLibrary},
    playback::{PlaybackAccounting, PlaybackChange, PlaybackSession},
};
use mt_audio::{Player, PlayerError, PlayerEvent, QueueItem};
use mt_types::TrackId;
use serde::Deserialize;
use specta::Type;
use tauri::{AppHandle, Manager, State, async_runtime};
use tauri_specta::Event;

use crate::{
    events::{DesktopChannels, LibraryChanged},
    media::DesktopMedia,
};

/// Transport actions shared by the window and system media controls.
#[derive(Clone, Copy, Debug, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlayerAction {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
    Seek {
        #[specta(type = specta_typescript::Number)]
        position_ms: u64,
    },
    Volume {
        percent: u8,
    },
}

struct Runtime {
    player: Player,
    relay: JoinHandle<()>,
}

struct PlayerSession {
    saved: PlaybackSession,
    runtime: Option<Runtime>,
    closed: bool,
}

pub struct DesktopPlayer(Mutex<PlayerSession>);

impl DesktopPlayer {
    pub fn new(saved: PlaybackSession) -> Self {
        Self(Mutex::new(PlayerSession {
            saved,
            runtime: None,
            closed: false,
        }))
    }

    pub fn restore(&self, app: &AppHandle) -> Result<(), AppError> {
        let mut state = self.0.lock().map_err(|_| AppError::Unavailable)?;
        if state.closed || state.runtime.is_some() || state.saved.queue.is_empty() {
            return Ok(());
        }
        let library = app.state::<Arc<DesktopLibrary>>();
        let (queue, index) = library.restore_queue(&state.saved)?;
        let position = state.saved.position_ms;
        state.load(app, queue, index, position)
    }

    fn play_queue(
        &self,
        app: &AppHandle,
        queue: Vec<QueueItem>,
        index: usize,
    ) -> Result<(), AppError> {
        let mut state = self.0.lock().map_err(|_| AppError::Unavailable)?;
        state.load(app, queue, index, 0)?;
        state.player()?.play().map_err(playback_error)
    }

    pub fn control(&self, app: &AppHandle, action: PlayerAction) -> Result<(), AppError> {
        let mut state = self.0.lock().map_err(|_| AppError::Unavailable)?;
        if state.closed {
            return Err(AppError::Unavailable);
        }
        if let PlayerAction::Volume { percent } = action {
            let percent = percent.min(100);
            if let Some(runtime) = &state.runtime {
                runtime
                    .player
                    .set_volume(f32::from(percent) / 100.0)
                    .map_err(playback_error)?;
            } else {
                publish_event(
                    app,
                    PlayerEvent {
                        volume_percent: percent,
                        ..PlayerEvent::default()
                    },
                );
            }
            state.saved.volume_percent = percent;
            return Ok(());
        }
        let player = state.player()?;
        match action {
            PlayerAction::Play => player.play(),
            PlayerAction::Pause => player.pause(),
            PlayerAction::Toggle => player.toggle(),
            PlayerAction::Next => player.next(),
            PlayerAction::Previous => player.prev(),
            PlayerAction::Stop => player.pause().and_then(|()| player.seek(0)),
            PlayerAction::Seek { position_ms } => player.seek(position_ms),
            PlayerAction::Volume { .. } => Ok(()),
        }
        .map_err(playback_error)
    }

    pub fn shutdown(&self, library: &DesktopLibrary) -> Result<(), AppError> {
        let (saved, runtime) = {
            let mut state = self.0.lock().map_err(|_| AppError::Unavailable)?;
            state.closed = true;
            if let Some(runtime) = &state.runtime {
                let _ = runtime.player.pause();
                if let Ok(update) = runtime
                    .player
                    .events()
                    .and_then(|updates| updates.recv().map_err(|_| PlayerError::Closed))
                {
                    if let Some(index) = update.queue_index {
                        state.saved.index = index;
                    }
                    state.saved.position_ms = update.position_ms;
                    state.saved.volume_percent = update.volume_percent;
                }
            }
            (state.saved.clone(), state.runtime.take())
        };
        if let Some(Runtime { player, relay }) = runtime {
            drop(player);
            let _ = relay.join();
        }
        library.save_playback_session(&saved)
    }
}

impl PlayerSession {
    fn player(&self) -> Result<&Player, AppError> {
        self.runtime
            .as_ref()
            .map(|runtime| &runtime.player)
            .ok_or(AppError::TrackUnavailable)
    }

    fn load(
        &mut self,
        app: &AppHandle,
        queue: Vec<QueueItem>,
        index: usize,
        position: u64,
    ) -> Result<(), AppError> {
        if self.closed {
            return Err(AppError::Unavailable);
        }
        let index_u32 = u32::try_from(index).map_err(|_| AppError::TrackUnavailable)?;
        if self.runtime.is_none() {
            let player = Player::new().map_err(playback_error)?;
            player
                .set_volume(f32::from(self.saved.volume_percent.min(100)) / 100.0)
                .map_err(playback_error)?;
            let events = player.events().map_err(playback_error)?;
            let app = app.clone();
            let relay = std::thread::Builder::new()
                .name("desktop-player-events".into())
                .spawn(move || relay_events(&app, events))
                .map_err(|_| AppError::Unavailable)?;
            self.runtime = Some(Runtime { player, relay });
        }
        let ids = queue.iter().map(|item| item.id.clone()).collect();
        self.player()?
            .load_queue(queue, index)
            .map_err(playback_error)?;
        self.saved.queue = ids;
        self.saved.index = index_u32;
        self.saved.position_ms = 0;
        if position > 0 {
            self.player()?.seek(position).map_err(playback_error)?;
            self.saved.position_ms = position;
        }
        Ok(())
    }
}

fn publish_event(app: &AppHandle, event: PlayerEvent) {
    let _ = app.state::<DesktopChannels>().player.publish(event.clone());
    if let Some(media) = app.try_state::<DesktopMedia>() {
        media.publish(event);
    }
}

fn relay_events(app: &AppHandle, updates: std::sync::mpsc::Receiver<PlayerEvent>) {
    let library = app.state::<Arc<DesktopLibrary>>();
    let mut accounting = PlaybackAccounting::default();
    let mut pending: VecDeque<(TrackId, PlaybackChange)> = VecDeque::new();
    let mut track = None;
    let mut duration = None;
    for event in updates {
        publish_event(app, event.clone());
        if track != event.track_id {
            track.clone_from(&event.track_id);
            duration = track
                .as_ref()
                .and_then(|id| library.track(id).ok())
                .and_then(|row| u64::try_from(row.duration_ms?).ok());
        }
        if let Some(id) = &event.track_id {
            let change = accounting.observe(&event, duration);
            if change.play || change.skip {
                pending.push_back((id.clone(), change));
            }
        }
        while let Some((id, change)) = pending.front() {
            match library.record_playback(id, *change) {
                Ok(revision) => {
                    pending.pop_front();
                    if let Some(revision) = revision {
                        let _ = LibraryChanged { revision }.emit(app);
                    }
                }
                Err(error) => {
                    eprintln!("could not record playback: {error}");
                    break;
                }
            }
        }
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
        app.state::<DesktopPlayer>().play_queue(&app, queue, start)
    })
    .await
    .map_err(|_| AppError::Unavailable)?
}

/// Toggle play and pause in the current queue.
#[tauri::command]
#[specta::specta]
pub async fn player_toggle(app: AppHandle) -> Result<(), AppError> {
    player_control(app, PlayerAction::Toggle).await
}

/// Control the desktop transport.
#[tauri::command]
#[specta::specta]
pub async fn player_control(app: AppHandle, action: PlayerAction) -> Result<(), AppError> {
    async_runtime::spawn_blocking(move || app.state::<DesktopPlayer>().control(&app, action))
        .await
        .map_err(|_| AppError::Unavailable)?
}
