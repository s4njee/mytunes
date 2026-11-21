# 0031 — Equalizer wheel controls

## Status

Accepted on 2026-11-21.

## Context

The wheel adjusts gain while a band is selected. The mockup shows three
playback settings below the bands without an interaction to reach them.

## Decision

The owner approved pressing centre after the fifth band to enter the
settings. From there the ring selects a row and centre toggles it. One
counterclockwise step from the first row returns to the fifth band.

## Consequences

The frequency labels match the centres used by the shared Rust DSP.
Enabled Volume limit caps the app's linear gain at 0.5 until there is a
user-adjustable limit. EQ presets and gain-cap adjustment are separate
from this screen. Gain and option changes are saved to the device database.
