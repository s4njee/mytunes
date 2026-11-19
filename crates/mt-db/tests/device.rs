use mt_db::device::{DeviceEvent, DeviceEventKind, DeviceStore};
use mt_db::{DESKTOP_SCHEMA_VERSION, DEVICE_SCHEMA_VERSION, DbError, open_desktop, open_device};
use rusqlite::Connection;
use serde_json::{Map, Value, json};
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn device_migration_creates_the_local_library_and_groups_compilations() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("device.sqlite");
    let db = open_device(&path).unwrap();
    let expected = [
        "albums",
        "artwork",
        "events",
        "playlist_items",
        "playlists",
        "state",
        "tracks",
    ];
    let mut query = db
        .prepare("SELECT name FROM sqlite_schema WHERE type IN ('table', 'view') ORDER BY name")
        .unwrap();
    let objects = query
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(objects, expected);
    drop(query);

    db.execute_batch(
        "INSERT INTO artwork VALUES ('cover', X'00', X'01');
         INSERT INTO tracks (id, rel_path, album, artist, compilation, year, artwork_id) VALUES
           ('a', 'tracks/a.flac', 'Collection', 'Artist A', 1, 1995, 'cover'),
           ('b', 'tracks/b.flac', 'Collection', 'Artist B', 1, 1995, 'cover'),
           ('c', 'tracks/c.flac', 'Collection', 'Artist A', 0, 1995, NULL);
         INSERT INTO playlists (id, name, kind) VALUES ('list', 'Favorites', 'manual');
         INSERT INTO playlist_items VALUES ('list', 0, 'a'), ('list', 1, 'b');",
    )
    .unwrap();
    drop(db);

    let db = open_device(&path).unwrap();
    assert_eq!(version(&db), DEVICE_SCHEMA_VERSION);
    assert_eq!(journal_mode(&db), "wal");
    let groups: i64 = db
        .query_row("SELECT count(*) FROM albums", [], |row| row.get(0))
        .unwrap();
    let compilation: (i64, String) = db
        .query_row(
            "SELECT track_count, artwork_id FROM albums WHERE group_artist = 'Various Artists'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(groups, 2);
    assert_eq!(compilation, (2, "cover".to_owned()));
    assert!(
        db.execute(
            "INSERT INTO playlist_items VALUES ('list', 2, 'unknown')",
            []
        )
        .is_err()
    );
}

#[test]
fn track_paths_are_relative_to_device_storage() {
    let dir = tempdir().unwrap();
    let db = open_device(dir.path().join("device.sqlite")).unwrap();

    db.execute(
        "INSERT INTO tracks (id, rel_path) VALUES ('local', 'songs/Águas.flac')",
        [],
    )
    .unwrap();
    for path in [
        "",
        "/home/music/track.flac",
        "C:/music/track.flac",
        "songs/../track.flac",
        "../track.flac",
        "songs/./track.flac",
        "songs//track.flac",
        "songs/",
        r"songs\track.flac",
        "songs/track\0.flac",
        "songs/Águas.flac",
    ] {
        assert!(
            db.execute(
                "INSERT INTO tracks (id, rel_path) VALUES ('new', ?1)",
                [path]
            )
            .is_err(),
            "accepted invalid or duplicate relative path {path:?}"
        );
    }
}

#[test]
fn database_openers_reject_the_other_schema_and_do_not_claim_unrelated_files() {
    let dir = tempdir().unwrap();
    let desktop = dir.path().join("library.sqlite");
    let device = dir.path().join("device.sqlite");
    let unrelated = dir.path().join("other.sqlite");
    drop(open_desktop(&desktop).unwrap());
    drop(open_device(&device).unwrap());
    let other = Connection::open(&unrelated).unwrap();
    other.execute("CREATE TABLE notes (note TEXT)", []).unwrap();
    drop(other);

    assert!(matches!(
        open_device(&desktop),
        Err(DbError::ApplicationMismatch { .. })
    ));
    assert!(matches!(
        open_desktop(&device),
        Err(DbError::ApplicationMismatch { .. })
    ));
    assert!(matches!(
        open_device(&unrelated),
        Err(DbError::ApplicationMismatch { .. })
    ));
    assert_eq!(
        version(&open_desktop(&desktop).unwrap()),
        DESKTOP_SCHEMA_VERSION
    );
    assert_eq!(
        version(&open_device(&device).unwrap()),
        DEVICE_SCHEMA_VERSION
    );
    let other = Connection::open(&unrelated).unwrap();
    assert_eq!(version(&other), 0);
    assert_eq!(journal_mode(&other), "delete");
    assert!(other.prepare("SELECT note FROM notes").is_ok());
    assert!(other.prepare("SELECT value FROM state").is_err());
}

#[test]
fn newer_device_schema_is_rejected_without_overwriting_its_version() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("device.sqlite");
    let db = open_device(&path).unwrap();
    let newer = DEVICE_SCHEMA_VERSION + 1;
    db.pragma_update(None, "user_version", newer).unwrap();
    drop(db);

    assert!(matches!(
        open_device(&path),
        Err(DbError::SchemaTooNew { found, supported })
            if found == newer && supported == DEVICE_SCHEMA_VERSION
    ));
    let db = Connection::open(path).unwrap();
    assert_eq!(version(&db), newer);
}

