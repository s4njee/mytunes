use std::fs::File;
use std::io::{self, BufReader, Read, Seek};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use lofty::config::{ParseOptions, ParsingMode};
use lofty::error::FileParseError;
use lofty::file::{AudioFile, FileType, TaggedFile, TaggedFileExt};
use lofty::iff::aiff::{AiffCompressionType, AiffFile};
use lofty::iff::wav::{WavFile, WavFormat};
use lofty::mp4::{Mp4Codec, Mp4File};
use lofty::mpeg::{Layer, MpegFile};
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, Tag};

/// Audio formats recognized by the metadata reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
    /// MPEG audio layer III.
    Mp3,
    /// Advanced Audio Coding.
    Aac,
    /// Apple Lossless.
    Alac,
    /// Free Lossless Audio Codec.
    Flac,
    /// Integer or floating-point PCM in a WAVE container.
    Wav,
    /// Integer or floating-point PCM in an AIFF container.
    Aiff,
    /// Vorbis in an Ogg container.
    Vorbis,
    /// Opus in an Ogg container.
    Opus,
}

impl AudioCodec {
    /// Stable lowercase identifier for library records.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Aac => "aac",
            Self::Alac => "alac",
            Self::Flac => "flac",
            Self::Wav => "wav",
            Self::Aiff => "aiff",
            Self::Vorbis => "vorbis",
            Self::Opus => "opus",
        }
    }
}

/// Audio stream properties read from container and frame headers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioProperties {
    /// Audio codec or PCM container.
    pub codec: AudioCodec,
    /// Reported playing time in milliseconds.
    pub duration_ms: u64,
    /// Sample rate in hertz. Opus reports its playback rate of 48 kHz.
    pub sample_rate_hz: u32,
    /// Encoded sample bit depth for PCM and lossless audio, when reported.
    pub bit_depth: Option<u8>,
    /// Audio bitrate in kilobits per second, when reported.
    pub bitrate_kbps: Option<u32>,
    /// Number of audio channels.
    pub channels: u8,
}

/// ReplayGain values carried in the file's tags.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReplayGain {
    /// Track gain in decibels.
    pub track_gain_db: Option<f64>,
    /// Album gain in decibels.
    pub album_gain_db: Option<f64>,
    /// Track peak relative to full scale.
    pub track_peak: Option<f64>,
    /// Album peak relative to full scale.
    pub album_peak: Option<f64>,
}

/// Text tags and audio properties for a single file.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackMetadata {
    /// Tagged title, or the filename stem when the tag is absent or blank.
    pub title: String,
    /// Track artist.
    pub artist: Option<String>,
    /// Album artist.
    pub album_artist: Option<String>,
    /// Album title.
    pub album: Option<String>,
    /// Composer.
    pub composer: Option<String>,
    /// Genre.
    pub genre: Option<String>,
    /// Tagged year.
    pub year: Option<u16>,
    /// Track number within the disc.
    pub track_number: Option<u32>,
    /// Track count for the disc.
    pub track_total: Option<u32>,
    /// Disc number within the album.
    pub disc_number: Option<u32>,
    /// Disc count for the album.
    pub disc_total: Option<u32>,
    /// Compilation flag, if explicitly tagged.
    pub compilation: Option<bool>,
    /// Alternate sort title.
    pub sort_title: Option<String>,
    /// Alternate sort artist.
    pub sort_artist: Option<String>,
    /// Alternate sort album.
    pub sort_album: Option<String>,
    /// Alternate sort album artist.
    pub sort_album_artist: Option<String>,
    /// Tagged volume normalization values.
    pub replay_gain: ReplayGain,
    /// Properties of the audio stream.
    pub audio: AudioProperties,
}

