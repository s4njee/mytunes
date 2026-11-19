use std::borrow::Cow;

use mt_types::TrackId;
use rusqlite::Row;

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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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

/// Metadata needed to render one row in the desktop track list.
#[derive(Clone, Debug, PartialEq, Eq)]
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
    pub year: Option<i64>,
    /// Disc position within the album.
    pub disc_no: Option<i64>,
    /// Track position on the disc.
    pub track_no: Option<i64>,
    /// Rating on a 0–100 scale.
    pub rating: i64,
    /// Number of completed plays.
    pub play_count: i64,
    /// When the track was last played.
    pub last_played_at: Option<i64>,
    /// When the track was added to the library.
    pub date_added: Option<i64>,
    /// Audio codec.
    pub codec: Option<String>,
    /// Stored bitrate.
    pub bitrate: Option<i64>,
    /// File size in bytes.
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
        let expression = search.expression();
        let source = if expression.is_some() {
            "tracks_fts JOIN tracks ON tracks.rowid = tracks_fts.rowid WHERE tracks_fts MATCH ?1"
        } else {
            "tracks"
        };
        let sql = format!(
            "SELECT tracks.id FROM {source} ORDER BY {}",
            sort.order_by()
        );
        let mut statement = self.db.prepare_cached(&sql)?;
        let rows =
            statement.query_map(rusqlite::params_from_iter(expression.iter()), read_track_id)?;
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
