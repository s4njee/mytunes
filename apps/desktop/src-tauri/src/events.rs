//! Typed updates delivered by the desktop app.

use std::sync::Mutex;

use mt_app::library::AppError;
use mt_audio::{PlayerEvent, PlayerState};
use mt_meta::scan::ScanProgress;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::ipc::Channel;
use tauri_specta::Event;

/// Notifies the UI to discard pages after the library changes.
#[derive(Clone, Debug, Deserialize, Serialize, Type, Event)]
pub struct LibraryChanged {
    /// The revision returned by the library after invalidating views.
    pub revision: u32,
}

/// Notifies the UI when a paired device becomes reachable.
#[derive(Clone, Debug, Deserialize, Serialize, Type, Event)]
pub struct DeviceOnline {
    /// Stable paired device identifier.
    pub id: String,
}

/// Transfer progress reported by the sync service.
#[derive(Clone, Debug, Serialize, Type)]
pub struct SyncProgress {
    /// The device being synchronized.
    pub device_id: String,
    /// Bytes delivered during the session.
    #[specta(type = specta_typescript::Number)]
    pub bytes_sent: u64,
    /// Bytes expected for the session.
    #[specta(type = specta_typescript::Number)]
    pub bytes_total: u64,
}

struct StreamState<T> {
    latest: Option<T>,
    channel: Option<Channel<T>>,
}

/// One stream to the main webview. A resubscribe replaces its previous channel.
pub struct DesktopStream<T>(Mutex<StreamState<T>>);

impl<T: Clone + Serialize> DesktopStream<T> {
    fn new(latest: Option<T>) -> Self {
        Self(Mutex::new(StreamState {
            latest,
            channel: None,
        }))
    }

    pub(crate) fn subscribe(&self, channel: Channel<T>) -> Result<(), AppError> {
        let mut state = self.0.lock().map_err(|_| AppError::Unavailable)?;
        if let Some(update) = &state.latest {
            channel
                .send(update.clone())
                .map_err(|_| AppError::Unavailable)?;
        }
        state.channel = Some(channel);
        Ok(())
    }

    /// Send a service update to the current webview and remember it for resubscription.
    ///
    /// # Errors
    /// Returns an error if the stream state is unavailable.
    pub fn publish(&self, update: T) -> Result<(), AppError> {
        let mut state = self.0.lock().map_err(|_| AppError::Unavailable)?;
        state.latest = Some(update.clone());
        if state
            .channel
            .as_ref()
            .is_some_and(|channel| channel.send(update).is_err())
        {
            state.channel = None;
        }
        Ok(())
    }
}

/// Channels shared by the desktop command layer and its service workers.
pub struct DesktopChannels {
    /// Transport snapshots and position updates.
    pub player: DesktopStream<PlayerEvent>,
    /// Progress for the active library scan.
    pub scan: DesktopStream<ScanProgress>,
    /// Progress for the active sync session.
    pub sync: DesktopStream<SyncProgress>,
}

impl Default for DesktopChannels {
    fn default() -> Self {
        Self {
            player: DesktopStream::new(Some(PlayerEvent {
                state: PlayerState::Empty,
                position_ms: 0,
                track_id: None,
                ended: false,
                error: None,
            })),
            scan: DesktopStream::new(None),
            sync: DesktopStream::new(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn resubscribe_replays_the_latest_update_and_replaces_the_previous_channel() {
        let streams = DesktopChannels::default();
        let (tx, rx) = mpsc::channel();
        streams
            .player
            .subscribe(Channel::new(move |body| {
                tx.send(body).expect("receive player message");
                Ok(())
            }))
            .expect("attach player channel");
        let state: serde_json::Value = rx
            .try_recv()
            .expect("initial state")
            .deserialize()
            .expect("deserialize player state");
        assert_eq!(state["state"], "empty");
        let event = PlayerEvent {
            state: PlayerState::Paused,
            position_ms: 420,
            track_id: None,
            ended: false,
            error: None,
        };
        streams
            .player
            .publish(event.clone())
            .expect("publish state");
        let state: serde_json::Value = rx
            .try_recv()
            .expect("published state")
            .deserialize()
            .expect("deserialize player state");
        assert_eq!(state["position_ms"], 420);

        let (tx, newer) = mpsc::channel();
        streams
            .player
            .subscribe(Channel::new(move |body| {
                tx.send(body).expect("receive player message");
                Ok(())
            }))
            .expect("replace player channel");
        assert!(newer.try_recv().is_ok());
        streams.player.publish(event).expect("publish state again");
        assert!(newer.try_recv().is_ok());
        assert!(rx.try_recv().is_err());
    }
}
