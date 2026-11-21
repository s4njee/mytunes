//! Build a development library from local audio selections.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use mt_meta::scan::{ReadTrack, is_supported_audio_path, scan_paths};
use rusqlite::{Connection, Statement, TransactionBehavior, named_params};

use crate::artwork::INSERT_ARTWORK;
use crate::import::{PendingCopies, copy_media};

/// Results of adding media to a development device library.
#[derive(Debug, Default)]
pub struct SeedReport {
    /// New tracks and restored media files.
    pub imported: u32,
    /// Previously seeded tracks with intact media.
    pub unchanged: u32,
    /// Inputs that could not be read or copied.
    pub errors: Vec<String>,
}

/// Inspect selected files or folders and copy usable audio beside the device database.
///
/// The caller owns the device connection and supplies its parent directory.
/// Reimporting identical content with the same extension keeps its track ID.
///
/// # Errors
/// Returns a SQLite error if the batch cannot be committed. Newly copied media
/// is cleaned up when the transaction rolls back.
pub fn seed_device(
    db: &mut Connection,
    root: &Path,
    sources: &[PathBuf],
) -> rusqlite::Result<SeedReport> {
    let mut report = SeedReport::default();
    let mut sources_for_scan = Vec::new();
    for source in sources {
        match source.canonicalize() {
            Ok(path) if path.is_dir() || (path.is_file() && is_supported_audio_path(&path)) => {
                sources_for_scan.push(path);
            }
            Ok(_) => report
                .errors
                .push(format!("{}: unsupported audio file", source.display())),
            Err(error) => report.errors.push(format!("{}: {error}", source.display())),
        }
    }

    let transaction = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut copied = PendingCopies::default();
    let known_artwork = {
        let mut query = transaction.prepare("SELECT id FROM artwork")?;
        query
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<HashSet<_>>>()?
    };
    let added_at = UNIX_EPOCH
        .elapsed()
        .ok()
        .and_then(|time| i64::try_from(time.as_millis()).ok());
    let library_music = root.join("music");
    if fs::symlink_metadata(&library_music).is_ok_and(|entry| !entry.is_dir()) {
        report.errors.push(format!(
            "{}: expected a media directory within the device library",
            library_music.display()
        ));
        return Ok(report);
    }
    {
        let mut artwork = transaction.prepare(INSERT_ARTWORK)?;
        let mut insert = transaction.prepare(INSERT_TRACK)?;
        let mut exists =
            transaction.prepare("SELECT EXISTS(SELECT 1 FROM tracks WHERE rel_path = ?1)")?;
        let walk_errors = scan_paths(
            &sources_for_scan,
            &HashMap::new(),
            &known_artwork,
            |path| !path.starts_with(&library_music),
            |_| {},
            |file| -> rusqlite::Result<()> {
                let mut track = match file.result {
                    Ok(Some(track)) => track,
                    Ok(None) => return Ok(()),
                    Err(error) => {
                        report
                            .errors
                            .push(format!("{}: {error}", file.path.display()));
                        return Ok(());
                    }
                };
                let Some(extension) = file.path.extension().and_then(|ext| ext.to_str()) else {
                    return Ok(());
                };
                let relative = seeded_path(blake3::Hash::from_bytes(track.hash), extension);
                let path = root.join(&relative);
                let new_file = match copy_media(&file.path, &path, &mut track) {
                    Ok(created) => created,
                    Err(error) => {
                        report
                            .errors
                            .push(format!("{}: {error}", file.path.display()));
                        return Ok(());
                    }
                };
                if new_file {
                    copied.0.push(path);
                }
                if exists.query_row([&relative], |row| row.get::<_, bool>(0))? {
                    if new_file {
                        report.imported += 1;
                    } else {
                        report.unchanged += 1;
                    }
                    return Ok(());
                }
                if let Some(cover) = file.artwork.as_deref()
                    && let Some(jpegs) = &cover.thumbnails
                {
                    artwork.execute((&cover.id, &jpegs.small, &jpegs.large))?;
                }
                let cover = file.artwork.as_deref().map(|cover| cover.id.as_str());
                insert_track(&mut insert, &relative, &track, cover, added_at)?;
                report.imported += 1;
                Ok(())
            },
        )?;
        report
            .errors
            .extend(walk_errors.into_iter().map(|error| error.to_string()));
    }
    transaction.execute_batch("COMMIT")?;
    copied.0.clear();
    Ok(report)
}

