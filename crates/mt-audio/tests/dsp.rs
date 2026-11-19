use std::path::PathBuf;

use mt_audio::{
    AudioDecoder, AudioQueue, AudioSpec, Dsp, DspError, DspSettings, ReplayGain, SoundCheck,
};

const MONO: AudioSpec = AudioSpec {
    sample_rate: 48_000,
    channels: 1,
};

#[test]
fn stepped_sine_sweep_matches_the_five_band_frequency_response() {
    // Use the published centers independently of the processor's constants.
    let centers = [60, 230, 910, 3_600, 14_000];
    let points = [20, 60, 120, 230, 460, 910, 1800, 3600, 7200, 14000, 21000];
    for (band, &center) in centers.iter().enumerate() {
        for gain in [-6, -3, -1, 0, 1, 3, 6] {
            let mut eq_db = [0; 5];
            eq_db[band] = gain;
            let mut dsp = Dsp::new(
                MONO,
                DspSettings {
                    eq_db,
                    ..DspSettings::default()
                },
                ReplayGain::default(),
            )
            .unwrap();
            for frequency in points {
                let omega = std::f64::consts::TAU * f64::from(frequency) / 48_000.0;
                let mut sine: Vec<f32> = (0..24_000)
                    .map(|frame| (0.15 * (omega * f64::from(frame)).sin()) as f32)
                    .collect();
                let before = power(&sine[12_000..]);
                dsp.process(&mut sine).unwrap();
                let observed = 10.0 * (power(&sine[12_000..]) / before).log10();
                // For Q=1 the prewarped frequency ratio determines the
                // analog peaking prototype's normalized magnitude.
                let ratio = (omega * 0.5).tan()
                    / (std::f64::consts::PI * f64::from(center) / 48_000.0).tan();
                let a = 10.0_f64.powf(f64::from(gain) / 40.0);
                let offset = (1.0 - ratio * ratio).powi(2);
                let power_ratio = (offset + (ratio * a).powi(2)) / (offset + (ratio / a).powi(2));
                let expected = 10.0 * power_ratio.log10();
                assert!(
                    (observed - expected).abs() <= 0.5,
                    "{center} Hz band, {frequency} Hz input, {gain} dB: {observed} vs {expected} dB"
                );
            }
        }
    }
}

fn power(samples: &[f32]) -> f64 {
    samples
        .iter()
        .map(|&sample| f64::from(sample).powi(2))
        .sum()
}

#[test]
fn volume_is_capped_and_control_changes_ramp_over_twenty_ms() {
    let mut dsp = Dsp::new(
        MONO,
        DspSettings {
            volume_limit: 0.5,
            ..DspSettings::default()
        },
        ReplayGain::default(),
    )
    .unwrap();
    let mut initial = [0.5; 4];
    dsp.process(&mut initial).unwrap();
    assert_eq!(initial, [0.25; 4]);
    dsp.set_settings(DspSettings {
        volume: 0.0,
        volume_limit: 0.5,
        ..DspSettings::default()
    })
    .unwrap();
    let mut first = [0.5; 480];
    let mut second = [0.5; 481];
    dsp.process(&mut first).unwrap();
    dsp.process(&mut second).unwrap();
    assert!((first[0] - 0.25 * (959.0 / 960.0)).abs() < 1e-6);
    assert!((first[479] - 0.125).abs() < 1e-6);
    assert_eq!(second[479..], [0.0, 0.0]);
    assert!(
        first
            .iter()
            .chain(&second)
            .copied()
            .all(|sample| (0.0..=0.25).contains(&sample))
    );
    assert!(matches!(
        dsp.set_settings(DspSettings {
            eq_db: [7, 0, 0, 0, 0],
            ..DspSettings::default()
        }),
        Err(DspError::Settings)
    ));
}

#[test]
fn sound_check_applies_track_or_album_gain_with_headroom_then_limits_peaks() {
    let gain = ReplayGain {
        track_db: Some(6.0),
        album_db: Some(-4.0),
    };
    for (mode, gain, expected) in [
        (SoundCheck::Off, gain, 0.25),
        (SoundCheck::Track, gain, 0.25),
        (SoundCheck::Album, gain, 0.25 * 10.0_f32.powf(-10.0 / 20.0)),
        (
            SoundCheck::Album,
            ReplayGain {
                album_db: None,
                ..gain
            },
            0.25,
        ),
        (
            SoundCheck::Track,
            ReplayGain::default(),
            0.25 * 10.0_f32.powf(-6.0 / 20.0),
        ),
    ] {
        let mut dsp = Dsp::new(
            MONO,
            DspSettings {
                sound_check: mode,
                ..DspSettings::default()
            },
            gain,
        )
        .unwrap();
        let mut samples = [0.25; 16];
        dsp.process(&mut samples).unwrap();
        assert!(
            samples
                .iter()
                .all(|sample| (sample - expected).abs() < 1e-6),
            "{mode:?}"
        );
    }

    let mut dsp = Dsp::new(
        MONO,
        DspSettings {
            sound_check: SoundCheck::Track,
            ..DspSettings::default()
        },
        ReplayGain {
            track_db: Some(12.0),
            album_db: None,
        },
    )
    .unwrap();
    let mut peaks = [0.5, -0.5, 0.9, -0.9];
    dsp.process(&mut peaks).unwrap();
    assert!((0.9..1.0).contains(&peaks[0]));
    assert_eq!(peaks[0], -peaks[1]);
    assert!(peaks[2] > peaks[0] && peaks[2] <= 1.0);
    assert_eq!(peaks[2], -peaks[3]);
}

#[test]
#[ignore = "run after cargo run -p gen-fixtures with ffmpeg installed"]
fn carries_replaygain_from_file_through_resampled_queue_boundaries() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio/generated");
    let paths = [
        root.join("flac_16_44.flac"),
        root.join("flac_replaygain.flac"),
    ];
    let expected = ReplayGain {
        track_db: Some(-6.5),
        album_db: Some(-6.2),
    };
    assert_eq!(
        AudioDecoder::open(&paths[1]).unwrap().replay_gain(),
        expected
    );
    let mut queue = AudioQueue::open(&paths, MONO, 0, true).unwrap();
    let mut count = 0;
    while let Some(block) = queue.next_block().unwrap() {
        if block.track_index == 0 {
            assert_eq!(block.replay_gain, ReplayGain::default());
        } else {
            assert_eq!(block.replay_gain, expected);
            count += block.samples.len();
        }
    }
    assert!(count > 0);
}
