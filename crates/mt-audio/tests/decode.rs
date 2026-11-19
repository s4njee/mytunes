use std::fs;
use std::path::Path;

use gen_fixtures::{Pcm, wav_bytes};
use mt_audio::{AudioDecodeError, AudioDecoder, AudioSpec};

#[test]
fn seeks_to_the_requested_pcm_frame_after_packet_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("timeline.wav");
    let sample_rate = 44_100;
    let pcm = Pcm {
        sample_rate,
        channels: 1,
        bits: 16,
        samples: (0..sample_rate * 3)
            .map(|index| f64::from(index) / (f64::from(sample_rate) * 3.0))
            .collect(),
    };
    fs::write(&path, wav_bytes(&pcm)).unwrap();
    let mut decoder = AudioDecoder::open(&path).unwrap();
    for ms in [1_037, 7, 2_083, 439, 0] {
        decoder.seek(ms).unwrap();
        let expected = (ms * u64::from(sample_rate) / 1_000) as usize;
        let samples = decoder.next_samples().unwrap().unwrap();
        assert!(!samples.is_empty());
        assert!(
            (f64::from(samples[0]) - pcm.samples[expected]).abs() <= 1.0 / 16_384.0,
            "{ms} ms"
        );
    }
    assert!(matches!(
        decoder.seek(9_000),
        Err(AudioDecodeError::InvalidSeek)
    ));
}

#[test]
#[ignore = "run after cargo run -p gen-fixtures with ffmpeg installed"]
fn streams_the_generated_audio_formats_and_seeks_mid_track() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio/generated");
    for (filename, rate, channels) in [
        ("mp3_cbr320.mp3", 44_100, 2),
        ("mp3_vbr_v2.mp3", 44_100, 2),
        ("aac_256.m4a", 44_100, 2),
        ("alac_16_44.m4a", 44_100, 2),
        ("flac_16_44.flac", 44_100, 2),
        ("flac_24_96.flac", 96_000, 2),
        ("flac_replaygain.flac", 44_100, 2),
        ("wav_16_44_untagged.wav", 44_100, 2),
        ("aiff_16_44.aiff", 44_100, 2),
        ("ogg_vorbis.ogg", 44_100, 2),
        ("opus.opus", 48_000, 2),
        ("gapless_a.flac", 44_100, 1),
        ("gapless_b.flac", 44_100, 1),
        ("gapless_a.mp3", 44_100, 1),
        ("gapless_b.mp3", 44_100, 1),
        ("gapless_a.m4a", 44_100, 1),
        ("gapless_b.m4a", 44_100, 1),
    ] {
        let mut decoder = AudioDecoder::open(fixtures.join(filename))
            .unwrap_or_else(|error| panic!("{filename} open: {error}"));
        assert_eq!(
            decoder.spec(),
            AudioSpec {
                sample_rate: rate,
                channels
            },
            "{filename}"
        );
        let mut total = 0;
        let mut packets = 0;
        while let Some(samples) = decoder
            .next_samples()
            .unwrap_or_else(|error| panic!("{filename} packet: {error}"))
        {
            assert_eq!(samples.len() % channels, 0, "{filename}");
            assert!(
                samples.iter().all(|sample| sample.is_finite()),
                "{filename}"
            );
            total += samples.len() / channels;
            packets += 1;
        }
        assert!(packets > 1, "{filename}");
        let millis = total as u64 * 1_000 / u64::from(rate);
        assert!((2_900..=3_100).contains(&millis), "{filename}: {millis} ms");

        decoder
            .seek(1_137)
            .unwrap_or_else(|error| panic!("{filename} seek: {error}"));
        let mut remaining = 0;
        while let Some(samples) = decoder
            .next_samples()
            .unwrap_or_else(|error| panic!("{filename} after seek: {error}"))
        {
            remaining += samples.len() / channels;
        }
        let ms_before_seek = (total - remaining) as u64 * 1_000 / u64::from(rate);
        assert!(
            ms_before_seek.abs_diff(1_137) <= 20,
            "{filename}: resumed at {ms_before_seek} ms"
        );
    }
}

#[test]
fn resamples_and_drains_the_tail_after_seek() {
    use gen_fixtures::sine;
    use mt_audio::RenderStream;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sine.wav");
    let samples = sine(44_100, 16, &[440.0], 0.731, 0.5);
    fs::write(&path, wav_bytes(&samples)).unwrap();

    let mut stream = RenderStream::open(
        &path,
        AudioSpec {
            sample_rate: 48_000,
            channels: 2,
        },
    )
    .unwrap();
    for seek_ms in [0, 137, 701, 0] {
        stream.seek(seek_ms).unwrap();
        let mut frames = 0;
        let mut energy = 0.0;
        while let Some(block) = stream.next_samples().unwrap() {
            for stereo in block.as_chunks::<2>().0 {
                assert_eq!(stereo[0], stereo[1]);
                energy += f64::from(stereo[0]).powi(2);
                frames += 1;
            }
        }
        let skipped = (seek_ms * 44_100 / 1_000) as usize;
        let expected = ((samples.frames() - skipped) * 48_000).div_ceil(44_100);
        assert_eq!(frames, expected, "seek offset {seek_ms}");
        assert!(energy / frames as f64 > 0.09, "seek offset {seek_ms}");
    }
}
