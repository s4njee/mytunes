//! SQLite schemas, migrations and typed queries for MyTunes libraries.

mod collation;
mod database;
mod desktop;
pub mod device;
pub mod queries;

pub use database::DbError;
pub use desktop::{DESKTOP_SCHEMA_VERSION, open_desktop};
pub use device::{DEVICE_SCHEMA_VERSION, open_device};
