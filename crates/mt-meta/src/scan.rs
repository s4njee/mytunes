//! Folder traversal, parallel metadata extraction, and scan progress.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, Metadata};
use std::io::{self, BufReader, Seek};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use rayon::prelude::*;
/// A directory traversal failure with the affected path, when known.
pub use walkdir::Error as WalkError;
use walkdir::WalkDir;

use crate::artwork::{ArtworkCache, TrackArtwork};
use crate::metadata::read_track_from;
use crate::{MetadataError, TrackMetadata};

const SCAN_PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

/// File attributes that determine whether a rescan needs to read the contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    /// File size in bytes.
    pub size_bytes: i64,
    /// Last modification time in nanoseconds since the Unix epoch.
    pub mtime_ns: i64,
}

impl FileStamp {
    fn from_metadata(metadata: &Metadata) -> io::Result<Self> {
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected a regular file",
            ));
        }
        let modified = metadata.modified()?;
        let nanos = match modified.duration_since(UNIX_EPOCH) {
            Ok(duration) => i128::try_from(duration.as_nanos()),
            Err(error) => i128::try_from(error.duration().as_nanos()).map(|nanos| -nanos),
        };
        let out_of_range = |_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "file attributes exceed library range",
            )
        };
        Ok(Self {
            size_bytes: i64::try_from(metadata.len()).map_err(out_of_range)?,
            mtime_ns: i64::try_from(nanos.map_err(out_of_range)?).map_err(out_of_range)?,
        })
    }
}

/// Previously scanned attributes used to skip reads.
#[derive(Debug, Clone, Copy)]
pub struct CachedTrack {
    /// Attributes of the audio file.
    pub stamp: FileStamp,
    /// Whether the file already supplied usable embedded artwork.
    pub has_embedded_artwork: bool,
}

/// Current progress for a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanProgress {
    /// Number of supported files processed, including unchanged or unreadable files.
    pub scanned: usize,
    /// Number of supported paths discovered so far. Final once traversal completes.
    pub total_estimate: usize,
    /// Most recently discovered or processed path.
    pub current_path: PathBuf,
}

/// Metadata, fingerprint, and content hash read from the same file.
#[derive(Debug)]
pub struct ReadTrack {
    /// Audio properties and tags.
    pub metadata: TrackMetadata,
    /// Attributes captured during extraction.
    pub stamp: FileStamp,
    /// BLAKE3 digest of the file bytes.
    pub hash: [u8; 32],
}

