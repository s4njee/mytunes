use std::{borrow::Cow, collections::HashSet};

use mt_types::{PlaylistId, TrackId};
use rusqlite::Row;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{LibraryQueries, QueryError, read_track_id};

// Keep the indexed keys in sync with the V2 migration.
const TITLE_SORT: &str =
    "coalesce(nullif(tracks.sort_title, ''), tracks.title) COLLATE mytunes_unicode_v1";
pub(super) const ARTIST_SORT: &str = "coalesce(nullif(tracks.sort_artist, ''), mytunes_artist_v1(tracks.artist)) COLLATE mytunes_unicode_v1";
pub(super) const ALBUM_SORT: &str =
    "coalesce(nullif(tracks.sort_album, ''), tracks.album) COLLATE mytunes_unicode_v1";
const ALBUM_ARTIST_SORT: &str = "coalesce(nullif(tracks.sort_album_artist, ''), mytunes_artist_v1(tracks.album_artist)) COLLATE mytunes_unicode_v1";
pub(crate) const ALBUM_DISPLAY_ARTIST: &str = "(CASE WHEN tracks.compilation THEN 'Various Artists'
    ELSE coalesce(nullif(tracks.album_artist, ''), nullif(tracks.artist, '')) END)";
pub(super) const ALBUM_GROUP_SORT: &str = "(CASE WHEN tracks.compilation THEN 'Various Artists'
    ELSE coalesce(nullif(tracks.sort_album_artist, ''), mytunes_artist_v1(nullif(tracks.album_artist, '')),
                  nullif(tracks.sort_artist, ''), mytunes_artist_v1(tracks.artist)) END) COLLATE mytunes_unicode_v1";

/// A column users can sort the track list by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TrackColumn {
    /// Whether the track is selected for sync.
    Checked,
    /// Track title.
    #[default]
    Title,
    /// Track duration in milliseconds.
    Duration,
    /// Track artist.
    Artist,
    /// Album grouping followed by disc and track position.
    Album,
    /// Tagged album artist.
    AlbumArtist,
    /// Genre.
    Genre,
    /// Composer.
    Composer,
    /// Release year.
    Year,
    /// Position within the album.
    TrackNumber,
    /// Rating.
    Rating,
    /// Play count.
    PlayCount,
    /// Last played timestamp.
    LastPlayed,
    /// Date added to the library.
    DateAdded,
    /// Codec followed by bitrate.
    Kind,
    /// File size in bytes.
    Size,
}

/// Direction of a track sort.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    /// Lowest or earliest values first.
    #[default]
    Ascending,
    /// Highest or latest values first.
    Descending,
}

impl SortDirection {
    fn sql(self) -> &'static str {
        match self {
            Self::Ascending => "ASC",
            Self::Descending => "DESC",
        }
    }
}

/// Column and direction to use when constructing a track snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub struct TrackSort {
    /// User-visible sort column.
    pub column: TrackColumn,
    /// User-selected direction.
    pub direction: SortDirection,
}

impl TrackSort {
    fn order_by(self) -> String {
        let direction = self.direction.sql();
        let key = match self.column {
            TrackColumn::Checked => "tracks.checked",
            TrackColumn::Title => TITLE_SORT,
            TrackColumn::Duration => "tracks.duration_ms",
            TrackColumn::Artist => ARTIST_SORT,
            TrackColumn::Album => {
                return format!(
                    "{ALBUM_SORT} {direction}, {ALBUM_GROUP_SORT} {direction}, tracks.year {direction},
                     tracks.album {direction}, {ALBUM_DISPLAY_ARTIST} {direction}, tracks.compilation {direction},
                     tracks.disc_no ASC, tracks.track_no ASC, tracks.id ASC"
                );
            }
            TrackColumn::AlbumArtist => ALBUM_ARTIST_SORT,
            TrackColumn::Genre => "tracks.genre COLLATE mytunes_unicode_v1",
            TrackColumn::Composer => "tracks.composer COLLATE mytunes_unicode_v1",
            TrackColumn::Year => "tracks.year",
            TrackColumn::TrackNumber => "tracks.track_no",
            TrackColumn::Rating => "tracks.rating",
            TrackColumn::PlayCount => "tracks.play_count",
            TrackColumn::LastPlayed => "tracks.last_played_at",
            TrackColumn::DateAdded => "tracks.date_added",
            TrackColumn::Kind => {
                return format!(
                    "tracks.codec COLLATE mytunes_unicode_v1 {direction}, tracks.bitrate {direction}, tracks.id {direction}"
                );
            }
            TrackColumn::Size => "tracks.size_bytes",
        };
        format!("{key} {direction}, tracks.id {direction}")
    }
}

