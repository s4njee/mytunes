//! Generates design tokens for both apps from `design/tokens.json`.
//!
//! ```text
//! cargo run -p gen-tokens            # write the generated files
//! cargo run -p gen-tokens -- --check # exit 1 if they are stale (CI)
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use gen_tokens::{Tokens, css, dart};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn main() -> ExitCode {
    let check = std::env::args().any(|a| a == "--check");
    let root = repo_root();

    let source = match std::fs::read_to_string(root.join("design/tokens.json")) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read design/tokens.json: {e}");
            return ExitCode::FAILURE;
        }
    };
    let tokens = match Tokens::parse(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("invalid design/tokens.json: {e}");
            return ExitCode::FAILURE;
        }
    };

    let outputs = [
        ("apps/desktop/src/theme/tokens.css", css(&tokens)),
        ("apps/mobile/lib/theme/tokens.g.dart", dart(&tokens)),
    ];

    let mut stale = false;
    for (rel, content) in outputs {
        let path = root.join(rel);
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        if current == content {
            continue;
        }
        if check {
            eprintln!("stale: {rel} (run `cargo run -p gen-tokens`)");
            stale = true;
            continue;
        }
        if let Some(dir) = path.parent()
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            eprintln!("cannot create {}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
        if let Err(e) = std::fs::write(&path, content) {
            eprintln!("cannot write {rel}: {e}");
            return ExitCode::FAILURE;
        }
        println!("wrote {rel}");
    }

    if stale {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
