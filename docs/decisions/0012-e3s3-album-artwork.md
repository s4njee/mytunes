# 0012 — E3.S3 Album artwork

- **Status** Accepted (2026-11-19)
- **Story** E3.S3

## Context

A folder import needs to find artwork, use the same selection across an
album, and supply thumbnail bytes for the desktop and sync clients.

## Decision

Use the first usable embedded front cover, then any other usable embedded
image. Folder fallback is case-insensitive and checks `cover.jpg`,
`folder.jpg`, and `front.*` in that order. The scanner skips symlink
sidecars. JPEG, PNG, WebP, GIF, and BMP source images are supported. A
missing or invalid image leaves the audio track importable. The decoder
accepts up to 32 MiB of encoded bytes and 8,192 pixels per side, subject
to its allocation limit.

Artwork is identified by the BLAKE3 digest of the original image bytes.
The scan's Rayon workers center-crop images and encode 300 px and 600 px
JPEGs at quality 85. They reuse an existing database digest or an image
prepared earlier in the same scan. SQLite stores one row per digest and
exposes both sizes through `artwork_thumbnail`. [Image JPEG encoder](https://docs.rs/image/0.25.10/image/codecs/jpeg/struct.JpegEncoder.html)

Desktop V4 records each track's own artwork source separately from its
displayed `artwork_id`. An album uses the same title, artist, year, and
compilation grouping as the library browser. Album covers prefer a present
track, then a front cover, another embedded image, and a folder image.
Ties use stored path order. A missing track's cover can supply an album
when none of its present tracks has a candidate. Untagged tracks keep
their own covers.

V4 resets the previous audio scan timestamp to discover embedded artwork
on the first rescan. Later scans check folder artwork even when the audio
fingerprint matches. Adding, replacing, or removing a folder cover updates
artwork independently of the audio revision. Thumbnail rows remain
available by digest for reuse.

## Verification

Workspace tests and Clippy pass locally. Integration tests check cover
precedence, center cropping and both JPEG sizes, album grouping across
folders, duplicate images, reassignment after a track disappears, and
folder artwork edits on unchanged audio. Generated MP3, M4A, and FLAC
fixtures verify embedded art extraction. Hosted CI is pending.
