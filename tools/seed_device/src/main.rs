use std::path::PathBuf;
use std::{env, fs};

use anyhow::{Context, Result, ensure};
use mt_db::seed::seed_device;

fn main() -> Result<()> {
    let mut args = env::args_os().skip(1);
    let usage = "Usage: seed-device <output-directory> <file-or-folder>...";
    let root = PathBuf::from(args.next().context(usage)?);
    if root.as_os_str() == "--help" || root.as_os_str() == "-h" {
        println!("{usage}");
        return Ok(());
    }
    let sources: Vec<_> = args.map(PathBuf::from).collect();
    ensure!(!sources.is_empty(), "{usage}");
    fs::create_dir_all(&root)?;
    let root = root.canonicalize()?;
    let destination = root.join("device.sqlite");
    let mut db = mt_db::open_device(&destination)?;
    let report = seed_device(&mut db, &root, &sources)?;
    drop(db);
    println!(
        "{} imported, {} already present",
        report.imported, report.unchanged
    );
    println!("Device library: {}", destination.display());
    for error in &report.errors {
        eprintln!("{error}");
    }
    ensure!(
        report.errors.is_empty(),
        "{} inputs failed to import",
        report.errors.len()
    );
    Ok(())
}
