//! SQLite schemas, migrations and typed queries for MyTunes libraries.

mod collation;
mod desktop;
pub mod queries;

pub use desktop::{DESKTOP_SCHEMA_VERSION, DbError, open_desktop};
