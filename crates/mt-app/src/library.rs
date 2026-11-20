//! Desktop IPC use cases with bounded paging and immutable view membership.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use mt_audio::QueueItem;
use mt_db::queries::{
    BrowserFacets, LibraryQueries, TrackFilter, TrackRow, TrackSearch, TrackSort, ViewTrack,
};
use mt_db::{ArtworkSize, artwork_thumbnail, open_desktop};
use mt_types::{PlaylistId, TrackId};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    playback::{PlaybackChange, PlaybackSession},
    settings,
    window::{self, WindowSettings},
};

/// Maximum track rows in one response.
pub const PAGE_SIZE: u32 = 200;
const MAX_VIEWS: usize = 8;
const PLAYBACK_KEY: &str = "desktop.playback";

/// Errors returned by desktop commands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppError {
    /// A page request is outside the supported size.
    #[error("A library page must have between 1 and {maximum} tracks.")]
    PageLimit {
        /// Maximum supported page size.
        maximum: u32,
    },
    /// The frontend should open a new view after a library change or eviction.
    #[error("This library view expired. Open it again to refresh the list.")]
    ViewExpired,
    /// SQLite could not read or write the library.
    #[error("Could not access the music library.")]
    Library,
    /// A folder cannot be monitored for filesystem changes.
    #[error("Could not watch this folder.")]
    WatchFolder,
    /// A track's file is unavailable or the selection is stale.
    #[error("This track is unavailable.")]
    TrackUnavailable,
    /// Opening, decoding, or controlling the player failed.
    #[error("Could not play music.")]
    Playback,
    /// A desktop service could not handle the request.
    #[error("The desktop service is temporarily unavailable.")]
    Unavailable,
}

/// The source of a track view.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewSource {
    /// The whole music library.
    #[default]
    Library,
    /// A stored playlist in position order unless a sort is specified.
    Playlist {
        /// Stable playlist ID.
        playlist_id: PlaylistId,
    },
}

/// The source, sort, and filters used to build a library snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(default)]
pub struct ViewSpec {
    /// Library or playlist membership.
    pub source: ViewSource,
    /// Optional user sort. The library defaults to title order.
    pub sort: Option<TrackSort>,
    /// Literal prefix search across indexed tags.
    pub query: Option<String>,
    /// Selected genre in the column browser, if any.
    pub genre: Option<String>,
    /// Selected artist in the column browser, if any.
    pub artist: Option<String>,
    /// Limit to tagged compilations or ordinary tracks when an artist is selected.
    pub compilation: Option<bool>,
    /// Selected album in the column browser, if any.
    pub album: Option<String>,
}

impl ViewSpec {
    fn filter(&self) -> TrackFilter<'_> {
        let playlist = match &self.source {
            ViewSource::Library => None,
            ViewSource::Playlist { playlist_id } => Some(playlist_id),
        };
        TrackFilter {
            playlist,
            genre: self.genre.as_deref(),
            artist: self.artist.as_deref(),
            compilation: self.compilation,
            album: self.album.as_deref(),
        }
    }

    fn search(&self) -> TrackSearch<'_> {
        self.query
            .as_deref()
            .map_or(TrackSearch::All, TrackSearch::Prefix)
    }
}

/// Handle to an ordered snapshot retained in the backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct OpenedView {
    /// Opaque identifier used to page the sorted snapshot.
    pub view_id: String,
    /// Number of tracks in the snapshot.
    pub count: u32,
    /// Duration and file size of all rows in the view.
    pub totals: ViewTotals,
    /// Current library generation.
    pub revision: u32,
}

/// Combined duration and file size for a view or selection.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Type)]
pub struct ViewTotals {
    /// Sum of known durations in milliseconds.
    #[specta(type = specta_typescript::Number)]
    pub duration_ms: u64,
    /// Sum of known file sizes in bytes.
    #[specta(type = specta_typescript::Number)]
    pub size_bytes: u64,
}