/// Failure to extract metadata from an audio file.
#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    /// Opening or probing the input failed.
    #[error("could not read audio file")]
    Io(#[from] io::Error),
    /// The container or codec is outside the supported formats.
    #[error("unsupported audio format")]
    UnsupportedFormat,
    /// The file requires DRM authorization for playback.
    #[error("DRM-protected audio cannot be imported")]
    ProtectedAudio,
    /// The metadata parser rejected the input.
    #[error("could not parse audio file")]
    InvalidAudio(#[from] FileParseError),
    /// The dependency unwound while parsing malformed metadata.
    #[error("audio metadata parser failed")]
    ParserFailure,
    /// The stream headers did not yield usable properties.
    #[error("missing or invalid audio stream properties")]
    InvalidProperties,
}

/// Read tags and audio properties from a local file.
///
/// The reader checks the file headers without decoding the audio or loading artwork.
///
/// # Errors
///
/// Returns a typed error if the file cannot be read, is protected, is unsupported,
/// or has invalid metadata or stream headers.
pub fn read_metadata(path: impl AsRef<Path>) -> Result<TrackMetadata, MetadataError> {
    let path = path.as_ref();
    read_metadata_from(BufReader::new(File::open(path)?), path)
}

/// Read metadata from a seekable stream starting at its beginning.
///
/// The filename supplies the title fallback and the expected format when probing
/// cannot identify the input.
///
/// # Errors
///
/// Returns the same errors as [`read_metadata`].
pub fn read_metadata_from<R: Read + Seek>(
    mut reader: R,
    filename: &Path,
) -> Result<TrackMetadata, MetadataError> {
    reader.rewind()?;

    // Lofty can unwind on corrupt headers. The parser owns the reader and
    // produces no library state until this call succeeds.
    catch_unwind(AssertUnwindSafe(|| parse_metadata(reader, filename)))
        .map_err(|_| MetadataError::ParserFailure)?
}

fn parse_metadata<R: Read + Seek>(
    reader: R,
    filename: &Path,
) -> Result<TrackMetadata, MetadataError> {
    let options = ParseOptions::new()
        .read_cover_art(false)
        .parsing_mode(ParsingMode::Strict);
    let probe = Probe::new(reader).options(options).guess_file_type()?;
    let file_type = probe
        .file_type()
        .or_else(|| FileType::from_path(filename))
        .ok_or(MetadataError::UnsupportedFormat)?;
    let mut reader = probe.into_inner();

    let (codec, file) = match file_type {
        FileType::Mpeg => {
            let file = MpegFile::read_from(&mut reader, options)?;
            if file.properties().layer() != &Layer::Layer3 {
                return Err(MetadataError::UnsupportedFormat);
            }
            (AudioCodec::Mp3, file.into())
        }
        FileType::Mp4 => {
            let file = Mp4File::read_from(&mut reader, options)?;
            if file.properties().is_drm_protected() {
                return Err(MetadataError::ProtectedAudio);
            }
            let codec = match file.properties().codec() {
                Some(Mp4Codec::AAC) => AudioCodec::Aac,
                Some(Mp4Codec::ALAC) => AudioCodec::Alac,
                Some(Mp4Codec::MP3) => AudioCodec::Mp3,
                Some(Mp4Codec::FLAC) => AudioCodec::Flac,
                _ => return Err(MetadataError::UnsupportedFormat),
            };
            (codec, file.into())
        }
        FileType::Wav => {
            let file = WavFile::read_from(&mut reader, options)?;
            if !matches!(
                file.properties().format(),
                WavFormat::PCM | WavFormat::IEEE_FLOAT
            ) {
                return Err(MetadataError::UnsupportedFormat);
            }
            (AudioCodec::Wav, file.into())
        }
        FileType::Aiff => {
            let file = AiffFile::read_from(&mut reader, options)?;
            if !matches!(
                file.properties().compression_type(),
                None | Some(
                    AiffCompressionType::None
                        | AiffCompressionType::sowt
                        | AiffCompressionType::fl32
                        | AiffCompressionType::FL32
                        | AiffCompressionType::fl64
                )
            ) {
                return Err(MetadataError::UnsupportedFormat);
            }
            (AudioCodec::Aiff, file.into())
        }
        FileType::Flac | FileType::Vorbis | FileType::Opus => {
            let codec = match file_type {
                FileType::Flac => AudioCodec::Flac,
                FileType::Vorbis => AudioCodec::Vorbis,
                _ => AudioCodec::Opus,
            };
            let file = Probe::with_file_type(reader, file_type)
                .options(options)
                .read()?;
            (codec, file)
        }
        _ => return Err(MetadataError::UnsupportedFormat),
    };

    let properties = file.properties();
    let duration = properties.duration();
    let duration_ms =
        u64::try_from(duration.as_millis()).map_err(|_| MetadataError::InvalidProperties)?;
    let sample_rate_hz = properties
        .sample_rate()
        .filter(|rate| *rate > 0)
        .ok_or(MetadataError::InvalidProperties)?;
    let channels = properties
        .channels()
        .filter(|channels| *channels > 0)
        .ok_or(MetadataError::InvalidProperties)?;
    if duration.is_zero() {
        return Err(MetadataError::InvalidProperties);
    }
    let bit_depth = match codec {
        AudioCodec::Flac | AudioCodec::Alac | AudioCodec::Wav | AudioCodec::Aiff => {
            properties.bit_depth().filter(|bits| *bits > 0)
        }
        _ => None,
    };
    let audio = AudioProperties {
        codec,
        duration_ms,
        sample_rate_hz,
        bit_depth,
        bitrate_kbps: properties.audio_bitrate().filter(|rate| *rate > 0),
        channels,
    };
    Ok(extract_tags(&file, filename, audio))
}

fn extract_tags(file: &TaggedFile, filename: &Path, audio: AudioProperties) -> TrackMetadata {
    let primary = file.primary_tag();
    let tags = primary.into_iter().chain(
        file.tags()
            .iter()
            .filter(|tag| Some(tag.tag_type()) != primary.map(Tag::tag_type)),
    );
    let text = |key| {
        tags.clone()
            .flat_map(|tag| tag.get_strings(key))
            .map(str::trim)
            .find(|value| !value.is_empty())
    };
    let string = |key| text(key).map(str::to_owned);
    let number = |key| {
        text(key)
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|n| *n > 0)
    };
    let gain = |key| {
        let value = text(key)?;
        let suffix = value.len().saturating_sub(2);
        let value = if value
            .get(suffix..)
            .is_some_and(|unit| unit.eq_ignore_ascii_case("db"))
        {
            value.get(..suffix)?.trim_end()
        } else {
            value
        };
        finite_number(value)
    };
    let peak = |key| {
        text(key)
            .and_then(finite_number)
            .filter(|value| *value >= 0.0)
    };
    TrackMetadata {
        title: string(ItemKey::TrackTitle).unwrap_or_else(|| {
            filename
                .file_stem()
                .unwrap_or(filename.as_os_str())
                .to_string_lossy()
                .into_owned()
        }),
        artist: string(ItemKey::TrackArtist),
        album_artist: string(ItemKey::AlbumArtist),
        album: string(ItemKey::AlbumTitle),
        composer: string(ItemKey::Composer),
        genre: string(ItemKey::Genre),
        year: tags
            .clone()
            .find_map(|tag| tag.date().map(|date| date.year)),
        track_number: number(ItemKey::TrackNumber),
        track_total: number(ItemKey::TrackTotal),
        disc_number: number(ItemKey::DiscNumber),
        disc_total: number(ItemKey::DiscTotal),
        compilation: text(ItemKey::FlagCompilation).and_then(|value| match value {
            "1" => Some(true),
            "0" => Some(false),
            _ => value.to_ascii_lowercase().parse().ok(),
        }),
        sort_title: string(ItemKey::TrackTitleSortOrder),
        sort_artist: string(ItemKey::TrackArtistSortOrder),
        sort_album: string(ItemKey::AlbumTitleSortOrder),
        sort_album_artist: string(ItemKey::AlbumArtistSortOrder),
        replay_gain: ReplayGain {
            track_gain_db: gain(ItemKey::ReplayGainTrackGain),
            album_gain_db: gain(ItemKey::ReplayGainAlbumGain),
            track_peak: peak(ItemKey::ReplayGainTrackPeak),
            album_peak: peak(ItemKey::ReplayGainAlbumPeak),
        },
        audio,
    }
}

