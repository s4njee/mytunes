//! Mobile library reads, thumbnail cache, state and event log.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use anyhow::{Context, Result, anyhow, ensure};
use flutter_rust_bridge::frb;
use mt_audio::Player;
use mt_db::device::{DeviceEvent, DeviceEventKind, DeviceStore};
use rusqlite::{Connection, Params, Row, params};

const MAX_PAGE: u32 = 200;
const TRACK_FIELDS: &str = "t.id, t.title, t.artist, t.album, t.duration_ms, t.artwork_id,
    t.codec, t.sample_rate, t.bit_depth, t.bitrate, t.disc_no, t.track_no";
pub(super) const ALBUM_ARTIST: &str = "CASE WHEN t.compilation THEN 'Various Artists'
    ELSE coalesce(nullif(t.album_artist, ''), nullif(t.artist, '')) END";

/// One opened device library and its lazily initialized player.
#[frb(opaque)]
pub struct DeviceLibrary {
    db: Mutex<Connection>,
    pub(super) root: PathBuf,
    pub(super) player: Mutex<Option<Player>>,
}

/// Open or migrate a device database. Media paths resolve beside this file.
pub fn open_device_db(path: String) -> Result<DeviceLibrary> {
    let db = mt_db::open_device(&path).context("Could not open the device library")?;
    let file = fs::canonicalize(path).context("Could not resolve the device library path")?;
    let root = file.parent().context("The library file has no directory")?;
    Ok(DeviceLibrary {
        db: Mutex::new(db),
        root: root.to_path_buf(),
        player: Mutex::new(None),
    })
}

/// A window into an ordered list. A request can contain up to 200 items.
pub struct LibraryPage {
    pub offset: u32,
    pub limit: u32,
}

impl LibraryPage {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.limit <= MAX_PAGE,
            "A library page can contain at most {MAX_PAGE} items"
        );
        Ok(())
    }
}

/// Counts for the mobile library display.
pub struct LibraryCounts {
    pub songs: u32,
    pub albums: u32,
    pub artists: u32,
    pub playlists: u32,
}

/// All the fields that distinguish one album in the device schema.
pub struct AlbumId {
    pub title: String,
    pub artist: Option<String>,
    pub year: Option<i32>,
    pub compilation: bool,
}

/// Album metadata and the identity accepted by `album_tracks`.
pub struct DeviceAlbum {
    pub id: AlbumId,
    pub track_count: u32,
    pub artwork_id: Option<String>,
}

/// A credited track artist and their song count.
pub struct DeviceArtist {
    pub name: String,
    pub track_count: u32,
}

/// Display metadata for a locally stored track. Playback takes the ID.
pub struct DeviceTrack {
    pub id: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub artwork_id: Option<String>,
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub bitrate: Option<u32>,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
}

/// Playlist metadata and hierarchy, in stored display order.
pub struct DevicePlaylist {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub parent_id: Option<String>,
    pub track_count: u32,
}

/// Available artwork thumbnail sizes.
pub enum ThumbnailSize {
    Small,
    Large,
}

/// Kind of change to add to the device event log.
pub enum LibraryEventKind {
    Play,
    Skip,
    Rating,
    OnTheGo,
}

/// An event with its caller-supplied UUID and JSON object payload.
pub struct LibraryEvent {
    pub id: String,
    pub timestamp_ms: i64,
    pub kind: LibraryEventKind,
    pub payload_json: String,
}

