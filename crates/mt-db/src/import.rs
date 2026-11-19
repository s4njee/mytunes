use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::artwork::{INSERT_ARTWORK, assign_album_artwork, source_priority};
use mt_meta::artwork::{ArtworkSource, TrackArtwork};
use mt_meta::scan::{
    CachedTrack, FileStamp, ReadTrack, ScanFileError, ScanProgress, WalkError, scan_tree,
};
use rusqlite::{Connection, TransactionBehavior, named_params};

/// Result of a committed library scan.
#[derive(Debug, Default)]
pub struct FolderImport {
    /// Number of tracks inserted or refreshed from a successful file read.
    pub read: usize,
    /// Number of files whose size and mtime matched the library.
    pub unchanged: usize,
    /// Number of existing tracks newly marked missing.
    pub marked_missing: usize,
    /// Files that could not be read. Existing rows are retained so they can be retried.
    pub file_errors: Vec<(PathBuf, ScanFileError)>,
    /// Inaccessible paths encountered during traversal.
    pub walk_errors: Vec<WalkError>,
}

/// Failure that prevented a folder scan from being committed.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// The selected directory could not be resolved or read.
    #[error("could not access scan folder {}", path.display())]
    Folder {
        /// Requested directory.
        path: PathBuf,
        /// Filesystem error.
        #[source]
        source: io::Error,
    },
    /// SQLite rejected the update.
    #[error("could not save folder scan to the library")]
    Database(#[from] rusqlite::Error),
}

/// Scan a folder and reconcile it with an open desktop library.
///
/// Progress callbacks run on the calling thread. Run this operation on a
/// background thread and use a callback that does not write to the library.
/// The scan holds a SQLite writer transaction to serialize competing imports,
/// and applies its updates together when the scan finishes.
///
/// # Errors
///
/// Returns an error if the root cannot be accessed or if the database
/// transaction fails. Individual file errors are returned in the scan report.
pub fn scan_library_folder(
    db: &mut Connection,
    folder: impl AsRef<Path>,
    on_progress: impl FnMut(ScanProgress),
) -> Result<FolderImport, ImportError> {
    let requested = folder.as_ref();
    let root = requested
        .canonicalize()
        .map_err(|source| ImportError::Folder {
            path: requested.to_owned(),
            source,
        })?;
    check_folder(&root)?;

    let transaction = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut existing = HashMap::new();
    let mut previous = HashMap::new();
    {
        let mut select = transaction.prepare(
            "SELECT file_path, size_bytes, file_mtime_ns, file_hash IS NOT NULL, missing,
                    coalesce(artwork_source_kind < 2 AND artwork_source_id IS NOT NULL, false)
             FROM tracks",
        )?;
        let mut rows = select.query([])?;
        while let Some(row) = rows.next()? {
            let path = PathBuf::from(row.get::<_, String>(0)?);
            if path == root || !path.starts_with(&root) {
                continue;
            }
            let size_bytes = row.get::<_, Option<i64>>(1)?;
            let mtime_ns = row.get::<_, Option<i64>>(2)?;
            let has_hash = row.get::<_, bool>(3)?;
            existing.insert(path.clone(), row.get::<_, bool>(4)?);
            if let (Some(size_bytes), Some(mtime_ns), true) = (size_bytes, mtime_ns, has_hash) {
                previous.insert(
                    path,
                    CachedTrack {
                        stamp: FileStamp {
                            size_bytes,
                            mtime_ns,
                        },
                        has_embedded_artwork: row.get(5)?,
                    },
                );
            }
        }
    }

    let known_artwork = {
        let mut statement = transaction.prepare("SELECT id FROM artwork")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<HashSet<_>>>()?
    };
    let mut stored = HashSet::new();
    let mut artwork_dirty = false;
    let mut report = FolderImport::default();

    {
        let mut upsert = transaction.prepare(UPSERT_TRACK)?;
        let mut insert_artwork = transaction.prepare(INSERT_ARTWORK)?;
        let mut refresh_folder_art = transaction.prepare(
            "UPDATE tracks SET artwork_source_id = ?1, artwork_source_kind = ?2
              WHERE file_path = ?3 AND coalesce(artwork_source_kind, ?4) = ?4
                AND artwork_source_id IS NOT ?1",
        )?;
        let mut set_missing = transaction
            .prepare("UPDATE tracks SET missing = ?1 WHERE file_path = ?2 AND missing <> ?1")?;
        let added_at_ms = SystemTime::UNIX_EPOCH
            .elapsed()
            .ok()
            .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok());
        report.walk_errors = scan_tree(
            &root,
            &previous,
            &known_artwork,
            on_progress,
            |file| -> rusqlite::Result<()> {
                let was_missing = existing.remove(&file.path) == Some(true);
                let artwork = file.artwork.as_deref();
                if let Some(cover) = artwork
                    && let Some(jpeg) = cover.thumbnails.as_ref()
                    && stored.insert(cover.id.clone())
                {
                    insert_artwork.execute((&cover.id, &jpeg.small, &jpeg.large))?;
                }
                match (file.result, file.path.to_str()) {
                    (Ok(Some(track)), Some(path)) => {
                        save_track(&mut upsert, path, &track, added_at_ms, artwork, true)?;
                        artwork_dirty = true;
                        report.read += 1;
                    }
                    (Ok(None), Some(path)) => {
                        if was_missing {
                            set_missing.execute((false, path))?;
                            artwork_dirty = true;
                        }
                        let folder_priority = source_priority(ArtworkSource::Folder);
                        let count = refresh_folder_art.execute((
                            artwork.map(|cover| cover.id.as_str()),
                            artwork.map(|_| folder_priority),
                            path,
                            folder_priority,
                        ))?;
                        artwork_dirty |= count != 0;
                        report.unchanged += 1;
                    }
                    (Err(error), _) => report.file_errors.push((file.path, error)),
                    (Ok(_), None) => report
                        .file_errors
                        .push((file.path, ScanFileError::NonUtf8Path)),
                }
                Ok(())
            },
        )?;

        check_folder(&root)?;
        if report.walk_errors.is_empty() {
            for (path, missing) in existing {
                if missing {
                    continue;
                }
                match missing_file(&path) {
                    Ok(true) => {
                        if let Some(path) = path.to_str() {
                            report.marked_missing += set_missing.execute((true, path))?;
                        }
                    }
                    Ok(false) => {}
                    Err(error) => report.file_errors.push((path, ScanFileError::Io(error))),
                }
            }
        }
    }

    if (artwork_dirty || report.marked_missing != 0)
        && (!known_artwork.is_empty() || !stored.is_empty())
    {
        assign_album_artwork(&transaction)?;
    }
    transaction.commit()?;
    Ok(report)
}

