//! Desktop playback session and listening statistics.

use mt_audio::PlayerEvent;
use mt_types::TrackId;
use serde::{Deserialize, Serialize};

/// The queue and transport state saved when the desktop application exits.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct PlaybackSession {
    /// Original order of the queue, including repeated entries.
    pub queue: Vec<TrackId>,
    /// Selected position in the original queue.
    pub index: u32,
    /// Position within the selected entry in milliseconds.
    pub position_ms: u64,
    /// Playback volume on a 0–100 scale.
    pub volume_percent: u8,
}

impl Default for PlaybackSession {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            index: 0,
            position_ms: 0,
            volume_percent: 65,
        }
    }
}

/// Counters to apply for one playback event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlaybackChange {
    /// Whether to count a completed play.
    pub play: bool,
    /// Whether to count an early manual skip.
    pub skip: bool,
}

/// Records each statistic at most once per pass through a queue entry.
#[derive(Default)]
pub struct PlaybackAccounting {
    pass: Option<u64>,
    counted_play: bool,
    counted_skip: bool,
}

impl PlaybackAccounting {
    /// Observe time actually delivered by the audio engine.
    pub fn observe(&mut self, event: &PlayerEvent, duration_ms: Option<u64>) -> PlaybackChange {
        if event.track_id.is_none() {
            return PlaybackChange::default();
        }
        if self.pass != Some(event.pass) {
            self.pass = Some(event.pass);
            self.counted_play = false;
            self.counted_skip = false;
        }
        let threshold = duration_ms
            .filter(|duration| *duration > 0)
            .map_or(240_000, |duration| duration.div_ceil(2).min(240_000));
        let play = !self.counted_play && event.played_ms >= threshold;
        let skip = !self.counted_skip
            && event.skipped
            && event.played_ms > 2_000
            && event.played_ms < 20_000;
        self.counted_play |= play;
        self.counted_skip |= skip;
        PlaybackChange { play, skip }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_counts_heard_time_once_and_skips_only_inside_the_early_window() {
        let mut accounting = PlaybackAccounting::default();
        let mut event = PlayerEvent {
            track_id: Some(TrackId("track".into())),
            pass: 1,
            position_ms: 90_000,
            played_ms: 9_999,
            ..PlayerEvent::default()
        };
        assert_eq!(
            accounting.observe(&event, Some(20_000)),
            PlaybackChange::default()
        );
        event.played_ms += 1;
        assert!(accounting.observe(&event, Some(20_000)).play);
        assert_eq!(
            accounting.observe(&event, Some(20_000)),
            PlaybackChange::default()
        );
        event.skipped = true;
        assert!(accounting.observe(&event, Some(20_000)).skip);
        assert!(!accounting.observe(&event, Some(20_000)).skip);

        for (pass, heard, skip) in [
            (2, 2_000, false),
            (3, 2_001, true),
            (4, 19_999, true),
            (5, 20_000, false),
        ] {
            event.pass = pass;
            event.played_ms = heard;
            assert_eq!(accounting.observe(&event, Some(25_000)).skip, skip);
        }
        event.pass = 6;
        event.skipped = false;
        event.played_ms = 239_999;
        assert!(!accounting.observe(&event, Some(600_000)).play);
        event.played_ms += 1;
        assert!(accounting.observe(&event, Some(600_000)).play);
    }
}
