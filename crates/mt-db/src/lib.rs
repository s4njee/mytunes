//! SQLite schemas, migrations and typed queries for MyTunes libraries.

mod artwork;
mod collation;
mod database;
mod desktop;
pub mod device;
mod import;
pub mod queries;
#[cfg(feature = "device-seeding")]
pub mod seed;
mod tag_edit;

pub use artwork::{ArtworkSize, artwork_thumbnail};
pub use database::DbError;
pub use desktop::{DESKTOP_SCHEMA_VERSION, open_desktop};
pub use device::{DEVICE_SCHEMA_VERSION, open_device};
pub use import::{
    FolderImport, ImportError, ManagedMedia, import_library_paths, scan_library_folder,
};
pub use tag_edit::{EditTrackError, edit_track_tags};
