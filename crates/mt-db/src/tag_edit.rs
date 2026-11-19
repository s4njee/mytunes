use std::path::Path;
use std::time::SystemTime;

use mt_meta::tag_write::{PreparedTagWrite, TagEdits, TagWriteError};
use mt_types::TrackId;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::artwork::{INSERT_ARTWORK, assign_album_artwork};
use crate::import::{UPSERT_TRACK, save_track};

/// Failure to update a track's file or library row.
#[derive(Debug, thiserror::Error)]
pub enum EditTrackError {
    /// The selected track is absent from the library.
    #[error("This track is no longer in the library.")]
    TrackNotFound,
    /// The music file could not be edited.
    #[error(transparent)]
    File(#[from] TagWriteError),
    /// SQLite rejected the edit.
    #[error("Could not update the music library.")]
    Database(#[from] rusqlite::Error),
}

/// Apply Get Info field changes to the audio file and its desktop library row.
///
/// The operation holds the library's writer lock while staging and verifying the
/// replacement. It leaves the track's identity, playback history and file
/// revision intact. Files that differ from their imported hash must be rescanned
/// before editing. Call on a background thread for large files or artwork.
///
/// # Errors
///
/// Returns a typed error if the track is missing, the source is read-only or
/// changed, the edit cannot be encoded, or SQLite rejects the update.
pub fn edit_track_tags(
    db: &mut Connection,
    id: &TrackId,
    edits: &TagEdits,
) -> Result<(), EditTrackError> {
    let transaction = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let (path, expected_hash) = transaction
        .query_row(
            "SELECT file_path, file_hash FROM tracks WHERE id = ?1",
            [&id.0],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<Vec<u8>>>(1)?)),
        )
        .optional()?
        .ok_or(EditTrackError::TrackNotFound)?;
    if edits == &TagEdits::default() {
        return Ok(());
    }
    let replacement = PreparedTagWrite::prepare(Path::new(&path), edits)?;
    if expected_hash
        .as_deref()
        .is_some_and(|hash| hash != replacement.original_hash)
    {
        return Err(TagWriteError::Changed.into());
    }
    let artwork = replacement.artwork.as_deref();
    if let Some(cover) = artwork
        && let Some(jpeg) = &cover.thumbnails
    {
        transaction.execute(INSERT_ARTWORK, (&cover.id, &jpeg.small, &jpeg.large))?;
    }
    {
        let mut upsert = transaction.prepare(UPSERT_TRACK)?;
        save_track(&mut upsert, &path, &replacement.track, None, artwork, false)?;
    }
    if let Some(rating) = edits.rating_value() {
        let timestamp = SystemTime::UNIX_EPOCH
            .elapsed()
            .ok()
            .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok());
        transaction.execute(
            "UPDATE tracks SET rating = ?1, rating_updated_at = ?2 WHERE id = ?3",
            (rating, timestamp, &id.0),
        )?;
    }
    assign_album_artwork(&transaction)?;

    let published = replacement.publish()?;
    if let Err(error) = transaction.commit() {
        published.restore()?;
        return Err(error.into());
    }
    Ok(())
}
