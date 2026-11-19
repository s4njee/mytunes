//! MyTunes desktop backend.
//!
//! Commands are thin wrappers over the desktop use cases and managed channels.
//! Tauri Specta generates their TypeScript bindings on debug startup and in CI.

mod artwork;
pub mod events;

use std::sync::Arc;

use events::{DesktopChannels, DeviceOnline, LibraryChanged, SyncProgress};
use mt_app::{
    TrackRow,
    library::{AppError, DesktopLibrary, OpenedView, ViewSpec},
};
use mt_audio::PlayerEvent;
use mt_meta::scan::ScanProgress;
use mt_types::Pong;
use tauri::{Manager, State, Url, WebviewWindowBuilder, async_runtime, ipc::Channel};
use tauri_specta::{Builder, ErrorHandlingMode, collect_commands, collect_events};

/// Where the generated TypeScript bindings live, relative to this crate.
pub const BINDINGS_PATH: &str = "../src/bindings.ts";

/// Connectivity check between the webview and the Rust core.
#[tauri::command]
#[specta::specta]
fn ping() -> Result<Pong, AppError> {
    Ok(mt_app::ping())
}

/// Materialize a sorted track view and return its handle and count.
#[tauri::command]
#[specta::specta]
async fn view_open(
    library: State<'_, Arc<DesktopLibrary>>,
    spec: ViewSpec,
) -> Result<OpenedView, AppError> {
    let library = library.inner().clone();
    async_runtime::spawn_blocking(move || library.view_open(&spec))
        .await
        .map_err(|_| AppError::Unavailable)?
}

/// Fetch a page of at most 200 tracks for a previously opened view.
#[tauri::command]
#[specta::specta]
async fn view_rows(
    library: State<'_, Arc<DesktopLibrary>>,
    view_id: String,
    offset: u32,
    limit: u32,
) -> Result<Vec<TrackRow>, AppError> {
    let library = library.inner().clone();
    async_runtime::spawn_blocking(move || library.view_rows(&view_id, offset, limit))
        .await
        .map_err(|_| AppError::Unavailable)?
}

/// Subscribe to the current transport snapshot and subsequent player events.
#[tauri::command]
#[specta::specta]
fn subscribe_player(
    channels: State<'_, DesktopChannels>,
    channel: Channel<PlayerEvent>,
) -> Result<(), AppError> {
    channels.player.subscribe(channel)
}

/// Subscribe to library scan progress.
#[tauri::command]
#[specta::specta]
fn subscribe_scan(
    channels: State<'_, DesktopChannels>,
    channel: Channel<ScanProgress>,
) -> Result<(), AppError> {
    channels.scan.subscribe(channel)
}

/// Subscribe to device sync progress.
#[tauri::command]
#[specta::specta]
fn subscribe_sync(
    channels: State<'_, DesktopChannels>,
    channel: Channel<SyncProgress>,
) -> Result<(), AppError> {
    channels.sync.subscribe(channel)
}

/// Builds the Tauri Specta command and event registry.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .error_handling(ErrorHandlingMode::Result)
        .commands(collect_commands![
            ping,
            view_open,
            view_rows,
            subscribe_player,
            subscribe_scan,
            subscribe_sync
        ])
        .events(collect_events![LibraryChanged, DeviceOnline])
}

/// Writes the TypeScript bindings for [`specta_builder`] to [`BINDINGS_PATH`].
///
/// # Errors
/// Returns an error if a type cannot be exported or the file cannot be written.
pub fn export_bindings(builder: &Builder<tauri::Wry>) -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(BINDINGS_PATH);
    let directory = path
        .parent()
        .ok_or_else(|| std::io::Error::other("Missing bindings directory"))?;
    let staged = tempfile::NamedTempFile::new_in(directory)?;
    builder.export(specta_typescript::Typescript::default(), staged.path())?;
    let raw = std::fs::read_to_string(staged.path())?;
    let normalized = raw
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    if std::fs::read_to_string(&path).ok().as_deref() != Some(normalized.as_str()) {
        std::fs::write(staged.path(), normalized)?;
        staged.persist(path)?;
    }
    Ok(())
}

fn local_navigation(url: &Url, dev: Option<&Url>) -> bool {
    matches!(
        (url.scheme(), url.host_str(), url.port()),
        ("tauri", Some("localhost"), None) | ("http" | "https", Some("tauri.localhost"), None)
    ) || dev.is_some_and(|origin| url.origin() == origin.origin())
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
        .register_asynchronous_uri_scheme_protocol("mtart", |context, request, responder| {
            let library = context
                .app_handle()
                .try_state::<Arc<DesktopLibrary>>()
                .map(|state| state.inner().clone());
            async_runtime::spawn_blocking(move || {
                responder.respond(artwork::serve(library.as_deref(), &request));
            });
        })
        .manage(DesktopChannels::default())
        .setup(move |app| {
            let data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data)?;
            let library = DesktopLibrary::open(data.join("library.sqlite"))?;
            app.manage(Arc::new(library));
            builder.mount_events(app);
            let window = app
                .config()
                .app
                .windows
                .first()
                .ok_or(AppError::Unavailable)?;
            let dev = if cfg!(dev) {
                app.config().build.dev_url.clone()
            } else {
                None
            };
            WebviewWindowBuilder::from_config(app, window)?
                .on_navigation(move |url| local_navigation(url, dev.as_ref()))
                .build()?;
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
        assert_eq!(ping().expect("ping returns normally").message, "pong");
    }
    #[test]
    fn webview_navigation_stays_on_the_app_and_configured_dev_origin() {
        let dev = Url::parse("http://localhost:1420/").expect("parse dev origin");
        for local in [
            "tauri://localhost/index.html",
            "http://tauri.localhost/index.html",
            "http://localhost:1420/path",
        ] {
            assert!(local_navigation(
                &Url::parse(local).expect("parse local URL"),
                Some(&dev)
            ));
        }
        for remote in [
            "https://example.org",
            "http://localhost:1421/",
            "http://tauri.localhost.example.org/",
            "file:///tmp/test.html",
        ] {
            assert!(!local_navigation(
                &Url::parse(remote).expect("parse remote URL"),
                Some(&dev)
            ));
        }
    }
}
