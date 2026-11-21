use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use mt_types::TrackId;

use crate::output::{DeviceOutput, PlaybackOutput};
use crate::play_order::PlayOrder;
use crate::sfx::{ClickRequest, Sfx, SfxError, SoundEffect};
use crate::{AudioQueue, Dsp, DspError, DspSettings, OutputError, RenderError, SoundCheck};

const POSITION_INTERVAL: Duration = Duration::from_millis(250);
const ACTIVE_POLL: Duration = Duration::from_millis(2);
const OUTPUT_TIMEOUT: Duration = Duration::from_secs(5);

/// A track and its local playback path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueItem {
    /// Stable library identifier.
    pub id: TrackId,
    /// Local media file.
    pub path: PathBuf,
    /// Shared album identity for album shuffle, if known.
    pub album: Option<String>,
}

/// Queue ordering mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShuffleMode {
    /// Preserve the supplied queue order.
    #[default]
    Off,
    /// Randomize individual songs.
    Songs,
    /// Randomize albums while preserving each album's supplied track order.
    Albums,
}

/// Track completion behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RepeatMode {
    /// Stop after the last track.
    #[default]
    Off,
    /// Replay the current track until manually advanced.
    One,
    /// Continue at the first item after the last track.
    All,
}

/// Player switches and the master gain cap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerFlags {
    /// Join adjacent tracks without inserting a gap.
    pub gapless: bool,
    /// Select track gain or ordered album gain.
    pub sound_check: SoundCheck,
    /// Maximum linear master gain from 0 to 1.
    pub volume_limit: f32,
}

impl Default for PlayerFlags {
    fn default() -> Self {
        Self {
            gapless: true,
            sound_check: SoundCheck::Off,
            volume_limit: 1.0,
        }
    }
}

/// Transport state visible to either UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PlayerState {
    /// No queue has been loaded.
    Empty,
    /// Playback is paused.
    Paused,
    /// Audio is playing.
    Playing,
    /// The last track reached its end.
    Ended,
    /// An error interrupted playback.
    Error,
}

/// Snapshot emitted on changes and at 4 Hz during playback.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PlayerEvent {
    /// Playback state.
    pub state: PlayerState,
    /// Offset of the audible track in milliseconds.
    #[specta(type = specta_typescript::Number)]
    pub position_ms: u64,
    /// Identifier of the track described by this event.
    pub track_id: Option<TrackId>,
    /// Index in the original queue, including repeated track IDs.
    pub queue_index: Option<u32>,
    /// Identity of the current pass through a queue entry.
    #[specta(type = specta_typescript::Number)]
    pub pass: u64,
    /// Milliseconds delivered for this pass, excluding seeks and paused time.
    #[specta(type = specta_typescript::Number)]
    pub played_ms: u64,
    /// Current volume on a 0–100 scale.
    pub volume_percent: u8,
    /// This track reached its natural end.
    pub ended: bool,
    /// The user moved away from a track before playback reached its end.
    pub skipped: bool,
    /// A playback or command error, when applicable.
    pub error: Option<String>,
}

impl Default for PlayerEvent {
    fn default() -> Self {
        Self {
            state: PlayerState::Empty,
            position_ms: 0,
            track_id: None,
            queue_index: None,
            pass: 0,
            played_ms: 0,
            volume_percent: 100,
            ended: false,
            skipped: false,
            error: None,
        }
    }
}