fn seeded_path(hash: blake3::Hash, extension: &str) -> String {
    let hash = hash.to_hex();
    format!(
        "music/{}/{hash}.{}",
        &hash[..2],
        extension.to_ascii_lowercase()
    )
}

const INSERT_TRACK: &str = "INSERT INTO tracks (
    id, rel_path, size_bytes, codec, sample_rate, bit_depth, bitrate, channels, duration_ms,
    title, artist, album_artist, album, composer, genre, year, track_no, track_total,
    disc_no, disc_total, compilation, date_added, replaygain_track_db, replaygain_album_db, artwork_id
) VALUES (
    :id, :path, :size, :codec, :sample_rate, :bit_depth, :bitrate, :channels, :duration,
    :title, :artist, :album_artist, :album, :composer, :genre, :year, :track_no, :track_total,
    :disc_no, :disc_total, :compilation, :added, :track_gain, :album_gain, :artwork
)";

fn insert_track(
    query: &mut Statement<'_>,
    relative: &str,
    track: &ReadTrack,
    artwork: Option<&str>,
    added_at: Option<i64>,
) -> rusqlite::Result<()> {
    let tags = &track.metadata;
    let audio = &tags.audio;
    let duration = i64::try_from(audio.duration_ms)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    query.execute(named_params! {
        ":id": uuid::Uuid::now_v7().to_string(),
        ":path": relative,
        ":size": track.stamp.size_bytes,
        ":codec": audio.codec.as_str(),
        ":sample_rate": audio.sample_rate_hz,
        ":bit_depth": audio.bit_depth,
        ":bitrate": audio.bitrate_kbps,
        ":channels": audio.channels,
        ":duration": duration,
        ":title": tags.title,
        ":artist": tags.artist,
        ":album_artist": tags.album_artist,
        ":album": tags.album,
        ":composer": tags.composer,
        ":genre": tags.genre,
        ":year": tags.year,
        ":track_no": tags.track_number,
        ":track_total": tags.track_total,
        ":disc_no": tags.disc_number,
        ":disc_total": tags.disc_total,
        ":compilation": tags.compilation.unwrap_or_default(),
        ":added": added_at,
        ":track_gain": tags.replay_gain.track_gain_db,
        ":album_gain": tags.replay_gain.album_gain_db,
        ":artwork": artwork,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gen_fixtures::{sine, wav_bytes};
    use std::fs;

    #[test]
    fn a_failed_commit_removes_only_the_files_created_by_the_import() {
        let inputs = tempfile::tempdir().expect("make input directory");
        let target = tempfile::tempdir().expect("make device directory");
        let root = target
            .path()
            .canonicalize()
            .expect("resolve device directory");
        let mut db = crate::open_device(root.join("device.sqlite")).expect("open device DB");
        let tone = |frequency| wav_bytes(&sine(44_100, 16, &[frequency], 0.1, 0.5));
        let retained = tone(440.0);
        let rejected = tone(220.0);
        let existing_source = inputs.path().join("Existing.wav");
        let new_source = inputs.path().join("Rejected.wav");
        fs::write(&existing_source, &retained).expect("write existing audio input");
        fs::write(&new_source, &rejected).expect("write rejected audio input");
        seed_device(&mut db, &root, &[existing_source]).expect("seed initial track");
        db.execute_batch(
            "CREATE TRIGGER fail_import BEFORE INSERT ON tracks
             BEGIN SELECT RAISE(ABORT, 'test rejected insert'); END;",
        )
        .expect("reject new database row");

        assert!(seed_device(&mut db, &root, &[new_source]).is_err());
        let path = |bytes: &[u8]| root.join(seeded_path(blake3::hash(bytes), "wav"));
        assert!(!path(&rejected).exists());
        assert_eq!(
            fs::read(path(&retained)).expect("read original copy"),
            retained
        );
        let count: u32 = db
            .query_row("SELECT count(*) FROM tracks", [], |row| row.get(0))
            .expect("count retained tracks");
        assert_eq!(count, 1);
    }
}
