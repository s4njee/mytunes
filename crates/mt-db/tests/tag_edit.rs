use std::fs;
use std::path::Path;

use gen_fixtures::{sine, wav_bytes};
use lofty::file::TaggedFileExt;
use lofty::tag::items::popularimeter::{Popularimeter, StarRating};
use lofty::tag::{Accessor, ItemKey, ItemValue, Tag, TagExt, TagItem, TagType};
use mt_db::{EditTrackError, edit_track_tags, open_desktop, scan_library_folder};
use mt_meta::read_metadata;
use mt_meta::tag_write::{TagEdit, TagEdits, TagWriteError};
use mt_types::TrackId;
use rusqlite::Connection;

fn import_tone(dir: &Path) -> (Connection, std::path::PathBuf, TrackId) {
    let path = dir.join("song.wav");
    fs::write(&path, wav_bytes(&sine(44_100, 16, &[440.0], 0.1, 0.5))).expect("write fixture WAV");
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_title("Original".into());
    tag.set_artist("Original artist".into());
    tag.set_comment("Keep this comment".into());
    tag.insert_text(ItemKey::Composer, "Keep this composer".into());
    tag.push(TagItem::new(
        ItemKey::Popularimeter,
        ItemValue::Text(Popularimeter::musicbee(StarRating::Two, 14).to_string()),
    ));
    tag.save_to_path(&path, Default::default())
        .expect("write fixture ID3");
    let mut db = open_desktop(dir.join("library.sqlite")).expect("open fresh library");
    scan_library_folder(&mut db, dir, |_| {}).expect("import fixture");
    let id = db
        .query_row("SELECT id FROM tracks", [], |row| row.get::<_, String>(0))
        .expect("find imported track");
    (db, path, TrackId(id))
}

fn rename() -> TagEdits {
    TagEdits {
        title: TagEdit::Set("Edited".into()),
        ..TagEdits::default()
    }
}

#[test]
fn saves_tags_and_fingerprint_without_changing_track_identity_or_revision() {
    let dir = tempfile::tempdir().unwrap();
    let (mut db, path, id) = import_tone(dir.path());
    db.execute("UPDATE tracks SET play_count = 6 WHERE id = ?1", [&id.0])
        .unwrap();
    let cover = {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(20, 30)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    };
    let edits = TagEdits {
        artist: TagEdit::Clear,
        front_cover: TagEdit::Set(cover.clone()),
        rating: TagEdit::Set(80),
        ..rename()
    };
    edit_track_tags(&mut db, &id, &edits).unwrap();
    let on_disk = read_metadata(&path).unwrap();
    assert_eq!(on_disk.title, "Edited");
    assert_eq!(on_disk.artist, None);
    assert_eq!(on_disk.composer.as_deref(), Some("Keep this composer"));
    let tags = lofty::read_from_path(&path).unwrap();
    let primary = tags.primary_tag().unwrap();
    assert_eq!(primary.comment().as_deref(), Some("Keep this comment"));
    let ratings = primary.ratings().collect::<Vec<_>>();
    assert!(
        ratings
            .iter()
            .any(|rate| rate.email() == "MyTunes" && rate.rating() == StarRating::Four)
    );
    assert!(
        ratings
            .iter()
            .any(|rate| rate.email() == "MusicBee" && rate.play_counter == 14)
    );
    assert_eq!(primary.pictures()[0].data(), cover.as_slice());

    let actual = db.query_row(
        "SELECT id, title, artist, rating, file_rev, play_count, artwork_id IS NOT NULL, size_bytes
         FROM tracks",
        [],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?, row.get::<_, i64>(3)?, row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?, row.get::<_, bool>(6)?, row.get::<_, i64>(7)?)),
    ).unwrap();
    assert_eq!(
        actual,
        (
            id.0.clone(),
            "Edited".into(),
            None,
            80,
            1,
            6,
            true,
            fs::metadata(&path).unwrap().len() as i64
        )
    );
    let report = scan_library_folder(&mut db, dir.path(), |_| {}).unwrap();
    assert_eq!((report.read, report.unchanged), (0, 1));

    let clear = TagEdits {
        title: TagEdit::Clear,
        rating: TagEdit::Clear,
        front_cover: TagEdit::Clear,
        ..TagEdits::default()
    };
    edit_track_tags(&mut db, &id, &clear).unwrap();
    assert_eq!(read_metadata(&path).unwrap().title, "song");
    let cleared = db
        .query_row(
            "SELECT title, rating, artwork_id, file_rev FROM tracks WHERE id = ?",
            [&id.0],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(cleared, ("song".into(), 0, None, 1));
}

#[test]
fn read_only_and_stale_sources_leave_the_library_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let (mut db, path, id) = import_tone(dir.path());
    let original = fs::read(&path).unwrap();
    let saved = fs::metadata(&path).unwrap().permissions();
    let mut readonly = saved.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let result = edit_track_tags(&mut db, &id, &rename());
    fs::set_permissions(&path, saved).unwrap();
    assert!(matches!(
        result,
        Err(EditTrackError::File(TagWriteError::ReadOnly))
    ));
    assert_eq!(fs::read(&path).unwrap(), original);

    let new_audio = wav_bytes(&sine(44_100, 16, &[880.0], 0.3, 0.5));
    fs::write(&path, &new_audio).unwrap();
    assert!(matches!(
        edit_track_tags(&mut db, &id, &rename()),
        Err(EditTrackError::File(TagWriteError::Changed))
    ));
    assert_eq!(fs::read(&path).unwrap(), new_audio);
    let original_title: String = db
        .query_row("SELECT title FROM tracks", [], |row| row.get(0))
        .unwrap();
    assert_eq!(original_title, "Original");
}

#[test]
fn restores_original_bytes_and_fingerprint_when_sqlite_commit_fails() {
    let dir = tempfile::tempdir().unwrap();
    let (mut db, path, id) = import_tone(dir.path());
    let original = fs::read(&path).unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    db.execute_batch(
        "CREATE TABLE parent (id INTEGER PRIMARY KEY);
         CREATE TABLE child (id INTEGER REFERENCES parent DEFERRABLE INITIALLY DEFERRED);
         CREATE TRIGGER reject_title_edit AFTER UPDATE OF title ON tracks
         WHEN new.title <> old.title
         BEGIN INSERT INTO child VALUES (42); END;",
    )
    .unwrap();

    assert!(matches!(
        edit_track_tags(&mut db, &id, &rename()),
        Err(EditTrackError::Database(_))
    ));
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    let title: String = db
        .query_row("SELECT title FROM tracks", [], |row| row.get(0))
        .unwrap();
    assert_eq!(title, "Original");
    assert_eq!(
        scan_library_folder(&mut db, dir.path(), |_| {})
            .unwrap()
            .unchanged,
        1
    );
}