fn finite_number(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::properties::FileProperties;
    use lofty::tag::TagType;

    fn extract(tags: Vec<Tag>) -> TrackMetadata {
        let file = TaggedFile::new(FileType::Mpeg, FileProperties::default(), tags);
        let audio = AudioProperties {
            codec: AudioCodec::Mp3,
            duration_ms: 1_000,
            sample_rate_hz: 44_100,
            bit_depth: None,
            bitrate_kbps: Some(320),
            channels: 2,
        };
        extract_tags(&file, Path::new("Untitled song.mp3"), audio)
    }

    #[test]
    fn primary_tags_win_and_secondary_tags_fill_missing_fields() {
        let mut legacy = Tag::new(TagType::Id3v1);
        legacy.insert_text(ItemKey::TrackTitle, "Legacy title".into());
        legacy.insert_text(ItemKey::AlbumTitle, "Legacy album".into());
        let mut primary = Tag::new(TagType::Id3v2);
        primary.insert_text(ItemKey::TrackTitle, " Primary title ".into());
        primary.insert_text(ItemKey::AlbumTitle, "  ".into());
        primary.insert_text(ItemKey::FlagCompilation, "0".into());
        let track = extract(vec![legacy, primary]);
        assert_eq!(track.title, "Primary title");
        assert_eq!(track.album.as_deref(), Some("Legacy album"));
        assert_eq!(track.compilation, Some(false));
    }

    #[test]
    fn blank_tags_and_invalid_numbers_do_not_create_library_values() {
        let mut tag = Tag::new(TagType::Id3v2);
        for (key, value) in [
            (ItemKey::TrackTitle, " \t "),
            (ItemKey::TrackArtist, " "),
            (ItemKey::AlbumTitle, "\t"),
            (ItemKey::TrackNumber, "0"),
            (ItemKey::DiscTotal, "-2"),
            (ItemKey::ReplayGainTrackGain, "NaN dB"),
            (ItemKey::ReplayGainAlbumGain, " -3.5 DB "),
            (ItemKey::ReplayGainTrackPeak, "inf"),
            (ItemKey::ReplayGainAlbumPeak, "-0.1"),
        ] {
            tag.insert_text(key, value.into());
        }
        let track = extract(vec![tag]);
        assert_eq!(track.title, "Untitled song");
        assert_eq!(track.artist, None);
        assert_eq!(track.album, None);
        assert_eq!(track.track_number, None);
        assert_eq!(track.disc_total, None);
        assert_eq!(
            track.replay_gain,
            ReplayGain {
                album_gain_db: Some(-3.5),
                ..ReplayGain::default()
            }
        );
    }
}
