//! Generates the audio fixtures used by tests (plan E0.S4).
//!
//! ```text
//! cargo run -p gen-fixtures                 # writes fixtures/audio/generated/ + manifest.json
//! cargo run -p gen-fixtures -- --out DIR    # elsewhere
//! ```
//!
//! WAV sources are synthesized in Rust. Every other format is encoded with
//! `ffmpeg` (must be on PATH). Without ffmpeg only the WAV fixtures are written
//! and the command exits non-zero so CI notices.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use gen_fixtures::{GAPLESS_SPLIT_FRAME, Pcm, sine, wav_bytes};
use serde_json::{Map, Value, json};

/// A set of tags to write. Keys are ffmpeg metadata keys.
type Tags = &'static [(&'static str, &'static str)];

const FULL: Tags = &[
    ("title", "Tone A4"),
    ("artist", "Test Tones"),
    ("album_artist", "Test Tones"),
    ("album", "Fixtures Vol. 1"),
    ("composer", "gen-fixtures"),
    ("genre", "Test"),
    ("date", "2026"),
    ("track", "3/12"),
    ("disc", "1/2"),
];

const UNICODE: Tags = &[
    ("title", "Ünïcödé — 日本語テスト ♪"),
    ("artist", "Björk Tëst"),
    ("album", "Café Tones"),
    ("genre", "Tést"),
    ("track", "1"),
];

const COMPILATION: Tags = &[
    ("title", "Various Tone"),
    ("artist", "Tone Artist B"),
    ("album_artist", "Various Artists"),
    ("album", "Compilation of Tones"),
    ("compilation", "1"),
    ("track", "7/20"),
];

const REPLAYGAIN: Tags = &[
    ("title", "Loud Tone"),
    ("artist", "Test Tones"),
    ("album", "Fixtures Vol. 1"),
    ("REPLAYGAIN_TRACK_GAIN", "-6.50 dB"),
    ("REPLAYGAIN_TRACK_PEAK", "0.988525"),
    ("REPLAYGAIN_ALBUM_GAIN", "-6.20 dB"),
    ("REPLAYGAIN_ALBUM_PEAK", "1.02"),
    ("TITLESORT", "Tone, Loud"),
    ("ARTISTSORT", "Tones, Test"),
    ("ALBUMSORT", "Fixtures 01"),
    ("ALBUMARTISTSORT", "Tones, Test"),
];

const NONE: Tags = &[];

/// One encoded fixture.
struct Spec {
    file: &'static str,
    source: &'static str,
    /// ffmpeg arguments selecting the codec (after inputs/maps).
    codec: &'static [&'static str],
    tags: Tags,
    art: bool,
    note: &'static str,
}

