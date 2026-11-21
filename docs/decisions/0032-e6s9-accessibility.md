# 0032 — Mobile screen reader navigation

## Status

Implemented on 2026-11-21.

## Context

The wheel is the touch navigation in the handoff. The app also needs
a way to select and activate items through VoiceOver and TalkBack.

## Decision

Semantic focus updates the wheel selection. A screen reader can
activate the focused item, adjust a band or playback position, and
toggle an EQ setting. Lists permit scrolling when accessible navigation
is active. The EQ status rows are 44 points high to meet the minimum
hit size. Their OFF values and track-row secondary metadata use the
readable tertiary text colour.

## Consequences

The same navigation and player commands serve the wheel and
assistive technology. Touch users continue to browse the lists with
the wheel by default. Physical device review is still required.
