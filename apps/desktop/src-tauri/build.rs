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
    let mut attributes = Attributes::new().app_manifest(manifest);
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // Tauri's default resource reaches the app binary only. Embed the same
        // Common Controls v6 dependency in the app and the Rust test harnesses.
        // https://github.com/tauri-apps/tauri/issues/13419
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' \
             name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
             processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
    tauri_build::try_build(attributes)?;
    Ok(())
}
