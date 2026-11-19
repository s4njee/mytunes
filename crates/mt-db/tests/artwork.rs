use std::fs;
use std::io::Cursor;
use std::path::Path;

use gen_fixtures::{sine, wav_bytes};
use image::{ImageFormat, Rgb, RgbImage};
use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::{Accessor, ItemKey, Tag, TagExt, TagType};
use mt_db::{ArtworkSize, artwork_thumbnail, open_desktop, scan_library_folder};
use rusqlite::Connection;
use tempfile::tempdir;

const GREEN: [u8; 3] = [19, 195, 32];
const BLUE: [u8; 3] = [42, 60, 211];
const RED: [u8; 3] = [205, 29, 39];

fn cover(color: [u8; 3], format: ImageFormat) -> Vec<u8> {
    // A wide cover with strongly contrasting wings. A centered square
    // contains only the center stripe.
    let image = RgbImage::from_fn(240, 80, |x, _| {
        Rgb(if (80..160).contains(&x) {
            color
        } else {
            [245, 214, 38]
        })
    });
    let mut out = Cursor::new(Vec::new());
    image
        .write_to(&mut out, format)
        .expect("encode cover fixture");
    out.into_inner()
}

fn write_track(
    path: &Path,
    title: &str,
    album: &str,
    artist: &str,
    pictures: &[(PictureType, &[u8])],
) {
    fs::create_dir_all(path.parent().expect("track directory")).expect("create media folder");
    fs::write(path, wav_bytes(&sine(44_100, 16, &[440.0], 0.1, 0.5))).expect("write audio fixture");
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_title(title.into());
    tag.set_album(album.into());
    tag.set_artist(artist.into());
    tag.insert_text(ItemKey::AlbumArtist, artist.into());
    for &(kind, bytes) in pictures {
        tag.push_picture(
            Picture::unchecked(bytes.to_vec())
                .pic_type(kind)
                .mime_type(MimeType::Png)
                .build(),
        );
    }
    tag.save_to_path(path, WriteOptions::default())
        .expect("save fixture ID3 tag");
}

fn artwork(db: &Connection, title: &str) -> Option<String> {
    db.query_row(
        "SELECT artwork_id FROM tracks WHERE title = ?",
        [title],
        |row| row.get(0),
    )
    .expect("read cover for imported track")
}

fn count_artwork(db: &Connection) -> i64 {
    db.query_row("SELECT count(*) FROM artwork", [], |row| row.get(0))
        .expect("count artwork rows")
}

fn assert_thumbnails(db: &Connection, id: &str, expected: [u8; 3]) {
    for (size, side) in [(ArtworkSize::Small, 300), (ArtworkSize::Large, 600)] {
        let jpeg = artwork_thumbnail(db, id, size)
            .expect("load JPEG")
            .expect("stored cover");
        let decoded = image::load_from_memory_with_format(&jpeg, ImageFormat::Jpeg)
            .expect("decode thumbnail as JPEG")
            .into_rgb8();
        assert_eq!(decoded.dimensions(), (side, side));
        for (x, y) in [(4, side / 2), (side / 2, side / 2), (side - 5, side / 2)] {
            let actual = decoded.get_pixel(x, y).0;
            assert!(
                actual
                    .into_iter()
                    .zip(expected)
                    .all(|(a, b)| a.abs_diff(b) <= 12),
                "thumbnail pixel at {x},{y}: {actual:?}, expected {expected:?}"
            );
        }
    }
}