impl DeviceLibrary {
    pub(super) fn connection(&self) -> Result<MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| anyhow!("The device library was interrupted. Reopen it to retry."))
    }

    /// Close the database and stop the audio worker off the Dart isolate.
    pub fn close(self) {
        drop(self);
    }

    /// Count tracks, album groups, credited artists and playlists.
    pub fn library_counts(&self) -> Result<LibraryCounts> {
        Ok(self.connection()?.query_row(
            "SELECT (SELECT count(*) FROM tracks), (SELECT count(*) FROM albums),
                    (SELECT count(DISTINCT artist) FROM tracks WHERE trim(artist) <> ''),
                    (SELECT count(*) FROM playlists)",
            [],
            |row| {
                Ok(LibraryCounts {
                    songs: row.get(0)?,
                    albums: row.get(1)?,
                    artists: row.get(2)?,
                    playlists: row.get(3)?,
                })
            },
        )?)
    }

    /// Return album groups ordered by title, then artist and release.
    pub fn albums(&self, page: LibraryPage) -> Result<Vec<DeviceAlbum>> {
        page.validate()?;
        self.rows(
            "SELECT album, group_artist, year, compilation, track_count, artwork_id FROM albums
             ORDER BY album COLLATE mytunes_unicode_v1, group_artist COLLATE mytunes_unicode_v1,
                      year, compilation, album, group_artist LIMIT ?1 OFFSET ?2",
            params![page.limit, page.offset],
            |row| {
                Ok(DeviceAlbum {
                    id: AlbumId {
                        title: row.get(0)?,
                        artist: row.get(1)?,
                        year: row.get(2)?,
                        compilation: row.get(3)?,
                    },
                    track_count: row.get(4)?,
                    artwork_id: row.get(5)?,
                })
            },
        )
    }

    /// Return one album's tracks in disc and track order.
    pub fn album_tracks(&self, id: AlbumId) -> Result<Vec<DeviceTrack>> {
        self.rows(
            &format!("SELECT {TRACK_FIELDS} FROM tracks t
                      WHERE t.album = ?1 AND ({ALBUM_ARTIST}) IS ?2 AND t.year IS ?3 AND t.compilation = ?4
                      ORDER BY coalesce(nullif(t.disc_no, 0), 1), t.track_no IS NULL, t.track_no,
                               t.title COLLATE mytunes_unicode_v1, t.id"),
            params![id.title, id.artist, id.year, id.compilation],
            read_track,
        )
    }

    /// Return each credited artist once, with their track count.
    pub fn artists(&self) -> Result<Vec<DeviceArtist>> {
        self.rows(
            "SELECT artist, count(*) FROM tracks WHERE trim(artist) <> '' GROUP BY artist
             ORDER BY mytunes_artist_v1(artist) COLLATE mytunes_unicode_v1, artist",
            [],
            |row| {
                Ok(DeviceArtist {
                    name: row.get(0)?,
                    track_count: row.get(1)?,
                })
            },
        )
    }

    /// Return a page of songs using the synced sort keys when present.
    pub fn songs(&self, page: LibraryPage) -> Result<Vec<DeviceTrack>> {
        page.validate()?;
        self.rows(
            &format!(
                "SELECT {TRACK_FIELDS} FROM tracks t
                      ORDER BY t.sort_key IS NULL, t.sort_key,
                               t.title COLLATE mytunes_unicode_v1, t.id LIMIT ?1 OFFSET ?2"
            ),
            params![page.limit, page.offset],
            read_track,
        )
    }

    /// Return playlists and folders in their stored order.
    pub fn playlists(&self) -> Result<Vec<DevicePlaylist>> {
        self.rows(
            "SELECT p.id, p.name, p.kind, p.parent_id,
                    (SELECT count(*) FROM playlist_items WHERE playlist_id = p.id)
             FROM playlists p ORDER BY p.sort_order, p.name COLLATE mytunes_unicode_v1, p.id",
            [],
            |row| {
                Ok(DevicePlaylist {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    parent_id: row.get(3)?,
                    track_count: row.get(4)?,
                })
            },
        )
    }

    /// Return a playlist's tracks in position order, including repeated tracks.
    pub fn playlist_tracks(&self, id: String) -> Result<Vec<DeviceTrack>> {
        self.rows(
            &format!(
                "SELECT {TRACK_FIELDS} FROM playlist_items p JOIN tracks t ON t.id = p.track_id
                      WHERE p.playlist_id = ?1 ORDER BY p.position"
            ),
            [id],
            read_track,
        )
    }

    /// Materialize an immutable thumbnail beside the device database, if present.
    pub fn artwork_path(&self, id: String, size: ThumbnailSize) -> Result<Option<String>> {
        let (size, pixels) = match size {
            ThumbnailSize::Small => (mt_db::ArtworkSize::Small, 300),
            ThumbnailSize::Large => (mt_db::ArtworkSize::Large, 600),
        };
        let cache = self.root.join("artwork-cache");
        let key = blake3::hash(id.as_bytes());
        let path = cache.join(format!("{key}-{pixels}.jpg"));
        if !path.is_file() {
            let bytes = {
                let db = self.connection()?;
                mt_db::artwork_thumbnail(&db, &id, size)?
            };
            let Some(bytes) = bytes else { return Ok(None) };
            fs::create_dir_all(&cache)?;
            let mut file = tempfile::NamedTempFile::new_in(cache)?;
            file.write_all(&bytes)?;
            file.persist(&path).map_err(|error| error.error)?;
        }
        Ok(Some(
            path.into_os_string()
                .into_string()
                .map_err(|_| anyhow!("Artwork path is not valid text"))?,
        ))
    }

    /// Read a JSON setting. Missing keys return `None`; JSON null returns `"null"`.
    pub fn settings_get(&self, key: String) -> Result<Option<String>> {
        let db = self.connection()?;
        Ok(DeviceStore::new(&db)
            .state(&key)?
            .map(|value| value.to_string()))
    }

    /// Replace one JSON setting.
    pub fn settings_set(&self, key: String, value_json: String) -> Result<()> {
        let value = serde_json::from_str(&value_json).context("Invalid JSON setting")?;
        let db = self.connection()?;
        Ok(DeviceStore::new(&db).set_state(&key, &value)?)
    }

    /// Append one event to the durable log for future sync.
    pub fn record_event(&self, event: LibraryEvent) -> Result<()> {
        let kind = match event.kind {
            LibraryEventKind::Play => DeviceEventKind::Play,
            LibraryEventKind::Skip => DeviceEventKind::Skip,
            LibraryEventKind::Rating => DeviceEventKind::Rating,
            LibraryEventKind::OnTheGo => DeviceEventKind::OnTheGo,
        };
        let event = DeviceEvent {
            event_id: event.id.parse().context("Event ID must be a UUID")?,
            ts: event.timestamp_ms,
            kind,
            payload: serde_json::from_str(&event.payload_json)
                .context("Event payload must be a JSON object")?,
        };
        let db = self.connection()?;
        Ok(DeviceStore::new(&db).append_event(&event)?)
    }

    fn rows<T>(
        &self,
        sql: &str,
        args: impl Params,
        read: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>> {
        let db = self.connection()?;
        let mut query = db.prepare_cached(sql)?;
        Ok(query
            .query_map(args, read)?
            .collect::<rusqlite::Result<_>>()?)
    }
}

fn read_track(row: &Row<'_>) -> rusqlite::Result<DeviceTrack> {
    Ok(DeviceTrack {
        id: row.get(0)?,
        title: row.get(1)?,
        artist: row.get(2)?,
        album: row.get(3)?,
        duration_ms: row.get(4)?,
        artwork_id: row.get(5)?,
        codec: row.get(6)?,
        sample_rate: row.get(7)?,
        bit_depth: row.get(8)?,
        bitrate: row.get(9)?,
        disc_number: row.get(10)?,
        track_number: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_paged_songs_album_groups_and_repeated_playlist_entries() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let library = open_device_db(dir.path().join("device.sqlite").display().to_string())?;
        library.connection()?.execute_batch(
            "INSERT INTO artwork VALUES ('cover', X'00', X'01');
             INSERT INTO tracks (id, rel_path, title, artist, album, compilation, year, disc_no, track_no, sort_key, artwork_id) VALUES
                ('a', 'music/a.wav', 'Zulu', 'Juno', 'Sessions', 1, NULL, 2, 1, X'00', 'cover'),
                ('b', 'music/b.wav', 'Écho', 'Élodie', 'Sessions', 1, NULL, 1, 2, NULL, 'cover'),
                ('c', 'music/c.wav', 'Dawn', 'Juno', 'Sessions', 0, NULL, NULL, NULL, NULL, NULL);
             INSERT INTO playlists (id, name, kind) VALUES ('mix', 'Mix', 'otg');
             INSERT INTO playlist_items VALUES ('mix', 0, 'b'), ('mix', 1, 'a'), ('mix', 2, 'b');",
        )?;
        let counts = library.library_counts()?;
        assert_eq!(
            (
                counts.songs,
                counts.albums,
                counts.artists,
                counts.playlists
            ),
            (3, 2, 2, 1)
        );
        assert_eq!(
            library.songs(LibraryPage {
                offset: 1,
                limit: 1
            })?[0]
                .id,
            "c"
        );
        assert!(
            library
                .songs(LibraryPage {
                    offset: 0,
                    limit: MAX_PAGE + 1
                })
                .is_err()
        );
        let compilation = library
            .albums(LibraryPage {
                offset: 0,
                limit: 2,
            })?
            .into_iter()
            .find(|album| album.id.compilation)
            .context("missing compilation")?;
        assert_eq!(compilation.id.artist.as_deref(), Some("Various Artists"));
        assert_eq!(compilation.artwork_id.as_deref(), Some("cover"));
        let album_tracks = library.album_tracks(compilation.id)?;
        assert!(
            album_tracks
                .iter()
                .map(|track| track.id.as_str())
                .eq(["b", "a"])
        );
        assert_eq!(library.artists()?[0].name, "Élodie");
        assert_eq!(library.playlists()?[0].track_count, 3);
        let playlist_tracks = library.playlist_tracks("mix".into())?;
        assert!(
            playlist_tracks
                .iter()
                .map(|track| track.id.as_str())
                .eq(["b", "a", "b"])
        );

        let small = library
            .artwork_path("cover".into(), ThumbnailSize::Small)?
            .context("missing thumbnail")?;
        let large = library
            .artwork_path("cover".into(), ThumbnailSize::Large)?
            .context("missing thumbnail")?;
        assert_ne!(small, large);
        assert_eq!(fs::read(small)?, [0]);
        assert_eq!(fs::read(large)?, [1]);
        assert!(
            library
                .artwork_path("../cover".into(), ThumbnailSize::Small)?
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn settings_and_events_survive_reopening_the_library() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("device.sqlite").display().to_string();
        let library = open_device_db(file.clone())?;
        assert!(library.settings_get("theme".into())?.is_none());
        library.settings_set("theme".into(), "null".into())?;
        let played = || LibraryEvent {
            id: "a1b2c3d4-0000-4000-8000-000000000000".into(),
            timestamp_ms: 25,
            kind: LibraryEventKind::Play,
            payload_json: r#"{"track_id":"played"}"#.into(),
        };
        library.record_event(played())?;
        assert!(library.record_event(played()).is_err());
        drop(library);

        let library = open_device_db(file)?;
        assert_eq!(
            library.settings_get("theme".into())?.as_deref(),
            Some("null")
        );
        let db = library.connection()?;
        let pending = DeviceStore::new(&db).pending_events(10)?;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].kind, DeviceEventKind::Play);
        assert_eq!(pending[0].payload["track_id"], "played");
        Ok(())
    }
}
