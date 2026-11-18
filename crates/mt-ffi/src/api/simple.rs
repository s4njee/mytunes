//! Connectivity check between Flutter and the Rust core.

/// Returns `"pong"`. Used by the mobile app's integration test (plan E0.S1 AC2).
#[flutter_rust_bridge::frb(sync)]
pub fn ping() -> String {
    mt_types::Pong::new().message
}

/// Version of the Rust core.
#[flutter_rust_bridge::frb(sync)]
pub fn core_version() -> String {
    mt_types::CORE_VERSION.to_owned()
}

/// Called once by `RustLib.init()`.
#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
}

#[cfg(test)]
mod tests {
    #[test]
    fn ping_returns_pong() {
        assert_eq!(super::ping(), "pong");
    }
}