fn check_folder(path: &Path) -> Result<(), ImportError> {
    // Open the directory even if the metadata is cached. An unavailable root
    // should never produce an empty scan that marks its tracks missing.
    fs::read_dir(path)
        .map(|_| ())
        .map_err(|source| ImportError::Folder {
            path: path.to_owned(),
            source,
        })
}

fn missing_file(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(!metadata.is_file()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn save_track(
    statement: &mut rusqlite::Statement<'_>,
    path: &str,
    track: &ReadTrack,
    added_at_ms: Option<i64>,
    artwork: Option<&TrackArtwork>,
    advance_revision: bool,
) -> rusqlite::Result<()> {
    let tags = &track.metadata;
    let audio = &tags.audio;
    let duration_ms = i64::try_from(audio.duration_ms)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    statement.execute(named_params! {
        ":id": uuid::Uuid::now_v7().to_string(),
        ":path": path,
        ":hash": track.hash.as_slice(),
        ":size": track.stamp.size_bytes,
        ":mtime": track.stamp.mtime_ns,
        ":codec": audio.codec.as_str(),
        ":sample_rate": audio.sample_rate_hz,
        ":bit_depth": audio.bit_depth,
        ":bitrate": audio.bitrate_kbps,
        ":channels": audio.channels,
        ":duration_ms": duration_ms,
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
        ":sort_title": tags.sort_title,
        ":sort_artist": tags.sort_artist,
        ":sort_album": tags.sort_album,
        ":sort_album_artist": tags.sort_album_artist,
        ":added": added_at_ms,
        ":modified": track.stamp.mtime_ns.div_euclid(1_000_000),
        ":track_gain": tags.replay_gain.track_gain_db,
        ":album_gain": tags.replay_gain.album_gain_db,
        ":artwork_source_id": artwork.map(|cover| cover.id.as_str()),
        ":artwork_source_kind": artwork.map(|cover| source_priority(cover.source)),
        ":advance_revision": advance_revision,
    })?;
    Ok(())
}

pub(crate) const UPSERT_TRACK: &str = "
    INSERT INTO tracks (
        id, file_path, file_hash, size_bytes, file_mtime_ns, codec, sample_rate, bit_depth,
        bitrate, channels, duration_ms, title, artist, album_artist, album, composer, genre,
        year, track_no, track_total, disc_no, disc_total, compilation,
        sort_title, sort_artist, sort_album, sort_album_artist,
        date_added, date_modified, replaygain_track_db, replaygain_album_db,
        artwork_source_id, artwork_source_kind
    ) VALUES (
        :id, :path, :hash, :size, :mtime, :codec, :sample_rate, :bit_depth,
        :bitrate, :channels, :duration_ms, :title, :artist, :album_artist, :album, :composer, :genre,
        :year, :track_no, :track_total, :disc_no, :disc_total, :compilation,
        :sort_title, :sort_artist, :sort_album, :sort_album_artist,
        :added, :modified, :track_gain, :album_gain, :artwork_source_id, :artwork_source_kind
    ) ON CONFLICT(file_path) DO UPDATE SET
        file_rev = tracks.file_rev + (:advance_revision AND tracks.file_hash IS NOT NULL
            AND tracks.file_hash <> excluded.file_hash),
        file_hash = excluded.file_hash, size_bytes = excluded.size_bytes,
        file_mtime_ns = excluded.file_mtime_ns, codec = excluded.codec,
        sample_rate = excluded.sample_rate, bit_depth = excluded.bit_depth,
        bitrate = excluded.bitrate, channels = excluded.channels,
        duration_ms = excluded.duration_ms, title = excluded.title, artist = excluded.artist,
        album_artist = excluded.album_artist, album = excluded.album, composer = excluded.composer,
        genre = excluded.genre, year = excluded.year, track_no = excluded.track_no,
        track_total = excluded.track_total, disc_no = excluded.disc_no, disc_total = excluded.disc_total,
        compilation = excluded.compilation, sort_title = excluded.sort_title,
        sort_artist = excluded.sort_artist, sort_album = excluded.sort_album,
        sort_album_artist = excluded.sort_album_artist, date_modified = excluded.date_modified,
        replaygain_track_db = excluded.replaygain_track_db,
        replaygain_album_db = excluded.replaygain_album_db,
        artwork_source_id = excluded.artwork_source_id, artwork_source_kind = excluded.artwork_source_kind,
        missing = 0";
