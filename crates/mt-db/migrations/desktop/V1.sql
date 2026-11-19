CREATE TABLE artwork (
    id TEXT PRIMARY KEY NOT NULL,
    thumb_300 BLOB NOT NULL,
    thumb_600 BLOB NOT NULL
);

CREATE TABLE tracks (
    rowid INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    file_path TEXT NOT NULL UNIQUE,
    file_rev INTEGER NOT NULL DEFAULT 1,
    file_hash BLOB,
    size_bytes INTEGER,
    codec TEXT,
    sample_rate INTEGER,
    bit_depth INTEGER,
    bitrate INTEGER,
    channels INTEGER,
    duration_ms INTEGER,
    title TEXT,
    artist TEXT,
    album_artist TEXT,
    album TEXT,
    composer TEXT,
    genre TEXT,
    year INTEGER,
    track_no INTEGER,
    track_total INTEGER,
    disc_no INTEGER,
    disc_total INTEGER,
    compilation INTEGER NOT NULL DEFAULT 0 CHECK (compilation IN (0, 1)),
    sort_title TEXT,
    sort_artist TEXT,
    sort_album TEXT,
    sort_album_artist TEXT,
    rating INTEGER NOT NULL DEFAULT 0 CHECK (rating IN (0, 20, 40, 60, 80, 100)),
    rating_updated_at INTEGER,
    play_count INTEGER NOT NULL DEFAULT 0,
    skip_count INTEGER NOT NULL DEFAULT 0,
    last_played_at INTEGER,
    date_added INTEGER,
    date_modified INTEGER,
    replaygain_track_db REAL,
    replaygain_album_db REAL,
    artwork_id TEXT REFERENCES artwork (id) ON DELETE SET NULL,
    checked INTEGER NOT NULL DEFAULT 1 CHECK (checked IN (0, 1)),
    missing INTEGER NOT NULL DEFAULT 0 CHECK (missing IN (0, 1))
);

CREATE INDEX tracks_artwork_id ON tracks (artwork_id);

CREATE VIEW albums AS
SELECT album_artist, album, year, count(*) AS track_count
FROM tracks
WHERE album IS NOT NULL
GROUP BY album_artist, album, year;

CREATE TABLE playlists (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('manual', 'smart', 'folder', 'otg')),
    parent_id TEXT REFERENCES playlists (id),
    smart_rules TEXT CHECK (json_valid(smart_rules) OR smart_rules IS NULL),
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX playlists_parent_id ON playlists (parent_id);

CREATE TABLE playlist_items (
    playlist_id TEXT NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    track_id TEXT NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    PRIMARY KEY (playlist_id, position)
);

CREATE INDEX playlist_items_track_id ON playlist_items (track_id);

CREATE TABLE devices (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    model TEXT,
    platform TEXT NOT NULL,
    cert_fingerprint BLOB NOT NULL,
    paired_at INTEGER NOT NULL,
    last_sync_at INTEGER,
    capacity_bytes INTEGER,
    free_bytes INTEGER,
    sync_mode TEXT NOT NULL CHECK (sync_mode IN ('all', 'selected', 'manual')),
    sync_selection TEXT CHECK (json_valid(sync_selection) OR sync_selection IS NULL),
    reserve_free_bytes INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE device_tracks (
    device_id TEXT NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    track_id TEXT NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    file_rev INTEGER NOT NULL,
    synced_at INTEGER NOT NULL,
    PRIMARY KEY (device_id, track_id)
);

CREATE INDEX device_tracks_track_id ON device_tracks (track_id);

CREATE TABLE applied_events (
    device_id TEXT NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    event_id TEXT NOT NULL,
    PRIMARY KEY (device_id, event_id)
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL CHECK (json_valid(value))
);

-- Use the stable integer key for FTS and keep the UUID as the public identifier.
CREATE VIRTUAL TABLE tracks_fts USING fts5(
    title, artist, album, album_artist, composer, genre,
    content = 'tracks', content_rowid = 'rowid',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER tracks_fts_insert AFTER INSERT ON tracks BEGIN
    INSERT INTO tracks_fts (rowid, title, artist, album, album_artist, composer, genre)
    VALUES (new.rowid, new.title, new.artist, new.album, new.album_artist, new.composer, new.genre);
END;

CREATE TRIGGER tracks_fts_delete AFTER DELETE ON tracks BEGIN
    INSERT INTO tracks_fts (tracks_fts, rowid, title, artist, album, album_artist, composer, genre)
    VALUES ('delete', old.rowid, old.title, old.artist, old.album, old.album_artist, old.composer, old.genre);
END;

CREATE TRIGGER tracks_fts_update AFTER UPDATE ON tracks
WHEN old.rowid IS NOT new.rowid OR old.title IS NOT new.title OR old.artist IS NOT new.artist
    OR old.album IS NOT new.album OR old.album_artist IS NOT new.album_artist
    OR old.composer IS NOT new.composer OR old.genre IS NOT new.genre
BEGIN
    INSERT INTO tracks_fts (tracks_fts, rowid, title, artist, album, album_artist, composer, genre)
    VALUES ('delete', old.rowid, old.title, old.artist, old.album, old.album_artist, old.composer, old.genre);
    INSERT INTO tracks_fts (rowid, title, artist, album, album_artist, composer, genre)
    VALUES (new.rowid, new.title, new.artist, new.album, new.album_artist, new.composer, new.genre);
END;