impl ViewTotals {
    fn add(&mut self, track: &ViewTrack) {
        self.duration_ms = self.duration_ms.saturating_add(track.duration_ms);
        self.size_bytes = self.size_bytes.saturating_add(track.size_bytes);
    }
}

/// Statistics for the selected positions in a snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Type)]
pub struct SelectionStats {
    /// Number of distinct selected positions.
    pub count: u32,
    /// Duration and file size of the selected rows.
    pub totals: ViewTotals,
}

/// Result of removing tracks and optionally moving their files to Trash.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct RemovalOutcome {
    /// Files that could not be moved to Trash.
    pub trash_errors: u32,
}

struct ViewSnapshot {
    handle: OpenedView,
    ordered_tracks: Arc<[ViewTrack]>,
    positions: Vec<usize>,
}

impl ViewSnapshot {
    fn track(&self, index: u32) -> Result<&ViewTrack, AppError> {
        self.positions
            .get(index as usize)
            .map(|&position| &self.ordered_tracks[position])
            .ok_or(AppError::TrackUnavailable)
    }

    fn track_ids(&self, positions: &[usize]) -> Vec<TrackId> {
        positions
            .iter()
            .map(|&position| self.ordered_tracks[position].id.clone())
            .collect()
    }
}

struct LibraryState {
    db: Connection,
    base_order: Option<(ViewSpec, Arc<[ViewTrack]>)>,
    views: VecDeque<ViewSnapshot>,
    revision: u32,
    next_id: u64,
}

impl LibraryState {
    fn snapshot(&self, id: &str) -> Result<&ViewSnapshot, AppError> {
        self.views
            .iter()
            .find(|view| view.handle.view_id == id)
            .ok_or(AppError::ViewExpired)
    }

    fn invalidate(&mut self) -> Result<u32, AppError> {
        self.revision = self.revision.checked_add(1).ok_or(AppError::Unavailable)?;
        self.base_order = None;
        self.views.clear();
        Ok(self.revision)
    }
}

/// The open desktop library and its active views.
pub struct DesktopLibrary {
    state: Mutex<LibraryState>,
    browser: Mutex<Connection>,
}

