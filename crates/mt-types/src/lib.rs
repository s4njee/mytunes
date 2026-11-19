//! Shared domain types for the MyTunes core.
//!
//! Types here derive `serde` (for the sync protocol and storage) and `specta`
//! (so the desktop frontend gets matching TypeScript types via tauri-specta).

use serde::{Deserialize, Serialize};

/// Version of the Rust core, shared by both apps.
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Stable public identifier of a library track.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(transparent)]
pub struct TrackId(pub String);

/// Stable public identifier of a library playlist.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(transparent)]
pub struct PlaylistId(pub String);

/// Reply to a connectivity check between a UI and the core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Pong {
    /// Always `"pong"`.
    pub message: String,
    /// The core version that answered.
    pub core_version: String,
}

impl Pong {
    /// Builds the standard reply.
    pub fn new() -> Self {
        Self {
            message: "pong".to_owned(),
            core_version: CORE_VERSION.to_owned(),
        }
    }
}

impl Default for Pong {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pong_says_pong() {
        let p = Pong::new();
        assert_eq!(p.message, "pong");
        assert_eq!(p.core_version, CORE_VERSION);
    }
}
