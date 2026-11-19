//! Streaming local audio playback for MyTunes.

mod decode;
mod dsp;
mod mp4_timing;
mod output;
mod play_order;
mod player;
mod queue;
mod render;
mod sfx;

pub use decode::{AudioDecodeError, AudioDecoder, AudioSpec};
pub use dsp::{Dsp, DspError, DspSettings, EQ_FREQUENCIES_HZ, ReplayGain, SoundCheck};
pub use output::{OutputError, play_file, play_queue, play_queue_with_dsp};
pub use player::{
    Player, PlayerError, PlayerEvent, PlayerFlags, PlayerState, QueueItem, RepeatMode, ShuffleMode,
};
pub use queue::{AudioQueue, QueuedBlock};
pub use render::{RenderError, RenderStream};
pub use sfx::{SfxError, SoundEffect};