/// An optional search over the six indexed track tags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrackSearch<'a> {
    /// Include all tracks.
    #[default]
    All,
    /// Treat whitespace-separated words as literal FTS5 prefixes.
    Prefix(&'a str),
    /// Use an FTS5 expression, such as a field-scoped search.
    Match(&'a str),
}

impl<'a> TrackSearch<'a> {
    fn expression(self) -> Option<Cow<'a, str>> {
        match self {
            Self::All => None,
            Self::Match(expression) if expression.trim().is_empty() => None,
            Self::Match(expression) => Some(Cow::Borrowed(expression)),
            Self::Prefix(text) if text.trim().is_empty() => None,
            Self::Prefix(text) => Some(Cow::Owned(
                text.split_whitespace()
                    .map(|word| format!("\"{}\"*", word.replace('"', "\"\"")))
                    .collect::<Vec<_>>()
                    .join(" "),
            )),
        }
    }
}

/// Optional exact filters for a desktop track snapshot.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrackFilter<'a> {
    /// Limit membership to this manual playlist, preserving repeated picks.
    pub playlist: Option<&'a PlaylistId>,
    /// Genre shown in the column browser.
    pub genre: Option<&'a str>,
    /// Track artist or the compilation group `Various Artists`.
    pub artist: Option<&'a str>,
    /// When set, distinguish tagged compilations from ordinary artist names.
    pub compilation: Option<bool>,
    /// Album title shown in the column browser.
    pub album: Option<&'a str>,
}

/// A classification offered by the column browser.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct BrowserOption {
    /// The tag's exact value, or “Compilations” for compilation tracks.
    pub name: String,
    /// Whether this represents the compilation group.
    pub compilation: bool,
}

/// Browser options in genre, artist and album order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Type)]
pub struct BrowserFacets {
    /// Genres in the current source and search.
    pub genres: Vec<BrowserOption>,
    /// Artists and compilations under the selected genre.
    pub artists: Vec<BrowserOption>,
    /// Albums under the selected genre and artist.
    pub albums: Vec<BrowserOption>,
}

fn track_scope<'a>(
    search: TrackSearch<'a>,
    filter: TrackFilter<'a>,
) -> (String, Vec<Cow<'a, str>>) {
    let mut sql = String::from(" FROM tracks");
    if filter.playlist.is_some() {
        sql.push_str(" JOIN playlist_items ON playlist_items.track_id = tracks.id");
    }
    let expression = search.expression();
    if expression.is_some() {
        sql.push_str(" JOIN tracks_fts ON tracks.rowid = tracks_fts.rowid");
    }
    let mut args = Vec::new();
    for (condition, value) in [
        ("tracks_fts MATCH ?", expression),
        (
            "playlist_items.playlist_id = ?",
            filter.playlist.map(|id| Cow::Borrowed(id.0.as_str())),
        ),
        ("tracks.genre = ?", filter.genre.map(Cow::Borrowed)),
        (
            "(CASE WHEN tracks.compilation THEN 'Various Artists' ELSE tracks.artist END) = ?",
            filter.artist.map(Cow::Borrowed),
        ),
        (
            "tracks.compilation = CAST(? AS INTEGER)",
            filter
                .compilation
                .map(|compilation| Cow::Borrowed(if compilation { "1" } else { "0" })),
        ),
        ("tracks.album = ?", filter.album.map(Cow::Borrowed)),
    ] {
        if let Some(value) = value {
            sql.push_str(if args.is_empty() { " WHERE " } else { " AND " });
            sql.push_str(condition);
            args.push(value);
        }
    }
    (sql, args)
}

