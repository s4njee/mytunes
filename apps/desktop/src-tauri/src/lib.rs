//! MyTunes desktop backend.
//!
//! Commands here are thin wrappers over [`mt_app`]. Their TypeScript bindings
//! are generated into `apps/desktop/src/bindings.ts` by tauri-specta: on every
//! debug run, and by the `bindings_are_exported` test (which CI uses to check
//! that the committed file is up to date).

use mt_types::Pong;
use tauri_specta::{Builder, ErrorHandlingMode, collect_commands};

/// Where the generated TypeScript bindings live, relative to this crate.
pub const BINDINGS_PATH: &str = "../src/bindings.ts";

/// Connectivity check between the webview and the Rust core.
#[tauri::command]
#[specta::specta]
fn ping() -> Pong {
    mt_app::ping()
}

/// Builds the tauri-specta builder holding every command and event.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .error_handling(ErrorHandlingMode::Result)
        .commands(collect_commands![ping])
}

/// Writes the TypeScript bindings for [`specta_builder`] to [`BINDINGS_PATH`].
pub fn export_bindings(builder: &Builder<tauri::Wry>) -> Result<(), String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(BINDINGS_PATH);
    builder
        .export(specta_typescript::Typescript::default(), path)
        .map_err(|e| e.to_string())
}

/// Starts the desktop app.
pub fn run() {
    let builder = specta_builder();

    #[cfg(debug_assertions)]
    if let Err(e) = export_bindings(&builder) {
        eprintln!("failed to export TypeScript bindings: {e}");
    }

    let result = tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!());

    if let Err(e) = result {
        eprintln!("error while running MyTunes: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_are_exported() {
        export_bindings(&specta_builder()).expect("export bindings");
    }

    #[test]
    fn ping_returns_pong() {
        assert_eq!(ping().message, "pong");
    }
}
