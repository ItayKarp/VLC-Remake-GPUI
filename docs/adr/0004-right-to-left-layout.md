# ADR 0004 — Right-to-left layout

- **Status:** Accepted.
- **Story:** S9.2.2
- **Date:** 2026-11-24

## Context

Hebrew and Arabic readers expect the interface mirrored: navigation on the right, a row's
first item at the right, text aligned right, "back" pointing right. Media players are a known
exception: transport controls and the seek bar describe time, which runs left to right in
those locales' players too (YouTube, Android's media controls, Windows' Media Player, VLC 3
itself keep them left to right).

GPUI has no layout direction. Its styles don't carry one to Taffy, and Taffy 0.15's flexbox
ignores `direction` (only leaf padding and absolute insets read it). Patching our GPUI fork to
mirror every element automatically would also mirror what must not mirror, and every position
the app computes from the pointer (seek bar, sliders, popovers anchored at a click, the
subtitle timeline) is in left-to-right window coordinates.

## Decision

1. **The layout mirrors** for right-to-left languages: the library panel (on the right, its
   rail at the outer edge), the queue, the main window's side panels, Settings (category list
   on the right, labels then controls from the right), menus, dialogs, toasts and onboarding.
   Rows lay out from the start side, padding, margins and edge insets swap sides, text aligns
   to the start, and submenus open toward the start side (Left opens them, Right closes them).
2. **These stay left to right**: the player's control bar (transport, seek bar, time codes,
   volume and its buttons), the miniplayer and music mode's transport, the subtitle timeline,
   sliders, video and the video's own overlays (OSD, statistics, subtitles, which follow their
   text's direction), and the title bar, whose macOS window buttons are always on the left.
3. **Icons** that point (back, forward, disclosure chevrons in menus) are mirrored with the
   layout; media icons (play, skip, rewind) are not.
4. **How:** app-side, not in GPUI. `i18n::DirectionalStyle` gives start/end versions of the
   row, padding, margin, inset and alignment styles (`flex_row_start`, `ps`, `pe`, `ms`, `me`,
   `inset_start`, `inset_end`, `text_start`), which pick the side from the interface
   language's direction. Views use them where the layout should mirror and keep left/right
   where it shouldn't; `i18n::mirrored` picks an icon or side. Popovers stay anchored to the
   control that opened them, below or above it; their contents mirror.
5. **Text** of either direction is shaped and ordered by the OS's shaper inside each line;
   that's unchanged.
6. **Checked with a pseudo-locale.** No right-to-left translation ships yet. Debug builds
   list "Pseudo RTL" (ar-XB) under Interface language: English words, reading forwards,
   inside a right-to-left paragraph, with the layout mirrored.

## Consequences

- New views have to use the start/end styles for mirroring layout; left/right now means
  "stays left to right". Review catches it, and the RTL pseudo-locale shows it.
- Mirroring can be dropped in favour of GPUI's own direction support if upstream adds it.
- Until a real right-to-left translation ships, mirroring is only seen in debug builds.
