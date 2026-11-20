use tauri_build::{AppManifest, Attributes};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = AppManifest::new().commands(&[
        "ping",
        "view_open",
        "view_rows",
        "view_selection_stats",
        "browser_facets",
        "window_settings_get",
        "window_settings_save",
        "import_choose_folder",
        "import_settings_get",
        "import_set_copy",
        "import_add_watch",
        "import_remove_watch",
        "play_view",
        "player_toggle",
        "player_control",
        "track_get",
        "track_set_checked",
        "track_reveal",
        "tracks_remove",
        "subscribe_player",
        "subscribe_scan",
        "subscribe_sync",
    ]);
    tauri_build::try_build(Attributes::new().app_manifest(manifest))?;
    Ok(())
}
