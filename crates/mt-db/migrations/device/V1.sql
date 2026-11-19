CREATE TABLE artwork (
    id TEXT PRIMARY KEY NOT NULL,
    thumb_300 BLOB NOT NULL,
    thumb_600 BLOB NOT NULL
) STRICT, WITHOUT ROWID;

CREATE TABLE tracks (
    id TEXT PRIMARY KEY NOT NULL,
    rel_path TEXT NOT NULL UNIQUE CHECK (
        rel_path <> '' AND instr(rel_path, char(0)) = 0
        AND instr(rel_path, char(92)) = 0 AND instr(rel_path, ':') = 0
        AND instr('/' || rel_path || '/', '//') = 0
        AND instr('/' || rel_path || '/', '/./') = 0
        AND instr('/' || rel_path || '/', '/../') = 0
    ),
    file_rev INTEGER NOT NULL DEFAULT 1,
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
    sort_key BLOB,
    rating INTEGER NOT NULL DEFAULT 0 CHECK (rating IN (0, 20, 40, 60, 80, 100)),
    rating_updated_at INTEGER,
    play_count INTEGER NOT NULL DEFAULT 0,
    skip_count INTEGER NOT NULL DEFAULT 0,
    last_played_at INTEGER,
    date_added INTEGER,
    replaygain_track_db REAL,
    replaygain_album_db REAL,
    artwork_id TEXT REFERENCES artwork (id) ON DELETE SET NULL
) STRICT, WITHOUT ROWID;

CREATE INDEX tracks_artwork_id ON tracks (artwork_id);

CREATE VIEW albums AS
SELECT album,
       CASE WHEN compilation THEN 'Various Artists'
            ELSE coalesce(nullif(album_artist, ''), nullif(artist, '')) END AS group_artist,
       year, compilation, count(*) AS track_count, min(artwork_id) AS artwork_id
FROM tracks
WHERE trim(album) <> ''
GROUP BY album, group_artist, year, compilation;

CREATE TABLE playlists (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('manual', 'smart', 'folder', 'otg')),
    parent_id TEXT REFERENCES playlists (id),
    sort_order INTEGER NOT NULL DEFAULT 0
) STRICT, WITHOUT ROWID;

CREATE TABLE playlist_items (
    playlist_id TEXT NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    track_id TEXT NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    PRIMARY KEY (playlist_id, position)
) STRICT, WITHOUT ROWID;

CREATE INDEX playlists_parent_id ON playlists (parent_id);
CREATE INDEX playlist_items_track_id ON playlist_items (track_id);

-- Keep events independent of the current track and playlist rows. Sync can
-- replace those rows before a pending event has been uploaded.
CREATE TABLE events (
    event_id BLOB PRIMARY KEY NOT NULL CHECK (length(event_id) = 16),
    ts INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('play', 'skip', 'rating', 'otg')),
    payload TEXT NOT NULL CHECK (json_valid(payload) AND json_type(payload) = 'object'),
    acked INTEGER NOT NULL DEFAULT 0 CHECK (acked IN (0, 1))
) STRICT, WITHOUT ROWID;

CREATE INDEX events_pending ON events (ts, event_id) WHERE acked = 0;

CREATE TRIGGER events_append BEFORE INSERT ON events
WHEN new.acked <> 0 OR EXISTS (SELECT 1 FROM events WHERE event_id = new.event_id)
BEGIN
    SELECT RAISE(ABORT, 'device events must be new and unacknowledged');
END;

CREATE TRIGGER events_immutable BEFORE UPDATE ON events
WHEN new.event_id IS NOT old.event_id OR new.ts IS NOT old.ts
    OR new.kind IS NOT old.kind OR new.payload IS NOT old.payload OR new.acked < old.acked
BEGIN
    SELECT RAISE(ABORT, 'device events are immutable except for acknowledging them');
END;

CREATE TRIGGER events_no_delete BEFORE DELETE ON events
BEGIN
    SELECT RAISE(ABORT, 'device events are append-only');
END;

CREATE TABLE state (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL CHECK (json_valid(value))
) STRICT, WITHOUT ROWID;