/// Failure to start or control the playback worker.
#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    /// Output device error.
    #[error(transparent)]
    Output(#[from] OutputError),
    /// Media could not be rendered.
    #[error(transparent)]
    Render(#[from] RenderError),
    /// Invalid effects settings.
    #[error(transparent)]
    Dsp(#[from] DspError),
    /// The worker could not start.
    #[error("Could not start the player worker")]
    Spawn(#[source] std::io::Error),
    /// The worker shut down.
    #[error("The player is closed")]
    Closed,
    /// Playback has no selected track.
    #[error("The playback queue is empty")]
    Empty,
    /// The selected index is outside the queue.
    #[error("The selected track is not in the playback queue")]
    InvalidIndex,
}

enum Command {
    Load(Vec<QueueItem>, usize),
    Play,
    Pause,
    Toggle,
    Seek(u64),
    Next,
    Prev,
    Eq([i8; 5]),
    Volume(f32),
    Flags(PlayerFlags),
    Shuffle(ShuffleMode),
    Repeat(RepeatMode),
    Subscribe(Sender<PlayerEvent>),
    Shutdown,
}

type Request = (Command, Sender<Result<(), PlayerError>>);

/// Shared player with one output stream and a dedicated transport and feeder thread.
///
/// Command methods wait for acknowledgement from the feeder. Applications can
/// call them from their command dispatcher without entering the audio callback.
pub struct Player {
    requests: Sender<Request>,
    worker: Option<JoinHandle<()>>,
    sfx: Sfx,
}

impl Player {
    /// Open the default playback device and start the player worker.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker or output device cannot start.
    pub fn new() -> Result<Self, PlayerError> {
        let sfx = Sfx::new();
        let click = sfx.click_request();
        let (requests, rx) = mpsc::channel();
        let (ready, initialized) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("mt-audio-player".into())
            .spawn(move || match start_transport(click) {
                Ok(transport) => {
                    if ready.send(Ok(())).is_ok() {
                        transport.run(rx);
                    }
                }
                Err(error) => {
                    let _ = ready.send(Err(error));
                }
            })
            .map_err(PlayerError::Spawn)?;
        match initialized.recv() {
            Ok(Ok(())) => Ok(Self {
                requests,
                worker: Some(worker),
                sfx,
            }),
            result => {
                let _ = worker.join();
                match result {
                    Ok(Err(error)) => Err(error),
                    _ => Err(PlayerError::Closed),
                }
            }
        }
    }

    /// Replace the queue and select the initial item in a paused state.
    pub fn load_queue(&self, items: Vec<QueueItem>, start_index: usize) -> Result<(), PlayerError> {
        self.request(Command::Load(items, start_index))
    }

    /// Resume playback of the selected track.
    pub fn play(&self) -> Result<(), PlayerError> {
        self.request(Command::Play)
    }

    /// Pause music playback.
    pub fn pause(&self) -> Result<(), PlayerError> {
        self.request(Command::Pause)
    }

    /// Pause when playing and resume otherwise.
    pub fn toggle(&self) -> Result<(), PlayerError> {
        self.request(Command::Toggle)
    }

    /// Reposition the selected track.
    pub fn seek(&self, position_ms: u64) -> Result<(), PlayerError> {
        self.request(Command::Seek(position_ms))
    }

    /// Advance to the next item, wrapping at the end.
    pub fn next(&self) -> Result<(), PlayerError> {
        self.request(Command::Next)
    }

    /// Restart after four seconds, or select the previous item with wraparound.
    pub fn prev(&self) -> Result<(), PlayerError> {
        self.request(Command::Prev)
    }

    /// Set the five EQ bands in integer decibels.
    pub fn set_eq(&self, gains: [i8; 5]) -> Result<(), PlayerError> {
        self.request(Command::Eq(gains))
    }

    /// Set the linear master gain from 0 to 1.
    pub fn set_volume(&self, volume: f32) -> Result<(), PlayerError> {
        self.request(Command::Volume(volume))
    }

    /// Apply gapless, Sound Check, and volume limit settings.
    pub fn set_flags(&self, flags: PlayerFlags) -> Result<(), PlayerError> {
        self.request(Command::Flags(flags))
    }

    /// Choose how the queue is shuffled.
    pub fn set_shuffle(&self, shuffle: ShuffleMode) -> Result<(), PlayerError> {
        self.request(Command::Shuffle(shuffle))
    }

    /// Choose what happens after a track or the queue ends.
    pub fn set_repeat(&self, repeat: RepeatMode) -> Result<(), PlayerError> {
        self.request(Command::Repeat(repeat))
    }

    /// Request a UI sound immediately, including while music is paused.
    ///
    /// # Errors
    ///
    /// Returns an error if the platform cannot play the effect.
    pub fn play_sfx(&self, effect: SoundEffect) -> Result<(), SfxError> {
        self.sfx.play(effect)
    }

    /// Subscribe to player events, beginning with the current snapshot.
    pub fn events(&self) -> Result<Receiver<PlayerEvent>, PlayerError> {
        let (tx, events) = mpsc::channel();
        self.request(Command::Subscribe(tx))?;
        Ok(events)
    }

    fn request(&self, command: Command) -> Result<(), PlayerError> {
        let (ack, result) = mpsc::channel();
        self.requests
            .send((command, ack))
            .map_err(|_| PlayerError::Closed)?;
        result.recv().map_err(|_| PlayerError::Closed)?
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.request(Command::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Clone, Copy)]
struct Marker {
    item: usize,
    frame: u64,
    position_ms: u64,
}

struct Source {
    queue: AudioQueue,
    items: Vec<usize>,
    current_block: Option<usize>,
    done: bool,
    drain_until: Option<Instant>,
}

struct Transport<T> {
    output: T,
    state: PlayerState,
    order: PlayOrder,
    current: Option<Marker>,
    position_ms: u64,
    heard_before_seek_ms: u64,
    pass: u64,
    markers: VecDeque<Marker>,
    submitted: u64,
    source: Option<Source>,
    dsp: Dsp,
    settings: DspSettings,
    gapless: bool,
    shuffle: ShuffleMode,
    repeat: RepeatMode,
    samples: Vec<f32>,
    sample_offset: usize,
    next_marker: Option<usize>,
    subscribers: Vec<Sender<PlayerEvent>>,
    next_position: Instant,
    last_delivery: (u64, Instant),
    output_needs_reopen: bool,
}

fn start_transport(click: Option<ClickRequest>) -> Result<Transport<DeviceOutput>, PlayerError> {
    let device = DeviceOutput::with_click(click)?;
    device.start()?;
    Transport::new(device)
}

impl<T: PlaybackOutput> Transport<T> {
    fn new(output: T) -> Result<Self, PlayerError> {
        let settings = DspSettings::default();
        let dsp = Dsp::new(output.spec(), settings, Default::default())?;
        Ok(Self {
            output,
            state: PlayerState::Empty,
            order: PlayOrder::default(),
            current: None,
            position_ms: 0,
            heard_before_seek_ms: 0,
            pass: 0,
            markers: VecDeque::new(),
            submitted: 0,
            source: None,
            dsp,
            settings,
            gapless: true,
            shuffle: ShuffleMode::Off,
            repeat: RepeatMode::Off,
            samples: Vec::new(),
            sample_offset: 0,
            next_marker: None,
            subscribers: Vec::new(),
            next_position: Instant::now(),
            last_delivery: (0, Instant::now()),
            output_needs_reopen: false,
        })
    }

    fn run(mut self, requests: Receiver<Request>) {
        loop {
            let interval = if self.state == PlayerState::Playing {
                ACTIVE_POLL
            } else {
                POSITION_INTERVAL
            };
            match requests.recv_timeout(interval) {
                Ok((Command::Shutdown, ack)) => {
                    let _ = ack.send(Ok(()));
                    break;
                }
                Ok((command, ack)) => {
                    let result = self.command(command);
                    if let Err(error) = &result {
                        self.emit(false, Some(error.to_string()));
                    }
                    let _ = ack.send(result);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if let Err(error) = self.tick(Instant::now()) {
                if matches!(error, PlayerError::Output(OutputError::Stalled)) {
                    self.output_needs_reopen = true;
                }
                self.output.set_playing(false);
                self.state = PlayerState::Error;
                self.emit(false, Some(error.to_string()));
            }
        }
    }

    fn command(&mut self, command: Command) -> Result<(), PlayerError> {
        self.update_position();
        let needs_stream = match &command {
            Command::Toggle => self.state != PlayerState::Playing,
            Command::Load(..)
            | Command::Play
            | Command::Seek(_)
            | Command::Next
            | Command::Prev
            | Command::Flags(_)
            | Command::Shuffle(_)
            | Command::Repeat(_) => true,
            _ => false,
        };
        if needs_stream && (self.output_needs_reopen || self.output.failed()) {
            self.reopen_output()?;
        }
        match command {
            Command::Load(items, index) => self.load(items, index),
            Command::Play => self.play(),
            Command::Pause => {
                self.pause();
                Ok(())
            }
            Command::Toggle if self.state == PlayerState::Playing => {
                self.pause();
                Ok(())
            }
            Command::Toggle => self.play(),
            Command::Seek(ms) => self.move_to(self.selected()?, ms, true),
            Command::Next => self.navigate(true),
            Command::Prev => self.navigate(false),
            Command::Eq(gains) => self.effects(DspSettings {
                eq_db: gains,
                ..self.settings
            }),
            Command::Volume(volume) => {
                self.effects(DspSettings {
                    volume,
                    ..self.settings
                })?;
                self.emit(false, None);
                Ok(())
            }
            Command::Flags(flags) => self.flags(flags),
            Command::Shuffle(mode) => self.shuffle(mode),
            Command::Repeat(mode) => self.repeat(mode),
            Command::Subscribe(events) => {
                let _ = events.send(self.event(false, None));
                self.subscribers.push(events);
                Ok(())
            }
            Command::Shutdown => Ok(()),
        }
    }

    fn load(&mut self, items: Vec<QueueItem>, index: usize) -> Result<(), PlayerError> {
        if items.is_empty() && index == 0 {
            self.clear()?;
            self.order = PlayOrder::default();
            self.current = None;
            self.position_ms = 0;
            self.heard_before_seek_ms = 0;
            self.state = PlayerState::Empty;
            self.emit(false, None);
            return Ok(());
        }
        if index >= items.len() {
            return Err(PlayerError::InvalidIndex);
        }
        let order = PlayOrder::new(items, self.shuffle, index);
        let source = self.prepare(&order, index, 0)?;
        self.emit_skip();
        self.install(source, index, 0, PlayerState::Paused, false)?;
        self.order = order;
        self.emit(false, None);
        Ok(())
    }

    fn play(&mut self) -> Result<(), PlayerError> {
        let item = self.selected()?;
        if matches!(self.state, PlayerState::Ended | PlayerState::Error) {
            self.move_to(item, 0, false)?;
        } else if self.source.is_none() {
            let position = self.position_ms;
            let source = self.prepare(&self.order, item, position)?;
            self.install(source, item, position, PlayerState::Paused, true)?;
        }
        if self.state != PlayerState::Playing {
            self.fill()?;
            self.state = PlayerState::Playing;
            self.next_position = Instant::now() + POSITION_INTERVAL;
            self.last_delivery = (self.output.delivered(), Instant::now());
            self.output.set_playing(true);
            self.emit(false, None);
        }
        Ok(())
    }

    fn pause(&mut self) {
        if self.state == PlayerState::Playing {
            self.output.set_playing(false);
            self.update_position();
            self.state = PlayerState::Paused;
            self.emit(false, None);
        }
    }

    fn selected(&self) -> Result<usize, PlayerError> {
        self.current
            .map(|marker| marker.item)
            .ok_or(PlayerError::Empty)
    }

    fn navigate(&mut self, next: bool) -> Result<(), PlayerError> {
        let item = self.selected()?;
        if !next && self.position_ms > 4_000 {
            return self.move_to(item, 0, true);
        }
        let target = self.order.neighbor(item, next).ok_or(PlayerError::Empty)?;
        self.move_to(target, 0, false)
    }

    fn effects(&mut self, settings: DspSettings) -> Result<(), PlayerError> {
        self.dsp.set_settings(settings)?;
        self.settings = settings;
        Ok(())
    }

    fn shuffle(&mut self, mode: ShuffleMode) -> Result<(), PlayerError> {
        if self.shuffle != mode {
            if let Some(current) = self.current {
                let mut order = self.order.clone();
                order.shuffle(mode, current.item);
                let source = self.prepare(&order, current.item, self.position_ms)?;
                self.install(
                    source,
                    current.item,
                    self.position_ms,
                    self.motion_state(),
                    true,
                )?;
                self.order = order;
                self.emit(false, None);
            }
            self.shuffle = mode;
        }
        Ok(())
    }

    fn repeat(&mut self, mode: RepeatMode) -> Result<(), PlayerError> {
        if self.repeat != mode {
            if let Some(current) = self.current {
                let source = prepare(
                    &self.order,
                    current.item,
                    self.position_ms,
                    mode,
                    self.gapless,
                    self.output.spec(),
                )?;
                self.install(
                    source,
                    current.item,
                    self.position_ms,
                    self.motion_state(),
                    true,
                )?;
                self.emit(false, None);
            }
            self.repeat = mode;
        }
        Ok(())
    }

    fn flags(&mut self, flags: PlayerFlags) -> Result<(), PlayerError> {
        let settings = DspSettings {
            sound_check: flags.sound_check,
            volume_limit: flags.volume_limit,
            ..self.settings
        };
        crate::dsp::validate(settings)?;
        if self.gapless != flags.gapless
            && let Some(current) = self.current
        {
            let source = prepare(
                &self.order,
                current.item,
                self.position_ms,
                self.repeat,
                flags.gapless,
                self.output.spec(),
            )?;
            self.install(
                source,
                current.item,
                self.position_ms,
                self.motion_state(),
                true,
            )?;
            self.emit(false, None);
        }
        self.gapless = flags.gapless;
        self.effects(settings)
    }

    fn motion_state(&self) -> PlayerState {
        if self.state == PlayerState::Playing {
            PlayerState::Playing
        } else {
            PlayerState::Paused
        }
    }

    fn prepare(
        &self,
        order: &PlayOrder,
        item: usize,
        position_ms: u64,
    ) -> Result<Source, PlayerError> {
        prepare(
            order,
            item,
            position_ms,
            self.repeat,
            self.gapless,
            self.output.spec(),
        )
    }

    fn move_to(&mut self, item: usize, position_ms: u64, resume: bool) -> Result<(), PlayerError> {
        let source = self.prepare(&self.order, item, position_ms)?;
        if !resume {
            self.emit_skip();
        }
        self.install(source, item, position_ms, self.motion_state(), resume)?;
        self.emit(false, None);
        Ok(())
    }

    fn clear(&mut self) -> Result<(), PlayerError> {
        self.submitted = self.output.flush().inspect_err(|error| {
            if matches!(error, OutputError::Stalled) {
                self.output_needs_reopen = true;
            }
        })?;
        self.markers.clear();
        self.samples.clear();
        self.sample_offset = 0;
        self.next_marker = None;
        self.source = None;
        Ok(())
    }

    fn reopen_output(&mut self) -> Result<(), PlayerError> {
        self.output_needs_reopen = true;
        // A failed stream can outlive a route change. Rebuild from the heard
        // position and let the user's next transport command choose playback.
        self.pause();
        let heard = self.played_ms();
        self.output.reopen()?;
        self.dsp = Dsp::new(self.output.spec(), self.settings, Default::default())?;
        self.clear()?;
        if let Some(current) = &mut self.current {
            current.frame = self.submitted;
            current.position_ms = self.position_ms;
            self.heard_before_seek_ms = heard;
            if self.state == PlayerState::Error {
                self.state = PlayerState::Paused;
            }
        }
        self.output_needs_reopen = false;
        Ok(())
    }

    fn install(
        &mut self,
        source: Source,
        item: usize,
        position_ms: u64,
        state: PlayerState,
        resume: bool,
    ) -> Result<(), PlayerError> {
        let heard = if resume { self.played_ms() } else { 0 };
        self.clear()?;
        self.heard_before_seek_ms = heard;
        if !resume {
            self.pass = self.pass.saturating_add(1);
        }
        self.dsp.set_replay_gain(source.queue.initial_replay_gain());
        self.dsp.reset();
        self.current = Some(Marker {
            item,
            frame: self.submitted,
            position_ms,
        });
        self.position_ms = position_ms;
        self.source = Some(source);
        self.state = state;
        self.next_position = Instant::now() + POSITION_INTERVAL;
        self.last_delivery = (self.submitted, Instant::now());
        self.output.set_playing(state == PlayerState::Playing);
        Ok(())
    }

    fn tick(&mut self, now: Instant) -> Result<(), PlayerError> {
        if self.state != PlayerState::Playing {
            return Ok(());
        }
        if self.output.failed() {
            return Err(OutputError::Stalled.into());
        }
        self.update_position();
        self.fill()?;
        let delivered = self.output.delivered();
        if delivered != self.last_delivery.0 || delivered == self.submitted {
            self.last_delivery = (delivered, now);
        } else if now.saturating_duration_since(self.last_delivery.1) >= OUTPUT_TIMEOUT {
            return Err(OutputError::Stalled.into());
        }
        if let Some(source) = &mut self.source
            && source.done
            && self.output.delivered() >= self.submitted
        {
            let deadline = source.drain_until.get_or_insert(now + self.output.tail());
            if now >= *deadline {
                self.update_position();
                self.output.set_playing(false);
                self.state = PlayerState::Ended;
                self.emit(true, None);
                return Ok(());
            }
        }
        if now >= self.next_position {
            self.next_position = now + POSITION_INTERVAL;
            self.emit(false, None);
        }
        Ok(())
    }

    fn fill(&mut self) -> Result<(), PlayerError> {
        let channels = self.output.spec().channels;
        for _ in 0..32 {
            if self.sample_offset < self.samples.len() {
                let frames = self.output.push(&self.samples[self.sample_offset..]);
                if frames == 0 {
                    break;
                }
                if let Some(item) = self.next_marker.take() {
                    self.markers.push_back(Marker {
                        item,
                        frame: self.submitted,
                        position_ms: 0,
                    });
                }
                self.submitted += frames as u64;
                self.sample_offset += frames * channels;
                continue;
            }
            let Some(source) = &mut self.source else {
                break;
            };
            if source.done {
                break;
            }
            match source.queue.next_block()? {
                Some(block) => {
                    let item = source.items[block.track_index];
                    if source.current_block != Some(block.track_index) {
                        if source.current_block.is_some() || block.track_index != 0 {
                            self.next_marker = Some(item);
                        }
                        source.current_block = Some(block.track_index);
                    }
                    self.dsp.set_replay_gain(block.replay_gain);
                    self.samples.clear();
                    self.samples.extend_from_slice(block.samples);
                    self.sample_offset = 0;
                    self.dsp.process(&mut self.samples)?;
                }
                None => {
                    let next = match self.repeat {
                        RepeatMode::Off => None,
                        RepeatMode::One => source.items.first().copied(),
                        RepeatMode::All => self.order.first(),
                    };
                    let Some(item) = next else {
                        source.done = true;
                        break;
                    };
                    let mut new = self.prepare(&self.order, item, 0)?;
                    // This source starts at a repeat boundary, including a repeat of the same ID.
                    self.next_marker = Some(item);
                    new.current_block = None;
                    self.source = Some(new);
                }
            }
        }
        Ok(())
    }

    fn update_position(&mut self) {
        if self.state != PlayerState::Playing {
            return;
        }
        let delivered = self.output.delivered();
        while self
            .markers
            .front()
            .is_some_and(|marker| marker.frame <= delivered)
        {
            if let Some(marker) = self.markers.pop_front() {
                self.position_at(marker.frame);
                self.emit(true, None);
                self.current = Some(marker);
                self.heard_before_seek_ms = 0;
                self.pass = self.pass.saturating_add(1);
                let next = self
                    .markers
                    .front()
                    .map_or(delivered, |next| delivered.min(next.frame));
                self.position_at(next);
                self.emit(false, None);
            }
        }
        self.position_at(delivered);
    }

    fn position_at(&mut self, frame: u64) {
        if let Some(current) = self.current {
            self.position_ms = current.position_ms.saturating_add(
                frame.saturating_sub(current.frame).saturating_mul(1_000)
                    / u64::from(self.output.spec().sample_rate),
            );
        }
    }

    fn event(&self, ended: bool, error: Option<String>) -> PlayerEvent {
        PlayerEvent {
            state: self.state,
            position_ms: self.position_ms,
            track_id: self
                .current
                .and_then(|marker| self.order.items.get(marker.item))
                .map(|item| item.id.clone()),
            queue_index: self
                .current
                .and_then(|marker| u32::try_from(marker.item).ok()),
            pass: self.pass,
            played_ms: self.played_ms(),
            volume_percent: (self.settings.volume * 100.0).round() as u8,
            ended,
            skipped: false,
            error,
        }
    }

    fn emit(&mut self, ended: bool, error: Option<String>) {
        self.publish(self.event(ended, error));
    }

    fn played_ms(&self) -> u64 {
        self.current.map_or(0, |marker| {
            self.heard_before_seek_ms
                .saturating_add(self.position_ms.saturating_sub(marker.position_ms))
        })
    }

    fn emit_skip(&mut self) {
        if matches!(self.state, PlayerState::Playing | PlayerState::Paused) && self.played_ms() > 0
        {
            let mut event = self.event(false, None);
            event.skipped = true;
            self.publish(event);
        }
    }

    fn publish(&mut self, event: PlayerEvent) {
        self.subscribers
            .retain(|events| events.send(event.clone()).is_ok());
    }
}

fn prepare(
    order: &PlayOrder,
    item: usize,
    position_ms: u64,
    repeat: RepeatMode,
    gapless: bool,
    spec: crate::AudioSpec,
) -> Result<Source, PlayerError> {
    let items = order.from(item, repeat).ok_or(PlayerError::InvalidIndex)?;
    let paths: Vec<_> = items
        .iter()
        .map(|&index| order.items[index].path.clone())
        .collect();
    Ok(Source {
        queue: AudioQueue::open(&paths, spec, position_ms, gapless)?,
        items,
        current_block: None,
        done: false,
        drain_until: None,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use gen_fixtures::{Pcm, wav_bytes};

    use super::*;
    use crate::AudioSpec;

    #[derive(Default)]
    struct TestOutput {
        queued: usize,
        delivered: u64,
        playing: Cell<bool>,
        failed: bool,
        reopened: bool,
    }

    impl TestOutput {
        fn consume(&mut self, frames: usize) {
            if self.playing.get() {
                let consumed = frames.min(self.queued);
                self.queued -= consumed;
                self.delivered += consumed as u64;
            }
        }
    }

    impl PlaybackOutput for TestOutput {
        fn spec(&self) -> AudioSpec {
            AudioSpec {
                sample_rate: if self.reopened { 44_100 } else { 48_000 },
                channels: 1,
            }
        }

        fn push(&mut self, samples: &[f32]) -> usize {
            let pushed = samples.len().min(12_000 - self.queued);
            self.queued += pushed;
            pushed
        }

        fn delivered(&self) -> u64 {
            self.delivered
        }

        fn tail(&self) -> Duration {
            Duration::ZERO
        }

        fn failed(&self) -> bool {
            self.failed
        }

        fn set_playing(&self, playing: bool) {
            self.playing.set(playing);
        }

        fn flush(&mut self) -> Result<u64, OutputError> {
            if self.failed {
                return Err(OutputError::Stalled);
            }
            self.queued = 0;
            self.set_playing(false);
            Ok(self.delivered)
        }

        fn reopen(&mut self) -> Result<(), OutputError> {
            *self = Self {
                reopened: true,
                ..Self::default()
            };
            Ok(())
        }
    }

    fn tracks(folder: &std::path::Path, frames: usize) -> Vec<QueueItem> {
        ["one", "two"]
            .map(|id| {
                let path = folder.join(format!("{id}.wav"));
                let wave = wav_bytes(&Pcm {
                    sample_rate: 48_000,
                    channels: 1,
                    bits: 16,
                    samples: vec![0.2; frames],
                });
                std::fs::write(&path, wave).expect("write playback fixture");
                QueueItem {
                    id: TrackId(id.into()),
                    path,
                    album: Some("album".into()),
                }
            })
            .to_vec()
    }

    #[test]
    fn a_seek_preserves_audible_time_without_counting_the_position_jump() {
        let directory = tempfile::tempdir().expect("create playback fixture directory");
        let mut transport = Transport::new(TestOutput::default()).expect("open test player");
        transport
            .command(Command::Load(tracks(directory.path(), 480_000), 0))
            .expect("load queue");
        transport.command(Command::Play).expect("start track");
        transport.output.consume(4_800);
        transport.update_position();
        let before = transport.event(false, None);
        assert_eq!(before.played_ms, 100);

        transport
            .command(Command::Seek(4_000))
            .expect("seek in track");
        let after = transport.event(false, None);
        assert_eq!(after.position_ms, 4_000);
        assert_eq!(after.played_ms, 100);
        assert_eq!(after.pass, before.pass);
        transport.fill().expect("fill test output");
        transport.output.consume(4_800);
        transport.update_position();
        assert_eq!(transport.event(false, None).played_ms, 200);
    }

    fn current(player: &Transport<TestOutput>) -> (&str, u64) {
        let item = player.selected().expect("loaded test track");
        (&player.order.items[item].id.0, player.position_ms)
    }

    #[test]
    fn play_after_route_loss_reopens_output_and_preserves_progress() {
        let directory = tempfile::tempdir().expect("create playback fixture directory");
        let mut player = Transport::new(TestOutput::default()).expect("open test player");
        player
            .command(Command::Load(tracks(directory.path(), 48_000), 1))
            .expect("load queue");
        player.command(Command::Play).expect("start second track");
        player.output.consume(4_800);
        player.output.failed = true;
        player
            .command(Command::Pause)
            .expect("pause when route is lost");
        let paused = player.event(false, None);
        assert_eq!(paused.state, PlayerState::Paused);
        assert_eq!(paused.position_ms, 100);
        assert!(!player.output.reopened);

        player
            .command(Command::Play)
            .expect("resume on the new route");
        assert!(player.output.reopened);
        assert_eq!(player.state, PlayerState::Playing);
        assert_eq!(current(&player), ("two", 100));
        player.output.consume(4_410);
        player.update_position();
        let resumed = player.event(false, None);
        assert_eq!(resumed.position_ms, 200);
        assert_eq!(resumed.played_ms, 200);
        assert_eq!(resumed.pass, paused.pass);
    }

    #[test]
    fn prev_restarts_only_past_four_seconds_and_manual_navigation_wraps() {
        let dir = tempfile::tempdir().unwrap();
        let mut player = Transport::new(TestOutput::default()).unwrap();
        player
            .command(Command::Load(tracks(dir.path(), 288_000), 0))
            .unwrap();

        player.command(Command::Seek(4_001)).unwrap();
        player.command(Command::Prev).unwrap();
        assert_eq!(current(&player), ("one", 0));

        player.command(Command::Seek(4_000)).unwrap();
        player.command(Command::Prev).unwrap();
        assert_eq!(current(&player), ("two", 0));

        player.command(Command::Next).unwrap();
        assert_eq!(current(&player), ("one", 0));
        assert_eq!(player.state, PlayerState::Paused);
    }

    #[test]
    fn states_emit_immediately_and_positions_follow_output_at_four_hz() {
        let dir = tempfile::tempdir().unwrap();
        let mut player = Transport::new(TestOutput::default()).unwrap();
        let (tx, events) = mpsc::channel();
        player.command(Command::Subscribe(tx)).unwrap();
        assert_eq!(events.try_recv().unwrap().state, PlayerState::Empty);
        player
            .command(Command::Load(tracks(dir.path(), 48_000), 0))
            .unwrap();
        assert_eq!(events.try_recv().unwrap().state, PlayerState::Paused);
        player.command(Command::Play).unwrap();
        assert_eq!(events.try_recv().unwrap().state, PlayerState::Playing);

        let first_tick = player.next_position;
        assert!(player.submitted >= 12_000);
        player.tick(first_tick - Duration::from_millis(1)).unwrap();
        assert!(events.try_recv().is_err());
        player.output.consume(12_000);
        player.tick(first_tick).unwrap();
        let position = events.try_recv().unwrap();
        assert_eq!(position.track_id, Some(TrackId("one".into())));
        assert_eq!(position.position_ms, 250);
        assert_eq!(
            player.next_position - first_tick,
            Duration::from_millis(250)
        );

        player.command(Command::Toggle).unwrap();
        assert_eq!(events.try_recv().unwrap().state, PlayerState::Paused);
        player.output.consume(12_000);
        player
            .tick(first_tick + Duration::from_millis(250))
            .unwrap();
        assert_eq!(current(&player), ("one", 250));
        assert!(events.try_recv().is_err());
        player.command(Command::Toggle).unwrap();
        assert_eq!(events.try_recv().unwrap().state, PlayerState::Playing);
    }

    #[test]
    fn queue_advances_when_the_boundary_is_consumed_and_then_ends() {
        let dir = tempfile::tempdir().unwrap();
        let mut player = Transport::new(TestOutput::default()).unwrap();
        let (tx, events) = mpsc::channel();
        player.command(Command::Subscribe(tx)).unwrap();
        player
            .command(Command::Load(tracks(dir.path(), 4_800), 0))
            .unwrap();
        player.command(Command::Play).unwrap();
        assert_eq!(events.try_iter().count(), 3);
        assert_eq!(player.submitted, 9_600);
        assert_eq!(current(&player), ("one", 0));
        player.output.consume(4_800);
        player.tick(Instant::now()).unwrap();
        let ended = events.try_recv().unwrap();
        assert!(ended.ended);
        assert_eq!(ended.track_id, Some(TrackId("one".into())));
        assert_eq!(ended.position_ms, 100);
        let advanced = events.try_recv().unwrap();
        assert_eq!(advanced.track_id, Some(TrackId("two".into())));
        assert_eq!(advanced.position_ms, 0);

        player.output.consume(4_800);
        player.tick(Instant::now()).unwrap();
        let end = events.try_recv().unwrap();
        assert_eq!(end.state, PlayerState::Ended);
        assert!(end.ended);
        assert_eq!(end.position_ms, 100);
        assert_eq!(end.track_id, Some(TrackId("two".into())));
        assert_eq!(player.submitted, 9_600);
    }

    #[test]
    fn repeat_one_replays_current_and_repeat_all_wraps_after_the_last() {
        let dir = tempfile::tempdir().unwrap();
        for (repeat, next) in [(RepeatMode::One, "two"), (RepeatMode::All, "one")] {
            let mut player = Transport::new(TestOutput::default()).unwrap();
            let (tx, events) = mpsc::channel();
            player.command(Command::Subscribe(tx)).unwrap();
            player.command(Command::Repeat(repeat)).unwrap();
            player
                .command(Command::Load(tracks(dir.path(), 4_800), 1))
                .unwrap();
            player.command(Command::Play).unwrap();
            assert_eq!(events.try_iter().count(), 3);
            player.output.consume(4_800);
            player.tick(Instant::now()).unwrap();
            assert_eq!(
                events.try_recv().unwrap().track_id,
                Some(TrackId("two".into()))
            );
            let repeated = events.try_recv().unwrap();
            assert_eq!(repeated.track_id, Some(TrackId(next.into())));
            assert_eq!(repeated.state, PlayerState::Playing);
            assert_eq!(repeated.position_ms, 0);
        }
    }
}
