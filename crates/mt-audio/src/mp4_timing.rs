use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use symphonia::core::meta::{MetadataRevision, RawValue};

/// A range of PCM frames on the untrimmed AAC media timeline.
#[derive(Clone, Copy)]
pub(crate) struct PlaybackWindow {
    pub start: u64,
    pub frames: u64,
}

pub(crate) fn aac_window(
    mut file: File,
    track_id: u32,
    rate: u32,
    metadata: Option<&MetadataRevision>,
) -> Option<PlaybackWindow> {
    if let Some(window) = metadata.and_then(tagged_window) {
        return Some(window);
    }

    let (offset, len) = movie_box(&mut file)?;
    file.seek(SeekFrom::Start(offset)).ok()?;
    let movie = mp4parse::read_mp4(&mut file.take(len)).ok()?;
    let track = movie
        .tracks
        .iter()
        .find(|track| track.track_id == Some(track_id))?;
    let track_scale = track.timescale?.0;
    let movie_scale = movie.timescale?.0;
    let start = scale(track.media_time?.0, track_scale, rate)?;
    let edit_frames = scale(track.edited_duration?.0, movie_scale, rate)?;
    let media_end = scale(track.duration?.0, track_scale, rate)?;
    let media_frames = media_end.checked_sub(start)?;
    // Older muxers use a 1 kHz movie timescale and round the edit length.
    // The media timeline retains the original length in audio sample units.
    let rounding = u64::from(rate).div_ceil(movie_scale);
    let frames = if edit_frames.abs_diff(media_frames) <= rounding {
        media_frames
    } else {
        edit_frames.min(media_frames)
    };
    (frames > 0).then_some(PlaybackWindow { start, frames })
}

fn tagged_window(metadata: &MetadataRevision) -> Option<PlaybackWindow> {
    metadata.media.tags.iter().find_map(|tag| {
        let raw = &tag.raw;
        if !raw.key.rsplit(':').next()?.eq_ignore_ascii_case("iTunSMPB") {
            return None;
        }
        match &raw.value {
            RawValue::String(value) => parse_smpb(value),
            _ => None,
        }
    })
}

fn parse_smpb(value: &str) -> Option<PlaybackWindow> {
    let mut values = value.split_whitespace().skip(1);
    let mut hex = || u64::from_str_radix(values.next()?, 16).ok();
    let start = hex()?;
    let padding = hex()?;
    let frames = hex()?;
    let _total_encoded = start.checked_add(frames)?.checked_add(padding)?;
    (frames > 0).then_some(PlaybackWindow { start, frames })
}

fn scale(value: u64, units_per_second: u64, rate: u32) -> Option<u64> {
    let units = u128::from(units_per_second);
    let numerator = u128::from(value) * u128::from(rate);
    let frames = (numerator + units / 2).checked_div(units)?;
    u64::try_from(frames).ok()
}

// Jump over media data rather than streaming the entire file through the
// metadata parser. Limit this second parse to the single movie box.
fn movie_box(file: &mut File) -> Option<(u64, u64)> {
    let file_len = file.metadata().ok()?.len();
    while file.stream_position().ok()? < file_len {
        let offset = file.stream_position().ok()?;
        let mut header = [0; 8];
        file.read_exact(&mut header).ok()?;
        let (size, header_size) = match u32::from_be_bytes(header[..4].try_into().ok()?) {
            0 => (file_len - offset, 8),
            1 => {
                let mut extended = [0; 8];
                file.read_exact(&mut extended).ok()?;
                (u64::from_be_bytes(extended), 16)
            }
            size => (u64::from(size), 8),
        };
        let end = offset.checked_add(size)?;
        if size < header_size || end > file_len {
            return None;
        }
        if &header[4..] == b"moov" {
            return (size <= 64 * 1024 * 1024).then_some((offset, size));
        }
        file.seek(SeekFrom::Start(end)).ok()?;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aac_delay_and_original_pcm_length_from_itunsmpb() {
        let parsed = parse_smpb(" 00000000 00000840 00000243 00000000000204D7 00000000 ").unwrap();
        assert_eq!((parsed.start, parsed.frames), (2112, 132_311));
        assert!(parse_smpb("00000000 FFFFFFFFFFFFFFFF 00000001 00000001").is_none());
    }
}
