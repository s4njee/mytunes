use std::fs;
use std::path::{Path, PathBuf};

use gen_fixtures::{GAPLESS_SPLIT_FRAME, Pcm, wav_bytes};
use mt_audio::{AudioDecoder, AudioQueue, AudioSpec, RenderStream};

const MONO: AudioSpec = AudioSpec {
    sample_rate: 44_100,
    channels: 1,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/audio/generated")
        .join(name)
}

fn collect_queue(paths: &[PathBuf], spec: AudioSpec, gapless: bool) -> (Vec<f32>, Vec<usize>) {
    let mut queue = AudioQueue::open(paths, spec, 0, gapless).expect("open gapless test queue");
    let mut output = Vec::new();
    let mut lengths = vec![0; paths.len()];
    while let Some(block) = queue.next_block().expect("render next queued block") {
        lengths[block.track_index] += block.samples.len() / spec.channels;
        output.extend_from_slice(block.samples);
    }
    (output, lengths)
}

fn collect_file(path: &Path, spec: AudioSpec) -> Vec<f32> {
    let mut render = RenderStream::open(path, spec).expect("open single reference track");
    let mut output = Vec::new();
    while let Some(samples) = render.next_samples().expect("decode reference PCM") {
        output.extend_from_slice(samples);
    }
    output
}

#[test]
#[ignore = "run after cargo run -p gen-fixtures with ffmpeg installed"]
fn flac_boundary_matches_the_unsplit_waveform_and_compressed_pairs_keep_their_lengths() {
    let reference = collect_file(&fixture("src/gapless_full.wav"), MONO);
    for extension in ["flac", "mp3", "m4a"] {
        let paths = [
            fixture(&format!("gapless_a.{extension}")),
            fixture(&format!("gapless_b.{extension}")),
        ];
        let (output, lengths) = collect_queue(&paths, MONO, true);
        assert_eq!(
            lengths,
            [GAPLESS_SPLIT_FRAME, reference.len() - GAPLESS_SPLIT_FRAME],
            "{extension}"
        );
        assert_eq!(output.len(), reference.len(), "{extension}");
        if extension == "flac" {
            for index in GAPLESS_SPLIT_FRAME - 1..=GAPLESS_SPLIT_FRAME + 1 {
                assert!(
                    (output[index] - reference[index]).abs() <= 1e-4,
                    "boundary sample {index}"
                );
            }
        }
    }
}

#[test]
#[ignore = "run after cargo run -p gen-fixtures with ffmpeg installed"]
fn aac_itunsmpb_takes_precedence_over_the_mp4_edit_list() {
    use lofty::mp4::{Atom, AtomData, AtomIdent, Ilst};
    use lofty::tag::TagExt;
    use std::borrow::Cow;

    let dir = tempfile::tempdir().unwrap();
    let tagged = dir.path().join("smpb.m4a");
    fs::copy(fixture("gapless_a.m4a"), &tagged).unwrap();
    // The fixture's raw encoded frames include AAC padding. Override its
    // presentation window to exercise a distinct iTunes timing value.
    let mut tags = Ilst::new();
    tags.insert(Atom::new(
        AtomIdent::Freeform {
            mean: Cow::Borrowed("com.apple.iTunes"),
            name: Cow::Borrowed("iTunSMPB"),
        },
        AtomData::UTF8(" 00000000 00000840 00000610 000000000001FBD0 00000000".into()),
    ));
    tags.save_to_path(&tagged, Default::default()).unwrap();
    let mut decoder = AudioDecoder::open(&tagged).unwrap();
    let mut frames = 0;
    while let Some(samples) = decoder.next_samples().unwrap() {
        frames += samples.len();
    }
    assert_eq!(frames, 130_000);
}

#[test]
fn disabled_gapless_applies_five_millisecond_fades_without_inserting_frames() {
    let dir = tempfile::tempdir().unwrap();
    let spec = AudioSpec {
        sample_rate: 48_000,
        channels: 2,
    };
    let paths = [
        dir.path().join("first.wav"),
        dir.path().join("second.wav"),
        dir.path().join("third.wav"),
    ];
    for (path, (frames, amplitude)) in paths.iter().zip([(700, 0.5), (96, -0.5), (800, 0.5)]) {
        fs::write(
            path,
            wav_bytes(&Pcm {
                sample_rate: spec.sample_rate,
                channels: 2,
                bits: 16,
                samples: vec![amplitude; frames * 2],
            }),
        )
        .unwrap();
    }
    let (plain, lengths) = collect_queue(&paths, spec, true);
    let (faded, faded_lengths) = collect_queue(&paths, spec, false);
    assert_eq!(lengths, [700, 96, 800]);
    assert_eq!(lengths, faded_lengths);
    assert_eq!(faded.len(), plain.len());
    assert_eq!(&faded[..(700 - 240) * 2], &plain[..(700 - 240) * 2]);
    assert_eq!(&faded[(700 - 1) * 2..(700 + 1) * 2], &[0.0; 4]);
    assert_eq!(&faded[(796 - 1) * 2..(796 + 1) * 2], &[0.0; 4]);
    assert_eq!(&faded[(796 + 239) * 2..], &plain[(796 + 239) * 2..]);
    for frame in 0..240 {
        let index = (700 - 240 + frame) * 2;
        let expected = plain[index] * (239 - frame) as f32 / 239.0;
        assert!((faded[index] - expected).abs() < 1e-6, "fade frame {frame}");
    }
}

#[test]
#[ignore = "run after cargo run -p gen-fixtures with ffmpeg installed"]
fn resampled_album_join_matches_the_unsplit_track() {
    let rate = 48_000;
    let target = AudioSpec {
        sample_rate: rate,
        channels: 1,
    };
    let reference = collect_file(&fixture("src/gapless_full.wav"), target);
    let paths = [fixture("gapless_a.flac"), fixture("gapless_b.flac")];
    let (output, lengths) = collect_queue(&paths, target, true);
    let split = (GAPLESS_SPLIT_FRAME * rate as usize).div_ceil(44_100);
    assert_eq!(lengths, [split, reference.len() - split]);
    assert_eq!(output.len(), reference.len());
    for (index, (actual, expected)) in output.iter().zip(reference).enumerate() {
        assert!((actual - expected).abs() <= 1e-4, "resampled frame {index}");
    }
}

#[test]
fn rate_changes_flush_the_current_run_and_preserve_track_markers() {
    let dir = tempfile::tempdir().unwrap();
    let rates = [44_100, 96_000, 48_000, 44_100];
    let mut paths = Vec::new();
    let mut counts = Vec::new();
    for (index, rate) in rates.into_iter().enumerate() {
        let path = dir.path().join(format!("{index}.wav"));
        fs::write(
            &path,
            wav_bytes(&Pcm {
                sample_rate: rate,
                channels: 1,
                bits: 16,
                samples: vec![0.5; 733],
            }),
        )
        .unwrap();
        paths.push(path);
        counts.push((733usize * 48_000).div_ceil(rate as usize));
    }
    let (output, lengths) = collect_queue(
        &paths,
        AudioSpec {
            sample_rate: 48_000,
            channels: 2,
        },
        true,
    );
    assert_eq!(lengths, counts);
    assert_eq!(output.len(), counts.into_iter().sum::<usize>() * 2);
}
