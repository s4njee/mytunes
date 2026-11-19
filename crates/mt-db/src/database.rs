use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, TransactionBehavior};

/// An error opening, configuring, or migrating a MyTunes database.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// The database was written by a newer version of MyTunes.
    #[error("database uses schema {found}, but this release supports up to {supported}")]
    SchemaTooNew {
        /// The version stored in the database.
        found: u32,
        /// The latest version this release understands.
        supported: u32,
    },
    /// The file is not the requested MyTunes database type.
    #[error("database application ID {found:#010x} does not match {expected:#010x}")]
    ApplicationMismatch {
        /// The file's SQLite application ID.
        found: i32,
        /// The SQLite application ID that this opener requires.
        expected: i32,
    },
    /// SQLite could not enable write-ahead logging.
    #[error("database requires WAL mode, but SQLite returned {mode}")]
    WalUnavailable {
        /// The journal mode SQLite selected.
        mode: String,
    },
    /// The bundled ICU collation could not be initialized.
    #[error("desktop library ICU collation error: {0}")]
    Collation(String),
    /// SQLite reported an error while opening, configuring, or migrating the file.
    #[error("SQLite database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub(crate) struct Schema {
    pub(crate) application_id: i32,
    pub(crate) migrations: &'static [(u32, &'static str)],
}

impl Schema {
    pub(crate) const fn version(&self) -> u32 {
        self.migrations[self.migrations.len() - 1].0
    }

    fn check(&self, db: &Connection) -> Result<u32, DbError> {
        let (application_id, found, blank): (i32, u32, bool) = db.query_row(
            "SELECT application_id, user_version,
                    CASE WHEN application_id = 0 AND user_version = 0 AND ?1 <> 0
                         THEN NOT EXISTS(SELECT 1 FROM sqlite_schema) ELSE 0 END
             FROM pragma_application_id() CROSS JOIN pragma_user_version()",
            [self.application_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if application_id != self.application_id && !blank {
            return Err(DbError::ApplicationMismatch {
                found: application_id,
                expected: self.application_id,
            });
        }
        if found > self.version() {
            return Err(DbError::SchemaTooNew {
                found,
                supported: self.version(),
            });
        }
        Ok(found)
    }

    fn migrate(&self, db: &mut Connection) -> Result<(), DbError> {
        let transaction = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Recheck after locking in case another opener initialized this file.
        let current = self.check(&transaction)?;
        for &(version, sql) in self
            .migrations
            .iter()
            .filter(|&&(version, _)| version > current)
        {
            transaction.execute_batch(sql)?;
            transaction.pragma_update(None, "user_version", version)?;
        }
        if current == 0 && self.application_id != 0 {
            transaction.pragma_update(None, "application_id", self.application_id)?;
        }
        transaction.commit()?;
        Ok(())
    }
}

pub(crate) fn open_database(
    path: &Path,
    schema: &Schema,
    configure: impl FnOnce(&Connection) -> Result<(), DbError>,
) -> Result<Connection, DbError> {
    let mut db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.busy_timeout(Duration::from_secs(5))?;

    let current = schema.check(&db)?;
    configure(&db)?;
    db.pragma_update(None, "foreign_keys", true)?;
    // REPLACE can delete rows. Recursive triggers let the FTS index observe
    // those deletes and ensure the event log's delete guard also runs.
    db.pragma_update(None, "recursive_triggers", true)?;
    db.pragma_update(None, "synchronous", "FULL")?;
    if current < schema.version() {
        schema.migrate(&mut db)?;
    }
    let mode: String = db.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    if mode != "wal" {
        return Err(DbError::WalUnavailable { mode });
    }
    Ok(db)
}
