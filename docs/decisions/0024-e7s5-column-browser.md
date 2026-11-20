# 0024 — E7.S5 Column browser and search

- **Status** Linux verification complete (2026-11-19)
- **Story** E7.S5

## Decision

The native View menu and the toolbar toggle the same persisted Column
Browser preference. Rust supplies genres, artists, and albums from
progressively narrower scopes. Each column shows an All row with the
number of choices. Tagged compilations have their own artist choice.
The webview pools facet rows so a library with thousands of albums
can render the browser without placing every option in the DOM.

After 80 ms of quiet, the search box sends the literal prefix search to
Rust. Search combines with the selected facets. The library caches the
sorted base view for its current source, sort, and facets. FTS5 supplies
the matching SQLite row IDs and Rust retains those IDs in the base view
order. Library revisions invalidate the cached order. Browser options use
a separate read connection while the webview fetches the result page.

## Verification

On a 50,000-track production WebKitGTK fixture, a prefix matching all
50,000 tracks returned the ordered snapshot and first 200-row page
in 46–49 ms over eight runs. A 22,600-match prefix took 34–38 ms.
The measurement starts after the intentional input debounce.

Linux WebDriver and native menu input verified View menu toggling,
Genre → Artist → Album, the compilation group, and search combined
with those selections. The Ctrl+F and Escape shortcuts passed.
Rust and frontend tests cover the filtering and search behavior.
Hosted CI is pending.
