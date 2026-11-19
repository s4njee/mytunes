//! Folder scanning and metadata extraction for local audio files.

pub mod artwork;
mod metadata;
pub mod scan;
pub mod tag_write;

pub use metadata::{
    AudioCodec, AudioProperties, MetadataError, ReplayGain, TrackMetadata, read_metadata,
    read_metadata_from,
};