/// Metadata needed to render one row in the desktop track list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct TrackRow {
    /// Stable public track identifier.
    pub id: TrackId,
    /// Whether the track is selected for sync.
    pub checked: bool,
    /// Whether its file is unavailable.
    pub missing: bool,
    /// Track title.
    pub title: Option<String>,
    /// Playback duration in milliseconds.
    #[specta(type = Option<specta_typescript::Number>)]
    pub duration_ms: Option<i64>,
    /// Tagged track artist.
    pub artist: Option<String>,
    /// Tagged album title.
    pub album: Option<String>,
    /// Tagged album artist.
    pub album_artist: Option<String>,
    /// Genre tag.
    pub genre: Option<String>,
    /// Composer tag.
    pub composer: Option<String>,
    /// Release year.
    #[specta(type = Option<specta_typescript::Number>)]
    pub year: Option<i64>,
    /// Disc position within the album.
    #[specta(type = Option<specta_typescript::Number>)]
    pub disc_no: Option<i64>,
    /// Track position on the disc.
    #[specta(type = Option<specta_typescript::Number>)]
    pub track_no: Option<i64>,
    /// Rating on a 0–100 scale.
    #[specta(type = specta_typescript::Number)]
    pub rating: i64,
    /// Number of completed plays.
    #[specta(type = specta_typescript::Number)]
    pub play_count: i64,
    /// When the track was last played.
    #[specta(type = Option<specta_typescript::Number>)]
    pub last_played_at: Option<i64>,
    /// When the track was added to the library.
    #[specta(type = Option<specta_typescript::Number>)]
    pub date_added: Option<i64>,
    /// Audio codec.
    pub codec: Option<String>,
    /// Stored bitrate.
    #[specta(type = Option<specta_typescript::Number>)]
    pub bitrate: Option<i64>,
    /// File size in bytes.
    #[specta(type = Option<specta_typescript::Number>)]
    pub size_bytes: Option<i64>,
    /// Associated artwork identifier.
    pub artwork_id: Option<String>,
}

impl TrackRow {
    fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: read_track_id(row)?,
            checked: row.get(1)?,
            missing: row.get(2)?,
            title: row.get(3)?,
            duration_ms: row.get(4)?,
            artist: row.get(5)?,
            album: row.get(6)?,
            album_artist: row.get(7)?,
            genre: row.get(8)?,
            composer: row.get(9)?,
            year: row.get(10)?,
            disc_no: row.get(11)?,
            track_no: row.get(12)?,
            rating: row.get(13)?,
            play_count: row.get(14)?,
            last_played_at: row.get(15)?,
            date_added: row.get(16)?,
            codec: row.get(17)?,
            bitrate: row.get(18)?,
            size_bytes: row.get(19)?,
            artwork_id: row.get(20)?,
        })
    }
}