#[test]
fn acknowledgments_preserve_events_and_track_removal_keeps_pending_history() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("device.sqlite");
    let db = open_device(&path).unwrap();
    let play = event(1, 10, DeviceEventKind::Play);
    let skip = event(2, 10, DeviceEventKind::Skip);
    let rating = event(3, 20, DeviceEventKind::Rating);
    let otg = event(4, 30, DeviceEventKind::OnTheGo);
    {
        let store = DeviceStore::new(&db);
        db.execute_batch(
            "INSERT INTO tracks (id, rel_path) VALUES ('a', 'a.flac');
             INSERT INTO playlists (id, name, kind) VALUES ('list', 'Favorites', 'manual');
             INSERT INTO playlist_items VALUES ('list', 0, 'a');",
        )
        .unwrap();
        for e in [&otg, &rating, &skip, &play] {
            store.append_event(e).unwrap();
        }
        assert!(store.append_event(&play).is_err());
        assert_eq!(
            store.pending_events(2).unwrap(),
            [play.clone(), skip.clone()]
        );
        assert!(store.pending_events(0).unwrap().is_empty());
        db.execute("DELETE FROM tracks WHERE id = 'a'", []).unwrap();
        let playlist_items: i64 = db
            .query_row("SELECT count(*) FROM playlist_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(playlist_items, 0);
        assert_eq!(
            store
                .acknowledge_events(&[play.event_id, play.event_id, rating.event_id, Uuid::nil()])
                .unwrap(),
            2
        );
        assert_eq!(store.acknowledge_events(&[rating.event_id]).unwrap(), 0);
    }
    drop(db);

    let db = open_device(&path).unwrap();
    let store = DeviceStore::new(&db);
    assert_eq!(store.pending_events(20).unwrap(), [skip, otg]);
    let remaining: i64 = db
        .query_row("SELECT count(*) FROM events", [], |row| row.get(0))
        .unwrap();
    let original_play = db
        .query_row(
            "SELECT ts, kind, payload, acked FROM events WHERE event_id = ?1",
            [play.event_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, bool>(3)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(remaining, 4);
    assert_eq!(
        original_play,
        (
            play.ts,
            "play".into(),
            serde_json::to_string(&play.payload).unwrap(),
            true
        )
    );
}

#[test]
fn event_payload_and_ack_history_are_immutable_even_through_raw_sql() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("device.sqlite");
    let id = Uuid::from_u128(1);
    let db = open_device(&path).unwrap();
    let store = DeviceStore::new(&db);
    store
        .append_event(&event(1, 10, DeviceEventKind::Play))
        .unwrap();
    store.acknowledge_events(&[id]).unwrap();
    drop(db);

    // External connections can have recursive_triggers off. REPLACE must
    // still fail before SQLite can silently discard the earlier event.
    let db = Connection::open(&path).unwrap();
    db.pragma_update(None, "recursive_triggers", false).unwrap();
    for sql in [
        "UPDATE events SET ts = 999 WHERE event_id = ?1",
        "UPDATE events SET kind = 'skip' WHERE event_id = ?1",
        "UPDATE events SET payload = '{\"changed\":true}' WHERE event_id = ?1",
        "UPDATE events SET acked = 0 WHERE event_id = ?1",
        "UPDATE events SET event_id = zeroblob(16) WHERE event_id = ?1",
        "DELETE FROM events WHERE event_id = ?1",
        "INSERT OR REPLACE INTO events (event_id, ts, kind, payload) VALUES (?1, 99, 'skip', '{}')",
    ] {
        assert!(db.execute(sql, [id]).is_err(), "{sql}");
    }
    assert!(db.execute(
        "INSERT INTO events (event_id, ts, kind, payload, acked) VALUES (?1, 1, 'play', '{}', 1)",
        [Uuid::from_u128(2)],
    ).is_err());
    assert!(
        db.execute(
            "INSERT INTO events (event_id, ts, kind, payload) VALUES (?1, 1, 'play', '[]')",
            [Uuid::from_u128(3)],
        )
        .is_err()
    );
    let acked: bool = db
        .query_row(
            "SELECT acked FROM events WHERE event_id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(acked);
}

#[test]
fn device_state_persists_json_and_restores_previous_values_after_rollback() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("device.sqlite");
    let mut db = open_device(&path).unwrap();
    let resume = json!({ "queue": ["a", "b"], "trackIndex": 1, "positionMs": 3837 });
    {
        let store = DeviceStore::new(&db);
        assert_eq!(store.state("absent").unwrap(), None);
        store.set_state("resume", &resume).unwrap();
        store.set_state("theme", &json!("light")).unwrap();
        store.set_state("theme", &json!("dark")).unwrap();
        store.set_state("eq", &Value::Null).unwrap();
    }
    let transaction = db.transaction().unwrap();
    DeviceStore::new(&transaction)
        .set_state("resume", &json!({ "positionMs": 0 }))
        .unwrap();
    transaction.rollback().unwrap();
    drop(db);

    let db = open_device(&path).unwrap();
    let store = DeviceStore::new(&db);
    assert_eq!(store.state("resume").unwrap(), Some(resume));
    assert_eq!(store.state("theme").unwrap(), Some(json!("dark")));
    assert_eq!(store.state("eq").unwrap(), Some(Value::Null));
}

fn event(id: u128, ts: i64, kind: DeviceEventKind) -> DeviceEvent {
    DeviceEvent {
        event_id: Uuid::from_u128(id),
        ts,
        kind,
        payload: Map::from_iter([("track_id".to_owned(), json!("a"))]),
    }
}

fn version(db: &Connection) -> u32 {
    db.pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version")
}

fn journal_mode(db: &Connection) -> String {
    db.pragma_query_value(None, "journal_mode", |row| row.get(0))
        .expect("read journal mode")
}
