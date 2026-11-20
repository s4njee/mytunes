use std::path::Path;

use rusqlite::Connection;

use crate::DbError;
use crate::database::{Schema, open_database};

const DESKTOP_SCHEMA: Schema = Schema {
    // Desktop V1 and V2 already use SQLite's default application ID.
    application_id: 0,
    migrations: &[
        (1, include_str!("../migrations/desktop/V1.sql")),
        (2, include_str!("../migrations/desktop/V2.sql")),
        (3, include_str!("../migrations/desktop/V3.sql")),
        (4, include_str!("../migrations/desktop/V4.sql")),
        (5, include_str!("../migrations/desktop/V5.sql")),
    ],
};

/// The most recent desktop library schema this release can open.
pub const DESKTOP_SCHEMA_VERSION: u32 = DESKTOP_SCHEMA.version();

/// Opens or creates a file-based desktop library and applies pending migrations.
///
/// Each returned connection uses foreign keys, WAL and full synchronization.
/// An existing library with a newer schema is rejected before its journal
/// mode or schema is changed.
pub fn open_desktop(path: impl AsRef<Path>) -> Result<Connection, DbError> {
    open_database(path.as_ref(), &DESKTOP_SCHEMA, crate::collation::register)
}
