use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, TransactionBehavior};

const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../migrations/desktop/V1.sql")),
    (2, include_str!("../migrations/desktop/V2.sql")),
];

/// The most recent desktop library schema this release can open.
pub const DESKTOP_SCHEMA_VERSION: u32 = MIGRATIONS[MIGRATIONS.len() - 1].0;

/// An error opening or initializing a desktop library.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// The library was written by a newer version of MyTunes.
    #[error("desktop library uses schema {found}, but this release supports up to {supported}")]
    SchemaTooNew {
        /// The version stored in the library.
        found: u32,
        /// The latest version this release understands.
        supported: u32,
    },
    /// SQLite could not enable write-ahead logging.
    #[error("desktop library requires WAL mode, but SQLite returned {mode}")]
    WalUnavailable {
        /// The journal mode SQLite selected.
        mode: String,
    },
    /// The bundled ICU collation could not be initialized.
    #[error("desktop library ICU collation error: {0}")]
    Collation(String),
    /// SQLite reported an error while opening, configuring, or migrating the file.
    #[error("desktop library SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Opens or creates a file-based desktop library and applies pending migrations.
///
/// Each returned connection uses foreign keys, WAL and full synchronization.
/// An existing library with a newer schema is rejected before its journal
/// mode or schema is changed.
pub fn open_desktop(path: impl AsRef<Path>) -> Result<Connection, DbError> {
    let mut db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.busy_timeout(Duration::from_secs(5))?;

    let current = supported_schema_version(&db)?;
    crate::collation::register(&db)?;
    db.pragma_update(None, "foreign_keys", true)?;
    // SQLite only fires delete triggers for rows removed by REPLACE when
    // recursive triggers are enabled. The FTS index needs those deletes.
    db.pragma_update(None, "recursive_triggers", true)?;
    db.pragma_update(None, "synchronous", "FULL")?;

    if current < DESKTOP_SCHEMA_VERSION {
        migrate(&mut db)?;
    }
    let mode: String = db.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    if mode != "wal" {
        return Err(DbError::WalUnavailable { mode });
    }

    Ok(db)
}

fn supported_schema_version(db: &Connection) -> Result<u32, DbError> {
    let found = db.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if found > DESKTOP_SCHEMA_VERSION {
        return Err(DbError::SchemaTooNew {
            found,
            supported: DESKTOP_SCHEMA_VERSION,
        });
    }
    Ok(found)
}

fn migrate(db: &mut Connection) -> Result<(), DbError> {
    let transaction = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // Another opener may have migrated the file while this connection
    // was waiting for the write lock.
    let current = supported_schema_version(&transaction)?;
    for &(version, sql) in MIGRATIONS.iter().filter(|&&(version, _)| version > current) {
        transaction.execute_batch(sql)?;
        transaction.pragma_update(None, "user_version", version)?;
    }
    transaction.commit()?;
    Ok(())
}
