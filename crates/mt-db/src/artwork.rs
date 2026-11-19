use mt_meta::artwork::ArtworkSource;
use rusqlite::{Connection, OptionalExtension};

use crate::queries::ALBUM_DISPLAY_ARTIST;

/// Available JPEG sizes in the artwork store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtworkSize {
    /// 300 by 300 pixels.
    Small,
    /// 600 by 600 pixels.
    Large,
}

/// Fetch the requested JPEG for an artwork digest, if it exists.
///
/// # Errors
///
/// Returns an error if SQLite cannot read the thumbnail.
pub fn artwork_thumbnail(
    db: &Connection,
    id: &str,
    size: ArtworkSize,
) -> rusqlite::Result<Option<Vec<u8>>> {
    let sql = match size {
        ArtworkSize::Small => "SELECT thumb_300 FROM artwork WHERE id = ?1",
        ArtworkSize::Large => "SELECT thumb_600 FROM artwork WHERE id = ?1",
    };
    db.prepare_cached(sql)?
        .query_row([id], |row| row.get(0))
        .optional()
}

pub(crate) fn source_priority(source: ArtworkSource) -> i64 {
    match source {
        ArtworkSource::EmbeddedFront => 0,
        ArtworkSource::Embedded => 1,
        ArtworkSource::Folder => 2,
    }
}

pub(crate) fn assign_album_artwork(db: &Connection) -> rusqlite::Result<()> {
    // Use the same group as the library browser. Untagged tracks choose their
    // own cover. A missing track can supply the cover if no present track has one.
    let sql = format!(
        "WITH selected AS MATERIALIZED (
            SELECT rowid AS track_rowid, first_value(artwork_source_id) OVER (
                PARTITION BY album, {ALBUM_DISPLAY_ARTIST}, year, compilation,
                    CASE WHEN coalesce(trim(album), '') = '' THEN rowid END
                ORDER BY artwork_source_id IS NULL, missing, artwork_source_kind, file_path
            ) AS cover
            FROM tracks
        )
        UPDATE tracks SET artwork_id = selected.cover
          FROM selected
         WHERE tracks.rowid = selected.track_rowid AND tracks.artwork_id IS NOT selected.cover",
    );
    db.execute(&sql, [])?;
    Ok(())
}
