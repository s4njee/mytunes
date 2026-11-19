# 0007: E2.S1 Desktop library schema

- **Status:** Accepted (2026-11-19)
- **Story:** E2.S1

## Context

The desktop needs a versioned library before queries and import can use
it. The owner requested that app development begin while the remaining
M0 platform checks are open. This branch starts from the app foundation
on main and the spike branches retain their evidence and harnesses.

## Decision

Bundle SQLite through `rusqlite` 0.43.0, the version exercised by the
table spike. The desktop opener rejects unsupported versions before
making persistent changes. It checks the version again under an
immediate transaction before applying pending migrations. Schema changes
and `user_version` commit together. Returned connections use WAL, full
synchronization, and foreign keys.

V1 keeps the UUID as the public track identifier and as the target of
foreign keys. An explicit integer SQLite rowid is the physical primary
key. It gives the external-content FTS5 table a stable key during
`VACUUM`, which can change rowids for tables without an explicit integer
primary key. Insert, update and delete triggers maintain the six indexed
tag columns. See [SQLite VACUUM](https://www.sqlite.org/lang_vacuum.html)
and [SQLite FTS5](https://www.sqlite.org/fts5.html#external_content_tables).

Albums are a derived view grouped by album artist, album and year.
Tracks without an album title stay in `tracks`. The query layer can
apply its own compilation and display rules.

## Consequences

Future migrations append a version and SQL file. Track UUIDs remain
stable across local maintenance and foreign-key relationships. The
integer rowid stays internal to storage and search. The next story can
build typed queries against V1.