impl DesktopLibrary {
    /// Open or initialize the SQLite library at a desktop app-data path.
    ///
    /// # Errors
    /// Returns a library error if SQLite cannot open or migrate the file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let path = path.as_ref();
        Ok(Self {
            state: Mutex::new(LibraryState {
                db: open_desktop(path).map_err(|_| AppError::Library)?,
                base_order: None,
                views: VecDeque::new(),
                revision: 0,
                next_id: 0,
            }),
            browser: Mutex::new(open_desktop(path).map_err(|_| AppError::Library)?),
        })
    }

    fn state(&self) -> Result<MutexGuard<'_, LibraryState>, AppError> {
        self.state.lock().map_err(|_| AppError::Unavailable)
    }

    /// Create a stable order of track IDs at the current library revision.
    ///
    /// # Errors
    /// Returns an error if the library cannot create the view.
    pub fn view_open(&self, spec: &ViewSpec) -> Result<OpenedView, AppError> {
        let mut state = self.state()?;
        let base = ViewSpec {
            source: spec.source.clone(),
            sort: if spec.source == ViewSource::Library {
                Some(spec.sort.unwrap_or_default())
            } else {
                spec.sort
            },
            ..ViewSpec::default()
        };
        if state
            .base_order
            .as_ref()
            .is_none_or(|(existing, _)| *existing != base)
        {
            let sorted = LibraryQueries::new(&state.db)
                .ordered_view_tracks(base.sort, base.filter())
                .map_err(|_| AppError::Library)?;
            state.base_order = Some((base, sorted.into()));
        }
        let matches = LibraryQueries::new(&state.db)
            .matching_row_ids(spec.search())
            .map_err(|_| AppError::Library)?;
        let (_, sorted) = state.base_order.as_ref().ok_or(AppError::Library)?;
        let ordered_tracks = Arc::clone(sorted);
        let mut positions = Vec::with_capacity(sorted.len());
        let mut totals = ViewTotals::default();
        let filter = spec.filter();
        let has_facets = filter.genre.is_some()
            || filter.artist.is_some()
            || filter.compilation.is_some()
            || filter.album.is_some();
        for (position, track) in sorted.iter().enumerate() {
            if (!has_facets || filter.matches_facets(track))
                && matches.as_ref().is_none_or(|set| set.contains(track.rowid))
            {
                positions.push(position);
                totals.add(track);
            }
        }
        let opened = OpenedView {
            view_id: state.next_id.to_string(),
            count: u32::try_from(positions.len()).map_err(|_| AppError::Library)?,
            totals,
            revision: state.revision,
        };
        state.next_id = state.next_id.checked_add(1).ok_or(AppError::Unavailable)?;
        if state.views.len() == MAX_VIEWS {
            state.views.pop_front();
        }
        state.views.push_back(ViewSnapshot {
            handle: opened.clone(),
            ordered_tracks,
            positions,
        });
        Ok(opened)
    }

    /// Return the progressively filtered browser choices.
    ///
    /// # Errors
    /// Returns an error if the browser cannot query the library.
    pub fn browser_facets(&self, spec: &ViewSpec) -> Result<BrowserFacets, AppError> {
        let mut db = self.browser.lock().map_err(|_| AppError::Unavailable)?;
        let snapshot = db.transaction().map_err(|_| AppError::Library)?;
        let facets = LibraryQueries::new(&snapshot)
            .browser_facets(spec.search(), spec.filter())
            .map_err(|_| AppError::Library)?;
        snapshot.commit().map_err(|_| AppError::Library)?;
        Ok(facets)
    }

    /// Read a bounded page in the snapshot's original track order.
    ///
    /// # Errors
    /// Returns an error if the view expired, the page is too large, or a row cannot be read.
    pub fn view_rows(
        &self,
        view_id: &str,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<TrackRow>, AppError> {
        if !(1..=PAGE_SIZE).contains(&limit) {
            return Err(AppError::PageLimit { maximum: PAGE_SIZE });
        }
        let mut state = self.state()?;
        let slot = state
            .views
            .iter()
            .position(|view| view.handle.view_id == view_id)
            .ok_or(AppError::ViewExpired)?;
        let view = state.views.remove(slot).ok_or(AppError::ViewExpired)?;
        let start = (offset as usize).min(view.positions.len());
        let end = start
            .saturating_add(limit as usize)
            .min(view.positions.len());
        let ids = view.track_ids(&view.positions[start..end]);
        let rows = LibraryQueries::new(&state.db)
            .track_page(&ids)
            .map_err(|_| AppError::Library);
        state.views.push_back(view);
        rows
    }

    /// Calculate statistics for the selected positions in the retained view.
    ///
    /// # Errors
    /// Returns an error if the snapshot expired or a position is out of range.
    pub fn selection_stats(
        &self,
        view_id: &str,
        indexes: &[u32],
    ) -> Result<SelectionStats, AppError> {
        let state = self.state()?;
        let view = state.snapshot(view_id)?;
        let mut seen = HashSet::new();
        let mut stats = SelectionStats::default();
        for &index in indexes {
            if seen.insert(index) {
                stats.totals.add(view.track(index)?);
                stats.count += 1;
            }
        }
        Ok(stats)
    }

    /// Invalidate views after a library change and return the new revision.
    ///
    /// # Errors
    /// Returns an error if the library cannot be updated.
    pub fn invalidate_views(&self) -> Result<u32, AppError> {
        self.state()?.invalidate()
    }

    /// Build a playback queue from a view, skipping unavailable files.
    ///
    /// # Errors
    /// Returns an error if the view expired or the requested track is unavailable.
    pub fn playback_queue(
        &self,
        view_id: &str,
        index: u32,
    ) -> Result<(Vec<QueueItem>, usize), AppError> {
        let state = self.state()?;
        let snapshot = state.snapshot(view_id)?;
        resolve_queue(&state.db, &snapshot.track_ids(&snapshot.positions), index)
    }

    /// Rebuild the saved queue using the current file locations.
    ///
    /// # Errors
    /// Returns an error if the saved track is no longer available.
    pub fn restore_queue(
        &self,
        session: &PlaybackSession,
    ) -> Result<(Vec<QueueItem>, usize), AppError> {
        resolve_queue(&self.state()?.db, &session.queue, session.index)
    }

    /// Read the saved queue and transport state.
    ///
    /// # Errors
    /// Returns a library error if the saved state cannot be read.
    pub fn playback_session(&self) -> Result<PlaybackSession, AppError> {
        settings::load(&self.state()?.db, PLAYBACK_KEY)
    }

    /// Persist playback state at shutdown.
    ///
    /// # Errors
    /// Returns a library error if the state cannot be saved.
    pub fn save_playback_session(&self, session: &PlaybackSession) -> Result<(), AppError> {
        settings::save(&self.state()?.db, PLAYBACK_KEY, session)
    }

    /// Update the counters for the track and invalidate displayed changes.
    ///
    /// # Errors
    /// Returns a library error if SQLite cannot record the counters.
    pub fn record_playback(
        &self,
        id: &TrackId,
        change: PlaybackChange,
    ) -> Result<Option<u32>, AppError> {
        let mut state = self.state()?;
        let now = std::time::UNIX_EPOCH
            .elapsed()
            .ok()
            .and_then(|duration| i64::try_from(duration.as_millis()).ok());
        let updated = state
            .db
            .execute(
                "UPDATE tracks SET play_count = play_count + ?1, skip_count = skip_count + ?2,
                last_played_at = CASE WHEN ?1 AND ?3 IS NOT NULL THEN ?3 ELSE last_played_at END
             WHERE id = ?4",
                (change.play, change.skip, now, &id.0),
            )
            .map_err(|_| AppError::Library)?;
        if change.play && updated > 0 {
            state.invalidate().map(Some)
        } else {
            Ok(None)
        }
    }

    /// Read the current metadata of a track by its library identifier.
    ///
    /// # Errors
    /// Returns an error if the track is no longer present.
    pub fn track(&self, id: &TrackId) -> Result<TrackRow, AppError> {
        let state = self.state()?;
        LibraryQueries::new(&state.db)
            .track_page(std::slice::from_ref(id))
            .map_err(|_| AppError::Library)?
            .pop()
            .ok_or(AppError::TrackUnavailable)
    }

    /// Resolve a track's local path for the native file manager.
    ///
    /// # Errors
    /// Returns an error if the track or its file is unavailable.
    pub fn file_to_reveal(&self, id: &TrackId) -> Result<PathBuf, AppError> {
        let state = self.state()?;
        LibraryQueries::new(&state.db)
            .playback_tracks(std::slice::from_ref(id))
            .map_err(|_| AppError::Library)?
            .pop()
            .map(|(_, track)| track.path)
            .filter(|path| path.is_file())
            .ok_or(AppError::TrackUnavailable)
    }

    /// Set a track's inclusion in sync and invalidate open views.
    ///
    /// # Errors
    /// Returns an error if the track is gone or the update fails.
    pub fn set_checked(&self, id: &TrackId, checked: bool) -> Result<u32, AppError> {
        let mut state = self.state()?;
        let changed = state
            .db
            .execute(
                "UPDATE tracks SET checked = ?1 WHERE id = ?2",
                (checked, &id.0),
            )
            .map_err(|_| AppError::Library)?;
        if changed == 0 {
            return Err(AppError::TrackUnavailable);
        }
        state.invalidate()
    }

    /// Remove the selected snapshot positions. Move available files to Trash if requested.
    ///
    /// # Errors
    /// Returns an error if the view expired, a position is invalid, or SQLite rejects the delete.
    pub fn remove_tracks(
        &self,
        view_id: &str,
        indexes: &[u32],
        trash_files: bool,
    ) -> Result<(u32, RemovalOutcome), AppError> {
        let (revision, paths) = {
            let mut state = self.state()?;
            let snapshot = state.snapshot(view_id)?;
            let ids = indexes
                .iter()
                .map(|&index| snapshot.track(index).map(|track| &track.id))
                .collect::<Result<HashSet<_>, _>>()?;
            let encoded = serde_json::to_string(&ids).map_err(|_| AppError::Library)?;
            let transaction = state.db.transaction().map_err(|_| AppError::Library)?;
            let paths = {
                let mut delete = transaction.prepare(
                    "DELETE FROM tracks WHERE id IN (SELECT value FROM json_each(?1)) RETURNING file_path"
                ).map_err(|_| AppError::Library)?;
                let rows = delete
                    .query_map([encoded], |row| row.get::<_, String>(0))
                    .map_err(|_| AppError::Library)?;
                rows.collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(|_| AppError::Library)?
            };
            transaction.commit().map_err(|_| AppError::Library)?;
            (state.invalidate()?, paths)
        };
        let mut trash_errors = 0u32;
        if trash_files {
            for path in paths {
                if !std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file()) {
                    trash_errors = trash_errors.saturating_add(1);
                    continue;
                }
                if let Err(error) = trash::delete(&path) {
                    eprintln!("could not move {path} to Trash: {error}");
                    trash_errors = trash_errors.saturating_add(1);
                }
            }
        }
        Ok((revision, RemovalOutcome { trash_errors }))
    }

    /// Read the saved desktop layout and appearance.
    ///
    /// # Errors
    /// Returns an error when the library cannot read its settings.
    pub fn window_settings(&self) -> Result<WindowSettings, AppError> {
        window::load(&self.state()?.db)
    }

    /// Persist the desktop layout and return the stored settings.
    ///
    /// # Errors
    /// Returns an error when the library cannot save its settings.
    pub fn save_window_settings(
        &self,
        settings: WindowSettings,
    ) -> Result<WindowSettings, AppError> {
        window::save(&self.state()?.db, settings)
    }

    /// Read an artwork JPEG by content digest and size.
    ///
    /// # Errors
    /// Returns a library error if the thumbnail cannot be read.
    pub fn artwork(&self, id: &str, size: ArtworkSize) -> Result<Option<Vec<u8>>, AppError> {
        artwork_thumbnail(&self.state()?.db, id, size).map_err(|_| AppError::Library)
    }
}

