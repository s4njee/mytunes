use mt_db::queries::{
    AlbumGroup, FacetCount, LibraryQueries, SortDirection, TrackColumn, TrackSearch, TrackSort,
};
use mt_db::{DESKTOP_SCHEMA_VERSION, open_desktop};
use mt_types::{PlaylistId, TrackId};
use rusqlite::Connection;
use tempfile::tempdir;

fn ids(values: &[&str]) -> Vec<TrackId> {
    values
        .iter()
        .map(|value| TrackId((*value).to_owned()))
        .collect()
}

fn sort(column: TrackColumn) -> TrackSort {
    TrackSort {
        column,
        direction: SortDirection::Ascending,
    }
}

#[test]
fn sorts_with_overrides_articles_and_unicode() {
    let dir = tempdir().unwrap();
    let db = open_desktop(dir.path().join("library.sqlite")).unwrap();
    db.execute_batch(
        "INSERT INTO tracks (id, file_path, title, sort_title, artist, sort_artist,
                             album_artist, sort_album_artist) VALUES
         ('custom', '/custom', 'Zoo', 'Aardvark', 'A Band', 'Zulu', 'Anna', 'Zulu'),
         ('beatles', '/beatles', 'Beatles', '', 'tHe Beatles', '', 'The Beatles', ''),
         ('edith', '/edith', 'Édith', NULL, 'Édith', NULL, 'Édith', NULL),
         ('zebra', '/zebra', 'zebra', NULL, 'Zebra', NULL, 'Zebra', NULL);",
    )
    .unwrap();
    let library = LibraryQueries::new(&db);
    assert_eq!(
        library
            .track_ids(sort(TrackColumn::Title), TrackSearch::All)
            .unwrap(),
        ids(&["custom", "beatles", "edith", "zebra"])
    );
    for column in [TrackColumn::Artist, TrackColumn::AlbumArtist] {
        assert_eq!(
            library.track_ids(sort(column), TrackSearch::All).unwrap(),
            ids(&["beatles", "edith", "zebra", "custom"])
        );
        assert_eq!(
            library
                .track_ids(
                    TrackSort {
                        column,
                        direction: SortDirection::Descending
                    },
                    TrackSearch::All
                )
                .unwrap(),
            ids(&["custom", "zebra", "edith", "beatles"])
        );
    }
    assert_eq!(
        library
            .artists()
            .unwrap()
            .into_iter()
            .map(|row| row.name)
            .collect::<Vec<_>>(),
        ["tHe Beatles", "Édith", "Zebra", "A Band"]
    );
}

#[test]
fn searches_indexed_tags_without_interpreting_literal_operators() {
    let dir = tempdir().unwrap();
    let db = open_desktop(dir.path().join("library.sqlite")).unwrap();
    db.execute_batch(
        "INSERT INTO tracks (id, file_path, title, artist, album_artist, genre) VALUES
         ('a', '/a', 'Blackbird', 'The Beatles', 'The Beatles', 'Folk'),
         ('b', '/b', 'Black Or White', 'Artist B', NULL, 'Pop'),
         ('c', '/c', 'Black Swan', 'Artist C', NULL, 'Classical');",
    )
    .unwrap();
    let library = LibraryQueries::new(&db);
    let tracks = |search| library.track_ids(TrackSort::default(), search);
    assert_eq!(
        tracks(TrackSearch::Prefix("black beatl")).unwrap(),
        ids(&["a"])
    );
    assert_eq!(
        tracks(TrackSearch::Prefix("black OR")).unwrap(),
        ids(&["b"])
    );
    assert_eq!(
        tracks(TrackSearch::Prefix("\"Black\" White")).unwrap(),
        ids(&["b"])
    );
    assert_eq!(
        tracks(TrackSearch::Match(
            "album_artist:beatles OR genre:classical"
        ))
        .unwrap(),
        ids(&["c", "a"])
    );
    assert_eq!(
        tracks(TrackSearch::Prefix("  ")).unwrap(),
        ids(&["b", "c", "a"])
    );
    assert!(tracks(TrackSearch::Match("(")).is_err());
}

#[test]
fn pages_follow_id_order_even_with_duplicates_and_unavailable_ids() {
    let dir = tempdir().unwrap();
    let db = open_desktop(dir.path().join("library.sqlite")).unwrap();
    db.execute_batch(
        "INSERT INTO artwork VALUES ('cover', X'AB', X'CD');
         INSERT INTO tracks (id, file_path, title, artist, duration_ms, rating, missing, checked,
                             codec, bitrate, size_bytes, year, artwork_id)
         VALUES ('a', '/a', 'Title A', 'Artist', 173000, 80, 1, 0, 'flac', 950, 42231, 1999, 'cover'),
                ('b', '/b', NULL, NULL, NULL, 0, 0, 1, NULL, NULL, NULL, NULL, NULL);",
    ).unwrap();
    let library = LibraryQueries::new(&db);
    let rows = library
        .track_page(&ids(&["b", "missing", "a", "b"]))
        .unwrap();
    assert_eq!(
        rows.iter().map(|row| row.id.clone()).collect::<Vec<_>>(),
        ids(&["b", "a", "b"])
    );
    assert_eq!(rows[0], rows[2]);
    assert_eq!(rows[0].title, None);
    let track = &rows[1];
    assert_eq!(track.title.as_deref(), Some("Title A"));
    assert_eq!(track.artist.as_deref(), Some("Artist"));
    assert_eq!(
        (track.missing, track.checked, track.rating),
        (true, false, 80)
    );
    assert_eq!(
        (track.duration_ms, track.bitrate, track.size_bytes),
        (Some(173000), Some(950), Some(42231))
    );
    assert_eq!(track.artwork_id.as_deref(), Some("cover"));
    assert!(library.track_page(&[]).unwrap().is_empty());
}

