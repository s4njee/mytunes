use mt_db::{DESKTOP_SCHEMA_VERSION, DbError, open_desktop};
use rusqlite::{Connection, ErrorCode};
use tempfile::tempdir;

#[test]
fn creates_and_reopens_a_versioned_library() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("library.sqlite");
    let db = open_desktop(&path).unwrap();
    let mut statement = db
        .prepare(
            "SELECT name FROM sqlite_schema WHERE name IN (
            'tracks', 'tracks_fts', 'albums', 'artwork', 'playlists', 'playlist_items',
            'devices', 'device_tracks', 'applied_events', 'settings'
        ) ORDER BY name",
        )
        .unwrap();
    let schema = statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        schema,
        [
            "albums",
            "applied_events",
            "artwork",
            "device_tracks",
            "devices",
            "playlist_items",
            "playlists",
            "settings",
            "tracks",
            "tracks_fts",
        ]
    );
    drop(statement);

    db.execute_batch(
        "INSERT INTO tracks (id, file_path, album_artist, album, year) VALUES
            ('a', '/a.flac', 'Joni Mitchell', 'Blue', 1971),
            ('b', '/b.flac', 'Joni Mitchell', 'Blue', 1971),
            ('c', '/c.flac', 'Joni Mitchell', 'Blue', 2024),
            ('d', '/d.flac', 'Someone Else', 'Blue', 1971),
            ('e', '/e.flac', NULL, NULL, NULL);",
    )
    .unwrap();
    drop(db);

    let db = open_desktop(&path).unwrap();
    assert_eq!(version(&db), DESKTOP_SCHEMA_VERSION);
    let groups: i64 = db
        .query_row("SELECT count(*) FROM albums", [], |r| r.get(0))
        .unwrap();
    let original_release: i64 = db
        .query_row(
            "SELECT track_count FROM albums WHERE album_artist = 'Joni Mitchell' AND year = 1971",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!((groups, original_release), (3, 2));
}

#[test]
fn enables_foreign_keys_and_wal_on_each_open() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("library.sqlite");
    for _ in 0..2 {
        let db = open_desktop(&path).unwrap();
        assert_eq!(journal_mode(&db), "wal");
        let enabled: i64 = db
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert_eq!(enabled, 1);
        let error = db
            .execute(
                "INSERT INTO playlist_items VALUES ('missing-playlist', 0, 'missing-track')",
                [],
            )
            .unwrap_err();
        assert_eq!(
            error.sqlite_error_code(),
            Some(ErrorCode::ConstraintViolation)
        );
    }
}

#[test]
fn failed_migration_rolls_back_schema_and_version() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("library.sqlite");
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE settings (key TEXT PRIMARY KEY);
         INSERT INTO settings VALUES ('preserve me');",
    )
    .unwrap();
    drop(db);

    assert!(matches!(open_desktop(&path), Err(DbError::Sqlite(_))));

    let db = Connection::open(&path).unwrap();
    assert_eq!(version(&db), 0);
    assert_eq!(journal_mode(&db), "delete");
    let tables: i64 = db
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE type = 'table'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tables, 1);
    let key: String = db
        .query_row("SELECT key FROM settings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(key, "preserve me");
}

#[test]
fn rejects_newer_libraries_without_migrating_or_changing_the_journal() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("library.sqlite");
    let newer = DESKTOP_SCHEMA_VERSION + 1;
    let db = Connection::open(&path).unwrap();
    db.pragma_update(None, "user_version", newer).unwrap();
    drop(db);

    assert!(matches!(
        open_desktop(&path),
        Err(DbError::SchemaTooNew { found, supported })
            if found == newer && supported == DESKTOP_SCHEMA_VERSION
    ));
    let db = Connection::open(&path).unwrap();
    assert_eq!(version(&db), newer);
    assert_eq!(journal_mode(&db), "delete");
    let objects: i64 = db
        .query_row("SELECT count(*) FROM sqlite_schema", [], |r| r.get(0))
        .unwrap();
    assert_eq!(objects, 0);
}

#[test]
fn search_tracks_inserts_updates_deletions_and_vacuum() {
    let dir = tempdir().unwrap();
    let db = open_desktop(dir.path().join("library.sqlite")).unwrap();
    db.execute_batch(
        "INSERT INTO tracks (id, file_path, title) VALUES ('gone', '/gone.flac', 'Prelude');
         INSERT INTO tracks (id, file_path, title, artist, album, album_artist, composer, genre)
            VALUES ('song', '/song.flac', 'Águas', 'Elis Regina', 'Elis & Tom',
                    'Antônio Jobim', 'Vinícius', 'Bossa Nova');",
    )
    .unwrap();
    for query in [
        "title:aguas",
        "artist:regina",
        "album:tom",
        "album_artist:antonio",
        "composer:vinicius",
        "genre:bossa",
    ] {
        assert_eq!(search(&db, query), ["song"], "{query}");
    }

    db.execute_batch(
        "UPDATE tracks SET title = 'Corcovado', composer = NULL WHERE id = 'song';
         UPDATE tracks SET play_count = 1 WHERE id = 'song';
         DELETE FROM tracks WHERE id = 'gone';
         VACUUM;",
    )
    .unwrap();
    assert!(search(&db, "aguas OR vinicius OR prelude").is_empty());
    assert_eq!(search(&db, "corcovado AND regina"), ["song"]);

    db.execute("DELETE FROM tracks WHERE id = 'song'", [])
        .unwrap();
    assert!(search(&db, "corcovado OR regina").is_empty());
    db.execute(
        "INSERT INTO tracks_fts (tracks_fts, rank) VALUES ('integrity-check', 1)",
        [],
    )
    .unwrap();
}

#[test]
fn replacing_a_track_does_not_leave_old_search_terms() {
    let dir = tempdir().unwrap();
    let db = open_desktop(dir.path().join("library.sqlite")).unwrap();
    db.execute_batch(
        "INSERT INTO tracks (id, file_path, title) VALUES ('old', '/song.flac', 'Polonaise');
         INSERT OR REPLACE INTO tracks (id, file_path, title) VALUES ('new', '/song.flac', 'Mazurka');",
    )
    .unwrap();
    assert!(search(&db, "polonaise").is_empty());
    assert_eq!(search(&db, "mazurka"), ["new"]);
}

fn version(db: &Connection) -> u32 {
    db.pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version")
}

fn journal_mode(db: &Connection) -> String {
    db.pragma_query_value(None, "journal_mode", |row| row.get(0))
        .expect("read journal mode")
}

fn search(db: &Connection, query: &str) -> Vec<String> {
    let mut statement = db
        .prepare(
            "SELECT tracks.id FROM tracks_fts JOIN tracks ON tracks.rowid = tracks_fts.rowid
             WHERE tracks_fts MATCH ?1 ORDER BY tracks.id",
        )
        .expect("prepare full-text query");
    statement
        .query_map([query], |row| row.get(0))
        .expect("execute full-text query")
        .collect::<Result<Vec<_>, _>>()
        .expect("read full-text matches")
}
