use tauri_build::{AppManifest, Attributes};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = AppManifest::new().commands(&[
        "ping",
        "view_open",
        "view_rows",
        "window_settings_get",
        "window_settings_save",
        "subscribe_player",
        "subscribe_scan",
        "subscribe_sync",
    ]);
    tauri_build::try_build(Attributes::new().app_manifest(manifest))?;
    Ok(())
}
