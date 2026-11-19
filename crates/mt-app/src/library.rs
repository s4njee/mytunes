//! Desktop IPC use cases with bounded paging and immutable view membership.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use mt_db::queries::{LibraryQueries, TrackFilter, TrackRow, TrackSearch, TrackSort};
use mt_db::{ArtworkSize, artwork_thumbnail, open_desktop};
use mt_types::{PlaylistId, TrackId};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Maximum track rows in one response.
pub const PAGE_SIZE: u32 = 200;
const MAX_VIEWS: usize = 8;

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
    /// A desktop service could not handle the request.
    #[error("The desktop service is temporarily unavailable.")]
    Unavailable,
}

/// The source of a track view.
#[derive(Clone, Debug, Default, Deserialize, Serialize, Type)]
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
#[derive(Clone, Debug, Default, Deserialize, Serialize, Type)]
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
    /// Selected album in the column browser, if any.
    pub album: Option<String>,
}

/// Handle to an ordered snapshot retained in the backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct OpenedView {
    /// Opaque identifier used to page the sorted snapshot.
    pub view_id: String,
    /// Number of tracks in the snapshot.
    pub count: u32,
    /// Current library generation.
    pub revision: u32,
}

struct ViewSnapshot {
    handle: OpenedView,
    ids: Vec<TrackId>,
}

struct LibraryState {
    db: Connection,
    views: VecDeque<ViewSnapshot>,
    revision: u32,
    next_id: u64,
}

/// The open desktop library and its active views.
pub struct DesktopLibrary(Mutex<LibraryState>);

impl DesktopLibrary {
    /// Open or initialize the SQLite library at a desktop app-data path.
    ///
    /// # Errors
    /// Returns a library error if SQLite cannot open or migrate the file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        Ok(Self(Mutex::new(LibraryState {
            db: open_desktop(path).map_err(|_| AppError::Library)?,
            views: VecDeque::new(),
            revision: 0,
            next_id: 0,
        })))
    }

    fn state(&self) -> Result<MutexGuard<'_, LibraryState>, AppError> {
        self.0.lock().map_err(|_| AppError::Unavailable)
    }

    /// Create a stable order of track IDs at the current library revision.
    ///
    /// # Errors
    /// Returns an error if the library cannot create the view.
    pub fn view_open(&self, spec: &ViewSpec) -> Result<OpenedView, AppError> {
        let mut state = self.state()?;
        let playlist = match &spec.source {
            ViewSource::Library => None,
            ViewSource::Playlist { playlist_id } => Some(playlist_id),
        };
        let filter = TrackFilter {
            playlist,
            genre: spec.genre.as_deref(),
            artist: spec.artist.as_deref(),
            album: spec.album.as_deref(),
        };
        let ids = LibraryQueries::new(&state.db)
            .track_ids_in(
                spec.sort,
                spec.query
                    .as_deref()
                    .map_or(TrackSearch::All, TrackSearch::Prefix),
                filter,
            )
            .map_err(|_| AppError::Library)?;
        let opened = OpenedView {
            view_id: state.next_id.to_string(),
            count: u32::try_from(ids.len()).map_err(|_| AppError::Library)?,
            revision: state.revision,
        };
        state.next_id = state.next_id.checked_add(1).ok_or(AppError::Unavailable)?;
        if state.views.len() == MAX_VIEWS {
            state.views.pop_front();
        }
        state.views.push_back(ViewSnapshot {
            handle: opened.clone(),
            ids,
        });
        Ok(opened)
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
        let start = (offset as usize).min(view.ids.len());
        let end = start.saturating_add(limit as usize).min(view.ids.len());
        let rows = LibraryQueries::new(&state.db)
            .track_page(&view.ids[start..end])
            .map_err(|_| AppError::Library);
        state.views.push_back(view);
        rows
    }

    /// Invalidate views after a library change and return the new revision.
    ///
    /// # Errors
    /// Returns an error if the library cannot be updated.
    pub fn invalidate_views(&self) -> Result<u32, AppError> {
        let mut state = self.state()?;
        state.revision = state.revision.checked_add(1).ok_or(AppError::Unavailable)?;
        state.views.clear();
        Ok(state.revision)
    }

    /// Read an artwork JPEG by content digest and size.
    ///
    /// # Errors
    /// Returns a library error if the thumbnail cannot be read.
    pub fn artwork(&self, id: &str, size: ArtworkSize) -> Result<Option<Vec<u8>>, AppError> {
        artwork_thumbnail(&self.state()?.db, id, size).map_err(|_| AppError::Library)
    }
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
