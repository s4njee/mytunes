//! Typed reads for the desktop library, including ordered snapshots and page lookups.

mod tracks;

use std::path::{Path, PathBuf};

use mt_types::{PlaylistId, TrackId};
use rusqlite::{Connection, Row};

pub(crate) use tracks::ALBUM_DISPLAY_ARTIST;
pub use tracks::{SortDirection, TrackColumn, TrackFilter, TrackRow, TrackSearch, TrackSort};

/// An error reading a library query.
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    /// SQLite rejected the query or could not read its results.
    #[error("library query: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The input IDs could not be encoded for a page lookup.
    #[error("library page IDs: {0}")]
    PageIds(#[from] serde_json::Error),
}

/// Reads the desktop library using a connection returned by [`crate::open_desktop`].
pub struct LibraryQueries<'db> {
    db: &'db Connection,
}

impl<'db> LibraryQueries<'db> {
    /// Uses an already-open desktop library connection.
    pub fn new(db: &'db Connection) -> Self {
        Self { db }
    }

    /// Lists albums by title, with separate releases by artist and year.
    ///
    /// Compilations use “Various Artists”. Other albums fall back to the
    /// track artist if the album artist tag is absent.
    pub fn albums(&self) -> Result<Vec<AlbumGroup>, QueryError> {
        let sql = format!(
            "WITH tagged AS (
                SELECT album,
                       {display_artist} AS group_artist,
                       year, compilation,
                       {album_sort} AS album_sort,
                       {artist_sort} AS group_artist_sort
                  FROM tracks
                 WHERE trim(album) <> ''
            )
            SELECT album, group_artist, year, compilation, count(*)
              FROM tagged
             GROUP BY album, group_artist, year, compilation
             ORDER BY min(album_sort COLLATE mytunes_unicode_v1),
                      min(group_artist_sort COLLATE mytunes_unicode_v1), year,
                      album, group_artist, compilation",
            display_artist = ALBUM_DISPLAY_ARTIST,
            album_sort = tracks::ALBUM_SORT,
            artist_sort = tracks::ALBUM_GROUP_SORT,
        );
        let mut statement = self.db.prepare(&sql)?;
        let rows = statement.query_map([], |row| {
            Ok(AlbumGroup {
                title: row.get(0)?,
                artist: row.get(1)?,
                year: row.get(2)?,
                compilation: row.get(3)?,
                track_count: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Lists distinct nonblank artists, with track counts and artist sort overrides.
    pub fn artists(&self) -> Result<Vec<FacetCount>, QueryError> {
        self.facet("artist", tracks::ARTIST_SORT)
    }

    /// Lists distinct nonblank genres and their track counts.
    pub fn genres(&self) -> Result<Vec<FacetCount>, QueryError> {
        self.facet("genre", "genre COLLATE mytunes_unicode_v1")
    }

    /// Lists distinct nonblank composers and their track counts.
    pub fn composers(&self) -> Result<Vec<FacetCount>, QueryError> {
        self.facet("composer", "composer COLLATE mytunes_unicode_v1")
    }

    fn facet(&self, column: &str, sort_key: &str) -> Result<Vec<FacetCount>, QueryError> {
        let sql = format!(
            "SELECT {column}, count(*) FROM tracks
             WHERE trim({column}) <> ''
             GROUP BY {column}
             ORDER BY min({sort_key}), {column}"
        );
        let mut statement = self.db.prepare(&sql)?;
        let rows = statement.query_map([], |row| {
            Ok(FacetCount {
                name: row.get(0)?,
                track_count: row.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Lists available tracks under a canonical folder in album and disc order.
    ///
    /// The path comparison uses components, so sibling folders with similar
    /// names are excluded. Within releases tracks sort by disc and track tag,
    /// with the path breaking ties and ordering untagged files.
    pub fn folder_tracks(&self, root: &Path) -> Result<Vec<FolderTrack>, QueryError> {
        let sql = format!(
            "SELECT id, file_path, title,
                    CASE WHEN trim(album) <> '' THEN
                        json_array(album, {ALBUM_DISPLAY_ARTIST}, year, compilation)
                    END
             FROM tracks WHERE missing = 0
             ORDER BY {album_sort}, {group_sort}, year, album, {ALBUM_DISPLAY_ARTIST},
                      compilation, disc_no, track_no, file_path",
            album_sort = tracks::ALBUM_SORT,
            group_sort = tracks::ALBUM_GROUP_SORT,
        );
        let mut statement = self.db.prepare(&sql)?;
        let mut rows = statement.query([])?;
        let mut found = Vec::new();
        while let Some(row) = rows.next()? {
            let path = PathBuf::from(row.get::<_, String>(1)?);
            if path != root && path.starts_with(root) {
                found.push(FolderTrack {
                    id: read_track_id(row)?,
                    path,
                    title: row.get(2)?,
                    album_key: row.get(3)?,
                });
            }
        }
        Ok(found)
    }

    /// Returns stored playlist members in position order, including repeated tracks.
    pub fn playlist_track_ids(&self, playlist: &PlaylistId) -> Result<Vec<TrackId>, QueryError> {
        let mut statement = self.db.prepare_cached(
            "SELECT track_id FROM playlist_items WHERE playlist_id = ?1 ORDER BY position",
        )?;
        let rows = statement.query_map([&playlist.0], read_track_id)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

fn read_track_id(row: &Row<'_>) -> rusqlite::Result<TrackId> {
    Ok(TrackId(row.get(0)?))
}

/// A local file and its playback grouping from the desktop library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderTrack {
    /// Stable library track identifier.
    pub id: TrackId,
    /// Local file path.
    pub path: PathBuf,
    /// Track title, if known.
    pub title: Option<String>,
    /// Opaque album identity shared by tracks in the same release.
    pub album_key: Option<String>,
}

/// One album grouping in the library browser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlbumGroup {
    /// Display title from the album tag.
    pub title: String,
    /// Album artist, or the track artist when no album artist was tagged.
    pub artist: Option<String>,
    /// Release year when one is known.
    pub year: Option<i64>,
    /// Whether the group uses the compilation artist.
    pub compilation: bool,
    /// Number of tracks in this group.
    pub track_count: i64,
}

/// A library browser label and its membership count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacetCount {
    /// Original tag value.
    pub name: String,
    /// Number of tracks that have this value.
    pub track_count: i64,
}
