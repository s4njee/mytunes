//! E2.S2 release-mode benchmark against a file-backed, 50,000-track library.

use std::hint::black_box;
use std::time::{Duration, Instant};

use mt_db::open_desktop;
use mt_db::queries::{LibraryQueries, SortDirection, TrackColumn, TrackSearch, TrackSort};

const TRACKS: usize = 50_000;
const PAGE_SIZE: usize = 200;
const SAMPLES: usize = 9;
const SORT_BUDGET: Duration = Duration::from_millis(60);
const PAGE_BUDGET: Duration = Duration::from_millis(5);

const COLUMNS: &[TrackColumn] = &[
    TrackColumn::Checked,
    TrackColumn::Title,
    TrackColumn::Duration,
    TrackColumn::Artist,
    TrackColumn::Album,
    TrackColumn::AlbumArtist,
    TrackColumn::Genre,
    TrackColumn::Composer,
    TrackColumn::Year,
    TrackColumn::TrackNumber,
    TrackColumn::Rating,
    TrackColumn::PlayCount,
    TrackColumn::LastPlayed,
    TrackColumn::DateAdded,
    TrackColumn::Kind,
    TrackColumn::Size,
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let db = open_desktop(directory.path().join("library.sqlite"))?;
    db.execute_batch(
        "WITH RECURSIVE indices(n) AS (
            VALUES (0) UNION ALL SELECT n + 1 FROM indices WHERE n < 49999
         ),
         scrambled(x) AS (SELECT (n * 8191) % 50000 FROM indices)
         INSERT INTO tracks (
             id, file_path, title, artist, album_artist, album, composer, genre,
             duration_ms, year, disc_no, track_no, rating, play_count, last_played_at,
             date_added, codec, bitrate, size_bytes, checked
         )
         SELECT printf('018fe800-0000-7000-8000-%012x', x), printf('/music/%05d.flac', x),
                printf('%s Song %05d', CASE x % 4 WHEN 0 THEN 'Águas'
                       WHEN 1 THEN 'Echo' WHEN 2 THEN 'Night' ELSE 'Sunrise' END, x),
                printf('%s Artist %04d', CASE x % 3 WHEN 0 THEN 'The'
                       WHEN 1 THEN 'Édith' ELSE 'A' END, x / 50),
                printf('Ensemble %04d', x / 50), printf('Release %04d', x / 20),
                printf('Composer %04d', x % 1000),
                CASE x % 3 WHEN 0 THEN 'Ambient' WHEN 1 THEN 'Classical' ELSE 'Jazz' END,
                90_000 + x * 3, 1960 + (x / 20) % 65, (x / 10) % 2 + 1, x % 10 + 1,
                (x % 6) * 20, x % 25, 1700000000 + x * 15,
                1600000000 + x * 2, CASE x % 2 WHEN 0 THEN 'flac' ELSE 'mp3' END,
                192 + x % 1024, 2_000_000 + x * 2, x % 2
           FROM scrambled;
         ANALYZE;",
    )?;

    let library = LibraryQueries::new(&db);
    let mut failures = Vec::new();
    let directions = [SortDirection::Ascending, SortDirection::Descending];
    for &column in COLUMNS {
        for direction in directions {
            let sort = TrackSort { column, direction };
            let name = format!("{column:?}/{direction:?}");
            let warmup = library.track_ids(sort, TrackSearch::All)?;
            assert_eq!(warmup.len(), TRACKS);
            let elapsed = measure(|_| {
                black_box(library.track_ids(sort, TrackSearch::All)?);
                Ok(())
            })?;
            println!("snapshot {name}: {:.2} ms", ms(elapsed));
            if elapsed >= SORT_BUDGET {
                failures.push(format!("snapshot {name}"));
            }
        }
    }

    let ordered = library.track_ids(TrackSort::default(), TrackSearch::All)?;
    let elapsed = measure(|sample| {
        let offset = (sample * 3451) % (TRACKS - PAGE_SIZE);
        let ids = &ordered[offset..offset + PAGE_SIZE];
        let rows = black_box(library.track_page(ids)?);
        assert!(rows.iter().map(|row| &row.id).eq(ids));
        Ok(())
    })?;
    println!("page ({PAGE_SIZE} rows): {:.2} ms", ms(elapsed));
    if elapsed >= PAGE_BUDGET {
        failures.push("page".into());
    }
    if !cfg!(debug_assertions) && !failures.is_empty() {
        return Err(format!("E2.S2 median budget exceeded: {}", failures.join(", ")).into());
    }
    Ok(())
}

fn measure(
    mut query: impl FnMut(usize) -> Result<(), mt_db::queries::QueryError>,
) -> Result<Duration, mt_db::queries::QueryError> {
    let mut results = [Duration::ZERO; SAMPLES];
    for (sample, time) in results.iter_mut().enumerate() {
        let start = Instant::now();
        query(sample)?;
        *time = start.elapsed();
    }
    results.sort_unstable();
    Ok(results[SAMPLES / 2])
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
