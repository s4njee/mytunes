use rusqlite::{Connection, OptionalExtension};
use serde::{Serialize, de::DeserializeOwned};

use crate::library::AppError;

pub(crate) fn load<T: DeserializeOwned + Default>(
    db: &Connection,
    key: &str,
) -> Result<T, AppError> {
    let stored: Option<String> = db
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|_| AppError::Library)?;
    stored.map_or_else(
        || Ok(T::default()),
        |encoded| serde_json::from_str(&encoded).map_err(|_| AppError::Library),
    )
}

pub(crate) fn save(db: &Connection, key: &str, settings: &impl Serialize) -> Result<(), AppError> {
    let encoded = serde_json::to_string(settings).map_err(|_| AppError::Library)?;
    db.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (key, encoded),
    )
    .map_err(|_| AppError::Library)?;
    Ok(())
}
