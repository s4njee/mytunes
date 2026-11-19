//! Streaming local audio playback for MyTunes.

mod decode;
mod output;
mod render;

pub use decode::{AudioDecodeError, AudioDecoder, AudioSpec};
pub use output::{OutputError, play_file};
pub use render::{RenderError, RenderStream};
