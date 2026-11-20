//! Mobile controls for the shared playback engine.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, ensure};
use flutter_rust_bridge::frb;
use mt_audio::{
    Player, PlayerEvent, PlayerFlags, PlayerState, QueueItem, RepeatMode, ShuffleMode, SoundCheck,
    SoundEffect,
};
use mt_types::TrackId;

use super::device::{ALBUM_ARTIST, DeviceLibrary};
use crate::frb_generated::StreamSink;

/// Current transport state.
pub enum PlaybackState {
    Empty,
    Paused,
    Playing,
    Ended,
    Error,
}

/// A player snapshot, emitted on changes and during playback.
pub struct PlaybackEvent {
    pub state: PlaybackState,
    pub position_ms: u64,
    pub track_id: Option<String>,
    pub queue_index: Option<u32>,
    pub pass: u64,
    pub played_ms: u64,
    pub volume_percent: u8,
    pub ended: bool,
    pub skipped: bool,
    pub error: Option<String>,
}

impl From<PlayerEvent> for PlaybackEvent {
    fn from(event: PlayerEvent) -> Self {
        Self {
            state: match event.state {
                PlayerState::Empty => PlaybackState::Empty,
                PlayerState::Paused => PlaybackState::Paused,
                PlayerState::Playing => PlaybackState::Playing,
                PlayerState::Ended => PlaybackState::Ended,
                PlayerState::Error => PlaybackState::Error,
            },
            position_ms: event.position_ms,
            track_id: event.track_id.map(|id| id.0),
            queue_index: event.queue_index,
            pass: event.pass,
            played_ms: event.played_ms,
            volume_percent: event.volume_percent,
            ended: event.ended,
            skipped: event.skipped,
            error: event.error,
        }
    }
}

/// ReplayGain selection for Sound Check.
pub enum PlaybackSoundCheck {
    Off,
    Track,
    Album,
}

/// Order in which queued tracks play.
pub enum PlaybackShuffle {
    Off,
    Songs,
    Albums,
}

/// Action when a track or the queue reaches its end.
pub enum PlaybackRepeat {
    Off,
    One,
    All,
}

/// Owns one player subscription. Dart disposes it when its stream is cancelled.
#[frb(opaque)]
pub struct PlayerEventSubscription {
    receiver: Mutex<Option<Receiver<PlayerEvent>>>,
    cancelled: Arc<AtomicBool>,
}

