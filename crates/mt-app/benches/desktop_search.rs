//! Search through the desktop view and paging APIs with 50,000 matches.

use std::time::{Duration, Instant};

use mt_app::library::{DesktopLibrary, ViewSpec};
use mt_db::open_desktop;

const TRACK_COUNT: u32 = 50_000;
const PAGE_SIZE: u32 = 200;
const SEARCH_BUDGET: Duration = Duration::from_millis(50);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite");
    open_desktop(&path)?.execute(
        "WITH RECURSIVE seed(n) AS (VALUES (1) UNION ALL SELECT n + 1 FROM seed WHERE n < ?1)
         INSERT INTO tracks (id, file_path, title, artist, album, genre, duration_ms, size_bytes)
         SELECT printf('id-%05d', n), printf('/music/%05d.flac', n), printf('Song %05d', n),
                printf('Artist %04d', n / 50), printf('Album %04d', n / 20),
                printf('Genre %d', n % 8), 90000, 2000000 FROM seed",
        [TRACK_COUNT],
    )?;
    let library = DesktopLibrary::open(path)?;
    library.view_open(&ViewSpec::default())?;
    let spec = ViewSpec {
        query: Some("song".into()),
        ..ViewSpec::default()
    };

    let mut samples = [Duration::ZERO; 9];
    for elapsed in &mut samples {
        let start = Instant::now();
        let view = library.view_open(&spec)?;
        let page = library.view_rows(&view.view_id, 0, PAGE_SIZE)?;
        *elapsed = start.elapsed();
        assert_eq!(view.count, TRACK_COUNT);
        assert_eq!(page.len(), PAGE_SIZE as usize);
    }
    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    println!(
        "desktop prefix search and first page: {:.2} ms",
        median.as_secs_f64() * 1000.0
    );
    if !cfg!(debug_assertions) && median >= SEARCH_BUDGET {
        return Err(format!("E7.S5 search budget exceeded: {median:?}").into());
    }
    Ok(())
}
