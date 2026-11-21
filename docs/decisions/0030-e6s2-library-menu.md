# 0030 — Library menu and the first playback

## Status

Accepted on 2026-11-21.

## Context

The M3 handoff places Now Playing first in the Library menu. On
first launch there is no track to return to.

## Decision

The owner confirmed that the row stays visible, dimmed, and
disabled until a track has played. MENU returns the wheel selection
to that first row.

## Consequences

Songs opens the full device library in the bridge's song sort
order. It reuses Queue rows and loads pages as the wheel moves.
SELECT plays the selected song. Now Playing becomes available
in the current session after playback starts. E6.S7 will restore
the player state across launches.