impl PlayerEventSubscription {
    /// Forward snapshots on a separate thread until the subscription is disposed.
    #[frb(stream_dart_await)]
    pub fn stream(&self, sink: StreamSink<PlaybackEvent>) -> Result<()> {
        let events = self
            .receiver
            .lock()
            .map_err(|_| anyhow!("The player subscription was interrupted"))?
            .take()
            .context("The player subscription has already started")?;
        let cancelled = Arc::clone(&self.cancelled);
        thread::Builder::new()
            .name("mt-mobile-events".into())
            .spawn(move || {
                while !cancelled.load(Ordering::Relaxed) {
                    match events.recv_timeout(Duration::from_secs(1)) {
                        Ok(event) => {
                            if cancelled.load(Ordering::Relaxed) || sink.add(event.into()).is_err()
                            {
                                break;
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => continue,
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .context("Could not start the player event stream")?;
        Ok(())
    }
}

impl Drop for PlayerEventSubscription {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl DeviceLibrary {
    /// Replace the queue from library IDs and select the initial item, paused.
    pub fn load_queue(&self, track_ids: Vec<String>, start_index: u32) -> Result<()> {
        let index = start_index as usize;
        ensure!(
            index < track_ids.len() || (track_ids.is_empty() && index == 0),
            "The selected track is not in the playback queue"
        );
        let items = self.queue_items(&track_ids)?;
        self.with_player(|player| Ok(player.load_queue(items, index)?))
    }

    /// Resume the current track.
    pub fn play(&self) -> Result<()> {
        self.with_player(|player| Ok(player.play()?))
    }

    /// Pause the current track.
    pub fn pause(&self) -> Result<()> {
        self.with_player(|player| Ok(player.pause()?))
    }

    /// Toggle between playback and pause.
    pub fn toggle(&self) -> Result<()> {
        self.with_player(|player| Ok(player.toggle()?))
    }

    /// Seek within the current track, in milliseconds.
    pub fn seek(&self, position_ms: u64) -> Result<()> {
        self.with_player(|player| Ok(player.seek(position_ms)?))
    }

    /// Play the next queue item.
    pub fn next(&self) -> Result<()> {
        self.with_player(|player| Ok(player.next()?))
    }

    /// Restart the track after four seconds or go back one queue item.
    pub fn prev(&self) -> Result<()> {
        self.with_player(|player| Ok(player.prev()?))
    }

    /// Apply the five EQ gains in decibels, with the lowest band first.
    pub fn set_eq(&self, gains: Vec<i8>) -> Result<()> {
        let gains = gains
            .try_into()
            .map_err(|_| anyhow!("The equalizer requires five gains"))?;
        self.with_player(|player| Ok(player.set_eq(gains)?))
    }

    /// Set the linear volume from 0 to 1.
    pub fn set_volume(&self, volume: f32) -> Result<()> {
        self.with_player(|player| Ok(player.set_volume(volume)?))
    }

    /// Set gapless playback, Sound Check and the linear volume cap.
    pub fn set_flags(
        &self,
        gapless: bool,
        sound_check: PlaybackSoundCheck,
        volume_limit: f32,
    ) -> Result<()> {
        let sound_check = match sound_check {
            PlaybackSoundCheck::Off => SoundCheck::Off,
            PlaybackSoundCheck::Track => SoundCheck::Track,
            PlaybackSoundCheck::Album => SoundCheck::Album,
        };
        let flags = PlayerFlags {
            gapless,
            sound_check,
            volume_limit,
        };
        self.with_player(|player| Ok(player.set_flags(flags)?))
    }

    /// Set the active shuffle mode.
    pub fn set_shuffle(&self, mode: PlaybackShuffle) -> Result<()> {
        let mode = match mode {
            PlaybackShuffle::Off => ShuffleMode::Off,
            PlaybackShuffle::Songs => ShuffleMode::Songs,
            PlaybackShuffle::Albums => ShuffleMode::Albums,
        };
        self.with_player(|player| Ok(player.set_shuffle(mode)?))
    }

    /// Set the active repeat mode.
    pub fn set_repeat(&self, mode: PlaybackRepeat) -> Result<()> {
        let mode = match mode {
            PlaybackRepeat::Off => RepeatMode::Off,
            PlaybackRepeat::One => RepeatMode::One,
            PlaybackRepeat::All => RepeatMode::All,
        };
        self.with_player(|player| Ok(player.set_repeat(mode)?))
    }

    /// Play a click using the platform's UI audio channel.
    pub fn play_click(&self) -> Result<()> {
        self.with_player(|player| Ok(player.play_sfx(SoundEffect::Click)?))
    }

    /// Subscribe to transport changes, including an initial snapshot.
    pub fn subscribe_player(&self) -> Result<PlayerEventSubscription> {
        let receiver = self.with_player(|player| Ok(player.events()?))?;
        Ok(PlayerEventSubscription {
            receiver: Mutex::new(Some(receiver)),
            cancelled: Arc::new(AtomicBool::new(false)),
        })
    }

    fn queue_items(&self, ids: &[String]) -> Result<Vec<QueueItem>> {
        let db = self.connection()?;
        let mut query = db.prepare_cached(&format!(
            "SELECT t.id, t.rel_path,
                    CASE WHEN trim(t.album) <> '' THEN json_array(t.album, {ALBUM_ARTIST}, t.year, t.compilation) END
             FROM json_each(?1) selected JOIN tracks t ON t.id = selected.value ORDER BY selected.key"
        ))?;
        let items = query
            .query_map([serde_json::to_string(ids)?], |row| {
                Ok(QueueItem {
                    id: TrackId(row.get(0)?),
                    path: self.root.join(row.get::<_, String>(1)?),
                    album: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ensure!(
            items.len() == ids.len(),
            "A queued track is no longer in the device library"
        );
        Ok(items)
    }

    fn with_player<T>(&self, action: impl FnOnce(&Player) -> Result<T>) -> Result<T> {
        #[cfg(target_os = "android")]
        ensure!(
            crate::android::audio_ready(),
            "Android audio has not been initialized"
        );
        let mut guard = self
            .player
            .lock()
            .map_err(|_| anyhow!("The player was interrupted. Reopen the library to retry."))?;
        if guard.is_none() {
            *guard = Some(Player::new()?);
        }
        action(guard.as_ref().context("The player is unavailable")?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::device::open_device_db;

    #[test]
    fn a_queue_resolves_known_paths_and_preserves_repeated_positions() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let library = open_device_db(dir.path().join("device.sqlite").display().to_string())?;
        library.connection()?.execute_batch(
            "INSERT INTO tracks (id, rel_path, album, album_artist) VALUES
                ('a', 'music/a.wav', 'Sessions', 'Jane'),
                ('b', 'music/b.wav', 'Sessions', 'Les');",
        )?;
        let items = library.queue_items(&["b".into(), "a".into(), "b".into()])?;
        assert!(
            items
                .iter()
                .map(|item| item.id.0.as_str())
                .eq(["b", "a", "b"])
        );
        assert_eq!(
            items[0].path,
            dir.path().canonicalize()?.join("music/b.wav")
        );
        assert_ne!(items[0].album, items[1].album);
        assert_eq!(items[0].album, items[2].album);
        assert!(
            library
                .queue_items(&["a".into(), "missing".into()])
                .is_err()
        );
        Ok(())
    }
}
