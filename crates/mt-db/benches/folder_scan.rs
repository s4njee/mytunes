use std::fs;
use std::time::Instant;

use gen_fixtures::{sine, wav_bytes};
use mt_db::{open_desktop, scan_library_folder};

fn main() {
    const FILES: usize = 10_000;
    let scratch = tempfile::tempdir().expect("temporary scan directory");
    let media = scratch.path().join("media");
    let fixture = wav_bytes(&sine(44_100, 16, &[440.0, 660.0], 3.0, 0.5));

    for album in 0..100 {
        let folder = media.join(format!("album-{album:03}"));
        fs::create_dir_all(&folder).expect("create album");
        for track in 0..FILES / 100 {
            fs::write(folder.join(format!("{track:03}.wav")), &fixture).expect("write fixture");
        }
    }

    let mut db = open_desktop(scratch.path().join("library.sqlite")).expect("open database");
    let start = Instant::now();
    let mut progress_times = Vec::new();
    let imported = scan_library_folder(&mut db, &media, |_| {
        progress_times.push(start.elapsed());
    })
    .expect("import fixtures");
    println!(
        "Imported {FILES} fixture-sized WAV files in {:.2?}",
        start.elapsed()
    );
    println!("{progress_times:?} since import start for progress callbacks");
    assert_eq!(imported.read, FILES);
    assert!(imported.file_errors.is_empty() && imported.walk_errors.is_empty());

    let start = Instant::now();
    let rescanned = scan_library_folder(&mut db, &media, |_| {}).expect("rescan fixtures");
    println!(
        "Rescanned {FILES} unchanged files in {:.2?}",
        start.elapsed()
    );
    assert_eq!(rescanned.unchanged, FILES);
    assert_eq!(rescanned.read, 0);
}
