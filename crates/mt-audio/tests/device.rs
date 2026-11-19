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
