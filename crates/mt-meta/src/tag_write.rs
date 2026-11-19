//! Staging and atomic replacement of tag edits for local audio files.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, BufReader, Cursor, Seek};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lofty::config::{ParseOptions, ParsingMode, WriteOptions};
use lofty::error::{FileEncodingError, FileParseError};
use lofty::file::TaggedFileExt;
use lofty::picture::{Picture, PictureType};
use lofty::probe::Probe;
use lofty::tag::items::popularimeter::{Popularimeter, StarRating};
use lofty::tag::items::timestamp::Timestamp;
use lofty::tag::{Accessor, ItemKey, Tag, TagExt, TagType};
use tempfile::{Builder, TempPath};

use crate::MetadataError;
use crate::artwork::{ArtworkCache, TrackArtwork};
use crate::metadata::read_track_from;
use crate::scan::{FileStamp, ReadTrack};

/// A field change in a tag edit.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum TagEdit<T> {
    /// Preserve the field's current value.
    #[default]
    Keep,
    /// Replace the field value.
    Set(T),
    /// Remove the field.
    Clear,
}

/// Editable fields in a track's tags.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TagEdits {
    /// Title.
    pub title: TagEdit<String>,
    /// Track artist.
    pub artist: TagEdit<String>,
    /// Album title.
    pub album: TagEdit<String>,
    /// Album artist.
    pub album_artist: TagEdit<String>,
    /// Genre.
    pub genre: TagEdit<String>,
    /// Four-digit year.
    pub year: TagEdit<u16>,
    /// Track number.
    pub track_number: TagEdit<u16>,
    /// Number of tracks on the disc.
    pub track_total: TagEdit<u16>,
    /// Disc number.
    pub disc_number: TagEdit<u16>,
    /// Number of discs in the release.
    pub disc_total: TagEdit<u16>,
    /// Compilation flag.
    pub compilation: TagEdit<bool>,
    /// Rating on the library's 0 to 100 scale in increments of 20.
    pub rating: TagEdit<u8>,
    /// Encoded image bytes for the embedded front cover.
    pub front_cover: TagEdit<Vec<u8>>,
}

impl TagEdits {
    /// Return the requested library rating, including zero when clearing it.
    pub fn rating_value(&self) -> Option<u8> {
        match self.rating {
            TagEdit::Keep => None,
            TagEdit::Set(rating) => Some(rating),
            TagEdit::Clear => Some(0),
        }
    }
}