fn resolve_queue(
    db: &Connection,
    ids: &[TrackId],
    index: u32,
) -> Result<(Vec<QueueItem>, usize), AppError> {
    if index as usize >= ids.len() {
        return Err(AppError::TrackUnavailable);
    }
    let tracks = LibraryQueries::new(db)
        .playback_tracks(ids)
        .map_err(|_| AppError::Library)?;
    let start = tracks
        .iter()
        .position(|(position, _)| *position == index)
        .ok_or(AppError::TrackUnavailable)?;
    let queue = tracks
        .into_iter()
        .map(|(_, track)| QueueItem {
            id: track.id,
            path: track.path,
            album: track.album_key,
        })
        .collect();
    Ok((queue, start))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library_with_tracks() -> (tempfile::TempDir, DesktopLibrary) {
        let dir = tempfile::tempdir().expect("create temporary library directory");
        let library =
            DesktopLibrary::open(dir.path().join("library.sqlite")).expect("open test library");
        library
            .state()
            .expect("lock test library")
            .db
            .execute_batch(
                "INSERT INTO tracks (id, file_path, title, artist, album, genre, compilation) VALUES
                 ('b', '/b', 'Blue Signal', 'The Makers', 'Coast', 'Electronic', 0),
                 ('a', '/a', 'Afterimage', 'The Makers', 'Coast', 'Electronic', 0),
                 ('c', '/c', 'Blue Room', 'Guests', 'Collections', 'Jazz', 1);
                 INSERT INTO playlists (id, name, kind) VALUES ('recent', 'Recent', 'manual');
                 INSERT INTO playlist_items (playlist_id, position, track_id) VALUES
                 ('recent', 0, 'b'), ('recent', 1, 'c'), ('recent', 2, 'b'), ('recent', 3, 'a');",
            )
            .expect("seed test library");
        (dir, library)
    }

    fn track_ids(rows: Vec<TrackRow>) -> Vec<String> {
        rows.into_iter().map(|row| row.id.0).collect()
    }

    #[test]
    fn totals_follow_the_filtered_snapshot_and_count_selected_positions() {
        let (_dir, library) = library_with_tracks();
        library
            .state()
            .expect("lock library")
            .db
            .execute_batch(
                "UPDATE tracks SET duration_ms = 120000, size_bytes = 1000000 WHERE id = 'b';
                 UPDATE tracks SET duration_ms = 60000, size_bytes = 2000000 WHERE id = 'c';
                 UPDATE tracks SET size_bytes = -1 WHERE id = 'a';",
            )
            .expect("seed durations and sizes");
        let all = library
            .view_open(&ViewSpec::default())
            .expect("open library");
        assert_eq!(all.count, 3);
        assert_eq!(
            all.totals,
            ViewTotals {
                duration_ms: 180_000,
                size_bytes: 3_000_000
            }
        );

        let filtered = library
            .view_open(&ViewSpec {
                source: ViewSource::Playlist {
                    playlist_id: PlaylistId("recent".into()),
                },
                query: Some("blue".into()),
                ..ViewSpec::default()
            })
            .expect("open filtered playlist");
        assert_eq!(filtered.count, 3);
        assert_eq!(
            filtered.totals,
            ViewTotals {
                duration_ms: 300_000,
                size_bytes: 4_000_000
            }
        );
        let selected = library
            .selection_stats(&filtered.view_id, &[0, 0, 2])
            .expect("read selection");
        assert_eq!(selected.count, 2);
        assert_eq!(
            selected.totals,
            ViewTotals {
                duration_ms: 240_000,
                size_bytes: 2_000_000
            }
        );
        assert_eq!(
            library.selection_stats(&filtered.view_id, &[3]),
            Err(AppError::TrackUnavailable)
        );
        library.invalidate_views().expect("invalidate the snapshot");
        assert_eq!(
            library.selection_stats(&filtered.view_id, &[0]),
            Err(AppError::ViewExpired)
        );
    }

    #[test]
    fn browser_refines_facets_and_distinguishes_tagged_compilations() {
        let (_dir, library) = library_with_tracks();
        library
            .state()
            .expect("lock library")
            .db
            .execute(
                "INSERT INTO tracks (id, file_path, title, artist, album, genre)
                      VALUES ('d', '/d', 'Blue Lantern', 'Compilations', 'Light', 'Jazz')",
                [],
            )
            .expect("add an artist whose name matches the compilation group");
        let spec = ViewSpec {
            query: Some("blue".into()),
            genre: Some("Jazz".into()),
            compilation: Some(true),
            ..ViewSpec::default()
        };
        let facets = library.browser_facets(&spec).expect("load browser facets");
        assert_eq!(
            facets
                .genres
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            ["Electronic", "Jazz"]
        );
        assert_eq!(
            facets
                .artists
                .iter()
                .map(|item| (&*item.name, item.compilation))
                .collect::<Vec<_>>(),
            [("Compilations", true), ("Compilations", false)]
        );
        assert_eq!(
            facets
                .albums
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            ["Collections"]
        );
        let view = library.view_open(&spec).expect("open the filtered view");
        assert_eq!(
            track_ids(library.view_rows(&view.view_id, 0, 3).expect("fetch page")),
            ["c"]
        );
    }

    #[test]
    fn queues_and_removals_use_the_same_snapshot_positions() {
        let (dir, library) = library_with_tracks();
        let file = dir.path().join("available.wav");
        std::fs::write(&file, b"test file").expect("create library file");
        {
            let state = library.state().expect("lock test library");
            state
                .db
                .execute("UPDATE tracks SET missing = 1 WHERE id = 'c'", [])
                .expect("mark track missing");
            state
                .db
                .execute(
                    "UPDATE tracks SET file_path = ?1 WHERE id = 'b'",
                    [file.to_str()],
                )
                .expect("attach local file");
        }
        let view = library
            .view_open(&ViewSpec::default())
            .expect("open sorted tracks");
        let (queue, start) = library
            .playback_queue(&view.view_id, 2)
            .expect("make playback queue");
        assert_eq!(start, 1);
        assert_eq!(
            queue
                .iter()
                .map(|item| item.id.0.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(queue[start].path, file);
        assert_eq!(
            library.playback_queue(&view.view_id, 1),
            Err(AppError::TrackUnavailable)
        );

        assert_eq!(
            library.remove_tracks(&view.view_id, &[60], false),
            Err(AppError::TrackUnavailable)
        );
        let (_, removed) = library
            .remove_tracks(&view.view_id, &[2, 2], false)
            .expect("remove track once");
        assert_eq!(removed.trash_errors, 0);
        assert!(file.is_file());
        assert_eq!(
            library.track(&TrackId("b".into())),
            Err(AppError::TrackUnavailable)
        );
        assert_eq!(
            library.view_rows(&view.view_id, 0, 3),
            Err(AppError::ViewExpired)
        );
    }

    #[test]
    fn view_snapshots_keep_order_and_compose_search_with_playlist_and_facets() {
        let (_dir, library) = library_with_tracks();
        let whole = library
            .view_open(&ViewSpec::default())
            .expect("open library");
        assert_eq!(whole.count, 3);
        assert_eq!(whole.revision, 0);
        assert_eq!(
            track_ids(library.view_rows(&whole.view_id, 1, 2).expect("fetch page")),
            ["c", "b"]
        );
        assert!(
            library
                .view_rows(&whole.view_id, whole.count, PAGE_SIZE)
                .expect("fetch empty page")
                .is_empty()
        );

        let playlist = ViewSource::Playlist {
            playlist_id: PlaylistId("recent".into()),
        };
        let view = library
            .view_open(&ViewSpec {
                source: playlist.clone(),
                query: Some("blu".into()),
                ..ViewSpec::default()
            })
            .expect("open searched playlist");
        assert_eq!(
            track_ids(
                library
                    .view_rows(&view.view_id, 0, 3)
                    .expect("fetch playlist")
            ),
            ["b", "c", "b"]
        );

        let view = library
            .view_open(&ViewSpec {
                source: playlist,
                query: Some("blue".into()),
                artist: Some("Various Artists".into()),
                genre: Some("Jazz".into()),
                album: Some("Collections".into()),
                ..ViewSpec::default()
            })
            .expect("open filtered playlist");
        assert_eq!(view.count, 1);
        assert_eq!(
            track_ids(
                library
                    .view_rows(&view.view_id, 0, 3)
                    .expect("fetch filter")
            ),
            ["c"]
        );
    }

    #[test]
    fn view_cache_is_bounded_and_library_revisions_expire_existing_handles() {
        let (_dir, library) = library_with_tracks();
        let open = || {
            library
                .view_open(&ViewSpec::default())
                .expect("open library")
        };
        let first = open();
        let second = open();
        for _ in 2..MAX_VIEWS {
            open();
        }
        library
            .view_rows(&first.view_id, 0, 1)
            .expect("keep first recent");
        open();
        assert_eq!(
            library.view_rows(&second.view_id, 0, 1),
            Err(AppError::ViewExpired)
        );
        assert_eq!(
            library.view_rows(&first.view_id, 0, 201),
            Err(AppError::PageLimit { maximum: PAGE_SIZE })
        );
        assert_eq!(library.invalidate_views(), Ok(1));
        assert_eq!(
            library.view_rows(&first.view_id, 0, 1),
            Err(AppError::ViewExpired)
        );
        let current = open();
        assert_eq!(current.revision, 1);
        assert_ne!(first.view_id, current.view_id);
    }
}
