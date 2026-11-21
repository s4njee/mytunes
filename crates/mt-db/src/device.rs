//! Device library storage, playback events and persistent settings.

use std::path::Path;

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::DbError;
use crate::database::{Schema, open_database};

const DEVICE_SCHEMA: Schema = Schema {
    application_id: i32::from_be_bytes(*b"MTDV"),
    migrations: &[(1, include_str!("../migrations/device/V1.sql"))],
};

/// The most recent device library schema this release can open.
pub const DEVICE_SCHEMA_VERSION: u32 = DEVICE_SCHEMA.version();

/// Opens or creates the device library using its own migration chain.
///
/// A fresh database is marked as a device database. Existing desktop and
/// unrelated database files are rejected.
pub fn open_device(path: impl AsRef<Path>) -> Result<Connection, DbError> {
    open_database(path.as_ref(), &DEVICE_SCHEMA, crate::collation::register)
}

/// An error reading or writing the device event log or state.
#[derive(Debug, thiserror::Error)]
pub enum DeviceStoreError {
    /// SQLite could not complete the operation.
    #[error("device store: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// State or event data could not be encoded or decoded as JSON.
    #[error("device store JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Kind of playback or library change reported back to the desktop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceEventKind {
    /// A counted play.
    Play,
    /// A counted skip.
    Skip,
    /// A track rating changed.
    Rating,
    /// An On-The-Go playlist was saved.
    OnTheGo,
}

impl DeviceEventKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Play => "play",
            Self::Skip => "skip",
            Self::Rating => "rating",
            Self::OnTheGo => "otg",
        }
    }
}

impl FromSql for DeviceEventKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "play" => Ok(Self::Play),
            "skip" => Ok(Self::Skip),
            "rating" => Ok(Self::Rating),
            "otg" => Ok(Self::OnTheGo),
            kind => Err(FromSqlError::other(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown device event kind: {kind}"),
            ))),
        }
    }
}

/// An event stored until the desktop confirms it has processed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceEvent {
    /// Stable event identifier used to deduplicate retries.
    pub event_id: Uuid,
    /// Event timestamp, as Unix milliseconds.
    pub ts: i64,
    /// What happened.
    pub kind: DeviceEventKind,
    /// Event-specific data, including any affected track or playlist IDs.
    pub payload: Map<String, Value>,
}

/// Access to the device library's event log and state.
pub struct DeviceStore<'db> {
    db: &'db Connection,
}

impl<'db> DeviceStore<'db> {
    /// Uses a connection returned by [`open_device`].
    pub fn new(db: &'db Connection) -> Self {
        Self { db }
    }

    /// Adds a pending event. Reusing an event ID fails and leaves the first event intact.
    pub fn append_event(&self, event: &DeviceEvent) -> Result<(), DeviceStoreError> {
        self.db.execute(
            "INSERT INTO events (event_id, ts, kind, payload) VALUES (?1, ?2, ?3, ?4)",
            params![
                event.event_id,
                event.ts,
                event.kind.as_str(),
                serde_json::to_string(&event.payload)?
            ],
        )?;
        Ok(())
    }

    /// Reads up to `limit` unacknowledged events in timestamp and event ID order.
    pub fn pending_events(&self, limit: u32) -> Result<Vec<DeviceEvent>, DeviceStoreError> {
        let mut query = self.db.prepare_cached(
            "SELECT event_id, ts, kind, payload FROM events
              WHERE acked = 0 ORDER BY ts, event_id LIMIT ?1",
        )?;
        let mut rows = query.query([limit])?;
        let mut events = Vec::new();
        while let Some(row) = rows.next()? {
            let payload: String = row.get(3)?;
            events.push(DeviceEvent {
                event_id: row.get(0)?,
                ts: row.get(1)?,
                kind: row.get(2)?,
                payload: serde_json::from_str(&payload)?,
            });
        }
        Ok(events)
    }

    /// Acknowledges a batch atomically and returns the number of newly acknowledged events.
    ///
    /// Unknown and already acknowledged IDs are ignored.
    pub fn acknowledge_events(&self, ids: &[Uuid]) -> Result<usize, DeviceStoreError> {
        if ids.is_empty() {
            return Ok(0);
        }
        // One bound value avoids SQLite's parameter limit on large batches.
        let hex_ids: Vec<_> = ids.iter().map(|id| id.simple().to_string()).collect();
        let count = self.db.execute(
            "UPDATE events SET acked = 1
             WHERE acked = 0 AND event_id IN (SELECT unhex(value) FROM json_each(?1))",
            [serde_json::to_string(&hex_ids)?],
        )?;
        Ok(count)
    }

    /// Reads one state key. A stored JSON null is distinct from a missing key.
    pub fn state(&self, key: &str) -> Result<Option<Value>, DeviceStoreError> {
        let stored: Option<String> = self
            .db
            .query_row("SELECT value FROM state WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(stored.as_deref().map(serde_json::from_str).transpose()?)
    }

    /// Saves or replaces one JSON state value.
    pub fn set_state(&self, key: &str, value: &Value) -> Result<(), DeviceStoreError> {
        self.db.execute(
            "INSERT INTO state (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }
}
