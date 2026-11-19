//! Local cover selection and generation of square JPEG thumbnails.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use image::{DynamicImage, ImageReader, Limits, codecs::jpeg::JpegEncoder, imageops::FilterType};
use lofty::file::{TaggedFile, TaggedFileExt};
use lofty::picture::PictureType;

const MAX_SOURCE_BYTES: usize = 32 * 1024 * 1024;

/// Source precedence for artwork within an album, in descending priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtworkSource {
    /// An embedded image identified as the front cover.
    EmbeddedFront,
    /// An embedded image without a front-cover designation.
    Embedded,
    /// An image next to the audio file.
    Folder,
}

/// JPEG thumbnails created from the same source image.
#[derive(Debug)]
pub struct ArtworkThumbnails {
    /// 300 by 300 pixel JPEG.
    pub small: Vec<u8>,
    /// 600 by 600 pixel JPEG.
    pub large: Vec<u8>,
}

/// An artwork candidate in order of preference for a track.
#[derive(Debug)]
pub struct TrackArtwork {
    /// BLAKE3 digest of the original encoded image, in lowercase hexadecimal.
    pub id: String,
    /// Origin of the image.
    pub source: ArtworkSource,
    /// JPEGs for an image that the library has yet to store.
    pub thumbnails: Option<Arc<ArtworkThumbnails>>,
}

type CachedThumbnails = Arc<OnceLock<Option<Arc<ArtworkThumbnails>>>>;
type CachedFolder = Arc<OnceLock<Option<Arc<TrackArtwork>>>>;

pub(crate) struct ArtworkCache<'a> {
    known: &'a HashSet<String>,
    images: Mutex<HashMap<String, CachedThumbnails>>,
    folders: Mutex<HashMap<PathBuf, CachedFolder>>,
}

impl<'a> ArtworkCache<'a> {
    pub(crate) fn new(known: &'a HashSet<String>) -> Self {
        Self {
            known,
            images: Mutex::default(),
            folders: Mutex::default(),
        }
    }

    pub(crate) fn embedded(&self, file: &TaggedFile) -> Option<Arc<TrackArtwork>> {
        let pictures = file
            .primary_tag()
            .into_iter()
            .chain(
                file.tags()
                    .iter()
                    .filter(|tag| tag.tag_type() != file.primary_tag_type()),
            )
            .flat_map(|tag| tag.pictures());

        for (front, source) in [
            (true, ArtworkSource::EmbeddedFront),
            (false, ArtworkSource::Embedded),
        ] {
            if let Some(cover) = pictures
                .clone()
                .filter(|picture| (picture.pic_type() == PictureType::CoverFront) == front)
                .find_map(|picture| self.prepare(picture.data(), source))
            {
                return Some(cover);
            }
        }
        None
    }

    pub(crate) fn folder(&self, track: &Path) -> Option<Arc<TrackArtwork>> {
        let folder = track.parent()?;
        let cached = self
            .folders
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .entry(folder.to_owned())
            .or_default()
            .clone();
        cached.get_or_init(|| self.read_folder(folder)).clone()
    }

    fn read_folder(&self, folder: &Path) -> Option<Arc<TrackArtwork>> {
        let mut candidates: Vec<_> = fs::read_dir(folder)
            .ok()?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let name = entry.file_name();
                let filename = name.to_str()?;
                let priority = if filename.eq_ignore_ascii_case("cover.jpg") {
                    0
                } else if filename.eq_ignore_ascii_case("folder.jpg") {
                    1
                } else if Path::new(filename)
                    .file_stem()?
                    .eq_ignore_ascii_case("front")
                {
                    2
                } else {
                    return None;
                };
                entry
                    .file_type()
                    .ok()?
                    .is_file()
                    .then(|| (priority, entry.path()))
            })
            .collect();
        candidates.sort_unstable();
        candidates.into_iter().find_map(|(_, path)| {
            let mut bytes = Vec::new();
            File::open(path)
                .ok()?
                .take(MAX_SOURCE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .ok()?;
            self.prepare(&bytes, ArtworkSource::Folder)
        })
    }

    fn prepare(&self, bytes: &[u8], source: ArtworkSource) -> Option<Arc<TrackArtwork>> {
        if bytes.is_empty() || bytes.len() > MAX_SOURCE_BYTES {
            return None;
        }
        let id = blake3::hash(bytes).to_hex().to_string();
        let thumbnails = if self.known.contains(&id) {
            None
        } else {
            let cached = self
                .images
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .entry(id.clone())
                .or_default()
                .clone();
            Some(
                cached
                    .get_or_init(|| {
                        std::panic::catch_unwind(|| render(bytes))
                            .ok()
                            .flatten()
                            .map(Arc::new)
                    })
                    .clone()?,
            )
        };
        Some(Arc::new(TrackArtwork {
            id,
            source,
            thumbnails,
        }))
    }
}

fn render(bytes: &[u8]) -> Option<ArtworkThumbnails> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let source = reader.decode().ok()?;
    let side = source.width().min(source.height());
    let square = source.crop_imm(
        (source.width() - side) / 2,
        (source.height() - side) / 2,
        side,
        side,
    );
    Some(ArtworkThumbnails {
        small: jpeg(&square, 300)?,
        large: jpeg(&square, 600)?,
    })
}

fn jpeg(source: &DynamicImage, side: u32) -> Option<Vec<u8>> {
    let resized = source
        .resize_exact(side, side, FilterType::Lanczos3)
        .into_rgb8();
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, 85)
        .encode_image(&resized)
        .ok()?;
    Some(bytes)
}
