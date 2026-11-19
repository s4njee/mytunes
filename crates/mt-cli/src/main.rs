use std::collections::HashMap;
use std::error::Error;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use mt_audio::{Player, PlayerState, QueueItem};
use mt_db::queries::LibraryQueries;
use mt_db::{FolderImport, open_desktop, scan_library_folder};

const USAGE: &str = "Usage:
  mt-cli scan <folder> [--db <library.sqlite>]
  mt-cli play <folder> [--eq <60Hz,230Hz,910Hz,3.6kHz,14kHz>]

Scan defaults to library.sqlite in the current directory.
EQ gains are integers from -6 to +6 dB.";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let Some(command) = args.next() else {
        println!("{USAGE}");
        return Ok(());
    };
    if command == "--help" || command == "-h" {
        println!("{USAGE}");
        return Ok(());
    }
    let folder = args.next().ok_or(USAGE)?;
    let arguments: Vec<_> = args.collect();
    match command.to_str() {
        Some("scan") => {
            let library = match arguments.as_slice() {
                [] => PathBuf::from("library.sqlite"),
                [option, path] if option == "--db" => PathBuf::from(path),
                _ => return Err(USAGE.into()),
            };
            let mut db = open_desktop(library)?;
            report(&scan_library_folder(&mut db, Path::new(&folder), |_| {})?);
            Ok(())
        }
        Some("play") => play(Path::new(&folder), parse_eq(&arguments)?),
        _ => Err(USAGE.into()),
    }
}

fn parse_eq(args: &[OsString]) -> Result<[i8; 5], Box<dyn Error>> {
    let text = match args {
        [] => return Ok([0; 5]),
        [option, value] if option == "--eq" => value.to_str().ok_or(USAGE)?,
        _ => return Err(USAGE.into()),
    };
    let values = text
        .split(',')
        .map(str::parse::<i8>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "EQ needs five comma-separated gains from -6 to +6 dB")?;
    let eq: [i8; 5] = values
        .try_into()
        .map_err(|_| "EQ needs five comma-separated gains from -6 to +6 dB")?;
    if !eq.iter().all(|gain| (-6..=6).contains(gain)) {
        return Err("EQ gains must be from -6 to +6 dB".into());
    }
    Ok(eq)
}

fn report(scan: &FolderImport) {
    for (path, error) in &scan.file_errors {
        eprintln!("Skipped {} ({error})", path.display());
    }
    for error in &scan.walk_errors {
        eprintln!("{error}");
    }
    println!(
        "{} read, {} unchanged, {} marked missing",
        scan.read, scan.unchanged, scan.marked_missing
    );
}

fn play(folder: &Path, eq: [i8; 5]) -> Result<(), Box<dyn Error>> {
    let folder = folder.canonicalize()?;
    let temp = tempfile::tempdir()?;
    let mut library = open_desktop(temp.path().join("library.sqlite"))?;
    report(&scan_library_folder(&mut library, &folder, |_| {})?);
    let tracks = LibraryQueries::new(&library).folder_tracks(&folder)?;
    if tracks.is_empty() {
        return Err("No playable files found in the folder".into());
    }
    let mut titles = HashMap::new();
    let items = tracks
        .into_iter()
        .map(|track| {
            let title = track
                .title
                .unwrap_or_else(|| track.path.display().to_string());
            titles.insert(track.id.clone(), title);
            QueueItem {
                id: track.id,
                path: track.path,
                album: track.album_key,
            }
        })
        .collect();

    let player = Player::new()?;
    let events = player.events()?;
    player.set_eq(eq)?;
    player.load_queue(items, 0)?;
    player.play()?;
    for event in events {
        if let Some(error) = event.error {
            return Err(error.into());
        }
        if event.state == PlayerState::Ended {
            return Ok(());
        }
        if !event.ended
            && event.state == PlayerState::Playing
            && let Some(id) = event.track_id
            && let Some(title) = titles.remove(&id)
        {
            println!("Playing {title}");
        }
    }
    Err("Playback ended without a completion event".into())
}