#[test]
fn prefers_front_cover_and_shares_one_digest_across_album_tracks_and_other_albums() {
    use PictureType::{CoverBack, CoverFront};

    let dir = tempdir().unwrap();
    let media = dir.path().join("media");
    let green = cover(GREEN, ImageFormat::Png);
    let red = cover(RED, ImageFormat::Png);
    write_track(
        &media.join("disc-1/1.wav"),
        "Front",
        "Record",
        "Artist",
        &[(CoverBack, &red), (CoverFront, &green)],
    );
    write_track(
        &media.join("disc-2/2.wav"),
        "No image",
        "Record",
        "Artist",
        &[],
    );
    write_track(
        &media.join("disc-2/3.wav"),
        "Other embedded",
        "Record",
        "Artist",
        &[(CoverBack, &red)],
    );
    write_track(
        &media.join("same/4.wav"),
        "Same cover",
        "Another record",
        "Other artist",
        &[(CoverFront, &green)],
    );
    write_track(
        &media.join("different/5.wav"),
        "Different artist",
        "Record",
        "Other artist",
        &[],
    );
    fs::write(
        media.join("disc-1/cover.jpg"),
        cover(BLUE, ImageFormat::Jpeg),
    )
    .unwrap();

    let mut db = open_desktop(dir.path().join("library.sqlite")).unwrap();
    let report = scan_library_folder(&mut db, &media, |_| {}).unwrap();
    assert_eq!(report.read, 5);
    assert!(report.file_errors.is_empty());
    let id = artwork(&db, "Front").unwrap();
    assert_thumbnails(&db, &id, GREEN);
    for title in ["No image", "Other embedded", "Same cover"] {
        assert_eq!(artwork(&db, title).as_deref(), Some(id.as_str()));
    }
    assert_eq!(artwork(&db, "Different artist"), None);
    assert_eq!(count_artwork(&db), 2);
    assert_eq!(
        artwork_thumbnail(&db, "unknown", ArtworkSize::Small).unwrap(),
        None
    );

    let report = scan_library_folder(&mut db, &media, |_| {}).unwrap();
    assert_eq!((report.read, report.unchanged), (0, 5));
    assert_eq!(artwork(&db, "Front").as_deref(), Some(id.as_str()));
    assert_eq!(count_artwork(&db), 2);

    fs::remove_file(media.join("disc-1/1.wav")).unwrap();
    let report = scan_library_folder(&mut db, &media, |_| {}).unwrap();
    assert_eq!(report.marked_missing, 1);
    let remaining_cover = artwork(&db, "Other embedded").unwrap();
    assert_thumbnails(&db, &remaining_cover, RED);
    assert_eq!(
        artwork(&db, "No image").as_deref(),
        Some(remaining_cover.as_str())
    );
    assert_eq!(artwork(&db, "Same cover").as_deref(), Some(id.as_str()));
}

#[test]
fn rechecks_folder_cover_precedence_and_removal_without_rereading_audio() {
    let dir = tempdir().unwrap();
    let media = dir.path().join("media");
    write_track(&media.join("1.wav"), "Song", "Record", "Artist", &[]);
    fs::write(media.join("Cover.JPG"), b"invalid image").unwrap();
    fs::write(media.join("folder.jpg"), cover(RED, ImageFormat::Jpeg)).unwrap();
    fs::write(media.join("Front.PNG"), cover(GREEN, ImageFormat::Png)).unwrap();

    let mut db = open_desktop(dir.path().join("library.sqlite")).unwrap();
    scan_library_folder(&mut db, &media, |_| {}).unwrap();
    let folder_id = artwork(&db, "Song").unwrap();
    assert_thumbnails(&db, &folder_id, RED);

    fs::write(media.join("Cover.JPG"), cover(BLUE, ImageFormat::Jpeg)).unwrap();
    let scan = scan_library_folder(&mut db, &media, |_| {}).unwrap();
    assert_eq!((scan.read, scan.unchanged), (0, 1));
    let cover_id = artwork(&db, "Song").unwrap();
    assert_ne!(cover_id, folder_id);
    assert_thumbnails(&db, &cover_id, BLUE);

    fs::remove_file(media.join("Cover.JPG")).unwrap();
    fs::remove_file(media.join("folder.jpg")).unwrap();
    assert_eq!(
        scan_library_folder(&mut db, &media, |_| {})
            .unwrap()
            .unchanged,
        1
    );
    assert_thumbnails(&db, &artwork(&db, "Song").unwrap(), GREEN);

    fs::remove_file(media.join("Front.PNG")).unwrap();
    assert_eq!(
        scan_library_folder(&mut db, &media, |_| {})
            .unwrap()
            .unchanged,
        1
    );
    assert_eq!(artwork(&db, "Song"), None);
}
