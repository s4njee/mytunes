//! Library metadata extraction for local audio files.

mod metadata;

pub use metadata::{
    AudioCodec, AudioProperties, MetadataError, ReplayGain, TrackMetadata, read_metadata,
    read_metadata_from,
};