/// An error applying edits to a local audio file.
#[derive(Debug, thiserror::Error)]
pub enum TagWriteError {
    /// The original cannot be opened for writing.
    #[error("This music file is read-only or you do not have permission to edit it.")]
    ReadOnly,
    /// The path was replaced or modified by another operation.
    #[error("The music file changed. Rescan it before editing its tags.")]
    Changed,
    /// Filesystem operation failed.
    #[error("Could not save music file tags: {0}")]
    Io(#[from] io::Error),
    /// Audio metadata could not be read.
    #[error(transparent)]
    Metadata(#[from] MetadataError),
    /// The tag parser rejected the source data.
    #[error("Could not read the existing music file tags")]
    Decode(#[from] FileParseError),
    /// Lofty could not encode the replacement tags.
    #[error("Could not write tags in this music file format")]
    Encode(#[from] FileEncodingError),
    /// A library parser aborted while working with the temporary copy.
    #[error("The tag parser failed while preparing the edit")]
    ParserFailure,
    /// One of the requested values cannot be written.
    #[error("Invalid tag value for {0}")]
    InvalidValue(&'static str),
    /// Automatic rollback failed and the original bytes remain at a separate path.
    #[error("Could not restore the original music file. It is saved at {}", path.display())]
    RestoreFailed {
        /// Path of the saved original file.
        path: PathBuf,
        /// Filesystem error that prevented the restore.
        #[source]
        source: io::Error,
    },
}

/// A validated replacement whose original path is still intact.
pub struct PreparedTagWrite {
    path: PathBuf,
    original_stamp: FileStamp,
    replacement: TempPath,
    /// Hash of the unedited source file.
    pub original_hash: [u8; 32],
    /// Metadata, size, mtime, and hash of the replacement file.
    pub track: ReadTrack,
    /// Selected embedded or folder artwork after the edit.
    pub artwork: Option<Arc<TrackArtwork>>,
}

/// A published replacement with a backup available for SQLite rollback.
pub struct PublishedTagWrite {
    path: PathBuf,
    backup: TempPath,
}

impl PreparedTagWrite {
    /// Copy and edit the tags beside the original file, then verify the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the file is read-only, changes while copying, fails
    /// parsing or encoding, or has a field value the writer cannot represent.
    pub fn prepare(path: &Path, edits: &TagEdits) -> Result<Self, TagWriteError> {
        let before = fs::symlink_metadata(path)?;
        let original_stamp = FileStamp::from_metadata(&before)?;
        let permissions = before.permissions();
        if permissions.readonly() {
            return Err(TagWriteError::ReadOnly);
        }
        let mut original = File::options()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|error| {
                if error.kind() == io::ErrorKind::PermissionDenied {
                    TagWriteError::ReadOnly
                } else {
                    TagWriteError::Io(error)
                }
            })?;
        if FileStamp::from_metadata(&original.metadata()?)? != original_stamp {
            return Err(TagWriteError::Changed);
        }
        let folder = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut replacement = Builder::new()
            .prefix(".mytunes-tags-")
            .tempfile_in(folder)?;
        io::copy(&mut original, &mut replacement)?;
        if FileStamp::from_metadata(&original.metadata()?)? != original_stamp {
            return Err(TagWriteError::Changed);
        }
        drop(original);

        let original_hash = hash_file(replacement.as_file_mut())?;
        // Check the actual codec and DRM status before opening tags for writing.
        crate::read_metadata_from(BufReader::new(replacement.as_file_mut()), path)?;
        let known = HashSet::new();
        let cache = ArtworkCache::new(&known);
        if let TagEdit::Set(bytes) = &edits.front_cover
            && !cache.is_valid_image(bytes)
        {
            return Err(TagWriteError::InvalidValue("front cover"));
        }
        catch_unwind(AssertUnwindSafe(|| {
            apply_edits(replacement.as_file_mut(), edits)
        }))
        .map_err(|_| TagWriteError::ParserFailure)??;

        let (metadata, artwork) = read_track_from(
            BufReader::new(replacement.as_file_mut()),
            path,
            Some(&cache),
        )?;
        let artwork = artwork.or_else(|| cache.folder(path));
        let hash = hash_file(replacement.as_file_mut())?;
        replacement.as_file().set_permissions(permissions)?;
        replacement.as_file().sync_all()?;
        let replacement = replacement.into_temp_path();
        let stamp = FileStamp::from_metadata(&fs::metadata(&replacement)?)?;
        Ok(Self {
            path: path.to_owned(),
            original_stamp,
            replacement,
            original_hash,
            track: ReadTrack {
                metadata,
                stamp,
                hash,
            },
            artwork,
        })
    }

    /// Atomically replace the original file while retaining the original bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the original changed or the replacement failed.
    pub fn publish(self) -> Result<PublishedTagWrite, TagWriteError> {
        let current = fs::symlink_metadata(&self.path)?;
        if current.permissions().readonly() {
            return Err(TagWriteError::ReadOnly);
        }
        if FileStamp::from_metadata(&current)? != self.original_stamp {
            return Err(TagWriteError::Changed);
        }
        let folder = self.replacement.parent().unwrap_or(Path::new("."));
        let builder = Builder::new();
        let backup = match builder.make_in(folder, |backup| fs::hard_link(&self.path, backup)) {
            Ok(link) => link.into_temp_path(),
            Err(_) => {
                let mut backup = builder.tempfile_in(folder)?;
                let mut original = File::open(&self.path)?;
                io::copy(&mut original, backup.as_file_mut())?;
                backup.as_file().set_permissions(current.permissions())?;
                backup.as_file().set_modified(current.modified()?)?;
                backup.as_file().sync_all()?;
                backup.into_temp_path()
            }
        };

        if FileStamp::from_metadata(&fs::symlink_metadata(&self.path)?)? != self.original_stamp {
            return Err(TagWriteError::Changed);
        }
        self.replacement
            .persist(&self.path)
            .map_err(|error| error.error)?;
        Ok(PublishedTagWrite {
            path: self.path,
            backup,
        })
    }
}

impl PublishedTagWrite {
    /// Restore the original file after an unsuccessful SQLite commit.
    ///
    /// Dropping this value after a successful commit discards the backup.
    ///
    /// # Errors
    ///
    /// Returns the retained backup path if the original path cannot be restored.
    pub fn restore(self) -> Result<(), TagWriteError> {
        self.backup.persist(self.path).map_err(|mut error| {
            error.path.disable_cleanup(true);
            TagWriteError::RestoreFailed {
                path: error.path.to_path_buf(),
                source: error.error,
            }
        })
    }
}

fn hash_file(file: &mut File) -> io::Result<[u8; 32]> {
    file.rewind()?;
    Ok(*blake3::Hasher::new()
        .update_reader(file)?
        .finalize()
        .as_bytes())
}

fn apply_edits(file: &mut File, edits: &TagEdits) -> Result<(), TagWriteError> {
    file.rewind()?;
    let options = ParseOptions::new().parsing_mode(ParsingMode::Strict);
    let tagged = Probe::new(file.try_clone()?)
        .options(options)
        .guess_file_type()?
        .read()?;
    let primary = tagged.primary_tag_type();

    for tag in tagged.tags().iter().filter(|tag| tag.tag_type() != primary) {
        if !tagged.tag_support(tag.tag_type()).is_writable() {
            continue;
        }
        let mut tag = tag.clone();
        if update_tag(&mut tag, edits, false)? {
            file.rewind()?;
            tag.save_to(file, WriteOptions::default())?;
        }
    }
    let mut tag = tagged
        .primary_tag()
        .cloned()
        .unwrap_or_else(|| Tag::new(primary));
    update_tag(&mut tag, edits, true)?;
    file.rewind()?;
    tag.save_to(file, WriteOptions::default())?;
    Ok(())
}

fn update_tag(tag: &mut Tag, edits: &TagEdits, primary: bool) -> Result<bool, TagWriteError> {
    let mut changed = false;
    for (key, edit) in [
        (ItemKey::TrackTitle, &edits.title),
        (ItemKey::TrackArtist, &edits.artist),
        (ItemKey::AlbumTitle, &edits.album),
        (ItemKey::AlbumArtist, &edits.album_artist),
        (ItemKey::Genre, &edits.genre),
    ] {
        changed |= update_text(tag, key, edit, primary)?;
    }
    for (key, edit) in [
        (ItemKey::TrackNumber, &edits.track_number),
        (ItemKey::TrackTotal, &edits.track_total),
        (ItemKey::DiscNumber, &edits.disc_number),
        (ItemKey::DiscTotal, &edits.disc_total),
    ] {
        changed |= match edit {
            TagEdit::Keep => false,
            TagEdit::Clear => remove_key(tag, key),
            TagEdit::Set(0) => return Err(TagWriteError::InvalidValue("track or disc number")),
            TagEdit::Set(value) if primary => {
                write_text(tag, key, value.to_string())?;
                true
            }
            TagEdit::Set(_) => false,
        };
    }
    match edits.year {
        TagEdit::Clear => {
            changed |= remove_key(tag, ItemKey::Year) | remove_key(tag, ItemKey::RecordingDate);
        }
        TagEdit::Set(year) if primary => {
            if !(1..=9999).contains(&year) {
                return Err(TagWriteError::InvalidValue("year"));
            }
            tag.set_date(Timestamp {
                year,
                ..tag.date().unwrap_or_default()
            });
            changed = true;
        }
        _ => {}
    }
    match edits.compilation {
        TagEdit::Clear => changed |= remove_key(tag, ItemKey::FlagCompilation),
        TagEdit::Set(value) if primary => {
            write_text(tag, ItemKey::FlagCompilation, u8::from(value).to_string())?;
            changed = true;
        }
        _ => {}
    }
    if primary && let Some(value) = edits.rating_value() {
        update_rating(tag, value)?;
        changed = true;
    }
    if !matches!(edits.front_cover, TagEdit::Keep) {
        let count = tag.pictures().len();
        if tag.tag_type() == TagType::Mp4Ilst {
            // MP4 stores cover images without front/back types.
            while !tag.pictures().is_empty() {
                tag.remove_picture(0);
            }
        } else {
            tag.remove_picture_type(PictureType::CoverFront);
        }
        changed |= count != tag.pictures().len();
        if primary && let TagEdit::Set(bytes) = &edits.front_cover {
            let mut picture = Picture::from_reader(&mut Cursor::new(bytes))
                .map_err(|_| TagWriteError::InvalidValue("front cover"))?;
            picture.set_pic_type(PictureType::CoverFront);
            tag.push_picture(picture);
            changed = true;
        }
    }
    Ok(changed)
}

fn update_text(
    tag: &mut Tag,
    key: ItemKey,
    edit: &TagEdit<String>,
    primary: bool,
) -> Result<bool, TagWriteError> {
    match edit {
        TagEdit::Keep => Ok(false),
        TagEdit::Clear => Ok(remove_key(tag, key)),
        TagEdit::Set(value) if value.trim().is_empty() => Ok(remove_key(tag, key)),
        TagEdit::Set(value) if primary => {
            if value.contains('\0') {
                return Err(TagWriteError::InvalidValue("text"));
            }
            write_text(tag, key, value.trim().to_owned())?;
            Ok(true)
        }
        TagEdit::Set(_) => Ok(false),
    }
}

fn write_text(tag: &mut Tag, key: ItemKey, value: String) -> Result<(), TagWriteError> {
    if tag.insert_text(key, value) {
        Ok(())
    } else {
        Err(TagWriteError::InvalidValue("field for this tag format"))
    }
}

fn remove_key(tag: &mut Tag, key: ItemKey) -> bool {
    let present = tag.get(key).is_some();
    tag.remove_key(key);
    present
}

fn update_rating(tag: &mut Tag, value: u8) -> Result<(), TagWriteError> {
    let rating = match value {
        0 => None,
        20 => Some(StarRating::One),
        40 => Some(StarRating::Two),
        60 => Some(StarRating::Three),
        80 => Some(StarRating::Four),
        100 => Some(StarRating::Five),
        _ => return Err(TagWriteError::InvalidValue("rating")),
    };
    let counter = tag
        .ratings()
        .find(|rating| rating.email() == "MyTunes")
        .map_or(0, |rating| rating.play_counter);
    if tag.tag_type() == TagType::Id3v2 {
        let _ = tag
            .take_filter(ItemKey::Popularimeter, |item| {
                item.value()
                    .text()
                    .is_some_and(|text| text.starts_with("MyTunes|"))
            })
            .count();
        if let Some(rating) = rating {
            let item = lofty::tag::TagItem::new(
                ItemKey::Popularimeter,
                lofty::tag::ItemValue::Text(
                    Popularimeter::custom("MyTunes", rating, counter).to_string(),
                ),
            );
            tag.push(item);
        }
    } else if let Some(rating) = rating {
        write_text(
            tag,
            ItemKey::Popularimeter,
            Popularimeter::musicbee(rating, 0).to_string(),
        )?;
    } else {
        tag.remove_key(ItemKey::Popularimeter);
    }
    Ok(())
}
