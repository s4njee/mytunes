//! Streaming local audio playback for MyTunes.

mod decode;
mod dsp;
mod mp4_timing;
mod output;
mod queue;
mod render;

pub use decode::{AudioDecodeError, AudioDecoder, AudioSpec};
pub use dsp::{Dsp, DspError, DspSettings, EQ_FREQUENCIES_HZ, ReplayGain, SoundCheck};
pub use output::{OutputError, play_file, play_queue, play_queue_with_dsp};
pub use queue::{AudioQueue, QueuedBlock};
pub use render::{RenderError, RenderStream};
