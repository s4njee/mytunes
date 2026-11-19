use std::fs;
use std::io::Cursor;
use std::path::Path;

use gen_fixtures::{sine, wav_bytes};
use mt_meta::{
    AudioCodec, MetadataError, ReplayGain, TrackMetadata, read_metadata, read_metadata_from,
};

fn fixture_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/audio/generated")
        .join(name)
}

#[test]
fn reads_untagged_audio_by_content_and_falls_back_to_the_stem() {
    let bytes = wav_bytes(&sine(44_100, 16, &[440.0], 0.1, 0.5));
    let mut reader = Cursor::new(bytes);
    reader.set_position(12);
    let track = read_metadata_from(reader, Path::new("library/My tone.unknown")).unwrap();
    assert_eq!(track.title, "My tone");
    assert_eq!(track.artist, None);
    assert_eq!(track.album, None);
    assert_eq!(track.audio.codec, AudioCodec::Wav);
    assert_eq!(track.audio.duration_ms, 100);
    assert_eq!(track.audio.sample_rate_hz, 44_100);
    assert_eq!(track.audio.bit_depth, Some(16));
    assert_eq!(track.audio.channels, 1);
    assert_eq!(track.audio.bitrate_kbps, Some(706));
}

#[test]
fn reports_io_unsupported_and_corrupt_input_separately() {
    assert!(matches!(
        read_metadata("does-not-exist.wav"),
        Err(MetadataError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound
    ));
    assert!(matches!(
        read_metadata_from(Cursor::new(b"plain text"), Path::new("note.txt")),
        Err(MetadataError::UnsupportedFormat)
    ));
    assert!(matches!(
        read_metadata_from(
            Cursor::new(b"RIFF\xff\xff\xff\xffWAVE"),
            Path::new("bad.wav")
        ),
        Err(MetadataError::InvalidAudio(_) | MetadataError::InvalidProperties)
    ));
}

fn assert_full_tags(track: &TrackMetadata) {
    assert_eq!(track.title, "Tone A4");
    assert_eq!(track.artist.as_deref(), Some("Test Tones"));
    assert_eq!(track.album_artist.as_deref(), Some("Test Tones"));
    assert_eq!(track.album.as_deref(), Some("Fixtures Vol. 1"));
    assert_eq!(track.composer.as_deref(), Some("gen-fixtures"));
    assert_eq!(track.genre.as_deref(), Some("Test"));
    assert_eq!(track.year, Some(2026));
    assert_eq!(track.track_number, Some(3));
    assert_eq!(track.track_total, Some(12));
    assert_eq!(track.disc_number, Some(1));
    assert_eq!(track.disc_total, Some(2));
}

#[test]
#[ignore = "run after cargo run -p gen-fixtures with ffmpeg installed"]
fn reads_all_generated_audio_formats() {
    use AudioCodec::{Aac, Aiff, Alac, Flac, Mp3, Opus, Vorbis, Wav};

    for (name, codec, rate, depth, channels) in [
        ("mp3_cbr320.mp3", Mp3, 44_100, None, 2),
        ("mp3_vbr_v2.mp3", Mp3, 44_100, None, 2),
        ("aac_256.m4a", Aac, 44_100, None, 2),
        ("alac_16_44.m4a", Alac, 44_100, Some(16), 2),
        ("flac_16_44.flac", Flac, 44_100, Some(16), 2),
        ("flac_24_96.flac", Flac, 96_000, Some(24), 2),
        ("flac_replaygain.flac", Flac, 44_100, Some(16), 2),
        ("wav_16_44_untagged.wav", Wav, 44_100, Some(16), 2),
        ("aiff_16_44.aiff", Aiff, 44_100, Some(16), 2),
        ("ogg_vorbis.ogg", Vorbis, 44_100, None, 2),
        ("opus.opus", Opus, 48_000, None, 2),
        ("gapless_a.flac", Flac, 44_100, Some(16), 1),
        ("gapless_b.flac", Flac, 44_100, Some(16), 1),
        ("gapless_a.mp3", Mp3, 44_100, None, 1),
        ("gapless_b.mp3", Mp3, 44_100, None, 1),
        ("gapless_a.m4a", Aac, 44_100, None, 1),
        ("gapless_b.m4a", Aac, 44_100, None, 1),
    ] {
        let track =
            read_metadata(fixture_path(name)).unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let audio = &track.audio;
        assert_eq!(audio.codec, codec, "{name}");
        assert_eq!(audio.sample_rate_hz, rate, "{name}");
        assert_eq!(audio.bit_depth, depth, "{name}");
        assert_eq!(audio.channels, channels, "{name}");
        assert!(audio.bitrate_kbps.is_some_and(|rate| rate > 0), "{name}");
        assert!(
            (2_900..=3_100).contains(&audio.duration_ms),
            "{name}: {audio:?}"
        );

        match name {
            "mp3_cbr320.mp3" | "aac_256.m4a" | "flac_16_44.flac" | "flac_24_96.flac"
            | "aiff_16_44.aiff" | "opus.opus" => assert_full_tags(&track),
            "mp3_vbr_v2.mp3" | "ogg_vorbis.ogg" => {
                assert_eq!(track.title, "Ünïcödé — 日本語テスト ♪", "{name}");
                assert_eq!(track.artist.as_deref(), Some("Björk Tëst"));
                assert_eq!(track.album.as_deref(), Some("Café Tones"));
                assert_eq!(track.genre.as_deref(), Some("Tést"));
                assert_eq!(track.track_number, Some(1));
            }
            "alac_16_44.m4a" => {
                assert_eq!(track.title, "Various Tone");
                assert_eq!(track.compilation, Some(true));
                assert_eq!(track.album_artist.as_deref(), Some("Various Artists"));
                assert_eq!(track.track_number, Some(7));
                assert_eq!(track.track_total, Some(20));
            }
            "wav_16_44_untagged.wav" => {
                assert_eq!(track.title, "wav_16_44_untagged");
                assert_eq!(track.artist, None);
                assert_eq!(track.album, None);
            }
            "flac_replaygain.flac" => {
                assert_eq!(track.title, "Loud Tone");
                assert_eq!(track.sort_title.as_deref(), Some("Tone, Loud"));
                assert_eq!(track.sort_artist.as_deref(), Some("Tones, Test"));
                assert_eq!(track.sort_album.as_deref(), Some("Fixtures 01"));
                assert_eq!(track.sort_album_artist.as_deref(), Some("Tones, Test"));
                assert_eq!(
                    track.replay_gain,
                    ReplayGain {
                        track_gain_db: Some(-6.50),
                        album_gain_db: Some(-6.20),
                        track_peak: Some(0.988525),
                        album_peak: Some(1.02),
                    }
                );
            }
            _ => {
                let part_a = name.starts_with("gapless_a.");
                assert_eq!(
                    track.title,
                    if part_a { "Gapless A" } else { "Gapless B" },
                    "{name}"
                );
                assert_eq!(
                    track.track_number,
                    Some(if part_a { 1 } else { 2 }),
                    "{name}"
                );
                assert_eq!(track.track_total, Some(2), "{name}");
                assert_eq!(track.artist, None, "{name}");
            }
        }
    }

    assert!(matches!(
        read_metadata(fixture_path("corrupt.mp3")),
        Err(MetadataError::InvalidAudio(_) | MetadataError::InvalidProperties)
    ));
}

#[test]
#[ignore = "run after cargo run -p gen-fixtures with ffmpeg installed"]
fn identifies_protected_mp4_from_its_sample_entry() {
    let mut bytes = fs::read(fixture_path("aac_256.m4a")).unwrap();
    let entry = bytes
        .windows(4)
        .position(|window| window == b"mp4a")
        .unwrap();
    bytes[entry..entry + 4].copy_from_slice(b"drms");
    assert!(matches!(
        read_metadata_from(Cursor::new(bytes), Path::new("renamed.m4a")),
        Err(MetadataError::ProtectedAudio)
    ));
}

#[test]
#[ignore = "run after cargo run -p gen-fixtures with ffmpeg installed"]
fn malformed_opus_granule_positions_return_a_typed_error() {
    let original = fs::read(fixture_path("opus.opus")).unwrap();
    let last_page = original
        .windows(4)
        .rposition(|bytes| bytes == b"OggS")
        .unwrap();
    for granule in [u64::MAX, u64::MAX - 1, 0, 1] {
        let mut bytes = original.clone();
        bytes[last_page + 6..last_page + 14].copy_from_slice(&granule.to_le_bytes());
        assert!(
            read_metadata_from(Cursor::new(bytes), Path::new("bad.opus")).is_err(),
            "accepted invalid granule position {granule}"
        );
    }
}
