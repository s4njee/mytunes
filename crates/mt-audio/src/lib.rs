//! Streaming local audio playback for MyTunes.

mod decode;
mod mp4_timing;
mod output;
mod queue;
mod render;

pub use decode::{AudioDecodeError, AudioDecoder, AudioSpec};
pub use output::{OutputError, play_file, play_queue};
pub use queue::{AudioQueue, QueuedBlock};
pub use render::{RenderError, RenderStream};