/// Reason a supported path could not be imported.
#[derive(Debug, thiserror::Error)]
pub enum ScanFileError {
    /// The path was inaccessible or ceased to refer to a regular file.
    #[error("could not read file attributes or contents")]
    Io(#[from] io::Error),
    /// Metadata extraction failed.
    #[error(transparent)]
    Metadata(#[from] MetadataError),
    /// A file was modified while the scan read it. It can be retried on rescan.
    #[error("file changed during scan")]
    ChangedDuringRead,
    /// The path cannot be stored in the library's UTF-8 path field.
    #[error("library paths must be valid UTF-8")]
    NonUtf8Path,
}

/// Outcome of processing one supported file.
#[derive(Debug)]
pub struct ScannedFile {
    /// Absolute or root-relative path as yielded by the traversal.
    pub path: PathBuf,
    /// Fresh data, an unchanged file, or a typed error.
    pub result: Result<Option<ReadTrack>, ScanFileError>,
    /// Cover found for this file. An unchanged audio file only checks folder artwork.
    pub artwork: Option<Arc<TrackArtwork>>,
}

/// Scan supported regular files under a directory. Nested symbolic links are skipped.
///
/// Cached paths with the same size and modification time require only a stat.
/// Rayon processes candidates in parallel. The caller receives each file outcome,
/// with progress emitted at the start, every 250 ms during work, and at completion.
/// Both callbacks run on the calling thread.
///
/// # Errors
///
/// If the file callback fails, the scan stops and returns its error. On
/// success, the result lists traversal errors for paths that could not be inspected.
pub fn scan_tree<E>(
    root: &Path,
    previous: &HashMap<PathBuf, CachedTrack>,
    known_artwork: &HashSet<String>,
    mut on_progress: impl FnMut(ScanProgress),
    mut on_file: impl FnMut(ScannedFile) -> Result<(), E>,
) -> Result<Vec<WalkError>, E> {
    let discovered = Mutex::new((0, root.to_owned()));
    let mut progress = ScanProgress {
        scanned: 0,
        total_estimate: 0,
        current_path: root.to_owned(),
    };
    on_progress(progress.clone());
    thread::scope(|scope| {
        let (send, receive) = mpsc::sync_channel(64);
        let discovered = &discovered;
        let worker = scope.spawn(move || {
            let mut paths = Vec::new();
            let mut walk_errors = Vec::new();
            for entry in WalkDir::new(root).follow_root_links(false) {
                match entry {
                    Ok(entry) if entry.file_type().is_file() && supported(entry.path()) => {
                        let path = entry.into_path();
                        {
                            let mut found =
                                discovered.lock().unwrap_or_else(|err| err.into_inner());
                            *found = (paths.len() + 1, path.clone());
                        }
                        paths.push(path);
                    }
                    Ok(_) => {}
                    Err(error) => walk_errors.push(error),
                }
            }
            let cache = ArtworkCache::new(known_artwork);
            let _ = paths.into_par_iter().try_for_each(|path| {
                let cached = previous.get(&path);
                let (result, artwork) =
                    match inspect_file(&path, cached.map(|track| track.stamp), &cache) {
                        Ok((track, artwork)) => {
                            let unchanged_embedded = track.is_none()
                                && cached.is_some_and(|track| track.has_embedded_artwork);
                            let artwork = artwork.or_else(|| {
                                if unchanged_embedded {
                                    None
                                } else {
                                    cache.folder(&path)
                                }
                            });
                            (Ok(track), artwork)
                        }
                        Err(error) => (Err(error), None),
                    };
                send.send(ScannedFile {
                    path,
                    result,
                    artwork,
                })
                .map_err(|_| ())
            });
            walk_errors
        });

        let mut last_event = Instant::now();
        loop {
            let remaining = SCAN_PROGRESS_INTERVAL.saturating_sub(last_event.elapsed());
            match receive.recv_timeout(remaining) {
                Ok(file) => {
                    progress.current_path.clone_from(&file.path);
                    if let Err(error) = on_file(file) {
                        drop(receive);
                        return Err(error);
                    }
                    progress.scanned += 1;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            if last_event.elapsed() >= SCAN_PROGRESS_INTERVAL {
                update_estimate(discovered, &mut progress);
                on_progress(progress.clone());
                last_event = Instant::now();
            }
        }
        let errors = worker
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
        update_estimate(discovered, &mut progress);
        on_progress(progress);
        Ok(errors)
    })
}

fn update_estimate(discovered: &Mutex<(usize, PathBuf)>, progress: &mut ScanProgress) {
    let (total, last_found) = &*discovered.lock().unwrap_or_else(|err| err.into_inner());
    progress.total_estimate = *total;
    if progress.scanned == 0 {
        progress.current_path.clone_from(last_found);
    }
}

fn supported(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    [
        "mp3", "m4a", "m4p", "mp4", "alac", "flac", "wav", "wave", "aif", "aiff", "aifc", "ogg",
        "oga", "opus",
    ]
    .iter()
    .any(|supported| extension.eq_ignore_ascii_case(supported))
}

fn inspect_file(
    path: &Path,
    previous: Option<FileStamp>,
    cache: &ArtworkCache<'_>,
) -> Result<(Option<ReadTrack>, Option<Arc<TrackArtwork>>), ScanFileError> {
    if path.to_str().is_none() {
        return Err(ScanFileError::NonUtf8Path);
    }
    let stamp = FileStamp::from_metadata(&fs::symlink_metadata(path)?)?;
    if previous == Some(stamp) {
        return Ok((None, None));
    }

    let mut reader = BufReader::new(File::open(path)?);
    if FileStamp::from_metadata(&reader.get_ref().metadata()?)? != stamp {
        return Err(ScanFileError::ChangedDuringRead);
    }
    let (metadata, artwork) = match read_track_from(&mut reader, path, Some(cache)) {
        Err(MetadataError::InvalidAudio(_) | MetadataError::ParserFailure) => {
            read_track_from(&mut reader, path, None)?
        }
        result => result?,
    };
    reader.rewind()?;
    let mut hash = blake3::Hasher::new();
    hash.update_reader(&mut reader)?;
    if FileStamp::from_metadata(&reader.get_ref().metadata()?)? != stamp
        || FileStamp::from_metadata(&fs::symlink_metadata(path)?)? != stamp
    {
        return Err(ScanFileError::ChangedDuringRead);
    }
    Ok((
        Some(ReadTrack {
            metadata,
            stamp,
            hash: *hash.finalize().as_bytes(),
        }),
        artwork,
    ))
}
