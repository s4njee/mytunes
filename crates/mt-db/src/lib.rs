//! SQLite schemas, migrations and typed queries for MyTunes libraries.

mod desktop;

pub use desktop::{DESKTOP_SCHEMA_VERSION, DbError, open_desktop};