impl LibraryQueries<'_> {
    /// Builds an ordered snapshot of stable track IDs with an optional FTS5 filter.
    pub fn track_ids(
        &self,
        sort: TrackSort,
        search: TrackSearch<'_>,
    ) -> Result<Vec<TrackId>, QueryError> {
        self.track_ids_in(Some(sort), search, TrackFilter::default())
    }

    /// Builds an ordered ID snapshot for the given source and exact filters.
    /// A playlist with no explicit sort uses its stored position order.
    pub fn track_ids_in(
        &self,
        sort: Option<TrackSort>,
        search: TrackSearch<'_>,
        filter: TrackFilter<'_>,
    ) -> Result<Vec<TrackId>, QueryError> {
        self.ordered_tracks(sort, search, filter, read_track_id)
    }

    /// Materialize the base view order alongside the FTS row identifier.
    pub fn ordered_track_keys(
        &self,
        sort: Option<TrackSort>,
        filter: TrackFilter<'_>,
    ) -> Result<Vec<(TrackId, i64)>, QueryError> {
        self.ordered_tracks(sort, TrackSearch::All, filter, |row| {
            Ok((read_track_id(row)?, row.get(1)?))
        })
    }

    /// Find matching rows without sorting them again.
    pub fn matching_row_ids(
        &self,
        search: TrackSearch<'_>,
    ) -> Result<Option<HashSet<i64>>, QueryError> {
        let Some(expression) = search.expression() else {
            return Ok(None);
        };
        let mut statement = self
            .db
            .prepare_cached("SELECT rowid FROM tracks_fts WHERE tracks_fts MATCH ?1")?;
        let rows = statement.query_map([expression.as_ref()], |row| row.get(0))?;
        Ok(Some(rows.collect::<rusqlite::Result<_>>()?))
    }

    fn ordered_tracks<T>(
        &self,
        sort: Option<TrackSort>,
        search: TrackSearch<'_>,
        filter: TrackFilter<'_>,
        read: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>, QueryError> {
        let (scope, args) = track_scope(search, filter);
        let mut sql = format!("SELECT tracks.id, tracks.rowid{scope}");
        sql.push_str(" ORDER BY ");
        if sort.is_none() && filter.playlist.is_some() {
            sql.push_str("playlist_items.position");
        } else {
            sql.push_str(&sort.unwrap_or_default().order_by());
        }
        let mut statement = self.db.prepare_cached(&sql)?;
        let rows = statement.query_map(
            rusqlite::params_from_iter(args.iter().map(Cow::as_ref)),
            read,
        )?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// List progressively narrower choices for the current browser.
    pub fn browser_facets(
        &self,
        search: TrackSearch<'_>,
        filter: TrackFilter<'_>,
    ) -> Result<BrowserFacets, QueryError> {
        let source = TrackFilter {
            playlist: filter.playlist,
            ..TrackFilter::default()
        };
        Ok(BrowserFacets {
            genres: self.browser_options(
                search,
                source,
                "tracks.genre",
                "0",
                "tracks.genre COLLATE mytunes_unicode_v1",
            )?,
            artists: self.browser_options(
                search,
                TrackFilter { genre: filter.genre, ..source },
                "CASE WHEN tracks.compilation THEN 'Compilations' ELSE tracks.artist END",
                "tracks.compilation",
                &format!("CASE WHEN tracks.compilation THEN 'Compilations' ELSE {ARTIST_SORT} END COLLATE mytunes_unicode_v1"),
            )?,
            albums: self.browser_options(
                search,
                TrackFilter { album: None, ..filter },
                "tracks.album",
                "0",
                ALBUM_SORT,
            )?,
        })
    }

    fn browser_options(
        &self,
        search: TrackSearch<'_>,
        filter: TrackFilter<'_>,
        name: &str,
        compilation: &str,
        sort: &str,
    ) -> Result<Vec<BrowserOption>, QueryError> {
        let (mut scope, args) = track_scope(search, filter);
        scope.push_str(if args.is_empty() { " WHERE " } else { " AND " });
        let sql = format!(
            "SELECT {name} AS name, {compilation} AS compilation {scope} trim({name}) <> ''
                 GROUP BY 1, 2 ORDER BY compilation DESC, min({sort}) COLLATE mytunes_unicode_v1, name",
        );
        let mut statement = self.db.prepare_cached(&sql)?;
        let rows = statement.query_map(
            rusqlite::params_from_iter(args.iter().map(Cow::as_ref)),
            |row| {
                Ok(BrowserOption {
                    name: row.get(0)?,
                    compilation: row.get(1)?,
                })
            },
        )?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Fetches the given IDs in caller order. Duplicate IDs repeat and deleted IDs are omitted.
    pub fn track_page(&self, ids: &[TrackId]) -> Result<Vec<TrackRow>, QueryError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut statement = self.db.prepare_cached(
            "SELECT t.id, t.checked, t.missing, t.title, t.duration_ms, t.artist, t.album, t.album_artist,
                    t.genre, t.composer, t.year, t.disc_no, t.track_no, t.rating, t.play_count,
                    t.last_played_at, t.date_added, t.codec, t.bitrate, t.size_bytes, t.artwork_id
               FROM json_each(?1) AS requested
               JOIN tracks AS t ON t.id = requested.value
              ORDER BY requested.key",
        )?;
        let rows = statement.query_map([serde_json::to_string(ids)?], TrackRow::read)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}
