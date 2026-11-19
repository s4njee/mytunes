-- Common text sorts keep covering indexes so opening a snapshot does not
-- repeatedly compare 50,000 localized names. Register the versioned
-- collation and scalar function before opening or migrating the library.
CREATE INDEX tracks_sort_title ON tracks (
    coalesce(nullif(sort_title, ''), title) COLLATE mytunes_unicode_v1, id
);

CREATE INDEX tracks_sort_artist ON tracks (
    coalesce(nullif(sort_artist, ''), mytunes_artist_v1(artist)) COLLATE mytunes_unicode_v1, id
);

CREATE INDEX tracks_sort_album_artist ON tracks (
    coalesce(nullif(sort_album_artist, ''), mytunes_artist_v1(album_artist)) COLLATE mytunes_unicode_v1, id
);

CREATE INDEX tracks_sort_genre ON tracks (genre COLLATE mytunes_unicode_v1, id);
CREATE INDEX tracks_sort_composer ON tracks (composer COLLATE mytunes_unicode_v1, id);
CREATE INDEX tracks_sort_kind ON tracks (codec COLLATE mytunes_unicode_v1, bitrate, id);

CREATE INDEX tracks_sort_album ON tracks (
    coalesce(nullif(sort_album, ''), album) COLLATE mytunes_unicode_v1,
    (CASE WHEN compilation THEN 'Various Artists'
        ELSE coalesce(nullif(sort_album_artist, ''), mytunes_artist_v1(nullif(album_artist, '')),
                      nullif(sort_artist, ''), mytunes_artist_v1(artist)) END) COLLATE mytunes_unicode_v1,
    year, album,
    (CASE WHEN compilation THEN 'Various Artists'
          ELSE coalesce(nullif(album_artist, ''), nullif(artist, '')) END),
    compilation, disc_no, track_no, id
);
