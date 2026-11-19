use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use gen_fixtures::{sine, wav_bytes};
use mt_db::{ImportError, open_desktop, scan_library_folder};
use mt_meta::scan::ScanFileError;
use rusqlite::Connection;
use tempfile::tempdir;

fn write_tone(path: &Path, seconds: f64) {
    fs::write(path, wav_bytes(&sine(44_100, 16, &[440.0], seconds, 0.5)))
        .expect("write audio fixture");
}

fn set_modified(path: &Path, offset_ns: u64) {
    File::options()
        .write(true)
        .open(path)
        .expect("open fixture for timestamp update")
        .set_modified(
            UNIX_EPOCH + Duration::from_secs(1_700_000_000) + Duration::from_nanos(offset_ns),
        )
        .expect("set fixture timestamp");
}

fn track(db: &Connection, title: &str) -> (String, bool, i64, i64, i64) {
    db.query_row(
        "SELECT id, missing, file_rev, duration_ms, rating FROM tracks WHERE title = ?",
        [title],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        },
    )
    .expect("find scanned track")
}

#[test]
fn imports_supported_files_and_reconciles_rescans_without_losing_track_history() {
    let dir = tempdir().unwrap();
    let library = dir.path().join("library.sqlite");
    let mut db = open_desktop(&library).unwrap();
    let media = dir.path().join("media");
    let album = media.join("album");
    fs::create_dir_all(&album).unwrap();
    let first = album.join("First.WAV");
    let second = album.join("Second.wav");
    let overlay = album.join("ignored.txt");
    write_tone(&first, 0.1);
    write_tone(&second, 0.2);
    write_tone(&overlay, 0.1);
    set_modified(&first, 0);

    let separate = dir.path().join("media-other");
    fs::create_dir(&separate).unwrap();
    write_tone(&separate.join("Outside.wav"), 0.1);
    scan_library_folder(&mut db, &separate, |_| {}).unwrap();
    fs::remove_file(separate.join("Outside.wav")).unwrap();

    let mut events = Vec::new();
    let report = scan_library_folder(&mut db, &media, |progress| events.push(progress)).unwrap();
    assert_eq!(
        (report.read, report.unchanged, report.marked_missing),
        (2, 0, 0)
    );
    assert!(report.file_errors.is_empty());
    assert!(report.walk_errors.is_empty());
    assert_eq!(events.first().unwrap().scanned, 0);
    assert_eq!(events.last().unwrap().scanned, 2);
    assert_eq!(events.last().unwrap().total_estimate, 2);
    assert!(
        events
            .windows(2)
            .all(|pair| pair[0].scanned <= pair[1].scanned
                && pair[0].total_estimate <= pair[1].total_estimate)
    );
    assert!(
        events
            .last()
            .unwrap()
            .current_path
            .starts_with(media.canonicalize().unwrap())
    );

    let (first_id, _, _, _, _) = track(&db, "First");
    let (second_id, _, _, _, _) = track(&db, "Second");
    assert_eq!(
        uuid::Uuid::parse_str(&first_id).unwrap().get_version_num(),
        7
    );
    db.execute("UPDATE tracks SET rating = 80 WHERE id = ?", [&first_id])
        .unwrap();
    db.execute(
        "INSERT INTO playlists (id, name, kind) VALUES ('p', 'Favorites', 'manual')",
        [],
    )
    .unwrap();
    db.execute(
        "INSERT INTO playlist_items VALUES ('p', 0, ?)",
        [&second_id],
    )
    .unwrap();

    drop(db);
    let mut db = open_desktop(&library).unwrap();
    let report = scan_library_folder(&mut db, &media, |_| {}).unwrap();
    assert_eq!((report.read, report.unchanged), (0, 2));

    set_modified(&first, 50_000);
    let report = scan_library_folder(&mut db, &media, |_| {}).unwrap();
    assert_eq!((report.read, report.unchanged), (1, 1));
    assert_eq!(track(&db, "First"), (first_id.clone(), false, 1, 100, 80));

    write_tone(&first, 0.3);
    set_modified(&first, 50_000);
    fs::remove_file(&second).unwrap();
    fs::create_dir(&second).unwrap();
    let report = scan_library_folder(&mut db, &media, |_| {}).unwrap();
    assert_eq!(
        (report.read, report.unchanged, report.marked_missing),
        (1, 0, 1)
    );
    assert_eq!(track(&db, "First"), (first_id, false, 2, 300, 80));
    assert_eq!(track(&db, "Second"), (second_id.clone(), true, 1, 200, 0));
    assert!(!track(&db, "Outside").1);
    let playlist_track: String = db
        .query_row(
            "SELECT track_id FROM playlist_items WHERE playlist_id = 'p'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(playlist_track, second_id);

    fs::remove_dir(&second).unwrap();
    write_tone(&second, 0.2);
    let report = scan_library_folder(&mut db, &media, |_| {}).unwrap();
    assert_eq!(
        (report.read, report.unchanged, report.marked_missing),
        (1, 1, 0)
    );
    assert_eq!(track(&db, "Second"), (second_id, false, 1, 200, 0));
}

#[test]
fn corrupt_files_and_a_missing_root_preserve_previous_library_rows() {
    let dir = tempdir().unwrap();
    let media = dir.path().join("media");
    fs::create_dir(&media).unwrap();
    let path = media.join("Track.wav");
    write_tone(&path, 0.1);
    let mut db = open_desktop(dir.path().join("library.sqlite")).unwrap();
    scan_library_folder(&mut db, &media, |_| {}).unwrap();
    let prior = track(&db, "Track");

    fs::write(&path, b"truncated WAV").unwrap();
    let report = scan_library_folder(&mut db, &media, |_| {}).unwrap();
    assert_eq!(report.read, 0);
    assert_eq!(report.marked_missing, 0);
    assert!(matches!(
        report.file_errors.as_slice(),
        [(_, ScanFileError::Metadata(_))]
    ));
    assert_eq!(track(&db, "Track"), prior);

    fs::rename(&media, dir.path().join("offline")).unwrap();
    assert!(matches!(
        scan_library_folder(&mut db, &media, |_| {}),
        Err(ImportError::Folder { .. })
    ));
    assert_eq!(track(&db, "Track"), prior);
}

#[test]
fn database_failure_rolls_back_the_whole_import() {
    let dir = tempdir().unwrap();
    let media = dir.path().join("media");
    fs::create_dir(&media).unwrap();
    write_tone(&media.join("One.wav"), 0.1);
    write_tone(&media.join("Two.wav"), 0.1);
    let mut db = open_desktop(dir.path().join("library.sqlite")).unwrap();
    db.execute_batch(
        "CREATE TRIGGER stop_second_insert BEFORE INSERT ON tracks
         WHEN EXISTS (SELECT 1 FROM tracks)
         BEGIN SELECT RAISE(ABORT, 'simulated failed write'); END;",
    )
    .unwrap();

    assert!(matches!(
        scan_library_folder(&mut db, &media, |_| {}),
        Err(ImportError::Database(_))
    ));
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM tracks", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[cfg(unix)]
#[test]
fn skips_symlink_loops_and_links_outside_the_folder() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let media = dir.path().join("media");
    fs::create_dir(&media).unwrap();
    write_tone(&media.join("Real.wav"), 0.1);
    write_tone(&dir.path().join("Outside.wav"), 0.1);
    symlink(&media, media.join("loop")).unwrap();
    symlink(dir.path().join("Outside.wav"), media.join("Linked.wav")).unwrap();
    let mut db = open_desktop(dir.path().join("library.sqlite")).unwrap();
    let report = scan_library_folder(&mut db, &media, |_| {}).unwrap();
    assert_eq!(report.read, 1);
    assert!(report.file_errors.is_empty() && report.walk_errors.is_empty());
    assert!(!track(&db, "Real").1);
}

#[test]
fn migrated_tracks_establish_a_fingerprint_without_changing_identity_or_revision() {
    let dir = tempdir().unwrap();
    let media = dir.path().join("media");
    fs::create_dir(&media).unwrap();
    let path = media.join("Track.wav");
    write_tone(&path, 0.1);
    let library = dir.path().join("library.sqlite");
    let old = Connection::open(&library).unwrap();
    old.execute_batch(include_str!("../migrations/desktop/V1.sql"))
        .unwrap();
    old.pragma_update(None, "user_version", 1).unwrap();
    old.execute(
        "INSERT INTO tracks (id, file_path, file_rev, title, rating) VALUES ('old-id', ?, 6, 'Track', 80)",
        [path.canonicalize().unwrap().to_str().unwrap()],
    ).unwrap();
    drop(old);

    let mut db = open_desktop(&library).unwrap();
    assert_eq!(
        scan_library_folder(&mut db, &media, |_| {}).unwrap().read,
        1
    );
    assert_eq!(track(&db, "Track"), ("old-id".into(), false, 6, 100, 80));
    assert_eq!(
        scan_library_folder(&mut db, &media, |_| {})
            .unwrap()
            .unchanged,
        1
    );
}

#[test]
fn concurrent_scans_share_the_committed_fingerprint() {
    use std::sync::Barrier;

    let dir = tempdir().unwrap();
    let media = dir.path().join("media");
    fs::create_dir(&media).unwrap();
    write_tone(&media.join("Track.wav"), 0.1);
    let library = dir.path().join("library.sqlite");
    let mut first = open_desktop(&library).unwrap();
    let mut second = open_desktop(&library).unwrap();
    let start = Barrier::new(2);

    let (one, two) = std::thread::scope(|scope| {
        let other = scope.spawn(|| {
            start.wait();
            scan_library_folder(&mut second, &media, |_| {}).expect("first concurrent scan")
        });
        start.wait();
        let report = scan_library_folder(&mut first, &media, |_| {}).unwrap();
        (report, other.join().unwrap())
    });

    assert_eq!((one.read + two.read, one.unchanged + two.unchanged), (1, 1));
    assert_eq!(track(&first, "Track").2, 1);
}
