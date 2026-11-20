//! Desktop application use cases, independent of Tauri.
//!
//! The Tauri command layer (`apps/desktop/src-tauri`) is a thin wrapper over
//! the functions in this crate, so they can be unit-tested without a webview.

pub mod import;
pub mod library;
mod settings;
pub mod window;

pub use mt_db::ArtworkSize;
pub use mt_db::queries::{BrowserFacets, TrackRow};

use mt_types::Pong;

/// Connectivity check used by the desktop frontend.
pub fn ping() -> Pong {
    Pong::new()
}

#[cfg(test)]
mod tests {
    #[test]
    fn ping_returns_pong() {
        assert_eq!(super::ping().message, "pong");
    }
}