#[test]
fn keeps_album_releases_separate_and_disc_order_when_sorting() {
    let dir = tempdir().unwrap();
    let db = open_desktop(dir.path().join("library.sqlite")).unwrap();
    db.execute_batch(
        "INSERT INTO tracks (id, file_path, album, sort_album, album_artist, artist, year,
                             compilation, disc_no, track_no) VALUES
         ('two', '/two', 'Collection', NULL, 'Solo One', 'Solo One', 2020, 1, 2, 1),
         ('one', '/one', 'Collection', '', 'Solo Two', 'Solo Two', 2020, 1, 1, 2),
         ('zero', '/zero', 'Collection', NULL, NULL, 'Solo Three', 2020, 1, 1, 1),
         ('later', '/later', 'Collection', NULL, NULL, 'Solo One', 2024, 0, 1, 1),
         ('earlier', '/earlier', 'Collection', NULL, NULL, 'Solo One', 2020, 0, 1, 1),
         ('override', '/override', 'Zoo', 'Aardvark', NULL, NULL, NULL, 0, 1, 1);",
    )
    .unwrap();
    let library = LibraryQueries::new(&db);
    let groups = library.albums().unwrap();
    assert_eq!(groups.len(), 4);
    assert_eq!(groups[0].title, "Zoo");
    assert!(groups.contains(&AlbumGroup {
        title: "Collection".into(),
        artist: Some("Various Artists".into()),
        year: Some(2020),
        compilation: true,
        track_count: 3,
    }));
    assert_eq!(
        groups
            .iter()
            .filter(|group| group.artist.as_deref() == Some("Solo One"))
            .count(),
        2
    );
    assert_eq!(
        library
            .track_ids(sort(TrackColumn::Album), TrackSearch::All)
            .unwrap(),
        ids(&["override", "earlier", "later", "zero", "one", "two"])
    );
    assert_eq!(
        library
            .track_ids(
                TrackSort {
                    column: TrackColumn::Album,
                    direction: SortDirection::Descending
                },
                TrackSearch::All,
            )
            .unwrap(),
        ids(&["zero", "one", "two", "later", "earlier", "override"])
    );
}

#[test]
fn counts_facets_and_keeps_stored_playlist_order() {
    let dir = tempdir().unwrap();
    let db = open_desktop(dir.path().join("library.sqlite")).unwrap();
    db.execute_batch(
        "INSERT INTO tracks (id, file_path, artist, genre, composer) VALUES
         ('a', '/a', 'Band', 'Ambient', 'Édith'),
         ('b', '/b', 'Band', 'Ambient', 'Zebra'),
         ('c', '/c', NULL, '', NULL);
         INSERT INTO playlists (id, name, kind) VALUES ('playlist', 'Playlist', 'manual');
         INSERT INTO playlist_items VALUES ('playlist', 20, 'b'), ('playlist', 10, 'a'), ('playlist', 30, 'a');",
    ).unwrap();
    let library = LibraryQueries::new(&db);
    assert_eq!(
        library.artists().unwrap(),
        [FacetCount {
            name: "Band".into(),
            track_count: 2
        }]
    );
    assert_eq!(
        library.genres().unwrap(),
        [FacetCount {
            name: "Ambient".into(),
            track_count: 2
        }]
    );
    assert_eq!(
        library.composers().unwrap(),
        [
            FacetCount {
                name: "Édith".into(),
                track_count: 1
            },
            FacetCount {
                name: "Zebra".into(),
                track_count: 1
            },
        ]
    );
    assert_eq!(
        library
            .playlist_track_ids(&PlaylistId("playlist".into()))
            .unwrap(),
        ids(&["a", "b", "a"])
    );
    assert!(
        library
            .playlist_track_ids(&PlaylistId("other".into()))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn migrates_a_populated_v1_without_losing_tracks_or_search_entries() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("library.sqlite");
    let v1 = Connection::open(&path).unwrap();
    v1.execute_batch(include_str!("../migrations/desktop/V1.sql"))
        .unwrap();
    v1.pragma_update(None, "user_version", 1).unwrap();
    v1.execute(
        "INSERT INTO tracks (id, file_path, title, artist) VALUES ('a', '/a', 'Águas', 'The Band')",
        [],
    )
    .unwrap();
    drop(v1);

    let db = open_desktop(&path).unwrap();
    let version: u32 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, DESKTOP_SCHEMA_VERSION);
    let library = LibraryQueries::new(&db);
    assert_eq!(
        library
            .track_ids(sort(TrackColumn::Artist), TrackSearch::Match("title:aguas"))
            .unwrap(),
        ids(&["a"])
    );
}

#[test]
fn albums_with_the_same_custom_sort_name_stay_together() {
    let dir = tempdir().unwrap();
    let db = open_desktop(dir.path().join("library.sqlite")).unwrap();
    db.execute_batch(
        "INSERT INTO tracks (id, file_path, album, sort_album, artist, disc_no, track_no) VALUES
         ('a2', '/a2', 'First', 'Shared', 'Band', 1, 2),
         ('b1', '/b1', 'Second', 'Shared', 'Band', 1, 1),
         ('a1', '/a1', 'First', 'Shared', 'Band', 1, 1),
         ('b2', '/b2', 'Second', 'Shared', 'Band', 1, 2);",
    )
    .unwrap();
    let library = LibraryQueries::new(&db);
    assert_eq!(
        library
            .track_ids(sort(TrackColumn::Album), TrackSearch::All)
            .unwrap(),
        ids(&["a1", "a2", "b1", "b2"])
    );
}