const SPECS: &[Spec] = &[
    Spec {
        file: "mp3_cbr320.mp3",
        source: "tone_44100_16.wav",
        codec: &["-c:a", "libmp3lame", "-b:a", "320k", "-id3v2_version", "3"],
        tags: FULL,
        art: true,
        note: "MP3 CBR 320, ID3v2.3, cover art",
    },
    Spec {
        file: "mp3_vbr_v2.mp3",
        source: "tone_44100_16.wav",
        codec: &["-c:a", "libmp3lame", "-q:a", "2", "-id3v2_version", "4"],
        tags: UNICODE,
        art: false,
        note: "MP3 VBR, ID3v2.4, Unicode tags",
    },
    Spec {
        file: "aac_256.m4a",
        source: "tone_44100_16.wav",
        codec: &["-c:a", "aac", "-b:a", "256k"],
        tags: FULL,
        art: true,
        note: "AAC-LC in MP4, cover art",
    },
    Spec {
        file: "alac_16_44.m4a",
        source: "tone_44100_16.wav",
        codec: &["-c:a", "alac"],
        tags: COMPILATION,
        art: false,
        note: "ALAC, compilation flag",
    },
    Spec {
        file: "flac_16_44.flac",
        source: "tone_44100_16.wav",
        codec: &["-c:a", "flac"],
        tags: FULL,
        art: true,
        note: "FLAC 16/44.1, cover art",
    },
    Spec {
        file: "flac_24_96.flac",
        source: "tone_96000_24.wav",
        codec: &["-c:a", "flac", "-sample_fmt", "s32"],
        tags: FULL,
        art: false,
        note: "FLAC 24/96 (hi-res badge)",
    },
    Spec {
        file: "flac_replaygain.flac",
        source: "tone_44100_16.wav",
        codec: &["-c:a", "flac"],
        tags: REPLAYGAIN,
        art: false,
        note: "ReplayGain tags",
    },
    Spec {
        file: "wav_16_44_untagged.wav",
        source: "tone_44100_16.wav",
        codec: &["-c:a", "pcm_s16le"],
        tags: NONE,
        art: false,
        note: "no tags: title falls back to filename",
    },
    Spec {
        file: "aiff_16_44.aiff",
        source: "tone_44100_16.wav",
        codec: &["-c:a", "pcm_s16be", "-write_id3v2", "1"],
        tags: FULL,
        art: false,
        note: "AIFF with ID3 chunk",
    },
    Spec {
        file: "ogg_vorbis.ogg",
        source: "tone_44100_16.wav",
        codec: &["-c:a", "libvorbis", "-q:a", "5"],
        tags: UNICODE,
        art: false,
        note: "Ogg Vorbis, Unicode Vorbis comments",
    },
    Spec {
        file: "opus.opus",
        source: "tone_48000_16.wav",
        codec: &["-c:a", "libopus", "-b:a", "160k"],
        tags: FULL,
        art: false,
        note: "Opus (optional format)",
    },
    // Gapless pairs: one continuous 1 kHz sine split into two tracks.
    Spec {
        file: "gapless_a.flac",
        source: "gapless_a.wav",
        codec: &["-c:a", "flac"],
        tags: &[
            ("title", "Gapless A"),
            ("album", "Gapless"),
            ("track", "1/2"),
        ],
        art: false,
        note: "gapless pair, lossless, part 1",
    },
    Spec {
        file: "gapless_b.flac",
        source: "gapless_b.wav",
        codec: &["-c:a", "flac"],
        tags: &[
            ("title", "Gapless B"),
            ("album", "Gapless"),
            ("track", "2/2"),
        ],
        art: false,
        note: "gapless pair, lossless, part 2",
    },
    Spec {
        file: "gapless_a.mp3",
        source: "gapless_a.wav",
        codec: &["-c:a", "libmp3lame", "-b:a", "256k"],
        tags: &[
            ("title", "Gapless A"),
            ("album", "Gapless MP3"),
            ("track", "1/2"),
        ],
        art: false,
        note: "gapless pair, LAME header with encoder delay/padding",
    },
    Spec {
        file: "gapless_b.mp3",
        source: "gapless_b.wav",
        codec: &["-c:a", "libmp3lame", "-b:a", "256k"],
        tags: &[
            ("title", "Gapless B"),
            ("album", "Gapless MP3"),
            ("track", "2/2"),
        ],
        art: false,
        note: "gapless pair, LAME header with encoder delay/padding",
    },
    Spec {
        file: "gapless_a.m4a",
        source: "gapless_a.wav",
        codec: &["-c:a", "aac", "-b:a", "256k"],
        tags: &[
            ("title", "Gapless A"),
            ("album", "Gapless AAC"),
            ("track", "1/2"),
        ],
        art: false,
        note: "gapless pair, AAC priming/padding",
    },
    Spec {
        file: "gapless_b.m4a",
        source: "gapless_b.wav",
        codec: &["-c:a", "aac", "-b:a", "256k"],
        tags: &[
            ("title", "Gapless B"),
            ("album", "Gapless AAC"),
            ("track", "2/2"),
        ],
        art: false,
        note: "gapless pair, AAC priming/padding",
    },
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("write {}: {e}", path.display()))
}

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn run_ffmpeg(args: &[String]) -> Result<(), String> {
    let out = Command::new("ffmpeg")
        .args(["-y", "-hide_banner", "-loglevel", "error", "-nostdin"])
        .args(args)
        .output()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "ffmpeg {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

fn pcm_info(p: &Pcm) -> Value {
    json!({
        "sample_rate": p.sample_rate,
        "bit_depth": p.bits,
        "channels": p.channels,
        "frames": p.frames(),
        "duration_ms": (p.frames() as f64 * 1000.0 / f64::from(p.sample_rate)).round() as u64,
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let out_dir = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("fixtures/audio/generated"));

    match generate(&out_dir) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("gen-fixtures: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Returns `Ok(false)` when only part of the set could be generated.
fn generate(out: &Path) -> Result<bool, String> {
    let src = out.join("src");
    std::fs::create_dir_all(&src).map_err(|e| format!("create {}: {e}", src.display()))?;

    // Sources: -12 dBFS tones (0.25 linear), different pitch per channel.
    let tone_44 = sine(44_100, 16, &[440.0, 660.0], 3.0, 0.25);
    let tone_48 = sine(48_000, 16, &[440.0, 660.0], 3.0, 0.25);
    let tone_96 = sine(96_000, 24, &[440.0, 660.0], 3.0, 0.25);
    let gapless_full = sine(44_100, 16, &[1000.0], 6.0, 0.5);
    let (gapless_a, gapless_b) = gapless_full.split_at_frame(GAPLESS_SPLIT_FRAME);

    let mut sources = Map::new();
    for (name, pcm) in [
        ("tone_44100_16.wav", &tone_44),
        ("tone_48000_16.wav", &tone_48),
        ("tone_96000_24.wav", &tone_96),
        ("gapless_full.wav", &gapless_full),
        ("gapless_a.wav", &gapless_a),
        ("gapless_b.wav", &gapless_b),
    ] {
        write(&src.join(name), &wav_bytes(pcm))?;
        sources.insert(name.to_owned(), pcm_info(pcm));
    }

    // Tiny committed smoke file (0.25 s mono), regenerated deterministically.
    let smoke = sine(44_100, 16, &[440.0], 0.25, 0.25);
    write(
        &repo_root().join("fixtures/audio/smoke.wav"),
        &wav_bytes(&smoke),
    )?;

    // Corrupt file: an ID3 header followed by garbage, for "never panic" tests.
    let mut corrupt = b"ID3\x04\x00\x00\x00\x00\x00\x10".to_vec();
    corrupt.extend((0u32..4096).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8));
    write(&out.join("corrupt.mp3"), &corrupt)?;

    let mut fixtures = Vec::new();
    fixtures.push(json!({ "file": "corrupt.mp3", "expect_error": true, "note": "truncated ID3 header + garbage" }));

    let complete = ffmpeg_available();
    if complete {
        let cover = out.join("src/cover.jpg");
        run_ffmpeg(&[
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            "color=c=0xF0A23C:s=600x600,drawbox=x=150:y=150:w=300:h=300:color=0x0E1116:t=fill"
                .into(),
            "-frames:v".into(),
            "1".into(),
            cover.display().to_string(),
        ])?;

        for spec in SPECS {
            let mut a: Vec<String> = vec!["-i".into(), src.join(spec.source).display().to_string()];
            if spec.art {
                a.extend([
                    "-i".into(),
                    cover.display().to_string(),
                    "-map".into(),
                    "0:a".into(),
                    "-map".into(),
                    "1:v".into(),
                    "-c:v".into(),
                    "mjpeg".into(),
                    "-disposition:v".into(),
                    "attached_pic".into(),
                    "-metadata:s:v".into(),
                    "comment=Cover (front)".into(),
                ]);
            } else {
                a.extend(["-map".into(), "0:a".into()]);
            }
            a.extend(["-map_metadata".into(), "-1".into()]);
            a.extend(spec.codec.iter().map(|s| (*s).to_owned()));
            for (k, v) in spec.tags {
                a.push("-metadata".into());
                a.push(format!("{k}={v}"));
            }
            a.push(out.join(spec.file).display().to_string());
            run_ffmpeg(&a)?;

            let tags: Map<String, Value> = spec
                .tags
                .iter()
                .map(|(k, v)| ((*k).to_owned(), json!(v)))
                .collect();
            fixtures.push(json!({
                "file": spec.file,
                "source": spec.source,
                "codec_args": spec.codec,
                "tags": tags,
                "has_art": spec.art,
                "note": spec.note,
            }));
        }
    } else {
        eprintln!("ffmpeg not found: only WAV sources and the corrupt fixture were written");
    }

    let manifest = json!({
        "$description": "Generated by `cargo run -p gen-fixtures`. Do not edit.",
        "complete": complete,
        "gapless": {
            "split_frame": GAPLESS_SPLIT_FRAME,
            "frequency_hz": 1000.0,
            "amplitude": 0.5,
            "sample_rate": 44_100,
            "parts": ["gapless_a", "gapless_b"],
            "reference": "src/gapless_full.wav",
        },
        "sources": sources,
        "fixtures": fixtures,
    });
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    write(&out.join("manifest.json"), text.as_bytes())?;
    println!("wrote {} fixtures to {}", fixtures.len(), out.display());
    Ok(complete)
}
