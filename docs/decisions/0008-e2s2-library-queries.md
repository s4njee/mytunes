# 0008: E2.S2 Library queries and collation

- **Status:** Accepted (2026-11-19)
- **Story:** E2.S2

## Context

The desktop view API needs to build an ordered snapshot once and then
fetch small pages by track ID. Track tables, album lists and browser facets
need consistent sorting and grouping.

## Decision

`LibraryQueries` returns typed track snapshots, pages, album groups,
facet counts and playlist contents. A page follows its input order and
preserves repeated IDs. It omits IDs for tracks deleted since the
snapshot was taken. The view layer will handle snapshot invalidation in
E7.S1. Playlist order follows stored positions, including repeated
tracks.

Track searches bind an FTS5 expression to the query. The literal prefix
mode quotes each word so input such as `OR` is searched as text. Callers
can also explicitly request FTS5 expression syntax.

Pin ICU4X 2.4.0 and register its case-insensitive, accent-aware root
collator as `mytunes_unicode_v1` on every desktop connection. Use explicit
sort tags when present. Where the artist tag supplies the sort value,
ignore a leading “The ” case-insensitively. The album browser groups by
album, album artist and year, with the track artist as a fallback.
Compilations use “Various Artists”. Album sort preserves disc and track
order in both directions.

V2 adds covering indexes for common text sorts. Their order depends on
the pinned ICU implementation and normalized artist expression. When
upgrading the ICU collation or changing the sort expression, rebuild
these indexes in a migration before opening them for normal use.
SQLite documents the need to rebuild indexes after changing a
collating sequence. [SQLite REINDEX](https://www.sqlite.org/lang_reindex.html)

## Performance

On Linux with a Ryzen 9 7900X, the release benchmark used a file-backed
library with 50,000 synthetic tracks. Across nine measured samples per
query, all 32 unfiltered sort cases passed the 60 ms median budget.
The slowest median was 19.78 ms. The title sort took 4.44 ms ascending
and the 200-row page median was 0.41 ms against a 5 ms budget.

Run `cargo bench --locked -p mt-db --bench library_queries` locally.
The Linux CI matrix runs the same benchmark. Search-specific latency
will be measured with the browser and desktop search stories.
