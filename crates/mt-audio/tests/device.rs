use std::path::Path;

#[test]
#[ignore = "requires a CPAL playback device and generated audio fixtures"]
fn plays_the_generated_audio_formats_through_cpal() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio/generated");
    for name in [
        "mp3_cbr320.mp3",
        "mp3_vbr_v2.mp3",
        "aac_256.m4a",
        "alac_16_44.m4a",
        "flac_16_44.flac",
        "flac_24_96.flac",
        "flac_replaygain.flac",
        "wav_16_44_untagged.wav",
        "aiff_16_44.aiff",
        "ogg_vorbis.ogg",
        "opus.opus",
        "gapless_a.flac",
        "gapless_b.flac",
        "gapless_a.mp3",
        "gapless_b.mp3",
        "gapless_a.m4a",
        "gapless_b.m4a",
    ] {
        let frames = mt_audio::play_file(fixtures.join(name), 0)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(frames > 0, "{name}");
    }
}

#[test]
#[ignore = "requires a CPAL playback device and generated audio fixtures"]
fn plays_a_gapless_pair_through_one_cpal_stream() {
    use cpal::traits::{DeviceTrait, HostTrait};
    use mt_audio::{DspSettings, SoundCheck};

    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio/generated");
    let rate = cpal::default_host()
        .default_output_device()
        .unwrap()
        .default_output_config()
        .unwrap()
        .sample_rate();
    for extension in ["flac", "mp3", "m4a"] {
        let paths = [
            fixtures.join(format!("gapless_a.{extension}")),
            fixtures.join(format!("gapless_b.{extension}")),
        ];
        assert_eq!(
            mt_audio::play_queue_with_dsp(
                &paths,
                0,
                true,
                DspSettings {
                    eq_db: [3, 1, -2, 4, 2],
                    volume: 0.75,
                    volume_limit: 0.5,
                    sound_check: SoundCheck::Album,
                }
            )
            .unwrap(),
            6 * u64::from(rate),
            "{extension}"
        );
    }
}

#[test]
#[ignore = "requires a CPAL playback device and generated audio fixtures"]
fn shared_player_emits_track_boundaries_through_cpal() {
    use mt_audio::{Player, PlayerState, QueueItem};
    use mt_types::TrackId;
    use std::time::Duration;

    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio/generated");
    let player = Player::new().unwrap();
    let events = player.events().unwrap();
    let pair = ["a", "b"]
        .into_iter()
        .map(|id| QueueItem {
            id: TrackId(id.into()),
            path: fixture_root.join(format!("gapless_{id}.flac")),
            album: Some("Gapless".into()),
        })
        .collect();
    player.load_queue(pair, 0).unwrap();
    player.seek(123).unwrap();
    player.play().unwrap();
    let mut ended = Vec::new();
    loop {
        let event = events.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(event.error.is_none(), "{event:?}");
        if event.ended {
            ended.push(event.track_id.unwrap());
        }
        if event.state == PlayerState::Ended {
            break;
        }
    }
    assert_eq!(ended, [TrackId("a".into()), TrackId("b".into())]);
}
