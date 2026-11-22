# Redesigned VLC — Product Requirements Document

**Status:** Draft v1 · **Date:** 2026-11-21 · **Baseline:** commit `1a44768` (GPUI onboarding flow and app icon)

This document is the build plan for the redesigned VLC desktop app, from the onboarding that already exists to a 1.0 release. It's written as **stages → stories → subtasks**, in the order they should be built, and each story lists its edge cases.

---

## Contents

1. [How to use this document](#1-how-to-use-this-document)
2. [Product overview](#2-product-overview)
3. [Where we are today](#3-where-we-are-today)
4. [Architecture and technical direction](#4-architecture-and-technical-direction)
5. [Roadmap at a glance](#5-roadmap-at-a-glance)
6. Stages
   - [Stage 0 — Foundations and onboarding hand-off](#stage-0--foundations-and-onboarding-hand-off)
   - [Stage 1 — Playback engine](#stage-1--playback-engine)
   - [Stage 2 — Player screen MVP](#stage-2--player-screen-mvp)
   - [Stage 3 — Queue, playback order and playlists](#stage-3--queue-playback-order-and-playlists)
   - [Stage 4 — Library](#stage-4--library)
   - [Stage 5 — Settings](#stage-5--settings)
   - [Stage 6 — Subtitles](#stage-6--subtitles)
   - [Stage 7 — Audio and video effects](#stage-7--audio-and-video-effects)
   - [Stage 8 — Advanced features](#stage-8--advanced-features)
   - [Stage 9 — Release hardening](#stage-9--release-hardening)
7. Appendices
   - [A. Cross-cutting edge-case catalog](#appendix-a--cross-cutting-edge-case-catalog)
   - [B. Decisions to make](#appendix-b--decisions-to-make)
   - [C. Design gaps and typos](#appendix-c--design-gaps-and-typos)
   - [D. Design reference index](#appendix-d--design-reference-index)

---

## 1. How to use this document

- **Work top to bottom.** Stages are in build order. Within a stage, do the stories in order unless a story's **Depends on** line says otherwise.
- **IDs:** a story is `S<stage>.<n>` (for example `S2.3`); a subtask is `S<stage>.<n>.<m>`. Tick the checkboxes (`- [ ]` → `- [x]`) as you go, and reference IDs in commits and PRs (`S2.3.2: hover thumbnail card`).
- **Each story has:** *Why* (when it isn't obvious) · *Depends on* · *Size* · *Design refs* · *Subtasks* · *Acceptance criteria* (where useful) · *Edge cases*.
- **Size** (one developer): **S** ≤ 2 days · **M** ≤ 1 week · **L** 2–3 weeks · **XL** > 3 weeks, so split it first.
- **Edge cases:** those listed in a story are specific to it. The [cross-cutting catalog](#appendix-a--cross-cutting-edge-case-catalog) applies to **every** story; read it before closing one.
- **Decisions** (`D1`…`D23`, in [Appendix B](#appendix-b--decisions-to-make)) block the stories that mention them. Resolve them before starting those stories.
- **Design refs** are paths under `docs/`. When a design and this document disagree, raise it; don't silently pick one. Known conflicts are listed in [Appendix C](#appendix-c--design-gaps-and-typos).

### Definition of Done (every story)

- [ ] It matches the design in **dark and light** themes and with at least one **non-default accent**.
- [ ] Every new control works with the **keyboard** and shows a focus state.
- [ ] Its edge cases are handled, or deferred with a tracked issue that's linked from the checkbox.
- [ ] Unit tests for logic (parsers, state machines, mappings), and a screenshot or manual check for UI.
- [ ] `cargo fmt`, `clippy -D warnings` and the tests pass on Windows, macOS and Linux CI.
- [ ] User-visible strings go through the i18n layer (after S9.2 lands, retrofit the earlier ones).
- [ ] From Stage 5 on: the feature's settings rows exist in the Settings window.
- [ ] Nothing blocks the UI thread: I/O, decoding and network work run in the background.

---

## 2. Product overview

**Vision:** a modern desktop media player with VLC's "plays everything" engine and a redesigned, library-first interface. It remembers where you stopped, organizes your media folders, and lets you reshape the player to fit you.

**Platforms:** Windows 10/11 (x64, arm64) · macOS 13+ (Apple Silicon and Intel) · Linux (X11 and Wayland).
**Stack:** Rust 2024 edition, GPUI (from Zed `main`), with a native playback backend (decided in S1.1; the recommendation is libVLC).

**Goals for 1.0**
1. First run → onboarding → a library that already shows the user's media, with no extra setup.
2. Smooth playback up to 4K60 and HDR, with designed controls, seek preview and resume.
3. Queue, playlists, subtitles (including bulk-adding them to a season), and audio and video effects.
4. Every onboarding choice and player behavior is configurable in Settings.
5. Accessible, localized, signed and updatable.

**Non-goals for 1.0:** mobile, TV or web versions; DRM-protected streaming services; tag editing; an extension system and skins (S8.14); built-in torrenting.

---

## 3. Where we are today

| Area | State | Where |
|---|---|---|
| Onboarding: Welcome and media folder → Theme and accent with a live preview → Keyboard shortcuts | ✅ Implemented | `src/onboarding.rs` |
| Theme modes (Light, Dark, System) and 9 accents, palette | ✅ Used only by onboarding | `src/theme.rs` |
| Settings persistence | ⚠️ Written on Finish, **never read** | `src/settings.rs` |
| Custom title bar (native traffic lights on macOS; min/close with native hit-testing on Windows) | ✅ | `src/window_chrome.rs` |
| Embedded SVG icons (19) and app icon pipeline (`.ico`, `.icns`, PNGs) | ✅ | `src/assets.rs`, `examples/gen_icons.rs`, `build.rs` |
| Design references: about 190 screen PNGs, 225 icons, 11 onboarding scenes | ✅ | `docs/ui/**`, `docs/icons/**`, `docs/onboarding_scenes/` |
| Playback, player UI, library, queue, settings UI | ❌ Not started | — |

### Gaps in the current onboarding implementation (each one is fixed by a story)

| # | Gap | Fixed in |
|---|---|---|
| 1 | Settings are never read, so onboarding shows on **every** launch. | S0.1, S0.2 |
| 2 | Finish closes the only window and `QuitMode::LastWindowClosed` quits the app (see the `TODO` at `src/onboarding.rs:120`). | S0.2.2 |
| 3 | A save failure is only printed to stderr, so the user's choices can be lost without any message. | S0.2.3 |
| 4 | The TOML is built with `format!` and only `\` is escaped, so a `"` or a newline in the path corrupts the file. | S0.1 |
| 5 | The default folder is `USERPROFILE\Videos`, which ignores Windows Known Folder redirection (OneDrive), and nothing checks that it exists. If `HOME`/`USERPROFILE` is unset, it becomes the relative path `Videos`. | S0.1, S4.2 |
| 6 | The window is fixed and non-resizable at 1088×792, which doesn't fit a 1366×768 screen at 125% scaling. | S0.2.7 |
| 7 | The chosen theme and accent apply only inside onboarding. | S0.3 |
| 8 | `on_accent` is `#111111`, which fails WCAG AA on the Blue and Purple accents. | S0.3.3 |
| 9 | The shortcut list is hard-coded and **contradicts** the menu and dialog designs (`<`/`>` versus N/P, Shift+R versus R). | S0.5, D3 |
| 10 | "Find the full list in the menu." — there's no menu yet. | S2.9, S2.10.4 |
| 11 | The theme tiles can't be chosen with the keyboard (only the accent responds to arrow keys), and there's no focus ring. | S0.4.14 |
| 12 | Backspace means "Back", which will misfire once any text input exists. | S0.5 |
| 13 | Onboarding asks for one video folder, but the library also has a Music section. | S4.2.7, D7 |
| 14 | There's no way to change the media folder after onboarding, and no Settings design for it. | S5.9.3, D11 |

---

## 4. Architecture and technical direction

```
┌───────────────────────── UI (GPUI views) ─────────────────────────┐
│ Onboarding · Player · Library (overlay + full) · Settings · Dialogs│
├──────────────── App state (GPUI entities / globals) ──────────────┤
│ Settings · Theme · Keymap/Actions · Player · Queue · Library model │
├─────────────────────── Services (background) ─────────────────────┤
│ Playback engine · Indexer + FS watcher · Thumbnailer · Metadata    │
│ Waveform profiler · OpenSubtitles client · SQLite DB               │
├──────────────────────────── Platform layer ───────────────────────┤
│ Single instance/IPC · Media keys/Now Playing · Keep-awake · Keychain│
│ File associations · Always-on-top · Global hotkeys · Known folders │
└────────────────────────────────────────────────────────────────────┘
```

**Rules**
1. The UI thread never waits on disk, network or the decoder. Background work reports back through channels and `cx.spawn`.
2. Each piece of state has one owner. For example, repeat and shuffle live in `Queue`, and both the queue buttons and Settings → Playback edit that one state.
3. Everything the user can trigger is a GPUI **action**, so the keymap, menus, tooltips and the onboarding cheat sheet all read the same source.
4. Layouts that can be customized later (the control bar and title bar) are **data-driven from day one** (S2.2.1).

**Biggest technical risk:** GPUI has no video element, and its zero-copy `surface()` exists only on macOS. **S1.1 must prove a video path on all three OSes before UI work builds on top of it.**

**Suggested module layout**

```
src/
  main.rs            app start, routing (onboarding vs main window)
  app/               windows, actions, keymap, menus, single-instance
  settings/          schema, load/save, migrations, registry (S5.1)
  theme.rs           palette, accents, contrast helpers
  ui/components/     button, dropdown, slider, toggle, popover, menu, modal, list…
  onboarding.rs      (exists)
  player/            player view, control bar, popovers, panels
  engine/            backend wrapper, state machine, video surface
  library/           model, indexer, watcher, thumbnails, metadata, views
  queue/             queue, playlists, import/export
  subtitles/         loading, matching, OpenSubtitles
  db/                SQLite, migrations
  platform/          windows.rs · macos.rs · linux.rs
```

**Where data lives**

| Data | Location | Notes |
|---|---|---|
| Preferences (theme, accent, keymap, …) | config dir `settings.toml` (Windows: Roaming `%APPDATA%`) | follows the user between machines |
| Machine-local state (media folders, window geometry, audio device) | local dir `local.toml` (Windows: `%LOCALAPPDATA%`) | paths aren't valid on other machines |
| Library, history, playlists, bookmarks | data dir `library.db` (SQLite, WAL) | |
| Thumbnails, waveform peaks | cache dir | safe to delete |
| Logs, crash reports | data dir `logs/` | size-capped |
| Passwords and tokens | OS keychain | never in files |

---

## 5. Roadmap at a glance

```mermaid
flowchart LR
  S0[Stage 0<br/>Foundations] --> S2[Stage 2<br/>Player MVP]
  S1[Stage 1<br/>Engine] --> S2
  S2 --> S3[Stage 3<br/>Queue and playlists]
  S3 --> S4[Stage 4<br/>Library]
  S1 --> S4
  S2 --> S5[Stage 5<br/>Settings]
  S2 --> S6[Stage 6<br/>Subtitles]
  S5 --> S7[Stage 7<br/>Effects]
  S6 --> S7
  S4 --> S8[Stage 8<br/>Advanced]
  S7 --> S8
  S8 --> S9[Stage 9<br/>Hardening]
```

| Stage | Theme | Milestone at the end | Can start |
|---|---|---|---|
| 0 | Foundations and onboarding hand-off | **M0 "Remembers me":** onboarding runs once, and the main window opens themed | now |
| 1 | Playback engine | **M1 "Plays a file":** the engine works and video draws in GPUI on all OSes | now; **S1.1 runs in parallel with Stage 0** |
| 2 | Player screen MVP | **M2 "Watchable" (internal alpha):** the designed player and every onboarding shortcut work | after 0 and 1 |
| 3 | Queue and playlists | Episodes play back to back; playlists are saved | after 2 |
| 4 | Library | **M3 "Library alpha":** onboarding → library of your folder → play → resume | after 3 (S4.1–S4.4 can start after S1.7) |
| 5 | Settings | **M4 "Configurable":** every onboarding choice can be changed | shell after 2; pages as features land |
| 6 | Subtitles | Auto-loading, appearance, sync, Add Subtitles dialog | after 2 |
| 7 | Audio and video effects | **M5 "Feature-complete beta"** | after 5.1 and 6 |
| 8 | Advanced (P1 → P2 → P3) | **M6 "1.0 candidate":** all P1 stories done | per story |
| 9 | Release hardening | **M7 "1.0"** | continuously from Stage 2; final pass last |

**Two parallel tracks keep developers unblocked:**
- **Track A (UI):** S0.1 → S0.2 → S0.3 → S0.4 → S0.5 → Stage 2 (against the mock engine from S1.2.7) → S5.1–S5.2.
- **Track B (engine and data):** S1.1 spike → S1.2–S1.7 → S4.1–S4.4 (indexer, metadata, thumbnails) → S6.1.

**Scope cuts:** **Alpha** = Stages 0–4 · **Beta** = + Stages 5–7 · **1.0** = + Stage 8 P1 stories + Stage 9.

---

## Stage 0 — Foundations and onboarding hand-off

> **Goal:** the app starts, remembers who you are, and onboarding leads somewhere. Nothing in later stages works without this stage.
> **Exit criteria:** second launch skips onboarding and opens the main window in the chosen theme and accent; settings survive crashes, corruption and read-only disks; the shared component kit and action/keymap system exist and are used by the main window shell.

### S0.1 — Typed, crash-safe settings store

**Why:** `src/settings.rs` builds TOML by hand (`format!`) and nothing ever reads it back, so onboarding runs on every launch. Every later stage adds settings, so this has to be solid first.
**Depends on:** —  **Size:** M  **Design refs:** none (data layer)

**Subtasks**
- [x] S0.1.1 Add `serde` and `toml` (or `toml_edit`, see S0.1.6). Define `Settings` with `schema_version`, `onboarding_complete`, `media_folders: Vec<PathBuf>` (onboarding writes one, the Library settings screen can manage several), `theme`, `accent`, and empty sections that later stages fill (`player`, `audio`, `video`, `subtitles`, `library`, `privacy`, `keymap`, `window`). Every field gets `#[serde(default)]`.
- [x] S0.1.2 Load settings once at startup into a GPUI `Global`, and expose `cx.update_global` helpers so views react to changes.
- [x] S0.1.3 Atomic save: write `settings.toml.tmp`, `fsync`, rename over `settings.toml`, keep the previous file as `settings.toml.bak`.
- [x] S0.1.4 Save on change with a short debounce (about 500 ms), and flush synchronously in `on_app_quit`.
- [x] S0.1.5 Read each field leniently: an unknown enum value (for example `accent = "magenta"` written by a newer build) falls back to that field's default and logs a warning. It never discards the whole file.
- [x] S0.1.6 Keep keys this build does not know about, so running an older build doesn't wipe a newer build's settings. Use `toml_edit` for this, which also keeps comments the user added by hand.
- [x] S0.1.7 Migrations: `schema_version` plus a list of `fn migrate_vN_to_vN+1`. The current onboarding file (no `schema_version`) is treated as v0.
- [x] S0.1.8 Split machine-specific data from user preferences. Theme, accent and keymap go in the roaming config dir. Media folder paths, window geometry and audio device IDs go in a **local** (non-roaming) file, because Windows Roaming AppData and synced home folders carry them to machines where the paths don't exist.
- [x] S0.1.9 Override the config location with `RVLC_CONFIG_DIR` (tests, portable mode, debugging).
- [x] S0.1.10 Unit tests: round-trip, corrupt file, unknown keys, unknown enum values, migration from v0, paths containing quotes, backslashes and unicode.

**Acceptance criteria**
- Kill the process (Task Manager / `kill -9`) during a save, 100 times in a loop: the settings file is never empty or half-written.
- A settings file from the current onboarding build loads correctly after migration.

**Edge cases**
- Corrupt or unparsable file: rename it to `settings.corrupt-<timestamp>.toml`, start from defaults, show a one-time notice, and don't delete the user's data.
- Empty (0-byte) file left behind by a crash on a filesystem without atomic rename (some network shares and FAT32 USB drives in portable mode): treat it as missing and restore from `.bak` if present.
- Config directory can't be resolved (`APPDATA`/`HOME` unset, which happens under some services and sandboxes): use platform known-folder APIs (the `dirs` crate). If that fails too, run with in-memory settings and show a banner saying changes won't be saved.
- Directory is read-only, the disk is full, or permission is denied: keep settings in memory, warn once per session, retry on the next change. Don't block the UI.
- Paths containing `"`, `\`, newlines (legal on Linux), emoji or right-to-left characters must round-trip exactly. The current hand-written escaping only handles `\`.
- Non-UTF-8 paths (unpaired surrogates on Windows, arbitrary bytes on Linux): `Path::display()` silently loses data. Store them with a lossless encoding, or reject them with a clear message.
- Two instances writing at once: prevented by single instance (S0.6). As a backstop, take an advisory file lock when writing.
- An antivirus or indexer briefly locks the file during rename on Windows (sharing violation): retry with backoff a few times before reporting an error.
- The file is a symlink, for example a dotfiles repo: write to the symlink's target and don't replace the link with a regular file.
- The clock jumps backwards: nothing should rely on file modification times for ordering.

---

### S0.2 — Launch routing and onboarding hand-off

**Why:** `Onboarding::finish` (`src/onboarding.rs:111`) saves and then calls `window.remove_window()`. With `QuitMode::LastWindowClosed` the app quits immediately (see the `TODO: open the library window` comment). A save failure is only `eprintln!`-ed, so the user can lose their choices without knowing.
**Depends on:** S0.1  **Size:** M  **Design refs:** `docs/onboarding_scenes/*`

**Subtasks**
- [x] S0.2.1 At startup, open onboarding if `onboarding_complete` is false, otherwise open the main window (S0.3 shell). *(Until S0.3.4 lands, the main window is a placeholder in `src/main_window.rs`.)*
- [x] S0.2.2 Finish order: save, then **open the main window**, then close onboarding. If onboarding closes first, the app quits.
- [x] S0.2.3 If saving fails, stay on step 3 and show an inline error with **Retry** and **Continue without saving**. The second option opens the main window, keeps the choices in memory, and runs onboarding again next launch.
- [x] S0.2.4 Closing onboarding early (title-bar close, Alt+F4, Cmd+Q, Cmd+W) quits without saving, and the next launch starts again at step 1. Decide whether partial progress should persist; the recommendation is no. *(Decided: no. Choices stay in the onboarding view until Finish. Cmd+Q/Cmd+W are bound on macOS.)*
- [x] S0.2.5 Add a `--reset-onboarding` CLI flag and a "Run setup again" entry (Help menu, and Settings → Interface in Stage 7). *(macOS Help menu, and the main window's `…` menu on every OS (S0.5.4).)*
- [ ] S0.2.6 Launching with a file on first run (the user double-clicks a video before ever opening the app): play the file immediately with default theme and accent, skip onboarding this time, and show it on the next plain launch. This needs a decision. *(Routing done per D4: the main window opens with the files and `onboarding_complete` stays false. Playing the file waits for the engine (Stage 1), and macOS Finder opens need `on_open_urls` (S0.6.3).)*
- [x] S0.2.7 Small screens: the onboarding window is fixed at 1088×792 and can't be resized. A 1366×768 laptop at 125% scaling has about 1093×582 logical pixels of work area, so the window doesn't fit. Detect the display's visible bounds and either scale the layout down to a minimum (about 900×640) with a scrollable body, or let the window shrink. *(The window shrinks to the visible area and becomes resizable, and the body scrolls.)*
- [ ] S0.2.8 Open onboarding on the display that has the cursor or the foreground window, not always the primary display. *(Done on Windows. "Run setup again" and the hand-off use the current window's display. macOS/Linux still use the primary display.)*
- [x] S0.2.9 Remember window geometry per window kind (main window, player if separate, settings) in local settings, and restore it. *(Main window now; `window.settings` is reserved for Stage 5. A GPUI bug makes windows opened on a monitor with a different DPI 1.25× too large; `app::correct_dpi_drift` fixes windowed ones, while the restore size of maximized ones can still be off.)*

**Acceptance criteria**
- Fresh install → onboarding → Finish → main window opens in the chosen theme and accent; quit and relaunch → main window opens directly.
- Saving onto a read-only config dir shows the error UI, and both options behave as specified.

**Edge cases**
- The restored window position is on a monitor that's no longer connected, or the resolution changed: clamp it into the visible area of an existing display and never restore off-screen.
- Saved bounds were from a display with different DPI: convert using logical pixels and the display scale.
- Restoring a maximized or fullscreen window: restore the state, but keep the normal bounds for un-maximizing.
- Wayland: the app can't position windows, so let the compositor place them.
- macOS: closing the last window doesn't quit by convention. Decide whether the main window's red button hides or quits the app, and support `on_reopen` (clicking the Dock icon with no windows open reopens the main window).
- The OS switches light/dark while onboarding is showing "System": it already re-renders (`observe_window_appearance`). Keep that behavior in the main window.
- The user picks a media folder in onboarding that is later deleted, renamed or on an unplugged drive: the main window must start anyway (see S4.2 edge cases).

---

### S0.3 — App-wide theme service and the main window shell

**Why:** theme and accent exist only inside the onboarding view. Every later screen needs them, and they must update live when changed in Settings.
**Depends on:** S0.1, S0.2  **Size:** M  **Design refs:** `docs/ui/player/screens/light/player_light.png`, `docs/ui/library/sidebar/*`, onboarding step 2

**Subtasks**
- [x] S0.3.1 Move `ThemeMode`, `Accent` and `Palette` into a `Theme` global. `Palette::new` stays the single source of colors. Every window observes the theme global and its own window appearance. *(`src/theme/`: views call `theme::observe` and `theme::palette`. The global mirrors settings, which still own and persist the values; onboarding previews its unsaved choices through `Theme::palette_for`.)*
- [x] S0.3.2 Add the palette tokens the library, player and settings designs need (sidebar background, selected item, card hover, divider, overlay scrim, tooltip, danger, success, focus ring). Sample them from the designs in both light and dark. *(Sampled where a design exists, mostly dark. The light values without a design, and tooltip, danger and success in both themes, are derived and marked in `palette.rs`. See C.2 for the light player design.)*
- [x] S0.3.3 Contrast: `on_accent` is fixed at `#111111`, which falls below WCAG AA (4.5:1) on Blue `#185DD3` (3.19:1) and Purple `#8E42FF` (3.93:1). White passes on both (5.92:1 and 4.80:1). Compute `on_accent` per accent as dark or white text, whichever contrasts more, and add a unit test that checks every accent in both themes.
- [x] S0.3.4 Main window shell: the custom title bar (reuse `window_chrome.rs`, extended to the player title bar layout in S2.1: logo, `…` menu, sidebar toggle, folder chip, title, Minimize/Maximize/Fullscreen/Close), and a content area that shows an empty state with **Open file…** and "drop files here". This is the target S0.2 hands off to until the Library lands in Stage 4. *(The sidebar toggle is left out until the library exists (Stage 4). For now the `…` menu holds Open file and Run setup again, and the folder chip shows the first media folder. Opened and dropped files are listed until the engine plays them.)*
- [x] S0.3.5 The main window can be resized, with a minimum size taken from the narrowest design (likely around 800×500), and supports maximize and double-click on the title bar to maximize.
- [x] S0.3.6 Respect the OS "reduce motion" setting for animations, and keep layouts working under Windows High Contrast / forced colors. *(GPUI doesn't read either setting, so `theme/system.rs` reads them: on Windows from `SystemParametersInfo`, on macOS and Linux from `defaults` and `gsettings`. They are re-read when a window is activated or the OS appearance changes. The result feeds `cx.set_reduce_motion`, and under Windows High Contrast every token maps to a system color. macOS and Linux don't expose high-contrast colors.)*

**Edge cases**
- On Linux desktops that don't report appearance, "System" falls back to dark and the Settings screen says so.
- The OS appearance changes while a popover, menu or modal is open: they repaint and don't close.
- A theme change during fullscreen playback must not flash the video area. Video letterboxing is always black, independent of theme.
- Accent-colored lines, icons and text on the **light** background `#F3F3F3` fall below the 3:1 minimum for UI components with Gray (1.98:1), Yellow (2.29:1), Teal (2.45:1), Pink (2.88:1) and Orange (2.90:1). Derive a darker "accent on light" variant per accent for strokes and text. Fills behind `on_accent` text are unaffected.

---

### S0.4 — Shared component kit

**Why:** the designs reuse the same pieces everywhere (popovers on the control bar, context menus, dropdowns, sliders, collapsible panel sections). Building them once avoids every stage reinventing them inconsistently.
**Depends on:** S0.3  **Size:** L  **Design refs:** `docs/ui/shared/components/*`, `docs/ui/settings/components/*`, `docs/ui/library/menus/*`, `docs/icons/*`

**Subtasks**
- [x] S0.4.1 Button: default and variant2 states (default, hover, pressed, focused, disabled). Move the onboarding `button()` helper into the kit. *(`ui::Button`: `button-default` is the `Subtle` style, `button-variant2` the `Filled` style, plus `Primary` (accent) and `Ghost`. Onboarding uses it; its `button()` helper is gone. See C.2.)*
- [x] S0.4.2 Icon button with a tooltip (shows the shortcut, for example "Play (Space)"). *(`ui::IconButton` and `ui::tooltip`: the tooltip shows the label and the shortcut, and the accessible name reads "Play (Space)".)*
- [x] S0.4.3 Dropdown/select: closed and open states, keyboard navigation, scrolls when there are many items, type-to-select. *(`ui::Dropdown`: opens below, or above when there is no room; Up/Down/Home/End/PageUp/PageDown, Enter/Space, Escape, Tab; type-to-select with repeated letters cycling; scrolls past 8 rows.)*
- [x] S0.4.4 Toggle: the designed square-knob pill with "ON"/"OFF" text (`shared/components/toggle-*`; ON is orange on brown), plus a checkbox. *(`ui::Toggle`, `ui::Checkbox`.)*
- [x] S0.4.4a Option rows (default, hover, selected with an orange ✓) used by dropdowns and menus; segmented "values" controls with 2 and 3 options (Relative | Pixels, Dark | Light | System); and the stacked choice with an inline field ("Remember last volume" / "Always starts at: 200%"). *(`ui::OptionRow`, `ui::Segmented`, `ui::StackedChoice`. The values controls are radio groups: one tab stop, arrows move the choice.)*
- [x] S0.4.4b Tabs with icon + label and an orange underline (the in-player panels). *(`ui::Tabs`; Left/Right switch tabs.)*
- [x] S0.4.4c Translucent surfaces: popovers, panels, menus and tooltips are about 95% opaque in the designs. Decide whether that means real backdrop blur (Windows Mica/Acrylic, macOS vibrancy via `set_background_appearance`) or flat colors. *(Decided: flat colors, no OS blur. GPUI can only blur the whole window background, and Mica/vibrancy behind a video would show the desktop, not the video. Panels keep the designs' 95%; menus, dropdowns and popovers are opaque because text underneath showed through. See C.2.)*
- [x] S0.4.5 Slider: horizontal and vertical (EQ bands), with a centered-zero variant (balance, EQ gain), drag, click to jump, arrow keys (Shift for bigger steps), double-click to reset to default, value tooltip while dragging, and an optional editable numeric field. *(`ui::Slider` with `SliderModel` (clamps, snaps, ignores NaN/infinity) and a `Throttle` for held keys; Home/End/PageUp/PageDown too. The numeric field accepts units and a decimal comma.)*
- [x] S0.4.6 Popover anchored to a trigger button that flips when it would leave the window. Used by the control bar popovers. *(`ui::popover`: attached to the trigger's bounds from the previous frame, so it follows the trigger when the window resizes or changes monitor; flips, shifts inside the window, and scrolls when it still doesn't fit.)*
- [x] S0.4.7 Context menu: separators, submenus, checkmarks and radio groups, disabled items, shortcut hints, keyboard navigation, closes on outside click, Escape or window blur. Evaluate `WindowKind::AnchoredPopup` for native menus that can extend past the window edge. *(`ui::ContextMenu`; Left/Escape close only a submenu; type-ahead. `WindowKind::AnchoredPopup` was evaluated and not used: it's a separate OS window that takes focus from the main one and isn't positioned on Wayland. Revisit if a menu must be larger than the window.)*
- [x] S0.4.8 Modal dialog: focus trap, Escape closes, Enter triggers the default button, scrim, sized to fit within the window. *(`ui::ModalLayer` and `ui::modal::Dialog`: nested dialogs, Escape closes the top one, focus returns to where it was.)*
- [x] S0.4.9 Text input: caret, selection, clipboard, IME composition, undo, placeholder, validation states. Single-line first; multi-line only if a design needs it. *(`ui::TextInput` with the editing rules in `EditState`: grapheme and word movement, undo/redo that groups typing, max length, error state. IME goes through GPUI's platform input handler; it hasn't been tried with a real IME yet.)*
- [x] S0.4.10 Collapsible section (the collapsed and expanded panel headers used in the audio and video panels). *(`ui::Collapsible`.)*
- [x] S0.4.11 Virtualized list and grid (`uniform_list` or a custom grid) for libraries with 10k+ items. *(`ui::list::VirtualList`: a list or a grid of fixed-width items on `uniform_list`, with keyboard selection by row and column.)*
- [x] S0.4.12 Toast/notification area and inline error banner. *(`ui::Toasts` (info and success dismiss themselves; errors stay) and `ui::banner`. The main window shows settings notices as banners.)*
- [x] S0.4.13 Icon pipeline: copy the icons each stage needs from `docs/icons/**` into `assets/icons/` with short names, register them in `assets.rs`, and add a test that every `icons::*` constant resolves. GPUI's `svg()` paints a single-color mask, so the `two_tone=yes` icons need either two layered SVGs (base and accent) or rasterizing. Pick one approach and document it. *(Two layered SVGs: `<name>.svg` and `<name>.accent.svg`, drawn by `ui::icon::two_tone` with the accent at runtime. Documented in `assets.rs`; tests check every constant resolves and both layers share a viewBox.)*
- [x] S0.4.14 Keyboard focus model: visible focus ring, Tab/Shift+Tab order, and arrow-key navigation inside groups (theme tiles, swatches, menus, grids). *(`ui::focus`: Tab/Shift+Tab on every window, a focus ring for keyboard focus only, and arrow keys inside groups. Onboarding's theme tiles and swatches are keyboard groups now, and its Enter/Back keys only act when no control has focus. Headless tests in `ui/tests.rs` cover menus, dialogs, dropdowns, sliders, inputs and Tab order. A component gallery (`--component-gallery`) shows every piece.)*

**Edge cases**
- Popovers and menus near any window edge flip or shift to stay visible. If they still don't fit (a tiny window), they scroll.
- The window resizes or moves to another monitor while a popover is open: reposition it or close it.
- Nested modals: Escape closes only the top one.
- Long labels truncate with an ellipsis and show the full text on hover.
- Right-to-left text (Hebrew and Arabic file names) must shape and truncate correctly. Mixed-direction titles need the Unicode bidi algorithm. *(Open: GPUI shapes RTL runs, and `TextInput` edits by grapheme, but mixed-direction lines aren't reordered and RTL truncation is untested.)*
- Very long words with no spaces (URLs, hashes) must wrap or truncate, never overflow.
- A slider receiving a value outside its range from settings or the engine clamps for display. NaN or infinite values coming from the engine never reach the UI.
- Holding a key on a slider auto-repeats without flooding the engine (throttle updates).

---

### S0.5 — Actions, keymap and menus

**Why:** onboarding step 3 teaches 12 shortcuts and says "Find the full list in the menu." Those promises have to be true. Shortcuts also need to depend on context (typing in the search box must not toggle playback).
**Depends on:** S0.4  **Size:** M  **Design refs:** onboarding step 3, `docs/ui/dialogs/keyboard_shortcuts/*`, `docs/ui/player/menus/main_menu/*`, `docs/ui/settings/screens/controls/*`

**Subtasks**
- [x] S0.5.1 Define a GPUI action for every command (play/pause, seek ±, previous/next item, previous/next frame, cycle subtitle/audio track, fullscreen, volume ±, toggle queue, toggle file browser, snapshot, record, open file, settings, and so on). *(`src/keymap/actions.rs`; each one has a row in the command table `keymap::commands::COMMANDS` with its label, area, default keys and held-key behavior. Until Stage 2 the main window only logs player commands; S2.10.3 gives them their effects.)*
- [x] S0.5.2 Default keymap with key contexts (`Player`, `Library`, `TextInput`, `Modal`). Single-letter and Space shortcuts don't fire while a text input has focus. *(Each binding's predicate comes from its area and key (`keymap::predicate`): letters and symbols skip controls marked `Typing` (text inputs, menus, dropdowns), Space/Enter/Escape/arrows skip any kit control marked `Control` (buttons press on Space, sliders move on arrows), and only global shortcuts work inside a `Modal`. The main window is `MainWindow Player` until the player gets its own view. Headless tests in `keymap/tests/dispatch.rs`.)*
- [x] S0.5.3 A single source of truth for shortcuts, so onboarding step 3, the Keyboard Shortcuts dialog, tooltips and the menus all read from the keymap instead of hard-coded constants (`LEFT_SHORTCUTS`/`RIGHT_SHORTCUTS` today). *(`keymap::shortcut` and `keymap::key_caps` read the binding the keymap ranks first. Onboarding, the main menu and the empty state's Open hint use them; the Keyboard Shortcuts dialog (S2.10.4) lists every binding through `keymap::shortcuts`; the control bar tooltips (Stage 2) will too.)*
- [x] S0.5.4 macOS menu bar via `cx.set_menus`. On Windows/Linux, the in-window main menu from the design. *(`keymap::menus`. The `…` menu lists the designed items plus Run Setup Again; commands nothing handles yet are greyed out, and Open Recent and Profile are empty submenus until they're designed. The macOS bar holds only commands with modifier shortcuts, because macOS would fire a plain-letter key equivalent even inside a text field.)*
- [x] S0.5.5 Keyboard layout handling: `<` and `>` are Shift+`,` and Shift+`.` on US layouts but separate keys on German ISO and other layouts, and AZERTY moves `,` and `.`. Decide on character-based versus physical-key bindings, test with US, UK, German, French AZERTY, Hebrew and Russian layouts, and react to `on_keyboard_layout_change`. *(Decided: character-based. GPUI matches the typed character, and on Windows it reports letters by their Latin virtual key, so letter shortcuts work on Hebrew and Russian too. `keymap/tests/layouts.rs` replays each layout's keystrokes as GPUI's Windows backend reports them; it hasn't been tried with real OS layouts or on macOS/Linux yet. A layout change re-renders the windows. Open: Ctrl+`\` needs AltGr on German and other layouts, and GPUI doesn't match it there.)*
- [x] S0.5.6 Key repeat: holding an arrow key seeks repeatedly with throttling (optionally accelerating). Holding Space doesn't toggle play/pause repeatedly. *(`keymap::repeat::RepeatGate`: toggles run once per press, and seek, frame step and volume run at most every 120 ms while held. Key-up or the window losing focus releases them. No acceleration.)*
- [x] S0.5.7 **Resolve the default keymap (decision D3) before writing it.** The designs contradict each other; see the shortcut conflict table in the Decisions section. On macOS, Ctrl maps to Cmd, and Settings should be Cmd+`,` by platform convention. *(Took the recommendation: P/N with `<`/`>` as aliases (onboarding shows P/N), Shift+R record, Shift+S snapshot, S stop, Ctrl+Shift+O open folder, Ctrl+Shift+C capture device, Ctrl+P settings (Cmd+`,` on macOS), Ctrl+Q quit. Tests check the table for clashes and keep Ctrl+C, Ctrl+F, Cmd+H and Cmd+M free.)*

**Edge cases**
- Shortcuts while IME composition is active (Chinese, Japanese, Korean input): the IME gets the keys.
- Dead keys (`^`, `` ` ``) on international layouts don't trigger actions.
- Numpad keys: decide whether NumpadPlus/NumpadMinus mirror the volume keys.
- Non-Latin layouts (Hebrew, Russian): letter shortcuts (F, L, Q, V, B) should still work, which argues for physical-key bindings or a Latin fallback.
- Shortcuts that collide with OS shortcuts (Cmd+H, Cmd+M on macOS; Alt+Space and Win+arrows on Windows): never override them.
- Backspace as "Back" in onboarding (`on_key_down`) must not fire while a text field has focus. This matters once any input is added there.
- A modal is open: player shortcuts are blocked except the ones the modal declares.

---

### S0.6 — Single instance, command line and OS "Open with"

**Why:** double-clicking five videos in Explorer or Finder must not open five players fighting over settings and the database.
**Depends on:** S0.2  **Size:** M  **Design refs:** none

**Subtasks**
- [x] S0.6.1 Single-instance lock with IPC (a named pipe on Windows, a Unix domain socket on macOS/Linux). A second launch forwards its arguments and the working directory, then exits. Add an "Allow multiple instances" setting for power users (Stage 7). *(`src/app/instance/`: an OS file lock on `instance.lock` next to `local.toml` picks the instance, so a crash never leaves a stale lock; the listener is an `interprocess` local socket. The second launch resolves its paths against its own working directory before sending them. Launches within 300 ms are batched (`instance/batch.rs`), and a hung instance shows "VLC is not responding" after 2 s. `library.allow_multiple_instances` exists in settings; its row comes with S5.9.1. The not-responding path hasnt been triggered by hand yet.)*
- [x] S0.6.2 Command-line parsing: paths, URLs, `--fullscreen`, `--reset-onboarding`, `--enqueue` (add to queue rather than replace). *(`app/cli.rs`, `app/media.rs`. `file://` URLs become paths, other URLs stay URLs. Until the queue exists (Stage 3), "replace" and "enqueue" act on the main windows list of opened items.)*
- [x] S0.6.3 macOS: `cx.on_open_urls` (Finder Open With, dropping onto the Dock icon), `on_reopen`, and Info.plist `CFBundleDocumentTypes` / `CFBundleURLTypes`. *(The URLs go through the same inbox as forwarded launches. The plist keys are in `packaging/macos/Info.plist.ext`, merged by cargo-bundle. Not yet tried on a Mac.)*
- [x] S0.6.4 File associations: Windows installer ProgIDs and "Default apps" registration; Linux `.desktop` file with `MimeType=`. *(One list in `src/app/file_types.rs`; tests keep `packaging/` in sync with it. Windows: `packaging/windows/register-file-types.ps1` registers per user until the installer (S9.4.1) does it. Linux: `packaging/linux/redesigned-vlc.desktop`.)*
- [x] S0.6.5 Add opened files to the OS recent documents list (`cx.add_recent_document`), subject to the privacy setting. *(`app/recent.rs`, gated by `privacy.save_recently_played`. GPUI only implements it on macOS, so Windows calls `SHAddToRecentDocs` directly. Linux is left out: GPUI has no hook and `recently-used.xbel` isnt written.)*

**Edge cases**
- Relative paths are resolved against the **launching** process's working directory, not the running instance's.
- Explorer launches N processes at once for a multi-file selection, and they race for the lock: the first takes it and the others forward. Batch arrivals within a short window (about 300 ms) into one queue operation.
- Windows command-line length limit (32,767 characters) with hundreds of files: some files are dropped by the OS. Document it and suggest dragging and dropping instead.
- A stale lock or socket after a crash: detect the dead owner (PID check or failed connect) and take over.
- The running instance is hung: the second instance times out (about 2 s) and shows "VLC is not responding" instead of hanging too.
- `file://` URLs with percent-encoding, UNC paths (`\\server\share`), `\\?\` long paths over 260 characters, Windows `.lnk` shortcuts and macOS aliases: normalize and resolve them. *(Open: URLs, UNC and long paths are handled; `.lnk` files and macOS aliases arent resolved yet.)*
- Launched while onboarding is showing: queue the files and play them after onboarding finishes, or follow the S0.2.6 decision. *(Queued and opened by the hand-off.)*
- Different users on the same machine (fast user switching): the lock is per user and per session, never global.

---

### S0.7 — Logging, crash reports and user-facing errors

**Depends on:** S0.1  **Size:** S

**Subtasks**
- [x] S0.7.1 `tracing` with a rolling file appender in the data dir (size-capped, about 5 files × 5 MB), and the level set through `RUST_LOG` or a setting. *(`src/logging/`: `logs/vlc.log` next to `local.toml`, rolling to `vlc.1.log` … `vlc.4.log` at 5 MB. `RUST_LOG` wins over `diagnostics.log_level` (default `info` for our crate, `warn` for GPUI and the rest), and a changed setting applies while running. `log` records from GPUI are bridged in. `RVLC_LOG` is gone.)*
- [x] S0.7.2 Panic hook: write a crash report (backtrace, version, OS, GPU) and offer to open the folder on next launch. Never upload anything automatically (see Privacy settings). *(`src/diagnostics/crash.rs`: `logs/crash-<time>-<pid>.txt`, the newest 10 kept. The next main window shows a toast with "Show Report", which reveals the file. Any panic, on any thread, writes the report and exits, so a dead background thread can't leave the UI frozen. The hook itself hasn't been triggered by hand yet.)*
- [x] S0.7.3 Replace every `eprintln!` with tracing plus, where the user is affected, a toast or banner. *(Every `log::` call is now `tracing::`; the main window has a toast stack, and a failed Run Setup Again shows an error toast there. Settings problems already had banners and onboarding its save error.)*
- [x] S0.7.4 Help → "Copy debug info" (version, OS, GPU, libVLC version, settings without paths). *(`CopyDebugInfo`: last item of the `…` menu, and Help on macOS. Preferences as TOML; of `local.toml` only `onboarding_complete` and the number of media folders. libVLC reads "not loaded yet" until Stage 1.)*

**Edge cases:** the log dir isn't writable (log to stderr only); paths in logs are personal data (redact the home dir as `~` and offer a "redact file names" option); a panic on a background thread must not leave the UI frozen. *(All three handled. The file always has the home dir as `~`; `privacy.redact_file_names_in_logs` makes `logging::path` show `<redacted>.mkv`, and launches are logged as counts, not paths. Its settings row comes with the Privacy screen.)*

---

### S0.8 — CI, builds and the GPUI dependency

**Depends on:** —  **Size:** S (then grows in Stage 9)

**Subtasks**
- [x] S0.8.1 CI matrix for Windows, macOS (arm64 and x64) and Linux (X11 and Wayland builds): `cargo fmt --check`, `clippy -D warnings`, `cargo test`, release build. *(`Jenkinsfile`, which replaced a GitHub Actions workflow that passed on every platform: Windows x64, macOS arm64 and x64, and Linux with both backends, X11 only and Wayland only, through the new `x11`/`wayland` crate features, then SonarQube analysis (`sonar-project.properties`: Clippy JSON and `cargo llvm-cov` coverage) and its quality gate. Results post to Discord. Screenshot images are archived per platform. Not run on Jenkins yet: agent labels, the SonarQube server and scanner, and the Discord webhook credential are placeholders in the Jenkinsfile's `environment` block. Running clippy and the tests off Windows for the first time turned up two Unix-only lints and three tests that assumed Windows; fixed. Also found that GPUI drew no text on macOS without `gpui_platform/font-kit`, and that Zed's workspace leaves `gpui_linux` with neither backend unless asked; both are now enabled in `Cargo.toml`. Macs without Xcode's Metal toolchain can build with `--features runtime_shaders`; the pipeline installs the toolchain instead.)*
- [x] S0.8.2 GPUI follows `zed` `main` (`branch = "main"` in `Cargo.toml`). Pin a specific `rev` in `Cargo.toml` (not only in `Cargo.lock`) and bump it on purpose, because upstream API breaks are frequent. *(Every GPUI entry is pinned to `2f808b3c`, the commit `Cargo.lock` already had; the comment above them says how to bump.)*
- [x] S0.8.3 Screenshot tests for key screens against the design PNGs using the existing `RVLC_STEP`/`RVLC_THEME`/`RVLC_ACCENT` dev overrides, extended to later screens. *(`src/screenshot_test.rs` renders a view off screen with GPUI's headless renderer and the real text system, crops it to the design's window area and counts pixels that differ by more than 40 per channel; it writes actual, expected and diff PNGs to `target/screenshots/`. The overrides are now `onboarding::DevOverrides`, which the tests set directly. All 11 onboarding scenes are covered and differ by 1.7–2.0% on macOS, so the limit is 2.5%. The diffs show real gaps: text a few pixels off, and step 2 has a Back button and larger theme tiles than its design. Runs on macOS (Metal) and on Linux when wgpu finds an adapter; Windows has no headless renderer in GPUI and skips, as does any platform without one unless `RVLC_SCREENSHOTS=require`. Later screens add a `Shot` each.)*

---

## Stage 1 — Playback engine

> **Goal:** a headless-tested engine that opens anything VLC users expect to open, and a proven way to draw its video inside GPUI on all three platforms.
> **Exit criteria:** the S1.1 decision record is accepted; a 4K60 file and a 1080p HDR file play smoothly in a GPUI window with GPUI controls drawn on top on Windows, macOS and Linux; the engine state machine passes its test suite.
> **This is the highest-risk stage. Start S1.1 in parallel with Stage 0.**

### S1.1 — Spike: playback backend and video inside GPUI (timeboxed, about 2 weeks)

**Why:** GPUI has no video element. Its `surface()` element accepts only a macOS `CVPixelBuffer` (`crates/gpui/src/elements/surface.rs` is `cfg(macos)`). Windows and Linux have no zero-copy path, and the designs draw controls, popovers and auto-hiding overlays **on top of** the video, which rules out the simplest "native child window" approach on some platforms.
**Depends on:** —  **Size:** L  **Output:** `docs/adr/0001-playback-backend.md`

**Subtasks**
- [x] S1.1.1 Compare backends: **libVLC** (3.x stable vs 4.x with GPU output callbacks), **libmpv** (render API), and **FFmpeg with a custom pipeline**. Criteria: codec and container coverage, network protocols (HTTP, HLS, DASH, RTSP, SMB, UPnP), DVD/Blu-ray/audio CD, subtitle rendering (SSA/ASS through libass), the filters the designs need (equalizer, compressor, spatializer, stereo widener, video adjust, crop, deinterlace, HDR tone-mapping), recording, snapshots, chapters, thumbnails, license (LGPL is fine with dynamic linking), redistributable size, and Rust binding maturity. The recommended default is **libVLC**: it matches the product, and every panel in the designs maps to an existing libVLC module. *(Done in `docs/adr/0001-playback-backend.md`: libVLC 3.0.x chosen. libVLC 4 is still unreleased, libmpv's render API offers only OpenGL and software, and FFmpeg alone means writing a player.)*
- [ ] S1.1.2 Prototype the video presentation paths and measure each: *(In progress. `examples/video_spike` prototypes (a) and (b) with a GPUI overlay and per-run measurements. On an M1, (b) plays 4K60 HEVC 10-bit with 0.00% dropped and (a) with 0.45% at twice the CPU. (c) is rejected for the overlay airspace problem; (d) is estimated only. Screenshot checks on macOS: odd-size edges and deinterlacing are correct on both paths, and HD colours are correct on the surface path after a GPUI fix (our Zed fork now honors each buffer's YCbCr matrix and range). The CPU path is correct too, because it now converts NV12 to BGRA itself. Windows and Linux are not measured yet.)*
  - (a) CPU frames (libVLC `video_set_callbacks`/`format_callbacks`) → GPUI `RenderImage` each frame. Works everywhere; measure CPU, dropped frames and latency at 1080p60, 4K30 and 4K60.
  - (b) macOS: `CVPixelBuffer` → `gpui::surface()` (zero copy).
  - (c) A native child window (HWND, NSView, X11 window) given to the backend, with GPUI drawing overlays in a transparent child or owned popup window. Check the airspace problem (GPUI can't draw over a foreign child window), transparency and input routing.
  - (d) GPU texture sharing (libVLC 4 D3D11/OpenGL output callbacks or libmpv's render API) into a custom GPUI primitive. This likely needs a GPUI fork or upstream patch; estimate the maintenance cost.
- [x] S1.1.3 Test files: 8K, 4K60 HEVC 10-bit, HDR10, HLG, Dolby Vision profile 5 (wrong colors without the right handling), VFR phone video with rotation metadata, anamorphic DVD (720×480 displayed at 16:9), interlaced 1080i, odd dimensions (1919×1079), 240 fps, audio-only, a single-frame image. *(`examples/video_spike/make_test_media.sh` generates the corpus and `measure.sh` runs it; macOS results are in the ADR. Dolby Vision profile 5 needs a real sample. Rotation fails with hardware decoding in libVLC 3.)*
- [ ] S1.1.4 Decide how the backend runtime ships (libVLC DLLs plus the `plugins/` folder on Windows, the dylib and plugins inside the `.app` on macOS, a system package dependency versus bundling on Linux) and its size cost. *(Partly: the macOS runtime is 82 MB / 32 MB compressed per architecture, plugins must keep their file times, and libVLC is loaded at run time. The Windows and Linux layouts are proposed, not tried.)*
- [ ] S1.1.5 Write the decision record (chosen backend, rendering path per OS, fallbacks, measured numbers, known limits). *(Drafted as **Proposed**; it becomes Accepted once the Windows and Linux numbers are in.)*

**Acceptance criteria**
- The 4K60 sample plays for 60 s with < 1% dropped frames and the GPUI overlay staying responsive (< 16 ms input-to-paint) on the reference machines for each OS.

**Edge cases the spike must cover**
- Video larger than the GPU's maximum texture size (8192 or 16384): downscale before upload.
- Chroma subsampling with odd dimensions: crop or pad correctly with no green edge lines.
- Pixel aspect ratio other than 1:1, and rotation of 90°/180°/270° from metadata.
- HDR content on an SDR display (tone-map) and on an HDR display (pass through if the OS and GPUI allow, otherwise tone-map).
- Remote desktop sessions (RDP, VNC), VMs without a GPU, and software rendering fallback.
- Laptops with two GPUs (Optimus, Apple dual GPU): the window moves between them.
- GPU device lost (a Windows driver reset or update during playback): recreate the renderer without crashing.

---

### S1.2 — Engine service and playback state machine

**Depends on:** S1.1  **Size:** L

**Subtasks**
- [x] S1.2.1 `Player` GPUI entity that owns the backend. Commands: `open`, `play`, `pause`, `toggle`, `stop`, `seek(absolute | relative | fraction)`, `frame_step(±1)`, `set_rate`, `set_volume`, `set_mute`, `select_track`, `set_audio_delay`, `set_subtitle_delay`. *(`src/engine/player.rs`. The backend is `engine::libvlc::LibVlcBackend`; `engine::default_backend` falls back to one that fails every open with `BackendUnavailable` when libVLC can't load. Video goes to libVLC's dummy output until S1.4. Not yet used by any screen.)*
- [x] S1.2.2 States: `Idle → Opening → Buffering ⇄ Playing ⇄ Paused → Ended | Stopped | Error(kind)`. Document the allowed transitions and test every edge in the graph. *(`src/engine/state.rs` documents the graph; a test checks every pair of states against it.)*
- [x] S1.2.3 Events: `StateChanged`, `PositionChanged` (throttled to about 15 Hz for the UI), `DurationChanged`, `SeekableChanged`, `PausableChanged`, `TracksChanged`, `Buffering(%)`, `VideoSizeChanged`, `ChaptersChanged`, `MetadataChanged`, `EndReached`, `Error`. *(`PlayerEvent`. Tracks, chapters and metadata events carry no payload; read them from the player.)*
- [x] S1.2.4 Threading: backend callbacks run on backend threads. Send them through a channel to the GPUI foreground executor, and never call back into libVLC from inside a libVLC event callback (documented deadlock). *(The libVLC callback only forwards; a worker thread owns every libVLC handle and makes every call, so `stop` on a hung stream never blocks the UI. libVLC 3 has no seek-done event: a seek completes at the next time report, or after 300 ms.)*
- [x] S1.2.5 Coalesce commands: rapid seeks (dragging the seek bar, holding an arrow key) keep only the latest target; frame stepping queues up to N steps. *(`src/engine/coalesce.rs`; N is 8.)*
- [x] S1.2.6 Shutdown: stop and release with a timeout (about 2 s), so a hung network stream never blocks quitting. *(`src/engine/shutdown.rs`, run on app quit.)*
- [x] S1.2.7 A mock backend behind the same interface, so the UI and state machine can be tested without media files. *(`src/engine/mock.rs`. The libVLC backend has smoke tests against the S1.1 media, ignored by default; run on macOS only so far.)*

**Edge cases**
- `open` while another open is in progress: cancel the first and ignore its late events (tag every event with an open generation id).
- Seek before the media reports it is seekable: queue it and apply it once seekable, or drop it if the media never becomes seekable (live streams).
- Duration unknown (0 or −1), changing (a file still being downloaded or recorded), or wrong in the container: the UI shows `--:--` and the seek bar is disabled or relative-only.
- Position reported past the duration, or going backwards slightly after a seek: clamp it and make the displayed position monotonic within a seek.
- End of file reached during a seek, and seeking to exactly 100%: produce `Ended` once, never twice.
- Pause during buffering: honor the pause once buffering ends.
- Changing the rate on audio-only media, and at extreme rates (0.25×, 4×+): audio pitch correction on, and video may drop frames.
- Media longer than 24 h: the time format grows to `d:hh:mm:ss` or `hhh:mm:ss`. Media shorter than 1 s, or a single still image.
- Media with zero playable streams, or only a data stream: `Error(NoPlayableStreams)`.
- A partly corrupt file plays up to the corruption: continue past it if the backend can, otherwise `Error(Corrupt)` at that position, and the queue moves on (S3 policy).
- Unsupported codec for one stream (audio plays, video doesn't): keep playing and show a "Video codec XYZ not supported" banner.
- DRM-protected media (FairPlay `.m4v`, Widevine): show a clear "protected content can't be played" error.
- The file is deleted, renamed, or its drive unplugged while playing: the backend reads fail, so stop with `Error(SourceLost)` and don't crash or spin. A network share dropping is the same, with a reconnect attempt.
- The file is locked by another process (Windows sharing violation): show a clear error.
- A zero-byte file, a directory passed as a file, a symlink loop, or a FIFO or device file: reject it before opening.

---

### S1.3 — Opening media: dialog, drag and drop, paste, recent

**Depends on:** S1.2, S0.4  **Size:** M  **Design refs:** main menu (`docs/ui/player/menus/main_menu/menu.png`)

**Subtasks**
- [x] S1.3.1 Open File(s) (Ctrl/Cmd+O) and Open Folder, through `cx.prompt_for_paths`. *(`main_window/opening.rs`. Every way of opening ends in `MainWindow::open`, which resolves the items off the UI thread (`opening::resolve`) and shows "Opening …" with Cancel meanwhile. Folders are scanned 8 levels deep, up to 10,000 files, in natural order, skipping hidden and system files and following a symlinked folder once (`opening/scan.rs`). Until Stage 2 and 3, the main window plays the opened list straight through with the S1.2 `Player`, and a status line says what is playing or why it couldn't.)*
- [x] S1.3.2 Drag and drop onto any window (`on_drop` with `ExternalPaths`), with a drop-target highlight while dragging. *(The window root takes the drop, so it works over menus, toasts and in fullscreen; `ui::drop_target` draws an accent frame over the whole window. Drops onto onboarding are queued and open after the hand-off (D4), with no message yet. Not tried by hand yet.)*
- [x] S1.3.3 Paste a path or URL (Ctrl/Cmd+V) in the player or library to open it. *(`OpenFromClipboard`, bound in the main window; a text input's own paste wins inside it. Copied files, and text with one absolute path, `~/` path or URL per line (`opening/paste.rs`). Not in the macOS menu bar, where Cmd+V would take paste from text fields.)*
- [x] S1.3.4 Playlist files (`.m3u`, `.m3u8`, `.pls`, `.xspf`, `.cue`, `.asx`) expand into the queue (S3) instead of being played as media. *(`opening/playlist/`: nested local playlists expand up to 8 deep, cycles once; a local HLS `.m3u8` plays as a stream; network playlists are left to libVLC. UTF-8, UTF-16 and Latin-1 files, and Windows paths read on macOS/Linux. A CUE sheet opens each audio file, not each track: items have no start and end time until Stage 3. Expands into the main window's list until the queue exists.)*
- [x] S1.3.5 A recent media list (Media → Recent), with a configurable count, "Clear recent", and the privacy toggle (Stage 7). *(`opening/recent.rs`: what the user opened (a file, folder, playlist or URL) in `local.toml`'s `recent_media`, newest first, at most `privacy.recent_media_count` (default 10). Shown in the `…` menu's Open Recent with Clear Recent; nothing is kept or shown while `privacy.save_recently_played` is off. Its settings rows come with S5.5. Not in the macOS menu bar yet: GPUI menu items need a data-carrying action for that.)*
- [x] S1.3.6 Probe by content, not extension: files with no or wrong extensions still open, and extensions are matched case-insensitively (`.MKV`). *(`opening/kind.rs`, `opening/sniff.rs`: a known media, playlist or subtitle extension is trusted, so large folders aren't read file by file; any other file is identified by its first 4 KB (containers, MPEG audio and transport streams, playlists, SRT/VTT/ASS). Picked files nothing recognizes still go to libVLC when nothing else was picked, since it probes far more formats.)*

**Edge cases**
- Dropping video and subtitle files together (`movie.mkv` + `movie.srt`): open the video and attach the subtitle. Dropping only a subtitle file while something is playing adds a subtitle track (S5). Dropping only a subtitle with nothing playing shows "Open a video first". *(Done: `Player::add_subtitle`, libVLC `add_slave`.)*
- Dropping a folder with thousands of files, or nested folders: scan asynchronously, cancellably, with a depth limit, and skip hidden and system files by default.
- Dropping non-media files (exe, pdf, txt) mixed with media: ignore them and show "3 files skipped". *(Done.)*
- Dropping onto the onboarding window: ignore or queue (see the S0.2.6 decision).
- Dropping during fullscreen, while a modal is open, or onto a popover: define which element is the drop target.
- Dragging a URL or link from a browser: GPUI's `ExternalPaths` carries file paths only, so handle text or URL drag payloads if the platform layer supports them, and fall back to paste.
- Shortcuts (`.lnk`), aliases and symlinks resolve to their targets, and broken ones show an error. *(Open: symlinks resolve and broken ones fail with "doesn't exist"; `.lnk` files and macOS aliases aren't resolved yet, as in S0.6.)*
- A file on a slow or sleeping network drive: the dialog returns but opening takes seconds, so show "Opening…" right away and let the user cancel.
- macOS sandbox or iCloud "Optimize Storage" placeholders (files not downloaded yet), and OneDrive Files On-Demand on Windows: opening triggers a download, so show progress or "Downloading from iCloud". *(Open: such files show the generic "Opening …" with Cancel, no download progress.)*
- Mount points that need permission on macOS (Downloads, Desktop and Documents ask for TCC consent the first time): handle a denial gracefully.

---

### S1.4 — Video output surface and fullscreen

**Depends on:** S1.1, S1.2  **Size:** M  **Design refs:** `docs/ui/player/screens/default/*`, `hide_ui/*`

**Subtasks**
- [x] S1.4.1 Video element: fit inside the container keeping the aspect ratio (letterbox and pillarbox are always black, whatever the theme), correct on HiDPI, honors rotation. *(`src/video/`. ADR 0001's paths: libVLC `vmem` (`engine/libvlc/vmem.rs`) writes NV12 into IOSurface `CVPixelBuffer`s on macOS and BGRA `RenderImage`s elsewhere (`RVLC_VIDEO_PATH=bgra` forces BGRA on macOS). A one-frame mailbox wakes the `VideoView`, which paints the picture with a canvas at the display aspect, edges snapped to device pixels (`video/layout.rs`). Local files are parsed before play (`libvlc/prepare.rs`), so libVLC crops coded padding (1920×1088 → 1080) and squares anamorphic pixels itself; a rotated or flipped track turns hardware decoding off so libVLC 3 can rotate it. MPEG-PS/TS reveal their shape only after decoding starts, so the worker re-reads it every 100 ms for up to 3 s (`libvlc/size_watch.rs`). Audio-only media shows a ♪ placeholder. Checked on macOS with the S1.1 corpus; colours still have ADR limit 4.)*
- [x] S1.4.2 Fullscreen: toggle with F and double-click on the video. Escape exits fullscreen first, before any other Escape meaning. Restore the previous window bounds on exit. *(`main_window/fullscreen.rs`. Keymap actions run before key listeners, so Escape leaves fullscreen before a menu or popover sees it; windowed, it propagates. A held F or Escape acts once. **Exception:** while a modal dialog has focus, player keys are blocked (S0.5.2), so Escape closes the dialog first. macOS uses native fullscreen (own Space); the OS restores the bounds and the saved geometry keeps the windowed ones. The title bar hides in fullscreen.)*
- [x] S1.4.3 Keep the display awake while video is playing (Windows `SetThreadExecutionState`, macOS `IOPMAssertion`, Linux `org.freedesktop.ScreenSaver.Inhibit` / systemd-inhibit). Release on pause, stop, minimize and audio-only playback. *(`src/power.rs`; held while playing or buffering video in a visible window. Linux holds the D-Bus inhibition (zbus) or a `systemd-inhibit` child on a thread. Seen in `pmset -g assertions` on macOS; Windows and Linux not compiled or run yet.)*
- [x] S1.4.4 Stop presenting frames while the window is minimized or fully hidden; audio keeps playing. *(GPUI's window visibility: while hidden the mailbox refuses frames, so nothing is uploaded or drawn (libVLC still decodes, for A/V sync), and the view redraws as soon as it shows. Not tried by hand yet.)*

**Edge cases**
- The window moves between monitors with different DPI or refresh rates (60 to 144 Hz): rescale and re-time presentation.
- The fullscreen monitor is disconnected: leave fullscreen and move the window to an existing display.
- macOS notch in fullscreen: video either avoids it or goes under it per the setting, with controls never under it.
- macOS native fullscreen (separate Space) versus borderless fullscreen: pick one, and make Mission Control and Cmd+Tab behave.
- Windows: an auto-hiding taskbar must still be reachable in fullscreen; on multi-monitor setups, fullscreen applies only to the window's current monitor.
- The display sleeps or the machine suspends during playback: on resume, pause (or keep paused) and re-initialize video output if the device was lost.
- Audio-only media in the video view: show artwork or a placeholder (music mode is Stage 8) and allow screen sleep.

---

### S1.5 — Audio output, volume and devices

**Depends on:** S1.2  **Size:** M  **Design refs:** `docs/ui/player/controls/volume/*`, `docs/icons/volume/*`

**Subtasks**
- [x] S1.5.1 Volume from 0 to the **Maximum volume** setting (100–300%, default 125% as VLC's slider (changed from 200% in S2.5), per Settings → Audio), mute toggle, volume step from settings. *(`engine/player/audio.rs`: `set_volume` caps at `max_volume`, lowering the maximum brings a louder volume down at once, `step_volume`, `toggle_mute`, and `PlayerEvent::VolumeChanged`. Settings in `settings/schema/audio.rs` (`prefs.audio`), applied by `main_window/audio.rs`, which follows changes live. Up/Down step by `volume_step_percent` (default 5%) once something was opened. Changing the volume unmutes, as in VLC. libVLC takes up to 300%, checked on macOS.)*
- [x] S1.5.2 **Startup volume** as designed: "Remember last volume" or "Always starts at N%". Recommendation: even in "Remember last volume" mode, restore at most 100% on launch, so a 200% boost doesn't blast the next session's first video. *(`prefs.audio.startup_volume` (`remember_last` or `fixed`, with `startup_volume_percent`); the last volume is kept in `local.toml` (`audio.last_volume_percent`) and restored at most 100% (D21); either is capped at the maximum.)*
- [x] S1.5.3 Choose the output device, or "System default", which follows OS default changes live. *(`Player::set_audio_device(OutputDevice)`, `audio_devices()`; the choice is `local.audio.output_device` (empty: system default). On macOS libVLC's "System Sound Output Device" (id `0`) follows the OS default; it is never listed as a device. Windows and Linux pass the empty id, not tried yet. No picker until S5.8.)*
- [x] S1.5.4 Hot-plug: when the active device disappears (Bluetooth headphones off, USB DAC unplugged), switch to the system default. Optionally pause on headphone disconnect (a setting, on by default on laptops). *(libVLC 3 has no hot-plug event, so the worker reads the device list every 2 s (`libvlc/audio.rs`). `engine/audio/hotplug.rs`: a missing chosen device plays through the default, and is used again when it comes back. `prefs.audio.pause_on_device_disconnect` is on by default everywhere, not only on laptops. Through the system default the backend can't tell which device the OS used, so any device disappearing while playing counts. A toast says which device went. Not tried by unplugging yet.)*
- [x] S1.5.5 Downmix multichannel (5.1/7.1) to stereo when the device is stereo ("Speaker setup: Automatic" in Settings → Audio); passthrough (AC3/DTS over HDMI or S/PDIF) is an advanced setting. *(`SpeakerSetup::Automatic` leaves the layout to libVLC's output, which mixes for the device; `Stereo` forces it with `libvlc_audio_set_channel`, re-applied to each media. `prefs.audio.passthrough` starts libVLC with `--spdif` (plus `--mmdevice-passthrough=1` on Windows) and applies to the next player; libVLC refuses unknown options, so it retries without them. Not checked with a 5.1 file or a receiver.)*
- [x] S1.5.6 "Pause when another app uses audio" (Settings → Playback): implement where the OS can report it (see the S5.7 edge cases). *(`prefs.player.pause_when_another_app_uses_audio`, on by default: libVLC's Corked event (PulseAudio cork requests, Windows communications ducking) pauses, and Uncorked resumes unless the user paused or played meanwhile. macOS reports nothing, so it never triggers there.)*

**Edge cases**
- No audio device at all (VMs, headless, or every device disabled): video plays silently with a "No audio device" indicator, never an error loop. *(A toast once per absence; `Player::has_no_audio_device` for S2.5's slider.)*
- Exclusive mode held by another app (Windows WASAPI exclusive): fall back to another device or play silently, with a notice.
- Sample rate or channel layout changes mid-stream (TV recordings): no crash or pop, re-open the output.
- Bluetooth latency of 150–300 ms causes visible lip-sync error: expose audio delay (S6) and, if the backend reports latency, compensate automatically.
- Volume above 100% clips: show the boost range in a different color as the design does, and consider a limiter.
- The OS mixer volume differs from the in-app volume: never change the OS volume.
- Volume changes while muted: unmute (VLC behavior) or keep muted; pick one and match the volume popover design. *(Unmutes.)*

---

### S1.6 — Track model (audio, video, subtitles)

**Depends on:** S1.2  **Size:** S  **Design refs:** onboarding shortcuts (V and B), `docs/ui/player/panels/subtitles/*`

**Subtasks**
- [x] S1.6.1 Enumerate tracks with id, kind, language, title, codec, and default/forced flags. Build display names like "English (SDH) — AC3 5.1" when the container provides no title. *(`engine/tracks.rs`: `Track` and `Tracks`, with `Tracks::label` ("English — AC3 5.1", "Commentary — AAC Stereo", "Track 2") from `tracks/label.rs`; a title that already names the codec stands alone. Languages are normalized (`en` = `eng` = `en-US`, `engine/language.rs`) and named in English; codecs get short names from libVLC's FourCCs (`tracks/codec.rs`). libVLC 3 reports neither flag nor a hearing-impaired one, so there is no "(SDH)": `libvlc/track_list.rs` marks a track forced when its title says "forced", and the default of each kind is what libVLC plays by itself once playback starts. Checked against `tracks.mkv`, added to the S1.1 corpus.)*
- [x] S1.6.2 Select a track, disable subtitles, and cycle audio (B) and subtitles (V) with an on-screen display (OSD) toast showing the new track. *(`Player::select_track` (shown at once, confirmed by the backend later) and `Player::cycle_track`; subtitles cycle through Off, audio doesn't turn off. Until the OSD exists (Stage 2), `main_window/tracks.rs` says "Audio: English — AC3 5.1", "Subtitles: Off" or "No subtitles" in a toast that replaces the previous one. Tried by hand on macOS with `tracks.mkv`; that found the player keys reaching nothing once the video replaced the focused Open button, so starting playback now focuses the window. libVLC 3 can't blend subtitles onto hardware-decoded frames ("blending YUVA to CVPN failed"), so once a subtitle has shown for 300 ms over hardware-decoded video, the worker reopens the media in software decoding at the same position, paused if it was, with the same tracks and added subtitle files, hiding the reopen from the player (`libvlc/decoding.rs`, ADR 0001 limit 10). It stays in software for the rest of that media. Checked on macOS, playing and paused; Windows and Linux hardware decoding not tried.)*
- [x] S1.6.3 Auto-select by preferred languages (Stage 7 settings), with forced subtitles shown when the audio language matches. *(`tracks/auto.rs`, run by the player on every track change until the user picks that kind: audio in the first preferred language present (the default track within it); subtitles in a preferred language, else the media's default, else a forced track in the audio's language. "Enable subtitles" off leaves only that forced track. Settings: `prefs.audio.preferred_languages`, `prefs.subtitles.enabled` and `prefs.subtitles.preferred_languages`; their rows come with S5.8 and S7.6. An added subtitle file counts as the user's pick.)*

**Edge cases**
- Tracks appear or disappear mid-playback (MPEG-TS and HLS add tracks late): update menus live and keep the user's choice when the track ids are stable. *(Done in the engine: every change is a `TracksChanged`, ids are libVLC's ES ids, and a kind the user picked is never re-picked.)*
- Tracks with no language tag or "und": label them "Track N". *(Done.)*
- Many subtitle tracks (50+ on some anime and Blu-ray rips): the menu scrolls and groups by language.
- Cycling subtitles includes an "Off" position. *(Done.)*
- Several video tracks (multi-angle): the video track menu appears only when there's more than one.

---

### S1.7 — Media identity, playback history and resume

**Why:** the library cards distinguish **watched** and **unwatched** and show progress (`docs/ui/library/media_cards/*`). The player needs resume. Both need a stable identity for "the same file".
**Depends on:** S1.2  **Size:** M

**Subtasks**
- [x] S1.7.1 Local database (SQLite through `rusqlite`) in the data dir, with migrations. It holds the media table (identity, path, size, mtime, duration, last position, watched flag, play count, last played) and the later library tables (S4). *(`src/db.rs`: `library.db` next to `local.toml`, WAL, `synchronous=NORMAL`, 2 s busy timeout. `db/migrations.rs` numbers migrations with `PRAGMA user_version`, one transaction each; S4 appends its tables there. `db/media.rs` is the `media` table (`location` holds the path or URL). It is only used from the history thread (`history/worker.rs`), so the UI never waits on disk.)*
- [x] S1.7.2 Identity is a fast content hash (for example the first and last 64 KB plus the size) combined with the path, so a file that's moved or renamed keeps its history and two copies on different paths are recognized as the same media. *(`history/identity.rs`: xxh3-128 of the size, first 64 KB and last 64 KB. `history/resolve.rs`: a path whose size and mtime still match skips hashing; otherwise the hash finds the row and moves it to the new path. A URL is its own identity.)*
- [x] S1.7.3 Save the position every few seconds and on pause, stop, close, quit and crash-safe intervals. *(`main_window/history.rs`: every 5 s while playing, and on pause, stop, error and end. A `history::Session` saves its last position when dropped (window closed, next media), and quitting waits for the history thread to finish writing. Checked on macOS by killing the app with `kill -9` 14 s in: the position from 4 s earlier was kept.)*
- [x] S1.7.4 Resume policy setting: Always, Ask (show a "Resume from 12:34?" toast with a button), or Never. Mark media watched at ≥ 95% or within the last 60 s, whichever comes first, configurable. *(`prefs.player.resume` (`always`, `ask` (default), `never`), `watched_percent` and `watched_remaining_seconds`; rows come with S5.7. Rules in `history/policy.rs`, shared with S4.1.2. Media under 2 minutes uses the percentage only, or a 50 s clip would be watched as it starts. Nothing is saved in the first 10 s of a playback, so opening media and closing it while the toast shows keeps the old point. Once watched, media starts from the beginning. Always and Ask (toast shown) checked on macOS; the toast's Resume button not clicked by hand yet.)*
- [x] S1.7.5 "Mark as watched/unwatched" and "Clear history" commands (the library context menus use them). *(Actions `MarkWatched`, `MarkUnwatched` and `ClearHistory` (no default keys, not in a menu until the library), handled by the main window for the current media; `History::set_watched` and `History::clear` for the library. Marking stops recording the current playback so its position doesn't undo the mark. Clear History also clears Open Recent.)*

**Edge cases**
- Media shorter than 2 minutes: don't store resume positions, just count plays. *(Done.)*
- A resume position in the last 10 s, or past the new end of a file that was re-encoded or trimmed: start from the beginning. *(Done, and also in the first 10 s.)*
- The same file modified in place (size or mtime changed): treat it as new media and keep the old history orphaned for 30 days. *(Done: the hash decides, so a file whose mtime changed but whose head, tail and size didn't (`touch`, restored from backup) stays the same media. Orphans are purged when the history thread starts, and come back if the file returns to its old content.)*
- Live streams and network URLs without a duration: no resume, but keep them in history. *(Done: plays are counted.)*
- Hard links and the same file reachable through two mount points: the hash identity treats them as one. *(Done by the hash.)*
- The database is locked by another instance (shouldn't happen with S0.6), or corrupt: recover with `PRAGMA integrity_check`, back up and recreate it. Losing history is acceptable, crashing is not. *(Done: a damaged file goes to `library.db.corrupt` and a new one starts. A locked or unreadable file, or one from a newer build, is left alone and history is kept in memory for that run.)*
- The database is on a slow or network home directory: use WAL mode and keep writes off the UI thread. *(Done.)*
- Privacy setting "Don't save history": no positions or recents are written, and switching it on offers to clear existing data. *(Done with `privacy.save_recently_played` off; switching it off shows a toast with Clear History. `privacy.remember_playback_position` (S5.5.1) off keeps plays and watched but no positions.)*

---

## Stage 2 — Player screen MVP

> **Goal:** open a file and watch it with the designed player: title bar, control bar, seek with preview, volume, track and settings popovers, menus, auto-hide and fullscreen. Every shortcut onboarding teaches works.
> **Exit criteria:** `docs/ui/player/screens/default/player_01.png`, `hide_ui/player_hide_ui.png` and `light/player_light.png` (minus the sidebar) are reproduced pixel-close with real playback; all 12 onboarding shortcuts work; the control bar is fully keyboard-operable.

Design facts the stories rely on (`docs/ui/player/**`):
- **Title bar:** VLC logo · `…` (main menu) · sidebar toggle · divider · folder chip (current folder) · centered title · Minimize, Maximize, **Fullscreen**, Close.
- **Video viewport:** rounded corners, inset about 10 px.
- **Control bar (54 px):** Previous · Play/Pause (▶ on an orange pill when paused, ❚❚ on a grey pill when playing) · Next · current time `00:02:10` · seek bar (dark track, dark-brown fill, orange 8 px handle) · duration `00:25:40` · speaker and the **wedge volume slider** · Audio · Subtitles · Settings (gear with ▶) · List.
- **Popovers and panels:** 400 px wide, anchored bottom-right 20 px above the bar, about 95% opaque, with tabs (icon + label, orange underline when active). The button that opened a popover is highlighted orange while it's open.

### S2.1 — Player layout and title bar

**Depends on:** S0.3, S0.4, S1.4  **Size:** M  **Design refs:** `player_01.png`, `player_light.png`, `window_chrome.rs`

**Subtasks**
- [x] S2.1.1 Extend `window_chrome::title_bar` with the player layout: logo, `…`, sidebar toggle (active and inactive), divider, folder chip, centered title, and Minimize/Maximize/Fullscreen/Close. Keep native hit-testing (`WindowControlArea`) on Windows and the native traffic lights on macOS. *(`main_window/title_bar.rs`. The sidebar toggle shows its inactive state, disabled, until the library sidebar exists (Stage 4); `IconButton::selected` is its active state.)*
- [x] S2.1.2 Title text rule: the media title (from metadata, else the file name with the extension) while media is loaded, otherwise "VLC Media Player". It respects Settings → Interface "Show the file name in the title bar". **The mocks disagree (`player_01` shows "VLC Media Player" while playing); confirm.** *(Went with D8's recommendation, which matches `player_light.png` and the setting's default (on, `interface_01.png`). `title_bar/text.rs`; `prefs.interface.show_file_name_in_title_bar`, its row comes with S5.4. "Loaded" is Opening through Paused.)*
- [x] S2.1.3 Folder chip: shows the containing folder's name, and clicking it opens the library at that folder (S4.10). Hidden for URLs and discs. *(With nothing loaded it shows the first media folder. Until the library exists, clicking opens the folder in the system file manager; S4.10 replaces that. The path is in its tooltip.)*
- [x] S2.1.4 Video viewport inset with rounded corners, and letterboxing inside the rounded rectangle. *(`main_window/viewport.rs`: 10 px from the sides and bottom, 6 px radius. Frames are images or macOS surfaces that GPUI's content masks don't clip, so the corners are painted over in the frame colour. Fullscreen is edge to edge. Checked on macOS.)*
- [x] S2.1.5 The OS window title (taskbar, Alt+Tab, Mission Control) is set through `window.set_window_title`. *(Set from the same rule as the title bar, only when it changes, so the title tag, the setting and Stop all reach it.)*

**Edge cases**
- Long titles: truncate with an ellipsis in the middle region and never overlap the left or right groups. A full-width title on a narrow window collapses the folder chip first. *(Done: `title_bar/fit.rs` measures the title and chip.)*
- Titles that contain right-to-left text, emoji, newlines or control characters (from bad tags): strip control characters and handle bidi. *(Done: control characters become spaces and explicit bidi formatting characters are dropped; GPUI's shaping orders right-to-left text.)*
- macOS: the traffic lights sit where the logo is in the design, so the left group shifts right by the traffic-light inset. *(Done.)*
- Linux: client-side decorations on Wayland (GPUI draws them), and server-side decorations on X11 with some window managers produce a double title bar. Respect Settings → Interface "Show window decorations". *(Not done: deferred to S5.4.1, which adds that setting; not tried on Linux.)*
- Windows 11 snap layouts appear when hovering Maximize; keep the `WindowControlArea::Max` hit-test.
- Double-clicking the title bar maximizes; double-clicking the video toggles fullscreen. These are separate hit regions.

---

### S2.2 — Control bar, Play/Pause and transport buttons

**Depends on:** S2.1, S1.2, S0.5  **Size:** M  **Design refs:** `controls-default.png`, `play_button/*`

**Subtasks**
- [x] S2.2.1 Lay out the bar from a **declarative item list** (it's needed for Customize Player in S8.7): the order, groups and spacers come from data, not hard-coded layout. *(`main_window/control_bar/layout.rs`: a list of item and gap slots; gaps split the groups. The time readouts, seek bar and volume level show the player's state but aren't interactive until S2.3–S2.5.)*
- [x] S2.2.2 Play/Pause: orange ▶ when paused or stopped, grey ❚❚ while playing, and a buffering spinner state (not designed; add one). *(`control_bar/play_button.rs`. The spinner turns on the grey pill while opening or buffering; pressing it pauses. The pill follows `Player::wants_to_play`, so it changes on the click.)*
- [x] S2.2.3 Previous and Next: previous restarts the current item if it's more than 3 s in, otherwise goes to the previous item. Both are disabled at the queue's ends unless Repeat is All. *(`main_window/transport.rs` and `transport/rules.rs`; P and N do the same. Previous stays enabled on the first item while it would restart it. Repeat All (S3.3) wraps them.)*
- [x] S2.2.4 Hover, pressed, focused and disabled states for every icon button, with a tooltip showing the action and its shortcut. *(The kit's `IconButton` and the pill; names and shortcuts come from the command table and keymap.)*
- [x] S2.2.5 Narrow windows: items drop in a defined priority order (for example Settings and List go into an overflow `…`) instead of overlapping. *(`control_bar/fit.rs`: Settings, List, Subtitles, Audio, Volume (as Mute), Duration, Previous/Next, the current time, then the seek bar. Buttons move into the `…` menu; readouts are hidden. Play/Pause always stays.)*

**Edge cases**
- Rapid clicks on Play/Pause (double-click speed) produce exactly the matching number of toggles, debounced against the engine state, and the icon never desyncs from the real state. *(Done: each click flips the player's requested state, not its reported one, and clicks skip the held-key gate.)*
- Pressing Play after `Ended` replays from the start (or plays the next queue item when Repeat is on). *(Replays, or goes on as the item's end would have under Repeat or after Stop after each item (S3.3). Play after an error tries the item again.)*
- Pressing Play with an empty queue opens the Open File dialog, or does nothing with a tooltip (decide). *(Decided: it opens the Open File dialog, unless an open is still resolving.)*
- The light theme mock shows Previous, Next and the gear faded. Decide whether that means "disabled" (no queue) or is just styling, and match it consistently. *(Decided: disabled. Previous and Next fade where they'd do nothing; the gear and List stay disabled until their panels exist (S2.7, Stage 3). S2.7 enabled the gear.)*

---

### S2.3 — Seek bar with hover thumbnail preview

**Depends on:** S2.2, S1.2, S4.4 (thumbnail extraction can start in S1)  **Size:** L  **Design refs:** `controls-default.png`, `seek_thumbnail/thumbnail.png`

**Subtasks**
- [x] S2.3.1 Track, fill and handle. Click to jump, drag to scrub with live preview frames while dragging (throttled), and release to commit. *(`main_window/seek_bar.rs`: pressing moves the handle there, a drag moves the handle and the hover card only, and the player seeks once on release. Preview frames during a drag come from the preview decoder (S2.3.3), never the main player.)*
- [x] S2.3.2 Hover card: a 16:9 preview frame and the time under it (`00:02:10`), following the cursor and clamped inside the window. *(`seek_bar/card.rs`: the picture is letterboxed by the player's video aspect; a grey frame shows until it arrives or when none can be made. Audio and network streams show a card with the time alone. Checked on macOS.)*
- [x] S2.3.3 Preview frames come from a **second, muted decoder instance** or a pre-generated thumbnail strip (trickplay). Never seek the main player to build the preview. *(A second libVLC instance (`engine/libvlc/preview.rs`): no audio or subtitles, software decoding, BGRA at most 480×270, on its own thread, created off the UI thread on the first hover over local video. `engine/preview/flow.rs` runs one request at a time and keeps only the newest waiting one; a picture counts only after the decoder paused and then reported a new time, because a paused libVLC shows its last picture again. Times are rounded to 1/400 of the length.)*
- [x] S2.3.4 Keyboard: Left/Right seek by the "Short skip" amount (S5.6); modifiers select the very short, medium and long amounts; Home and End go to the start and end. *(`main_window/seeking.rs`. Shift: very short (3 s); alone: short (10 s); Alt/Option: medium (60 s); Ctrl/Cmd: long (300 s). Amounts in `prefs.controls.skip` (`very_short`, `short`, `medium`, `long`, seconds); rows come with S5.6. Navigation keys with modifiers no longer fire while a control has focus, so Alt+Left still moves a text caret.)*
- [x] S2.3.5 A buffered-range indicator for network streams (not designed; propose a subtle lighter band). *(A slightly lighter band ahead of the handle. libVLC 3 reports only how full its input cache is, not downloaded ranges, so the band covers that share of the 1 s network cache. Not checked against a real stream.)*
- [x] S2.3.6 Accessible as a slider: it announces the current time and the duration. *(Role slider, "Seek", value in seconds and "00:02:10 of 00:25:40"; a tab stop with a focus ring. Not checked with VoiceOver.)*

**Edge cases**
- Unknown or zero duration: hide the handle and disable seeking, and show live-stream styling (a "LIVE" chip; not designed). *(Done.)*
- Non-seekable media (some HTTP streams): clicks do nothing and the cursor shows "not allowed". *(Done.)*
- Seeking in a file without an index (AVI without an idx1 chunk, broken MKV cues): seeks are slow or inaccurate, so keep the UI responsive and show the position the engine actually reached.
- The hover preview for audio-only media shows artwork or just the time. For DRM or corrupt regions, show a grey frame.
- The preview decoder is slow (a 4K HEVC file on a weak CPU): show the time immediately and the frame when it's ready, and drop stale requests. *(Done; a picture taking over 3 s is given up.)*
- Very long media (10 h+): pixel resolution limits precision, so use fine-scrub (hold Shift while dragging for 1/10 speed). *(Done.)*
- Very short media (< 1 s): the whole bar maps to a few frames, with no division by zero. *(Done.)*
- Dragging the handle outside the window: keep the drag captured and clamp. *(Done; checked on macOS.)*
- A touch screen or pen on Windows: taps and drags work like mouse input. Hover isn't available, so show the preview while dragging.

---

### S2.4 — Current time (editable) and duration (total or remaining)

**Depends on:** S2.2  **Size:** S  **Design refs:** `current_time/*`, `duration/*`

**Subtasks**
- [x] S2.4.1 Current time: hover shows a dark pill; click turns it into an editable field (bright digits, fixed colons, orange caret). Enter jumps, Escape or blur cancels. *(`control_bar/readouts.rs`, with the kit's `TextInput` in its new `Look::Bare`. The field opens with the time selected, so typing replaces it; the colons are typed, not a mask, because S2.4.2 accepts forms without them. Media that can't seek gets no pill and no field. Not checked on screen yet.)*
- [x] S2.4.2 Accepted input: `hh:mm:ss`, `mm:ss`, plain seconds, `1h2m3s`, `+30`/`-30` (relative), and a percentage (`50%`). Show invalid input with an error tint and don't jump. *(`control_bar/time_entry.rs`. `+`/`-` take any of the other forms (`-1:00`, `+2m`); seconds may have a fraction. Invalid text turns red on a red-tinted pill until the next edit.)*
- [x] S2.4.3 Duration: click toggles between total and remaining (`-00:23:30`); the choice persists globally. Remaining time counts down live. *(`prefs.interface.duration_readout`, `total` or `remaining`. Whole seconds of both readouts add up to the length.)*
- [x] S2.4.4 Format: always `hh:mm:ss` as designed (`00:02:10`), growing to `hhh:mm:ss` beyond 99 h. Use tabular (monospaced) digits so the bar doesn't jitter. *(`clock.rs`; the `tnum` font feature, and the readouts are a fixed 72 px wide.)*

**Edge cases**
- Typing a time past the duration: clamp to the end (or reject; decide) and never seek past the end. *(Decided: clamp; the player already clamps seeks to the length. Percentages over 100 % are the end too.)*
- Typing a negative time: clamp to 0. *(Done: `-` is a jump back, which stops at the start.)*
- While editing, playback keeps advancing: freeze the field's text (don't overwrite what the user is typing). *(Done; `+`/`-` jump from where playback is on Enter.)*
- Remaining time when the duration is unknown: show `--:--:--`, and the toggle is disabled. *(Done.)*
- At playback rates other than 1×, remaining time can show wall-clock remaining or media remaining (decide; VLC shows media time). *(Decided: media time.)*
- Locales with non-Latin digits: keep ASCII digits for time codes. *(Done: shown and accepted digits are ASCII only.)*

---

### S2.5 — Volume control (wedge slider, mute, boost)

**Depends on:** S1.5, S0.4.5  **Size:** M  **Design refs:** `volume/volume-*.png`, `volume/audio_value.png`, `docs/icons/volume/*`, `docs/icons/media_controls/volume/*`

**Subtasks**
- [x] S2.5.1 The wedge shape (narrow on the left, tall on the right, rounded left end) with a **position-locked gradient** (green `#78A613` → olive → amber `#9A7513` at 100% → orange → red `#C93129` at 200%). *(`main_window/volume_slider/wedge.rs`: 89×27.5 px, 10.5 px at the left end with 3 px corners, the fill inset 1.5 px; painted as paths, one two-stop gradient per stretch between stops, so colours belong to levels. A screenshot test compares 0/50/100/150/200% with the designs.)*
- [x] S2.5.2 **Non-linear mapping** measured from the designs: 0–100% fills about 71% of the width and 100–200% the remaining 29%. Implement it as a piecewise-linear function with the breakpoint at 100%, so it extends cleanly when the maximum volume is 100% or 300% (Settings → Audio "Maximum volume"). *(`volume_slider/scale.rs`: each 100% of boost is 29/71 as long as 0–100%, so 100% sits at 71% of a 200% track, 91% of the default 125% track and 55% of a 300% one.)*
- [x] S2.5.3 Value tooltip ("100 %") while dragging, scrolling or using the keyboard. *(The `audio_value.png` bubble over the wedge, while dragging and for 1 s after a key or the wheel.)*
- [x] S2.5.4 Clicking the speaker toggles mute. The speaker icon changes with the level (none, low, medium, high, from `docs/icons/media_controls/volume/*`); the muted look isn't designed, so design it. *(None at 0%, low below 34%, medium below 67%, high above. Muted: the speaker struck through (`volume-muted.svg`) and the fill at a third of its opacity, so the level unmuting restores stays visible.)*
- [x] S2.5.5 Scroll wheel over the slider or the video (Settings → Controls mouse mapping), and Up/Down keys, in "Volume step" increments. *(`prefs.controls.wheel`, `volume` or `nothing`, for the video; the wedge always takes the wheel. A wheel notch is one step; trackpad scrolling adds up 40 px per step, at most one step per event. With focus the wedge also takes Left/Right, Home (0%) and End (100%, not the boost).)*
- [x] S2.5.6 Snap or detent at 100% while dragging (a soft magnet of about ±3%), so users don't drift into the boost range by accident. *(Drags set whole percents and hold at 100% from 97% to 103%.)*
- [x] S2.5.7 An OSD toast on the video when the volume changes by keyboard or scroll while the bar is hidden. *(`main_window/osd.rs`: "Volume 80 %" at the video's top right for 1.5 s, whenever the control bar is hidden (S2.8).)*

**Edge cases**
- Maximum volume lowered below the current level (S5.8): clamp immediately.
- Trackpad inertial scrolling sends dozens of events: accumulate deltas into steps (S5.6 edge case). *(Done.)*
- Changing the volume while muted: unmute, matching VLC behavior (decide and document). *(Decided: it unmutes.)*
- Hardware volume keys change the **OS** volume, not the app's; don't intercept them.
- Volume at 0% isn't the same as muted: unmuting restores the previous level, and dragging up from 0 doesn't change the mute flag. *(Done.)*
- No audio device (S1.5): the slider is disabled and its tooltip explains why. *(Done.)*

---

### S2.6 — Track popover (Video | Audio | Subtitles)

**Depends on:** S1.6, S1.5, S0.4.6  **Size:** M  **Design refs:** `controls-video.png`, `controls-audio.png`, `controls-subtitles.png`

**Subtasks**
- [x] S2.6.1 One popover with three tabs. The Audio button opens the Audio tab and the Subtitles button opens the Subtitles tab. **No bar button opens the Video tab in the design (nothing is highlighted in `controls-video.png`); decide on the entry point** (the recommendation: the tab is reachable from inside the popover and appears only when there are 2+ video tracks). *(`main_window/track_popover.rs`. Decided as recommended (D22). 400×524 px, 20 px from the window's right edge and above the bar, shorter in a short window. The buttons show as on while their tab is open; from the overflow `…` menu the entries open the same popover. B and V still cycle tracks. Not checked on screen yet.)*
- [x] S2.6.2 **Embedded Tracks:** "None" plus each track, with a check on the selected one. *(`track_popover/sections.rs`. Names are the S1.6 labels ("English — AC3 5.1"). Picking one counts as the user's pick, so auto-selection leaves that kind alone; the popover stays open. Subtitle files the user added are listed under External Tracks instead.)*
- [x] S2.6.3 Audio tab, **Output Device:** device list with a check, and a refresh icon to re-enumerate. *("System default" first, then the devices; the check is on the device sound goes to. Picking one sets `local.audio.output_device`. Refresh asks the backend to list devices now (`Backend::refresh_audio_devices`) instead of at the next poll.)*
- [x] S2.6.4 Subtitles tab, **External Tracks:** loaded subtitle files with a pencil (edit: opens the Add Subtitles dialog, S6.4) and "+" (add from a file). *(`Player::subtitle_files`: each file added to the media (a drop, "+") and the track it became; libVLC lists an added file after the media's own streams, so a file takes the last new subtitle track. "+" asks for files and adds the subtitle ones, naming the first that isn't. The pencil stays disabled until the Add Subtitles dialog exists (S6.4).)*
- [x] S2.6.5 The **Show Preferences** footer opens the matching tab of the settings panel (S2.7). **Write down the difference from "Open Advanced Settings"** (proposal: Show Preferences opens the in-player panel; Open Advanced Settings opens the Settings window category). *(Decided as proposed: Show Preferences opens the in-player panel, which changes the playing media; Open Advanced Settings opens the Settings window's category, which holds the defaults. Show Preferences was disabled until S2.7 added the panel.)*
- [x] S2.6.6 Close on outside click, Escape, clicking the same button again, or opening another popover. Only one popover is open at a time. *(The other button switches tabs. Opening either `…` menu closes the popover and the other way round. It also closes when the window loses focus (so the "+" file dialog closes it), when the media stops, and in fullscreen, which hides the bar until S2.8.)*

**Edge cases**
- The track list changes while the popover is open (S1.6 late tracks): update in place and keep the scroll position. *(Done: drawn from the player every frame; each tab scrolls on its own.)*
- The selected output device disappears while the popover is open: move the check to the system default and show a note. *(Done: "The chosen device isn't connected. Sound plays through the system default.")*
- 50+ tracks: the list scrolls inside the popover with a maximum height that fits the window. *(Done.)*
- A small window height: the popover never extends above the title bar; it becomes scrollable instead. *(Done.)*
- Fullscreen with auto-hide: an open popover keeps the controls visible until it closes. *(Done in S2.8.)*

---

### S2.7 — Settings panel shell and the Playback tab

**Depends on:** S2.6  **Size:** M  **Design refs:** `controls-playback.png`, and the Video, Audio and Subtitle settings screenshots (their content lands in Stages 6 and 7)

**Subtasks**
- [x] S2.7.1 The panel opens from the gear button with tabs **Playback | Video | Audio | Subtitles**. It's taller than the track popover (945 px in the design), scrolls, and has a sticky footer on the Video, Audio and Subtitles tabs (**Default Preset ^** and **⚙ Open Advanced Settings**). *(`main_window/settings_panel.rs`; placement and surface shared with the track popover in `bar_panel.rs`. It closes like the popover, and only one of the two is open at a time; the popover's Show Preferences now opens the matching tab. Video, Audio and Subtitles say "Nothing to adjust here yet." until Stages 6 and 7. Default Preset waits for presets (S5.11) and Open Advanced Settings for the Settings window (S5.2), both disabled. Not checked on screen yet.)*
- [x] S2.7.2 Playback tab: **Speed**, a text field ("1x") plus a slider from 0.25 to 2 with ticks every 0.25 and labels. The tab also has these rows, implemented in later stories: **Show Subtitle Timeline on Seekbar** (S6.6), **Show Statistics** (S8.13), **Reverse** (S8.13), **Sleep Timer** (S8.13) and **Audio Visualizer** (S7.8). Until each story ships, its row is hidden, never a dead toggle. *(`settings_panel/playback.rs`. The kit slider gained marks (ticks and labels) and a disabled state; it moves in 0.05 steps, Shift and PageUp/PageDown by 0.25.)*
- [x] S2.7.3 Speed shortcuts (VLC uses `[`/`]` and `=` to reset) and an OSD toast "Speed 1.25×". *(`main_window/speed.rs`: `]` Faster and `[` Slower step to the next quarter (a typed 1.1 goes to 1.25 or 1); held, they repeat throttled. `=` is Normal speed.)*
- [x] S2.7.4 Speed persists for the session only by default. Settings → Playback "Default playback speed" (S5.7) is the default for each new item. *(Decided: a launch starts at `prefs.player.default_speed` (1 by default; its row comes with S5.7); a picked speed carries from item to item for the rest of the session and is never saved, as in VLC.)*

**Edge cases**
- Speed typed outside the slider's range (0.1×, 8×): allow 0.25–4× in the field (the backend's sane range), keep the slider clamped, and show the value. *(Done: `MAX_RATE` is now 4; typed values are clamped to 0.25–4.)*
- Speed typed as "1.5", "1,5" (a decimal comma), "1.5x" or "150%": accept them all. *(Done, and `1.5×`; anything else turns the field red.)*
- A non-seekable live stream: speed is disabled (you can't fast-forward live). *(Done: the controls are disabled and the keys do nothing.)*
- Audio pitch correction at non-1× speeds is on (scaletempo); above 4× audio is muted. *(libVLC's time-stretch is on by default; speed stops at 4×.)*

---

### S2.8 — Auto-hide, Hide UI mode and fullscreen behavior

**Depends on:** S2.1, S1.4  **Size:** M  **Design refs:** `hide_ui/player_hide_ui.png`, Settings → Interface "Playback Controls"

**Subtasks**
- [x] S2.8.1 Auto-hide after the "Hide after" delay (Settings → Interface, default 3 s, see S5.4) following the "Auto-hide the controls" mode (In fullscreen only, Always, Never). Mouse movement over the window shows the controls; the cursor hides along with them. *(`main_window/auto_hide.rs`, rules in `auto_hide/rules.rs`; `prefs.interface.auto_hide_controls` and `hide_controls_after_ms`, rows with S5.4. Decided: "Hide after" is clamped to 0.5–10 s, so 0 isn't allowed. Only video hides anything; the empty state keeps its bars. "Always" in a window floats both bars over the video, inset all round as in Hide UI, so showing them never resizes the video. One timer task on the monotonic clock. The cursor hides through the OS (`cursor.rs`: macOS and Windows; Linux keeps it, as GPUI can't hide it there) and only while the pointer is over the active window.)*
- [x] S2.8.2 **Hide UI** (Ctrl+\ and right-click → Hide UI): only the rounded video viewport with its 10 px margin remains (as designed). Exit with the same shortcut, Escape, or the right-click menu. *(`main_window/hide_ui.rs`. The right-click menu on the video held only Hide UI / Show UI until S2.9.2. The macOS traffic lights hide with the title bar. Escape leaves Hide UI before fullscreen.)*
- [x] S2.8.3 The controls stay visible while: paused (configurable), the pointer is over the bar or a popover, a popover or menu is open, the seek bar is being dragged, or keyboard focus is in the bar. *(Also while stopped or opening, and while the volume wedge is dragged. `prefs.interface.keep_controls_while_paused` (on by default) is the "configurable"; its row comes with S5.4. Keyboard focus counts while the last input was a key, so Tab brings the bars back; a mouse move ends it. When a hold ends the delay starts over; a button clicked before the bars hide hands the keyboard back to the player.)*
- [x] S2.8.4 Fullscreen hides the title bar and keeps the control bar (subject to auto-hide). "Show controls in fullscreen" off means they never show in fullscreen, except on Escape or with the "Show interface controls in fullscreen" key (I). *(`prefs.interface.show_controls_in_fullscreen`, row with S5.4. The control bar floats over the edge-to-edge video. Escape leaves fullscreen, which brings them back; I shows them until the pointer next rests, and also wakes auto-hidden controls in a window. The track popover and settings panel now work in fullscreen. Held keys are forgotten when fullscreen switches, as macOS drops the key-up of the key that switched it, which made the next Escape or F do nothing. Checked on macOS: fullscreen shows the bar for the delay then hides it, a mouse move and I bring it back, Cmd+\ and Escape enter and leave Hide UI. The right-click menu, drag-to-move and the held-bar cases were not driven on screen.)*

**Edge cases**
- The mouse rests over the hidden control bar area: it doesn't flicker (hysteresis on show and hide). *(Moves under 4 px don't count, and the pointer over a bar holds it up.)*
- A second monitor: moving the mouse on another screen doesn't wake the controls. *(Moves outside the window are ignored, and the cursor is only hidden while over the window.)*
- Hide UI mode while a modal or the Settings window is open: the modals still show. *(Only the bars and what hangs off them hide; toasts and modals stay.)*
- In Hide UI mode the window has no title bar to drag, so dragging the video moves the window (Windows and Linux) and the OS still allows resizing at the edges. *(Done on macOS and Linux; GPUI can't start a window move on Windows, so there it doesn't drag yet.)*
- Fullscreen plus Hide UI plus a keyboard-only user: there must always be a keyboard way back (Escape). *(Escape leaves Hide UI, then fullscreen.)*
- A trackpad tap-to-click on the video when single click = Play/Pause: don't also wake the controls in a way that eats the click. *(Only moves wake them, never a press, and they show at the edges, not over the video's middle.)*

---

### S2.9 — Main menu (`…`) and the player right-click menu

**Depends on:** S0.4.7, S0.5  **Size:** M  **Design refs:** `menus/main_menu/menu.png`, `menus/right_click_menu/right_click_menu.png`, `menus/profiles/profiles.png`

**Subtasks**
- [x] S2.9.1 Main menu (from `…`): Open File(s) · Open Folder · Open Network Stream · Open Disc · Open Capture Device · Convert · Open Recent ▸ · Profile (*current*) ▸ · Keyboard Shortcuts · Customize Player · Settings · About VLC, with the shortcuts shown in the design. *(`main_window/main_menu.rs` from `keymap::menus::MAIN_MENU`. Shortcuts are the D3 keymap's, not the design's Ctrl+F/Ctrl+C. For now it shows Open File(s), Open Folder, Open Recent, Run Setup Again, About VLC and Copy Debug Info; the rest are pending (S2.9.4).)*
- [x] S2.9.2 Right-click on the video: Play · Stop · Previous Item · Next Item · Record · Hide UI · Playback ▸ · Video ▸ · Audio ▸ · Subtitle ▸ · More Settings ▸ · Playlist ▸ · Open Media ▸ · Quit VLC. *(`main_window/video_menu.rs`. Play reads Pause while playing, Hide UI reads Show UI in Hide UI mode. Items run the command as its key does, without the held-key gate. Record (S8.6) is hidden until its story ships; Playlist came with S3.2; Repeat, Shuffle, Stop After Each Item and When Playback Finishes joined it with S3.3. New icons: stop, playback, quit, and a drawn Hide UI icon.)*
- [x] S2.9.3 **Write the submenu contents** (none are designed): Open Recent (last N items plus "Clear"), Playback (speed, jump to time, chapters, A-B loop?), Video (track, aspect, crop, zoom, fullscreen, snapshot), Audio (track, device, stereo mode, visualizer), Subtitle (track, add file, delay), Playlist (repeat, shuffle, show queue), Open Media (the open actions). *(`video_menu/playback.rs` and `video_menu/media.rs`. Decided: **Playback:** Speed ▸ (0.25× to 2× in quarters, then Faster, Slower, Normal Speed), Chapter ▸ when there are chapters, Jump Forward, Jump Backward, Jump to Time… (opens the current-time field, S2.4). **Video:** Video Track ▸, Fullscreen. **Audio:** Audio Track ▸, Audio Device ▸, Stereo Mode ▸ (Automatic or Stereo, the "Speaker setup" setting), Mute, Increase/Decrease Volume. **Subtitle:** Subtitle Track ▸, Add Subtitle File…, Increase/Decrease Delay (50 ms, said over the video), Reset Delay. **More Settings:** the settings panel's four tabs (S2.7). **Open Media:** Open File(s), Open Folder, Open from Clipboard, Open Recent ▸. Open Recent lists the recent media and Clear Recent (S1.3.5). Left out until the engine or their stories have them: A-B loop, aspect, crop, zoom, snapshot (S8.4), visualizer (S7.8), Playlist (S3).)*
- [x] S2.9.4 Items not implemented yet (Convert, Disc, Capture, Profiles, Customize, Record) are **hidden**, not shown disabled, until their stories ship. *(`keymap::menus::PENDING` lists them, plus Settings (S5.2), Snapshot (S8.4) and the queue (S3.2, since shipped); S2.10 added the file browser (S4.10) and Next program (S8.9) and shipped Keyboard Shortcuts. Remove a command there when it ships. Both menus and the macOS bar filter by it.)*
- [x] S2.9.5 On macOS the same commands also appear in the native menu bar (S0.5.4). *(New Playback, Audio, Subtitle and View menus. Commands with plain-key shortcuts (Space, S, F, …) go through the `FromMenuBar` action and show no shortcut there, because macOS would fire a plain key equivalent inside text fields. The bar is static, so it says "Play / Pause" and has no track or recent lists. Not checked on macOS yet.)*
- [x] S2.9.6 **About VLC** dialog (not designed): version, backend and libVLC version, licenses, credits (see S9.9). *(`main_window/about.rs`: a kit dialog with the version, "Playback engine: libVLC 3.0.x", the MIT and LGPL notes, credits, and Copy Debug Info. Shift+F1, the `…` menu and the macOS VLC menu open it. Debug info now reports the real libVLC version; crash reports report it only if libVLC was already loaded.)*

**Edge cases**
- The shortcut labels in the menus come from the keymap (S0.5.3), so they're never stale and show ⌘ on macOS. *(Done.)*
- A menu opened with the keyboard (Shift+F10 or the context-menu key) appears at the focused element, not at the mouse. *(Done: the new Show context menu command opens the video menu inside the video's top-left corner with its first item highlighted. GPUI reports the context-menu key only on Windows.)*
- A right-click while a popover is open closes the popover first. *(Done: popovers and the other menus close, then the menu opens.)*
- Disabled items explain why on hover ("No media playing"). *(Done: the kit menu's `why_disabled` shows a tooltip on a disabled item or empty submenu.)*

---

### S2.10 — Mouse and keyboard on the video, OSD messages, read-only Keyboard Shortcuts dialog

**Depends on:** S0.5, S2.2  **Size:** M  **Design refs:** onboarding step 3, `dialogs/keyboard_shortcuts/keyboard_shortcuts.png`

**Subtasks**
- [x] S2.10.1 Defaults from Settings → Controls: single click = Play/Pause, double click = Toggle fullscreen, wheel = Volume, horizontal wheel = Seek. *(`main_window/video_mouse.rs`; `prefs.controls.click`, `double_click` and `horizontal_wheel`, rows with S5.6. Sideways scrolling seeks by the very short skip per step, left is back; a scroll goes to the axis it mostly moves along (`main_window/wheel.rs`). A click that closes a popover or menu, activates the window, or lands on a bar floating over the video does nothing to playback. Not checked on screen yet.)*
- [x] S2.10.2 OSD (on-screen display) toasts in the top-left of the video for volume, seek target, speed, track changes and "T" (Show playback position and time). Not designed; design them and respect "reduce motion". *(`main_window/osd.rs`: the volume bubble's look in the video's top-left corner, fading in over 120 ms unless motion is reduced. Seeks say how far and where they landed ("+10 s · 00:01:23 / 00:45:00"), T the position and length; B and V now name the track here instead of in a toast. The volume still shows in its bubble while the control bar is up.)*
- [x] S2.10.3 Wire every onboarding shortcut: Space, ←/→, `<`/`>` (or N/P, see D3), `,`/`.` (frame step), V, B, F, ↑/↓, Q, L, Shift+S, Shift+R. Each one works or, until its feature lands, shows "Coming soon" in debug builds only. *(`,` and `.` step frames (`main_window/frame_step.rs`); L (S4.10), Shift+S (S8.4), Shift+R (S8.6) and X (S8.9) are in `keymap::menus::PENDING` and say "Coming soon: …" over the video in debug builds; Q shows the queue since S3.2. A test checks every onboarding key either works or is pending.)*
- [x] S2.10.4 The Keyboard Shortcuts dialog, read-only in this stage (editing is S5.6): search, sections, Action | Hotkey | Global Hotkey columns. It's the "full list in the menu" that onboarding promises. *(`main_window/shortcuts_dialog.rs`, sections in `keymap::sections`. The search finds every word in the names, the keys or both (All Fields, Action, Hotkey); keys match however they're spaced. No pencils or Save; Close only. Global Hotkey is empty, as there are none yet. Pending commands are left out. Opens from the `…` menu and the macOS app menu. The kit dialog gained a width and a toolbar that doesn't scroll.)*

**Edge cases**
- Frame stepping (`,`/`.`): stepping backwards needs a seek to the previous keyframe and decoding forward (slow on long GOPs), so show a spinner if it takes more than 200 ms. Frame step pauses playback. *(Done: "Stepping…" with a spinner in the OSD's place, still when motion is reduced.)*
- Double-click timing between the single-click and double-click actions (S5.6 edge case). *(While a double click means something, a single click waits 300 ms and a second click cancels it; with double click set to nothing it acts at once.)*
- Seeking with the arrow keys past the end goes to the end and fires `Ended` once. Past the start clamps to 0. *(The player clamps jumps (S2.3.4).)*
- Caps Lock is on: `S` must not become Shift+S (Snapshot) instead of Stop. Match on the shift modifier, not letter case. *(Covered by `keymap/tests/layouts.rs`.)*

---

### S2.11 — OS media integration (media keys and the system "Now Playing")

**Depends on:** S1.2  **Size:** M

**Subtasks**
- [ ] S2.11.1 Hardware media keys (Play/Pause, Next, Previous, Stop) through the OS media session: Windows SMTC, macOS `MPRemoteCommandCenter`, Linux MPRIS over D-Bus.
- [ ] S2.11.2 Now Playing metadata (title, artist, artwork, position) published to the OS (lock screen, Control Center, Windows media flyout).
- [ ] S2.11.3 Windows taskbar thumbnail toolbar (Previous, Play/Pause, Next) and a progress overlay on the taskbar icon (optional).

**Edge cases**
- Another media app (a browser, Spotify) owns the media session: the OS decides who gets the keys. Don't fight it by re-registering in a loop.
- Bluetooth headset buttons send media keys: they behave the same as keyboard media keys.
- A media key pressed while the app is minimized or in the background: it works (that's the point).
- Linux without D-Bus (minimal window managers): skip MPRIS silently.

---

## Stage 3 — Queue, playback order and playlists

> **Goal:** play many things in a row, control the order, and save collections as playlists.
> **Exit criteria:** the Queue tab, Repeat/Shuffle, the New Playlist dialog and playlist import/export work. The queue survives restarts.

### S3.1 — Queue model, next and previous

**Depends on:** S1.2  **Size:** M

**Subtasks**
- [x] S3.1.1 A `Queue` entity: an ordered list of items (local file, URL, disc), the current index, and play history (for Previous under shuffle). *(`src/queue.rs`, without GPUI; the main window owns one. Entries carry ids, so history and duplicates survive moves. Discs join `Media` with S8.9. The play history became shuffle's (S3.3).)*
- [x] S3.1.2 Opening media **replaces** the queue by default; "Enqueue" (Add to Queue, `--enqueue`, Shift+drop) **appends**; "Play next" inserts after the current item. *(`Placement::PlayNext` joins `Replace` and `Enqueue`. Add to Queue… and Play Next… are in the video menu's Open Media ▸ and pick files; added items start only when nothing is playing.)*
- [x] S3.1.3 Opening one file from a folder optionally queues its siblings in natural order (a setting, on for TV episodes). Settings → Library "When opening a folder: Include subfolders" (S5.9) controls recursion. *(`opening/siblings.rs`. `prefs.player.queue_siblings`: `never`, `episodes` (default: a file named `S01E02` or `1x02` queues the folder's other episodes, nothing else) or `always`. `prefs.library.include_subfolders` (default on) applies to opened folders too. Only a single file opened in place of the queue does this; its row comes with S5.7 and S5.9.)*
- [x] S3.1.4 Auto-advance on `Ended`; on `Error`, skip to the next item with a toast ("Couldn't play X, skipped"), and stop after N consecutive failures (for example 5) to avoid spinning through a broken list. *(`main_window/queue_play.rs` and `queue/skips.rs`: 5 in a row, counted until something reaches Playing. The last item's error stays on screen with no skip.)*

**Edge cases**
- The queue contains the same file twice: allowed, and each entry is independent. *(Done.)*
- The currently playing item is removed from the queue: playback continues and the item stays in history (decide); or stop (VLC behavior). *(Decided: playback continues, and Next plays the item that took its place. The panel's Delete does this (S3.2).)*
- The queue is replaced while the current item is buffering: cancel cleanly (S1.2 generation id). *(Done: the new open takes a new generation.)*
- An item's file is deleted before it's reached: skip it with a toast. *(Done: the engine's preflight fails it and the queue skips it.)*
- 10,000 items (a dropped music folder): operations are O(1) or O(log n) where possible, and the UI is virtualized. *(Lookups and Next/Previous are O(1); insert and remove move the items after them. The queue panel's list is virtualized (S3.2).)*
- Queue items that are playlist files (`.m3u`): expand them (S1.3.4), with cycle detection (A includes B includes A) and a depth limit. *(Done in S1.3.4; every placement resolves the same way.)*

---

### S3.2 — Queue panel (List → Queue tab, and the miniplayer queue)

**Depends on:** S3.1, S0.4.11  **Size:** M  **Design refs:** `controls-queue.png`, `miniplayer-variant2.png`

**Subtasks**
- [x] S3.2.1 The List panel (tabs **Queue | Bookmarks | Chapters**) opens from the List button and **Q**. *(`main_window/queue_panel.rs`, in the settings panel's place: opening one closes the other. It stays open while the video is used; the List button, Q and Escape close it, and it keeps the bar up. Bookmarks (S8.3) and Chapters (S8.2) say they come later. The video menu's Playlist ▸ has Show/Hide Queue and Clear Queue.)*
- [x] S3.2.2 Header: "N items - Xh Ym" (count and total duration) · sort · repeat · shuffle · "−" (remove selected) · "+" (add files). *(`queue_panel/header.rs`, `queue/lengths.rs`. Lengths come from a background libVLC probe of local files' headers (`engine/lengths.rs`), started when the panel first opens, and from the player for what plays. Repeat and Shuffle came with S3.3. "+" is Add to Queue.)*
- [x] S3.2.3 Rows: drag handle, name and duration. Drag to reorder; double-click to play; Delete removes; multi-select. **The current-item highlight isn't designed; add one.** *(`queue_panel/rows.rs` on `uniform_list`; the current item's name is in the accent with an accent bar at its left. Click, Ctrl/Cmd+click, Shift+click; arrows, Home/End, PageUp/PageDown (Shift extends), Ctrl/Cmd+A, Enter plays, Delete/Backspace remove. Dragging a selected row moves the whole selection.)*
- [x] S3.2.4 Drop files from the OS or library onto the panel to insert them at the drop position. *(OS drops are resolved like any open (folders, playlists) and inserted before the row under the gap; a line shows the gap. Library drops come with Stage 4.)*
- [x] S3.2.5 Sort the queue (by name, duration, date added, shuffle once) as a one-time reorder, not a persistent sort mode. *(`queue/order.rs`: natural name order, unknown durations last, stable.)*

**Edge cases**
- The total duration with unknown durations: show the known sum plus "+" and compute the rest lazily. **The mock "2h 3m" disagrees with its rows (1h 35m); always compute it.** *(Done: "4 items - 1h 12m+"; the mock's rows add up to 1h 34m.)*
- "−" with nothing selected: disabled. *(Done; Sort is disabled below two items.)*
- Reordering the currently playing item: playback continues. *(Done: the current entry is followed by id.)*
- Dragging a row out of the panel: cancel the drag, never delete silently. *(Done.)*
- Removing all items while one is playing: decide whether playback stops (recommended: playback continues until the current item ends). *(Decided as recommended.)*

---

### S3.3 — Repeat, shuffle and end-of-queue behavior

**Depends on:** S3.1  **Size:** S  **Design refs:** queue header icons, `docs/icons/media_controls/repeat/*` and `shuffle/*`, Settings → Playback "Order"

**Subtasks**
- [x] S3.3.1 Repeat: Off → All → One (a cycling button with three visible states; the ON states aren't designed). *(`queue/repeat.rs`. The queue header's button shows All and One as on, One with a drawn 1 inside the arrows (`repeat-one.svg`); its tooltip and an OSD message name the mode. Repeat All wraps Next and Previous; Repeat One replays at an item's natural end. Also a command (no default key) and Playlist ▸ Repeat ▸ in the video menu.)*
- [x] S3.3.2 Shuffle: a random permutation without repeats until every item has played, then a fresh permutation (avoid playing the same item twice across the boundary). Previous under shuffle follows the history. *(`queue/shuffle.rs`, kept by entry id so moving and sorting rows don't disturb it. Previous goes back through what played and Next then returns. Without Repeat All, the queue stops once every item has played. Opening media plays what was opened; the rest is shuffled. The queue's history moved here from S3.1.)*
- [x] S3.3.3 Stop after each item (Settings → Playback), and "When playback finishes" (Stop, Close the player, Quit, …). *(`prefs.player.stop_after_each_item` and `prefs.player.when_finished` (`stop` or `quit`), in Playlist ▸ until Settings → Playback (S5.7). After a stop, Play goes on to the next item. Decided: "Close the player" joins with the library (Stage 4), the place it would close to; sleep and shut down aren't offered.)*
- [x] S3.3.4 The queue buttons and Settings → Playback write the **same** state. *(Repeat and Shuffle live in `Queue`; the header buttons, the commands and the video menu all change them through `main_window/play_order.rs`, which Settings → Playback (S5.7) uses too. They persist with the session (S3.4, since shipped).)*

**Edge cases**
- Shuffle switched on mid-queue: the current item stays current and the rest is shuffled. *(Done.)*
- Items added during shuffle: insert them at random positions in the remaining permutation. *(Done.)*
- Repeat One plus Next: Next still moves to the next item (Repeat One only applies to natural end). *(Done; a failed item is skipped, not retried.)*
- A one-item queue with Repeat All behaves like Repeat One. *(Done, shuffled or not.)*
- A queue where every item fails while Repeat All is on: stop after N failures (S3.1.4), never loop forever. *(Done: the skip count doesn't care about Repeat.)*

---

### S3.4 — Session restore

**Depends on:** S3.1, S1.7  **Size:** S

**Subtasks**
- [x] S3.4.1 Persist the queue, the current item and its position, repeat and shuffle state on change (debounced) and on quit. *(`src/session.rs`: `session.toml` next to `local.toml`, written atomically like the settings on a `session` thread that keeps only the newest snapshot; quitting waits for it. `main_window/session.rs` watches the window: a change is written after 1 s, the playing position every 15 s, and starting to play an item at once (`session/cadence.rs`). Not saved, and forgotten, while "Save recently played items" is off.)*
- [x] S3.4.2 On launch, restore the queue paused at the saved position (don't autoplay); it's controlled by a setting. *(`prefs.player.restore_session` ("Continue where you left off", default on; its row comes with S5.7). The engine gained `Player::open_at` with libVLC's `start-time` and `start-paused`, so nothing plays or sounds before the position; no resume toast for it. Files opened at launch replace the session; `--enqueue` adds to it. Play with nothing loaded plays the current item. Rules in `session/restore.rs`.)*

**Edge cases**
- Restored items that are offline or missing are kept but greyed. *(Done: checked in the background at launch; one that plays later stops being greyed. A missing current item waits for Play instead of showing an error.)*
- A crash loop caused by a restored item (the file crashes the decoder): if the last session ended in a crash while playing item X, don't auto-load X, and show "VLC closed unexpectedly while playing X". *(Done: the snapshot says whether an item was playing, written the moment it starts and cleared by pause, stop and quit; a launch that finds it set leaves X current but unloaded, with that toast. A `kill -9` while playing counts too. With multiple instances allowed, the last to save wins.)*
- A restored URL has expired: the error appears only when the user plays it. *(Done: URLs are never loaded on restore.)*

---

### S3.5 — Saved playlists and the New Playlist dialog

**Depends on:** S3.1, S4.1  **Size:** M  **Design refs:** `docs/ui/dialogs/new_playlist/new_playlist.png`, `sidebar-playlists.png`, `media_cards/playlist-*`

**Subtasks**
- [x] S3.5.1 The New Playlist dialog: **Playlist Details** (title field, placeholder "e.g., Favorite Videos", and a thumbnail picker "+"), **Select Files** (a dashed drop zone "Drag files into the box or click here to select files." plus "+"), Cancel and Save. Save is enabled once the title is non-empty. *(`main_window/new_playlist.rs`, with a screenshot test against the design. The whole drop zone is the "+": click it, or Enter on it, to pick files. Playlists are rows in `library.db` (migration 2, `db/playlists.rs`), kept by location like the session's queue, and written on the history thread, which owns the database (`src/playlists.rs`). New Playlist and Save Queue as Playlist are commands (no default keys), in the macOS File menu and the video menu's Playlist ▸.)*
- [ ] S3.5.2 Playlists in the library (Playlists section): "+ Create playlist", rows with the playlist glyph, hover `•••` and the playlist context menu (S4.8.3). *(The Playlists section lists the saved playlists, each with the playlist menu (S4.8.3). "+ Create playlist" is still to come; meanwhile the video menu's Playlist ▸ Saved Playlists ▸ lists each playlist with Play, Add to Queue, Rename…, Duplicate and Delete….)*
- [ ] S3.5.3 Open a playlist: a list view with reordering, removing, and adding items (not designed; reuse the queue row style). *(Comes with the library's Playlists section (S3.5.2). Adding items works now: Add to Playlist ▸.)*
- [x] S3.5.4 Rename, delete (with confirmation and undo), and duplicate. *(`main_window/saved_playlists.rs`. Delete asks first, then its toast has Undo, which puts the playlist back with the same id. A duplicate is numbered like a taken title.)*
- [x] S3.5.5 "Add to Playlist" from the file menu and the queue ("Save queue as playlist"). *(Until the library's file menu (S4.8.1) exists, the video menu's Playlist ▸ has Add to Playlist ▸ for the playing item (a saved playlist, or New Playlist… holding it) and Save Queue as Playlist…, which opens the dialog with the queue's items. S4.8.1 reuses `MainWindow::add_to_playlist`.)*
- [ ] S3.5.6 The playlist thumbnail is a custom image, or an automatic 2×2 collage of its first items' thumbnails. *(The custom image is picked, checked and saved with the playlist; showing it, and the collage, need the thumbnail cache (S4.4) and a place that shows playlists (S3.5.2).)*

**Edge cases**
- Duplicate playlist names: allowed but warned, or auto-suffixed (decide). *(Decided: auto-suffixed, "Mix (2)", ignoring case, for new, renamed and duplicated playlists (`playlists/title.rs`).)*
- A title with leading or trailing spaces, only whitespace, or 500 characters: trim it, require at least one visible character, cap at about 100 characters. *(Done: zero-width characters don't count as visible; the field stops at 100.)*
- A thumbnail image that is huge, animated (GIF or WebP), corrupt, or not an image: validate it, take the first frame, and downscale. *(Validated now, by content and under 25 MB (`playlists/thumbnail.rs`); the first frame and downscaling come with the thumbnail cache (S3.5.6).)*
- A playlist item whose file is moved: resolve it by content hash (S1.7.2), otherwise show it as missing with "Locate…". *(Comes with the playlist view (S3.5.3); a missing item fails and is skipped like any queued file (S3.1.4).)*
- An empty playlist: allowed, with an empty state. *(Allowed; its menu says it is empty and disables Play and Add to Queue. The view's empty state comes with S3.5.3.)*
- Dropping 1,000 files into the dialog's drop zone: add them asynchronously with a progress indicator; Save stays disabled until adding is finished. *(Done: they resolve in the background like any open (folders, playlist files); the zone says "Adding files…" and then how many will be added.)*
- Cancel with unsaved changes: confirm discarding when files were added. *(Done for Cancel, ✕ and Escape; files that came from the queue don't count as added.)*

---

### S3.6 — Playlist import and export

**Depends on:** S3.5  **Size:** S  **Design refs:** playlist context menu "Export Playlist"

**Subtasks**
- [x] S3.6.1 Export to `.m3u8` (UTF-8, `#EXTINF` with duration and title) and `.xspf`, with relative or absolute paths (an option in the save dialog). *(`playlists/export.rs`; Saved Playlists ▸ each playlist ▸ Export… until the playlist context menu (S4.8.3). The native save dialog can't hold options, so a small dialog asks for the format (M3U8 | XSPF) and "Relative paths" (default on) first; a name typed with the other extension gets that format. Durations come from the history (unknown is `-1` / no `<duration>`); titles are file names. A file with no relative path (another Windows drive) is written absolute; an M3U path starting with `#` gets `./`. Round-trip tests read every export back with the importer.)*
- [x] S3.6.2 Import `.m3u`, `.m3u8`, `.pls`, `.xspf`, `.cue` and `.asx` by drag, open, or library "Add". *(Drag and open already expand them into the queue (S1.3.4), and into the New Playlist dialog's drop zone. New: Import Playlist… (a command, no default key, in the macOS File menu and the video menu's Playlist ▸) saves each picked file as a saved playlist under its file name (`playlists/import.rs`). The library's "Add" comes with Stage 4 and reuses `playlists::import`.)*

**Edge cases**
- `.m3u` files in legacy encodings (Windows-1252 and others): detect the encoding (BOM, then heuristics). *(Done: BOM, then BOM-less UTF-16 (zeros in every other byte), then UTF-8, then Windows-1252, which is Latin-1 for every letter. Other code pages (Cyrillic, Shift-JIS) read as Windows-1252.)*
- Relative paths resolve against the playlist file's folder, not the working directory. Windows backslash paths in a playlist opened on macOS or Linux must be normalized. *(Done in S1.3.4.)*
- Entries that are URLs, missing files, nested playlists or duplicates: import with markers and never fail the whole import because of one bad line. *(Done: URLs and duplicates are kept, nested local playlists expanded, missing files kept and counted in the toast ("Imported “Mix” (120 items). 3 files are missing."). Per-item markers come with the playlist view (S3.5.3). An empty playlist imports empty.)*
- An extremely large playlist (100k lines) or a malformed one: parse with streaming, a size cap and a time cap. *(Done: 16 MB per file, 100,000 items and 10 s per expansion; what was read by then is kept. Lines that aren't entries are skipped.)*
- Exporting to a read-only location or a full disk: show the error, and never leave a half-written file (atomic write). *(Done: written beside the target, flushed, then renamed over it; on failure the temporary file is removed and the toast says why.)*

---

## Stage 4 — Library

> **Goal:** the media folder chosen in onboarding becomes a browsable, searchable library with thumbnails, watch progress, and a Home page that answers "what was I watching?".
> **Exit criteria:** a 20,000-file library indexes in the background without blocking the UI; the sidebar overlay and full library mode match `docs/ui/library/**` and `docs/ui/player/screens/default/player_02..07`; changes on disk show up without a manual rescan.

Design facts the stories rely on:
- **Icon rail:** Home, Videos, Music, Network, Playlists, History/Recent, "+". Each icon has three states: outline (inactive), **orange filled** (at that section's root) and **grey filled** (inside a subfolder of that section).
- **Header:** back, forward, breadcrumb with `▸` separators (it collapses to `… ▸ parent ▸ current` from 3 levels deep), `•••` more, and pop-out.
- **Toolbar:** "N items" or "1 item", Sort (an **orange dot** when a non-default sort is active), View (Grid or List), Search.
- **Two presentations:**
  - The **sidebar overlay** over the player (`player_02`, `player_light`, `player_music_02`). It overlays the video and doesn't resize it.
  - **Full library mode** (`player_03..07`): labeled nav, grid or list, and the playing item continues in a docked **miniplayer** card.

### S4.1 — Library data model and database schema

**Depends on:** S1.7  **Size:** M

**Subtasks**
- [x] S4.1.1 Tables: `roots` (media folders, kind video/music/mixed, status), `folders`, `media` (from S1.7 plus kind, title, artist, album, album artist, track no., disc no., genre, year, date added, codec summary, resolution, has-artwork), `artwork`/`thumbnails` (cache keys), `favorites`, `playlists`/`playlist_items`, `network_streams` (with category Podcasts/Radio/…), `visits` (folder and playlist visit counts for Home), `plays` (for Listen Again and Frequently Listened Artists). *(Migration 3 in `db/migrations.rs`, with foreign keys now enforced. A `files` table joins folders to media, so one media row can have several paths. Table modules: `db/roots.rs`, `db/folders.rs`, `db/files.rs`, `db/metadata.rs` and `db/activity.rs` (plays, visits, favorites, and the Home queries). Has-artwork is `media.artwork_id`. `media::record_play` now also writes a `plays` row, and existing play counts become one play each at their last play. Clear History now resets history and deletes plays and visits, but keeps media rows that the library or a favorite uses, along with their tags. `network_streams.category` holds S4.11.3's `recent`, `podcasts` or `radio`. Nothing calls the table modules yet. The indexer (S4.2) is the first caller.)*
- [x] S4.1.2 Watch state per media: `unwatched | in_progress(fraction) | watched`, derived from S1.7. Define the "watched" threshold once (S1.7.4) and reuse it. *(`library/watch_state.rs`: `WatchState::of(&Saved, WatchedRule)`, derived and never stored. A saved position that the current rule counts as watched (the setting was lowered after the save) shows as watched.)*
- [x] S4.1.3 Migrations are tested from every released schema version. *(`db/tests.rs` builds every older schema with a row in it, opens it, and checks that the row, its play count and the new tables survive; migrating from before version 3 also checks the added date and the plays it backfills. No version has shipped yet, so this covers every version.)*

**Edge cases**
- The same file is under two roots (nested roots, a symlink into another root): store one media row and show it in both trees. *(Done: nested roots share their folder rows. Removing a root deletes only the folders that no other root reaches. Files under any root point at one media row by identity.)*
- Case-insensitive filesystems (Windows, default macOS) versus case-sensitive ones (Linux, or case-sensitive APFS): path comparisons must follow the filesystem, not a global rule. *(Done: `library/path_key.rs`. `Case::detect` checks whether the folder's name with its letters' case swapped finds the same folder, and the result is stored per root. Folders and files are looked up by a `key` that folds case only on case-insensitive roots.)*
- Unicode normalization: macOS stores file names in NFD, the user types NFC. Normalize for search and comparison, but keep the original bytes for opening. *(Done for comparison: keys are NFC on macOS, whose file systems fold normalization. NTFS and Linux file systems keep the two forms as different names, so keys there don't fold them. `path` and `name` keep the original, which `stored_path` encodes losslessly. Search folds case and accents itself (S4.7.5).)*

---

### S4.2 — Media folder indexing (scanner and file watcher)

**Why:** onboarding saves `media_folder` and nothing reads it yet.
**Depends on:** S4.1, S0.1  **Size:** L

**Subtasks**
- [x] S4.2.1 On first launch after onboarding, register the onboarding folder as the first root and start a background scan that shows progress ("Indexing 1,234 of ~5,000"). The UI is usable during the scan. *(`library.rs`: the `Library` global starts the indexer thread (`library/indexer/`) and follows the settings: `local.media_folders` are video roots, `local.music_folders` music roots; roots no longer in the settings are removed. The indexer reaches the database through the history thread (`History::database`). Until the library views (S4.6) exist, the main window's empty state lists each media folder with its status or progress (`main_window/library_status.rs`). The estimate is the last scan's count, or on a first scan the files found scaled by the folders still to list.)*
- [x] S4.2.2 Scanner: walk the tree on a background thread pool, recognize media by extension and then a content probe for ambiguous files, skip hidden and system files (setting S5.9), and insert in batches. *(`library/walk.rs` (up to 4 threads, a bounded channel), `library/recognize.rs` (known non-media extensions such as pictures, documents and `.part` are never opened), `library/apply.rs` (500 files per transaction). `prefs.library.show_hidden_files` (default off); its row comes with S5.9.1. A second pass (`library/identify.rs`) hashes new and changed files (S1.7.2) and links them to media rows, so moved files keep their history and copies share a row.)*
- [x] S4.2.3 Incremental rescans compare size and mtime, so unchanged files aren't re-probed. *(Every launch rescans every root this way, which also resumes a scan an earlier run didn't finish: files already written aren't read again.)*
- [x] S4.2.4 Live updates through a filesystem watcher (the `notify` crate: ReadDirectoryChangesW, FSEvents, inotify) with debouncing (about 2 s), plus a periodic reconcile because watchers drop events. *(`library/watch.rs`, `library/indexer/targets.rs`: a changed file relists its folder, a changed folder is walked whole. Reconcile every 15 minutes; missing or unanswering roots are checked every 30 s.)*
- [x] S4.2.5 **Resync** (folder context menu) forces a rescan of that subtree. *(`library::resync`, which reads every file again. Until the folder menu (S4.8.2) exists, each media folder's row in the empty state has a Resync button.)*
- [ ] S4.2.6 Root status: OK, Missing (drive unplugged or folder deleted), Permission denied, Slow (network). Show it in the folder row and in Settings → Library. *(Detected and stored (`library/root_check.rs`: a read that takes over 2 s is Slow, over 10 s is skipped until the next check) and shown in the empty state's rows, with the count of unreadable subfolders. Showing it in the library's folder rows (S4.6) and Settings → Library (S5.9.3) waits for those screens.)*
- [x] S4.2.7 Music sources: onboarding asks for one folder (default `~/Videos`). **Decision:** auto-add the OS Music folder (`~/Music`) as a music root when it exists (recommended), or ask the first time the Music section opens. The Music section must not be empty just because onboarding only asked about videos. *(Auto-added (D7), once: `local.music_folder_added` remembers it, so removing it in Settings (S5.9.3) sticks. `library/sources.rs`.)*

**Edge cases**
- The onboarding folder doesn't exist (the default `~/Videos` was never created, or Windows redirected Videos to OneDrive and `USERPROFILE\Videos` is empty): show an empty state with "Choose another folder" and "Create this folder". Use the Windows Known Folder API (FOLDERID_Videos) instead of `USERPROFILE\Videos`. *(Done: the folder's row offers both buttons; the default already comes from the Known Folder API (S0.1).)*
- The root is on a removable drive that's unplugged: keep its items in the DB marked offline, greyed in the UI with "Drive not connected", and don't delete history. Re-attach when the drive returns, even if it gets a different drive letter (match by volume ID or serial number when possible). *(Partly: a missing root keeps its files and is rechecked every 30 s, and only a folder that was listed loses rows, so a drive pulled mid-scan deletes nothing. Matching a drive that comes back under another letter isn't done.)*
- The root is on a network share (SMB or NFS) that's slow or disconnects mid-scan: use timeouts, skip and retry later, and don't freeze. Watchers on network shares are unreliable, so rely on periodic reconcile. *(Partly: the root check times out and retries; a read that hangs in the middle of a walk still holds that scan until the OS gives up. The UI never waits on it.)*
- Very large trees (100k+ files, a whole-drive root): stream results, cap memory, show progress, make it cancellable, and persist scan progress so a restart resumes. *(Done: streamed through a bounded channel, cancelled on quit, resumed through S4.2.3.)*
- Symlink and junction loops: track visited inodes or file IDs. *(Done: device and inode on Unix, the resolved path on Windows.)*
- Permission-denied subfolders (`System Volume Information`, macOS TCC-protected folders): skip them silently and record them in the root status details. *(Done: `unreadable_folders` (migration 4).)*
- Files still being written (a download in progress or a recording): wait until size and mtime are stable before probing, and re-probe when they change. *(Done: a file is hashed once unchanged for 3 s; the indexer comes back for it.)*
- Files deleted while the scan is running: handle "not found" gracefully.
- Path length above 260 characters on Windows: use `\\?\` paths. *(Rust's `std` adds the prefix for long paths itself.)*
- File names with invalid UTF-8 or UTF-16: keep raw `OsString`s, display them lossily, and open them with the raw path. *(Done: names stay `OsString`s and are stored with `stored_path`.)*
- Cloud placeholders (OneDrive Files On-Demand, iCloud Drive, Dropbox online-only): **don't** read their content during scans, because that triggers mass downloads. Index metadata only and mark them "cloud". *(Done: `library/placeholder.rs` (`SF_DATALESS` on macOS, the cloud attributes on Windows); `files.cloud`.)*
- Antivirus scanning slows every file open: use a bounded thread pool so the machine isn't hammered.
- The user moves a whole folder inside the root: the watcher sees deletes and creates, so match by content hash (S1.7.2) and keep watch history. *(Done: a media row no file points at any more moves to the new file.)*
- Laptop on battery: lower scan priority; pause thumbnailing on battery-saver. *(Not done: scans aren't throttled on battery yet.)*

---

### S4.3 — Metadata and artwork extraction

**Depends on:** S4.2, S1.1 (the backend can parse metadata)  **Size:** M

**Subtasks**
- [x] S4.3.1 Read tags (title, artist, album, album artist, track and disc numbers, genre, year), embedded artwork, duration, and video resolution and codecs. *(`library/extract.rs`: after the indexer identifies files, it reads 16 media at a time between its commands, through `engine::tags::TagReader`; `engine/libvlc/tags.rs` parses with libVLC's own instance (local fetch, so libVLC saves embedded covers in its art cache) and `tags/values.rs` turns FourCCs into names (`H.264 / AAC`). A still-picture video track (a cover) doesn't make a song a video. Migration 5 adds `media.probed_ms`: a media is read once, a damaged one too, and identifying its file again (Resync, a move) reads it again. A length learned by playing is kept over the header's. Without libVLC nothing is marked read. Covers are cached by content hash, downscaled to 600 px JPEG, in the cache dir (`library/artwork_cache.rs`, `settings::cache_dir`).)*
- [x] S4.3.2 Use folder artwork (`cover.jpg`, `folder.jpg`, `AlbumArt*.jpg`) for albums without embedded art. *(`library/folder_art.rs`: cover, folder, front, then the largest `AlbumArt*`, as JPEG or PNG in any case. Songs only; videos don't take a folder's picture. libVLC's own pick from the folder is ignored so every song in a folder gets the same cover.)*
- [x] S4.3.3 Parse TV episode names (`Show_S01E05`, `Show.1x05`, `Show - 105`) for nicer display titles. The raw file name stays visible where the design shows it. *(`library/episode.rs`, shown as `Show S01E05 · Title`; `library/display.rs` picks the title tag, else the episode, else the file name. Only computed, never stored. Properties & Metadata uses it now; the views (S4.6) will. A 4-digit `Show - 1999` is a year, not an episode.)*
- [x] S4.3.4 Online metadata and artwork **only** when Privacy → "Fetch titles and cover art from the internet" is on (S5.5). Off by default. *(`prefs.privacy.fetch_online_metadata` (default off) adds libVLC's network fetch for media read from then on; its row comes with S5.5. Media already read aren't looked up again when it's turned on, and a cover libVLC found online is recorded as embedded, since libVLC doesn't say which it was.)*
- [x] S4.3.5 **Properties & Metadata** (context menu): a read-only info view first; tag editing is a later story (not designed). *(`main_window/properties.rs`, `library/properties.rs`: a dialog of the playing file's title, tags, kind, length, resolution, codecs, cover source, size and folder, leaving out what isn't known. Until the file menu (S4.8.1) exists it opens from the video menu and Cmd/Ctrl+I (`ShowProperties`).)*

**Edge cases**
- Tags in legacy encodings (ID3v1 Latin-1 versus Windows-1251/1255 or Shift-JIS): detect and convert, and offer a per-file override. *(Not done: tags are taken as libVLC decodes them.)*
- Multiple artists (`A; B`, `A feat. B`): split for the Artists view using a configurable separator. *(`display::split_artists` takes the separators; the Artists view (S4.6.2) uses the defaults, and the setting is still to come. `&` and `/` aren't defaults: `Simon & Garfunkel`, `AC/DC`.)*
- Huge embedded artwork (a 20 MB PNG): decode on a background thread and downscale for the cache. *(Done: on the indexer's thread, to 600 px; images over 64 MB aren't read.)*
- Missing tags: fall back to the file name, then "Unknown Artist" / "Unknown Album". *(Done: `library/display.rs`; the album artist stands in for a missing artist.)*
- Duration missing in the container: probe it lazily and show `--:--` until it's known. *(Partly: the length stays unknown until playing the media learns it; showing `--:--` is the views' job.)*

---

### S4.4 — Thumbnails

**Depends on:** S4.2, S1.1  **Size:** M  **Design refs:** list rows with 64×64 thumbnails; the grid in `player_03`; the seek thumbnail (S2)

**Subtasks**
- [x] S4.4.1 A background thumbnailer takes a frame at about 10–20% of the duration (avoiding black intros) and skips black or uniform frames. *(`library/thumbnailer/` runs on a thread of its own; `engine/libvlc/thumbnail.rs` grabs with a third muted libVLC instance (software decoding, `:start-time`, the first picture shown, 8 s timeout), sharing the seek preview's `frame_sink.rs`. `thumbnailer/pick.rs` tries 15%, 25%, 10%, then 40% of the length (30 s, 10 s, 2 s when it's unknown) and keeps the first picture that is neither black (mean luma under 24) nor flat (deviation under 10), else the most varied one. Videos only: songs use their cover (S4.3). It fills the cache in the background, newest media first, 250 ms apart, looking for new videos every 15 s.)*
- [x] S4.4.2 Disk cache in the cache dir, keyed by media identity plus size class (list 64 px, grid about 240 px, HiDPI ×2), with LRU eviction and a size cap. *(`library/thumbnail_cache.rs`: `cache/thumbnails/`, JPEG, each picture cropped to fill its tile. List tiles are 32×32 logical (64 px in the 2× designs); grid tiles 320×180, as `player_03` draws them, not 240. Videos are keyed by a hash of their identity, songs by their cover's key, so an album shares its pictures. Migration 6 adds `thumbnails.bytes` and `used_ms`; showing a thumbnail touches it, and past 256 MB the least recently shown are deleted down to 90%. A deleted folder is recreated and its pictures made again.)*
- [x] S4.4.3 Placeholders while generating: grey (`#303030`) for video, a green music-note tile for audio without art (as in the designs), and a playlist glyph for playlists. *(`ui/thumbnail.rs`: `ThumbnailTile` draws the picture for the display's scale, or the placeholder; colors sampled from `sidebar-home`, with light-theme counterparts that no design shows. The note (`music-note.svg`) isn't in the icon set and is redrawn after the design. Screenshot tests compare the song and video tiles with the designs. The views (S4.6) are its first callers.)*
- [x] S4.4.4 Priority queue: items visible on screen first, and cancel work for items that scroll away. *(`library/thumbnails.rs` is the views' API: `show(on_screen)` whenever that changes, `status(media)` to draw. `thumbnailer/queue.rs`: what is on screen comes first, in order; a new list replaces the old, and a running grab stops between tries once its media scrolled away. The views (S4.6) are its first callers.)*

**Edge cases**
- A file the decoder crashes or hangs on: run the thumbnailer with a timeout (ideally in a separate process, so one bad file can't kill the app), and mark the file "no thumbnail" so it isn't retried every launch. *(Partly: in process, with an 8 s timeout per grab. `media.no_thumbnail_ms` is set before each try and cleared on success, so a file that fails, or crashes the app, isn't tried again until Resync. A separate process isn't done; a libVLC hang in `stop` would stop thumbnails for the session.)*
- Audio-only files: use artwork, never try to grab a frame. *(Done.)*
- DRM or corrupt files: show the placeholder and log. *(Done: no picture means the placeholder, for good.)*
- The cache dir was deleted while the app runs: recreate it lazily. *(Done.)*
- Thousands of thumbnails at once on first index: throttle CPU and GPU use, and pause during active playback so playback never stutters. *(Done: one grab at a time, in software, 250 ms apart; the background work waits while any player opens, buffers or plays. What is on screen is still made during playback.)*

---

### S4.5 — Navigation model: rail, back/forward, breadcrumb

**Depends on:** S0.4, S4.1  **Size:** M  **Design refs:** `docs/ui/library/sidebar/*`, `player_02`, `player_03`

**Subtasks**
- [x] S4.5.1 Route model: `Home | Videos(path) | Music(category, path) | Network(category) | Playlists(id?) | History | Search(query, scope)`. *(`navigation/route.rs`. `Videos(None)` lists the video folders. Music is `MusicRoute`: the categories, `Songs(folder?)` (Songs browses the music folders, as `sidebar-music2/3` show), and `Albums`, `Artists` and `Genres` with an optional name. A search's scope is a rail section; Home searches everything.)*
- [x] S4.5.2 Back and forward history stacks (mouse buttons 4 and 5, Alt+←/→, Cmd+[ and ]). **The mock always draws Back enabled and Forward disabled; implement real enabled states.** *(`navigation/history.rs`: going somewhere new clears Forward, and each stack keeps 100 routes. The buttons are enabled only when a route that still exists lies that way. The keys work while the sidebar has the keyboard: Alt+Left and Alt+Right (they seek in the player, but the library's bindings are deeper in the key context), plus Cmd+[ and ] on macOS.)*
- [x] S4.5.3 Breadcrumb segments are clickable, and the leading `…` opens a menu of the hidden ancestors. *(`navigation/breadcrumb.rs`, `main_window/library_panel/crumbs.rs`. A folder goes through its innermost media folder. More than two segments collapse to `… ▸ parent ▸ current`, and the menu lists the hidden ones nearest first. The last segment is the current place, so it isn't a link.)*
- [x] S4.5.4 Rail icon states: outline, orange filled (section root) and grey filled (deeper in the section). *(`navigation/rail.rs`. The filled icons are the set's `selected` variants, colored at runtime. History has no filled icon in the set, so its outline is colored instead. A search fills its scope's icon grey.)*
- [x] S4.5.5 Item count, with "1 item" versus "N items" pluralization (through i18n, S9.2). Action rows ("+ Add", "+ Create playlist") aren't counted. *(`navigation::item_count`, with thousands separators. It moves into the i18n layer with S9.2. The panel counts the rows it lists; action rows come with S4.6.)*
- [x] S4.5.6 Remember the last route per presentation (overlay and full library) across launches. *(`local.library_routes` in `local.toml`, written by `navigation/stored.rs` as text such as `videos:/media/Shows`, with paths encoded like other stored paths. Text that doesn't parse opens Home. Only the overlay exists so far; full library mode uses its own slot from S4.10.2.)*
- [x] S4.5.7 Define the rail's **"+"** and **History** items (not designed). The suggestion is "+" = Add media (file, folder, network stream) and History = recently played list. *("+" opens a menu: Add Folder to Library… (added as a video folder, then shown), Open File(s)… and Open Folder…. S4.11 added Open Network Stream…. History is a section listing Open Recent's media; clicking one plays it.)*

**Edge cases**
- The current folder is deleted or its drive unplugged while viewing: show "This folder is no longer available" with Back. Don't crash or show a silently empty list. *(Done when the folder is shown, and again whenever the indexer reports a change. Nothing watches the shown folder itself.)*
- Navigating back to a route that no longer exists (a deleted playlist): skip it in history. *(Done: skipped and forgotten. A playlist deleted while it shows says "This playlist no longer exists.")*
- Deep paths (10+ levels): the breadcrumb collapses and the window title stays short. *(Done: two segments show after `…`, and each one is cut with an ellipsis. The window title names the media, not the route.)*
- The breadcrumb for a file opened from outside any root (Downloads): show its real folder path and mark it "Not in library". *(Done for a folder route outside every media folder: the crumbs are its real path, and the count row says "Not in library". The folder chip doesn't open the library yet, S4.10.)*

---

### S4.6 — Videos and Music browsing views

**Depends on:** S4.5, S4.3, S4.4  **Size:** L  **Design refs:** `sidebar-videos*.png`, `sidebar-default.png`, `sidebar-music*.png`, `media_cards/*`

**Subtasks**
- [x] S4.6.1 Videos: a folder tree under each root, with a "+ Add" row at each folder level (add a file or folder to the library). **The design shows no "+ Add" in a leaf folder of files (`sidebar-default`); confirm.** *(`library/folder_view.rs`: the disk says what a folder holds, so unindexed folders and folders outside the library still list; the database adds each file's media, kind and history. Videos lists video and Songs audio; a file not probed yet goes by its extension (edge case decided; no "Show all files" toggle yet). "+ Add" follows the designs: the media folders' list always has it, a folder only when it has subfolders. It asks for a folder and adds it to the video (or music) folders: the library has no single-file roots. Shown in `main_window/library_panel/listing.rs`.)*
- [x] S4.6.2 Music: the categories Songs, Albums, Artists, Genres, each with its own list. Album → tracks are ordered by disc and track number. *(Songs browses the music folders like Videos, listing audio. Albums, Artists and Genres (not designed past `sidebar-music`) list their names as rows, then one name's songs as cards titled by `display::title`: `library/music.rs` over `db/songs.rs`, which reads every audio media with a file, once each. Names compare ignoring case, sort naturally with "Unknown …" last; a tag naming several artists lists the song under each (`display::ARTIST_SEPARATORS`, not configurable yet). An album is its name, so same-named albums by different artists merge. Album tracks: disc (untagged = 1), then track (untagged last), then title; an artist's songs go by album first, a genre's by artist then album.)*
- [x] S4.6.3 List row (media card): 64×64 thumbnail, file name, watched styling (dim text and an orange progress bar along the bottom of the thumbnail), hover (brighter text and a `•••` button), and a hover play overlay (`player_04`). *(`main_window/library_panel/card.rs`, per D9: dim = watched only, the bar is the real progress and full width when watched, none without a length. `•••` and right-click open the file menu (S4.8.1). Thumbnails come from S4.4's service for the cards on screen.)*
- [x] S4.6.4 Grid view: tiles with the thumbnail and the name below, and the same watched and hover states. **Grid isn't designed in the sidebar overlay, only in full library mode; confirm whether the overlay supports Grid.** *(Confirmed: the overlay shows two 16:9 tiles to a line (`main_window/library_panel/grid.rs`), each line one item of the listing's `uniform_list`. A card's tile has the list's watched bar, hover play button and `•••`, which sits on the picture's top-right corner so the name keeps the tile's width; rows (folders, category names, "+ Add") become their icon on a plain tile. Selection, the card menu and opening work as in the list; Up and Down jump a line. The grid pictures (320×180) are drawn scaled down. Full library mode will fit more columns (S4.10.2).)*
- [x] S4.6.5 Double-click or Enter plays the item, and its siblings become the queue (folder playback, S3). Single click selects. Arrow keys move the selection. *(`main_window/library_panel/browse.rs`: the listing is one tab stop in a `uniform_list`; Home, End and the page keys move too. The siblings are the folder's listed cards, in their order. Folder rows still open on one click, as in S4.5. The selection is kept by media ID (`selection.rs`), so a rename or reload keeps it.)*
- [x] S4.6.6 Multi-select (Shift- and Ctrl/Cmd-click) for batch actions (Add to Queue or Playlist, Mark as Watched). *(`main_window/library_panel/selection.rs`, as in the queue panel: Ctrl/Cmd+click adds or removes, Shift+click and Shift+arrows select a range from the anchor, Ctrl/Cmd+A selects all. The card menu (`•••` or right-click) acts on every selected card in list order: Play, Play Next, Add to Queue, Add to Playlist ▸ (or New Playlist…), Mark as Watched/Unwatched. On a card outside the selection it selects that card first. The listing is a control, so the player's arrow bindings (volume) skip it; it handles Alt+Left and Right itself. Headless tests: `library_panel/tests.rs`.)*
- [x] S4.6.7 Drag items out of the library onto the queue, onto a playlist, or into another app (export the file path). *(`main_window/library_panel/drag.rs`: a card (list or grid) drags its file, or every selected card's when it is selected, fixed when the drag starts. On the queue the files go where they land, as Finder drops do; on a playlist's row they join that playlist (with the usual toast). Holding the drag over a rail icon shows that section, so Playlists is reachable mid-drag. Leaving the window hands the files to the OS (the fork's `external_drag_payload`): checked on macOS by dragging an episode into a Finder window, which copied it. Folder rows don't drag, and a drop on the video does nothing (dropping Finder files there still opens them). Headless tests drop cards on the queue and switch section from the rail.)*

**Edge cases**
- A folder containing both video and audio: decide whether Videos shows audio files (the recommendation is that each section shows its own kind, and "Show all files" is a toggle).
- A folder with 10,000 files: the virtualized list (S0.4.11) scrolls at 60 fps, and thumbnails load lazily.
- A file renamed while selected: keep the selection by media ID.
- Natural sort: `E2` comes before `E10`, and that holds in every language.
- A progress bar on media of unknown duration: no bar.
- The watched bar in the design covers about 86% of the thumbnail (55/64 px). Decide whether "watched" draws a full-width bar or the real progress.
- Text brightness: the designs dim watched items **and** also folders, songs and Continue-Watching entries, while light mode reverses the emphasis. Write down one rule (proposed: dim = watched only; folders use normal text) and get design sign-off.

---

### S4.7 — Sort, view and search

**Depends on:** S4.6  **Size:** M  **Design refs:** `sorting_options.png`, `view_options.png`

**Subtasks**
- [x] S4.7.1 The 16 sort options: Unwatched First, Watched First, Newest First, Oldest First, Title A-Z, Title Z-A, Artist A-Z, Artist Z-A, Album A-Z, Album Z-A, Track Number Ascending/Descending, URI Ascending/Descending, Disc Number Ascending/Descending. Show only the options that apply to the current view (no Artist sorts in a video folder) — **confirm with design**. *(`navigation/sort.rs` says what each place offers, `library/sort.rs` how cards compare; the header's Sort opens `library_panel/sort_menu.rs`. Only card listings sort: a Videos or Songs folder and one album, artist or genre; elsewhere Sort is disabled. A video folder offers the watch, date, Title and URI sorts; an album hides the Album sorts and an artist the Artist sorts. Title is what the card says (the file name in a folder). Artist and Album continue by album, disc and track; Disc by track; unwatched → in progress → watched. Title Z-A and URI Descending also turn the subfolders around; other sorts keep them in natural order. Newest and Oldest explain themselves on hover (date added; media from before `added_ms` existed count as unknown).)*
- [x] S4.7.2 The default sort per view, and the orange dot when the active sort isn't the default. Sort is remembered per folder (or per section; decide). *(Per place (D10), in `local.library_sorts` by the route's stored text, and only while it isn't the default. Defaults keep S4.6's orders: folders Title A-Z, an album Disc Number Ascending, an artist Album A-Z, a genre Artist A-Z.)*
- [x] S4.7.3 Secondary sort is always natural file name, so results are stable. *(Then the full path. Unknown values (no artist, no track number, never added) go last in both directions.)*
- [x] S4.7.4 View toggle: Grid or List, remembered per section. *(The header's View button shows the current view's icon and opens the `view_options` menu (`library_panel/view_menu.rs`). `local.library_grid` names the sections shown as a grid (`navigation/view.rs`); List is the default. Menu icons are accent-coloured like every menu's, where the design draws them grey.)*
- [x] S4.7.5 Search: the scope is the current view by default with a toggle for "Everywhere"; incremental, debounced (about 150 ms), diacritic- and case-insensitive, matching title, file name, artist and album. Use SQLite FTS5. *(Not designed: the header's Search opens a row under the count with the field and an Everywhere toggle (`library_panel/search_field.rs`). The scope is the section (Videos or Music) the search began in; elsewhere it looks everywhere. Index: migration 7's FTS5 table `search` (`db/search.rs`), one row per file holding its name, title, artist, album artist and album, kept current by triggers on `files` and `media`. The trigram tokenizer with `remove_diacritics` matches inside words, so CJK text without spaces works; words under three characters are checked in Rust against each hit, folded the same way (NFD, accents dropped, lowercase; `library/search.rs`). Every word must match. A media in several places is found once; results go by title, at most 1,000. Refining the query replaces the search route instead of stacking one per keystroke; emptying the field goes back; Escape closes the field. Results don't sort (Sort is disabled there).)*
- [x] S4.7.6 Empty states: "No results for 'xyz'", "This folder is empty", "Nothing watched yet" (not designed, so design them). *(`library_panel/empty.rs`: a muted 32 px icon, a title, a muted hint, centred under the header. A search says "No results for “…”"; one limited to a section also offers Search Everywhere. A Videos or Songs folder says "This folder is empty" (no media of its kind, no subfolders); History "Nothing watched yet"; Albums, Artists and Genres "No music yet"; Playlists "No playlists yet"; anything else "Nothing here yet". Home's own empty state comes with S4.9.)*

**Edge cases**
- "Newest First" means date added to the library, not file mtime (state it in a tooltip). Files with future mtimes (a wrong camera clock) don't distort it.
- Sorting by Artist with missing artists: "Unknown" items go last in both directions.
- A search query of only whitespace or punctuation, or one that's very long: trim it and cap its length.
- Right-to-left queries, CJK queries without spaces, and emoji: FTS5's tokenizer must support them (use a trigram or ICU tokenizer for CJK).
- Typing fast while results load: show only the latest query's results (cancel stale queries).

---

### S4.8 — Context menus (file, folder, playlist) and their actions

**Depends on:** S4.6, Stage 3, S0.4.7  **Size:** L  **Design refs:** `file_context_menu.png`, `folder_context_menu.png`, `playlist_context_menu.png`

**Subtasks**
- [x] S4.8.1 **File** menu: Play · Mark as Watched (toggles to "Mark as Unwatched") · Favorite (toggles) · Stream (S8.12) · Convert (S8.10) · Add to Queue · Add to Playlist (a playlist picker with "New playlist…") · Show in File Explorer (shown as "Show in Finder" on macOS, "Show in Files" on Linux) · Go to Source (jump to the item's folder in the library) · Add Subtitle Tracks (S6.4) · Add Audio Tracks (external audio file) · Properties & Metadata. *(`main_window/library_panel/file_menu.rs`, replacing S4.6.3's card menu, so Play Next is gone as in the design. Like the main menu, it leaves out what hasn't shipped: Stream, Convert and Add Subtitle Tracks join with S8.12, S8.10 and S6.4. Favorite is a check item kept in the `library::favorites` global over the `favorites` table, disabled for a file the library hasn't identified. Go to Source opens the file's folder in Videos or Songs (the one matching its kind first) and selects it; outside every media folder it shows the file in the OS file manager. Add Audio Tracks picks media files and adds them to the file as libVLC input slaves (`Command::AddAudio`, carried over a switch to software decoding like subtitles), playing the file first if it isn't; the first added track plays, and J and K (VLC's keys, new `AudioDelayEarlier`/`AudioDelayLater`) move the audio by 50 ms with an OSD readout.)*
- [x] S4.8.2 **Folder** menu: Add File · Add Folder · Edit Subtitle Tracks (S6.4 "To This Folder") · Edit Audio Tracks · Resync · Info. *(`folder_menu.rs` on folder rows in Videos and Songs. Add File and Add Folder copy what is picked into the folder (`library/copy_in.rs`: never overwriting, a taken name gets " 2", a folder can't go into itself) for the indexer to find. Info lists the name, location, subfolders, media files and their size. Edit Subtitle Tracks waits for S6.4, and Edit Audio Tracks for a design of which audio goes with which video, so both are left out.)*
- [x] S4.8.3 **Playlist** menu: Play · Favorite · Shuffle (play shuffled) · Export Playlist (S3.6) · Add to Queue · Add Subtitle Tracks (S6.4 "To This Playlist") · Add Audio Tracks · Properties & Metadata. *(`playlist_menu.rs` on the Playlists section's rows. Several playlists play one after another. Shuffle replaces the queue, turns Shuffle on and starts at a random item. Properties & Metadata shows the title, item count and total length ("At least …" while some lengths are unknown). Add Subtitle Tracks (S6.4) and Add Audio Tracks (no design for which audio goes with which item) are left out.)*
- [x] S4.8.4 Open menus with right-click, the `•••` hover button, the context-menu key and Shift+F10. Multi-selection applies the action to all selected items, and actions that make no sense for many items are disabled. *(`item_menu.rs`. Cards, folder rows and playlist rows have a menu, in the list and the grid. A menu acts on the selected items of its own kind; on an unselected item it selects that one first. The keys open the keyboard item's menu under it with the first action highlighted (after Select All, the first selected item's). Show in Finder, Go to Source, Add Audio Tracks, Properties, Add File, Add Folder, Info and Export Playlist are disabled for several items, saying "Works on one item at a time".)*
- [x] S4.8.5 Favorites: where they show up isn't designed (no Favorites section). Propose a Favorites row on Home. *(Proposal for S4.9: a "Favorites" row on Home, after Continue Watching, of favorite media and playlists, newest favorite first (the `favorites` table's `added_ms`), hidden while empty. Clear History leaves favorites alone, and a deleted playlist's favorite goes with it.)*

**Edge cases**
- "Show in File Explorer" for a file on an offline drive or a network share: show an error toast. For a file inside an archive or a URL: disable the item.
- "Go to Source" for a file outside every root: open its folder in the OS file manager or offer "Add folder to library".
- "Add Audio Tracks" with an audio file of a different duration or frame rate: allow it, and expose audio delay to fix sync.
- Actions on items that disappear while the menu is open (deleted on disk): run the action and show "File not found".
- "Mark as Watched" on a folder or playlist: apply it to all children? It isn't in the folder menu today, so don't add it without design.

---

### S4.9 — Home page

**Depends on:** S4.6, S1.7  **Size:** M  **Design refs:** `sidebar-home.png`, `player_07.png`

**Subtasks**
- [x] S4.9.1 Sections: **Continue Watching** (in-progress videos, most recent first, each with a progress sliver), **Listen Again** (recently played audio with artwork), **Frequently Visited Folders**, **Frequently Visited Playlists**, **Frequently Listened Artists**. Each section has a `>` that opens the full list. *(`library/home.rs` reads every section on the history thread (`db/played.rs` for the media rows, `db/activity.rs` for visits and artists), up to 100 items each; `main_window/library_panel/home.rs` shows the first three of each under its heading in the sidebar (`home/page.rs`). Home's items are the listing's own, media first and then places, so they select, open, drag and have their menus as anywhere else; a media opened on Home plays alone. `>` goes to the new `Route::HomeSection` (stored as `home:continue-watching` and so on), which lists the section in full through the ordinary listing. Listen Again says "title - artist". The Favorites section of S4.8.5 sits after Listen Again rather than after Continue Watching, so the page's order (media, then playlists) stays the keyboard's order. Visits to library folders and saved playlists are counted on arrival. Home is a list in the sidebar, so its View button is disabled there; full library mode shows `player_07`'s lines of tiles (S4.10.2).)*
- [x] S4.9.2 Hide sections with no data. Show a welcoming empty Home on first run (not designed) that links to "Open file", "Add folder" and the indexing progress. *(`home/welcome.rs`: "Welcome to VLC", Open File… and Add Folder… buttons, and each media folder's status and scan progress. Each section's full list has its own empty state.)*
- [x] S4.9.3 Continue Watching removes an item when it becomes watched and supports "Remove from Continue Watching" without changing the item's progress. *(Migration 8 adds `media.hidden_from_home_ms`; the file menu offers "Remove from Continue Watching" for cards on it, and the item returns once played again. Watched media, and positions the watched rule now counts as watched, drop off at the next read.)*

**Edge cases**
- Every Continue Watching item is offline (drive unplugged): show them greyed, or hide them with a "3 items unavailable" note. *(Hidden, with the note under the section.)*
- Privacy "Save recently played items" off: Continue Watching, Listen Again and the frequency sections are hidden or empty, with an explanation. *(Only Favorites is read, under a note saying why; with no favorites either, Home says so instead of the welcome. Visits aren't counted.)*
- The "Frequently visited" ranking must decay over time (for example exponential decay), so a folder visited 100 times last year doesn't stay on top forever. *(`library/frequency.rs`: scores halve every 30 days. Migration 8 adds `visits.score`, raised on each visit; artists sum their plays the same way.)*
- Mock data inconsistency to ignore: Home shows E05–E07 in progress while the S01 list shows them unwatched.

---

### S4.10 — Sidebar overlay in the player and full library mode with the docked miniplayer

**Depends on:** S4.5–S4.9, Stage 2  **Size:** L  **Design refs:** `player_02`–`player_07`, `player_light`, `player_music_02`, `docs/ui/player/miniplayer/*`

**Subtasks**
- [x] S4.10.1 The title-bar sidebar toggle opens the rail and panel **over** the video (the video doesn't move, as designed). The toggle shows an active state. **L** (from onboarding: "Show / Hide File Browser") toggles it. *(With S4.5: the toggle and L show the sidebar between the bars, the toggle lights while it shows, and the sidebar gets the keyboard. It hides with the controls, in a window and in fullscreen, and the pointer over it or one of its menus holds them up (`library_panel/mode.rs`, `auto_hide.rs`). Below 854 px it takes the whole window (`library_panel/layout.rs`). The folder chip opens it at the playing file's folder.)*
- [x] S4.10.2 The panel's pop-out icon switches to **full library mode**: labeled nav, grid or list, and search field. In this mode the title bar shows the playing item's title and no control bar. *(`library_panel/full.rs`: the labeled navigation, one header row with a search box that opens the search field in place, and the listing laid out for the window's width: 316 px tiles and 400 px list columns at 1920 px (`layout.rs`). `auto_hide::rules` gives full library mode the title bar and no control bar, and nothing hides. The route carries over from the sidebar and is remembered in its own slot. The title bar keeps the lit sidebar toggle, which with L goes back to the player (the designs show no way back with nothing playing); the folder chip hides, as designed. Home shows each section as one line of tiles (`player_07`): videos as the grid's 16:9 tiles, songs (with their artist) and places as squares, as many as a line of video tiles holds (`home/page.rs`); the keyboard still steps through Home one item at a time.)*
- [x] S4.10.3 Docked miniplayer card in a corner of full library mode: video frame, a progress bar, file name, folder and artist chips, and a queue toggle. Hover shows ✕ (stop and close), pop-out (back to the full player), and previous, play/pause and next. When the media ends it shows Replay (variant5). *(`main_window/miniplayer.rs`, `miniplayer/card.rs`: the video in the card, a progress bar that seeks on click, the name, a folder chip (to the folder in the library) and an artist chip when the media has an artist tag. Not yet checked with media playing in the running app.)*
- [x] S4.10.4 The miniplayer queue expands above the card: "N items – total duration", sort, "−" and "+", and rows with drag handles and durations (variant2 and variant4). *(`queue_panel/docked.rs` docks the List panel's Queue tab, so it also has Repeat and Shuffle. Q toggles it in full library mode. Docked in a top corner, it opens below the card.)*
- [x] S4.10.5 Docking corner: drag the card to snap it to any corner (the designs show bottom-right and bottom-left), remembered across launches. *(`miniplayer/dock.rs`: it docks in the quarter it is let go in, saved as `local.miniplayer_corner`. A drag starts after 4 px, so its buttons still click.)*
- [x] S4.10.6 Clicking a library item while something plays: replace the current item or add it to the queue, following Settings → Library "Add to the playlist instead of replacing it". *(`prefs.library.add_instead_of_replacing`, off by default; on, a card or a History row goes to the queue's end while something plays. Settings → Library itself is S5.9.)*

**Edge cases**
- The overlay is open and the user presses Space: playback toggles only when the panel doesn't have focus on a text field (S0.5 contexts). Arrow keys navigate the list while it has focus, and seek otherwise.
- The overlay is open in fullscreen: it auto-hides with the controls, or stays pinned (decide). Never leave a stuck panel when the UI hides.
- The window is narrow: the overlay panel takes the full width, and the miniplayer card shrinks or hides below a minimum size.
- The miniplayer covers grid items (in `player_03` it hides the 5th item of row 3): add bottom padding to the scroll content equal to the card height so every item can scroll into view.
- Audio-only media in the miniplayer: show artwork instead of a black frame.
- Nothing is playing: no miniplayer (`player_06`).
- The queue in the miniplayer is long (500 items): the list is virtualized and the header total stays correct. **The mock total "2h 3m" doesn't match the rows (about 1h 35m); compute it, and show "2h 3m+" if some durations are unknown.**
- The window title rule: the playing item's title while something plays, otherwise "VLC Media Player". The mocks are inconsistent (`player_01` shows "VLC Media Player" during playback). Decide, and respect Settings → Interface "Show the file name in the title bar".

---

### S4.11 — Network section and the Open Network Stream dialog

**Depends on:** S4.5, S1.2, S3.5  **Size:** M  **Design refs:** `sidebar-network.png`, `docs/ui/dialogs/open_network_stream/*`, main menu "Open Network Stream Ctrl+N"

**Subtasks**
- [x] S4.11.1 The dialog: **Network URL** is a multi-line field ("You can enter multiple links to load."), one URL per line. **Playlist Name (optional)**. Cancel and Save, with Save disabled until at least one valid URL is entered. *(`main_window/network_stream.rs`, matched to the design by a screenshot test. `TextInput::with_rows` is the kit's multi-line mode: Enter starts a line, Up and Down move by line, Home and End go to the line's ends, the wheel scrolls, and Cmd+Enter (Ctrl+Enter) submits, as Enter does in the name field. Notes under the field say what is left out: entries that aren't media links, repeats, and links with a password. Opened from Ctrl+N, the main menu, the rail's "+" menu and Network's "+ Add Network Stream".)*
- [x] S4.11.2 Save without a name: play the URL(s) now and add them to Network → Recent streams. Save with a name: create a saved playlist (S3) shown under Network. *(Recent Streams keeps the latest 50 in `network_streams` (`db/network.rs`, the `network::Streams` global), follows "Save recently played items", and Clear History empties it. A named Save only saves; Network lists every saved playlist whose items are all links (`Summary::streams_only`), and Playlists lists it too.)*
- [x] S4.11.3 The Network root lists "+ Add Network Stream" and categories (Podcasts, Radio in the design). **Decision:** fixed built-in categories versus user-created ones. Podcasts implies RSS feed parsing (S8.17). *(Decided: built in (`NetworkCategory`): Recent Streams (shown once it has streams), Podcasts and Radio; playlists already organize streams freely. A stream row's menu (`stream_menu.rs`, not designed) has Play, Add to Queue, Add to Radio, Copy Link and Remove. Podcasts stays empty until S8.17 subscribes to feeds.)*
- [x] S4.11.4 Validation: accept http, https, rtsp, rtmp, mms, udp, rtp, smb, ftp, sftp, file, and magnet (if supported); reject `javascript:`, `data:` and other non-media schemes. *(`network/links.rs`, also rtsps, rtmps, mmsh and ftps. Magnet links are refused: libVLC doesn't play torrents. Pasted links are split on spaces and lines, quotes, angle brackets and trailing commas are dropped, and repeats (scheme and host ignoring case) are left out. Links with a user name or password are masked on screen and never stored, in Recent Streams or a playlist, since the keychain isn't wired up; Open Recent (S1.3.5) leaves them out too.)*

**Edge cases**
- URLs pasted with whitespace, surrounding quotes, trailing commas, or several URLs on one line separated by spaces: normalize them.
- Duplicate URLs: de-duplicate and show a note.
- URLs with credentials (`https://user:pass@host/…`): store the credentials in the keychain, not in the DB, and mask them in the UI.
- An HTTP URL that returns HTML (a webpage, not a stream): detect it and show "This link is a web page, not a media stream". Don't play HTML as audio.
- An HLS stream with only some variants available, or one that changes resolution: adaptive switching follows Playback → "Preferred video quality".
- An expired or rotating URL (a signed CDN link): it fails on replay, so show a clear error and an "Edit URL" action.
- Captive portal or offline: a "No network connection" error, with retry.
- Self-signed certificates: a clear error, governed by Settings → Network certificate verification (S5.10.1).
- A live stream has no duration: the seek bar is hidden or limited to the buffer, and no resume (S1.7).
- An internet radio stream's ICY metadata updates the "now playing" title live.

---

## Stage 5 — Settings

> **Goal:** every choice made in onboarding can be changed later, and every behavior earlier stages shipped with hard-coded defaults becomes configurable.
> **Exit criteria:** all 13 categories render from one settings registry; search finds every setting including Advanced ones; changes apply live (or show a clear "applies to the next file" / "restart required" note); the keyboard-shortcut editor detects conflicts.
> **Rule from here on:** a feature story in a later stage isn't done until its settings rows exist in this window.

Design facts the stories rely on (from `docs/ui/settings/**`):
- Layout: a search field at the top left; a sidebar with three groups (**App**: General, Interface, Privacy, Controls · **Media**: Playback, Video, Audio, Subtitles, Library · **System**: Network, Performance, Recording, Extensions); a page title and a close ✕ on the right; label/control rows with an `(i)` info icon; a collapsible **Advanced Options** bar at the bottom of a page.
- There's no Save or Apply button, so changes apply immediately. The Keyboard Shortcuts dialog is the exception: it has **Cancel** and **Save** (Save is disabled until something changes).
- The sidebar label and the page title differ (General → "Global Settings", Library → "Playlists, Folders & Files", and so on).

### S5.1 — Settings registry (one declarative list drives UI, search, persistence and validation)

**Depends on:** S0.1, S0.4  **Size:** M

**Subtasks**
- [ ] S5.1.1 Each entry in a `SettingDef` table has: key, category, section, label, info text, control kind (toggle, dropdown, segmented, slider with ticks and labels, number with unit, text, path picker, hex color, swatches, button, list), default, range and step, validation, **dependency** (enabled when another setting has some value), **apply mode** (`Live`, `NextMedia` or `Restart`), `advanced: bool`, and search keywords.
- [ ] S5.1.2 Each page renders automatically from the registry, with hand-built pages only where the design needs them (equalizer, presets, extensions list).
- [ ] S5.1.3 A dependent row is dimmed and disabled while its parent is off. The designs show this for Library "Apply to files opened from the file manager" and Network buffering. Apply the same rule consistently to Normalize loudness → Target level, Equalizer → preset and bands, Spatial audio → mode, Save a log → Log file and Log details, custom skin → Skin file, Web remote → Port, last.fm → username, password and URL.
- [ ] S5.1.4 Each `(i)` icon shows a tooltip with the info text. The designs have no tooltip copy, so write it (content task).
- [ ] S5.1.5 Settings marked `Restart` show a "Restart VLC to apply" banner with a **Restart now** button. `NextMedia` settings show "Applies to the next file you open".

**Edge cases**
- A setting's dependency parent is itself disabled (chains): the child stays disabled.
- A value from an older or newer schema is outside the new range: clamp it and log.
- Typing in numeric fields: allow intermediate states ("-", "", "1.") while editing, validate on blur or Enter, revert on Escape, and never save a half-typed value.

---

### S5.2 — Settings window shell, navigation and search

**Depends on:** S5.1  **Size:** M  **Design refs:** `docs/ui/settings/screens/*`, `docs/ui/settings/components/setting_category-*`

**Subtasks**
- [ ] S5.2.1 Open Settings from the main menu, the gear icon, Ctrl/Cmd+`,` and the macOS app menu "Settings…". **Decision:** a separate OS window or an in-app modal (the designs are ambiguous). The recommendation is a separate window, so it can sit beside the player while tuning video.
- [ ] S5.2.2 Sidebar groups and categories with selected and unselected states; the last-opened category is remembered.
- [ ] S5.2.3 Search filters across every category, matching label, info text and keywords. It auto-expands **Advanced Options** when a match is inside, highlights matches, and shows a "No settings match "xyz"" empty state (not designed, so design it).
- [ ] S5.2.4 Advanced Options collapse/expand per page, remembered per session.
- [ ] S5.2.5 Keyboard: Ctrl/Cmd+F focuses search, arrows move between categories, Escape clears search, then a second Escape closes the window.

**Edge cases**
- Settings is opened from two places at once: a single window is focused, never two.
- A setting is changed elsewhere while Settings is open (the volume popover, the theme from the menu, or a second device syncing a file): the visible control updates live.
- The window is resized very narrow: the sidebar collapses to icons, and the label column wraps.
- The interface language changes while Settings is open: the window re-renders in place with the same category selected.

---

### S5.3 — General ("Global Settings"): appearance, startup, diagnostics

**Depends on:** S5.2, S0.3, S0.7  **Size:** M  **Design refs:** `global_01.png`, `global_02.png`

**Subtasks**
- [ ] S5.3.1 **Theme** (Dark | Light | System) and **Accent color** (9 swatches) edit the same settings onboarding wrote, live, in every window. **Decision:** the order differs between screens (onboarding is Light/Dark/System, Settings is Dark/Light/System). Pick one.
- [ ] S5.3.2 **Interface language** dropdown (depends on S9.2 i18n). A change re-renders live where possible, otherwise it's `Restart`.
- [ ] S5.3.3 Startup: **Show confirmation and error dialogs**, **Check for updates automatically** (S9.5).
- [ ] S5.3.4 Diagnostics: **Developer mode**, **Save a log**, **Log file** (path picker), **Log details** (Errors only, …). Advanced: **Show minor warnings**, **Additional command line options**, **Log format**, **Collect playback statistics**.
- [ ] S5.3.5 Add **Run setup again** (re-opens onboarding pre-filled with the current values; Cancel returns without changes).

**Edge cases**
- The log file path points to a folder that doesn't exist, a read-only location, a network share that disconnects, or a file another program holds open: validate on change and fall back to the default log location with a warning.
- "Additional command line options" holds invalid flags: validate them against the backend's known options and show the parsing error inline. Never pass them to a shell.
- Developer mode exposes unstable features: label them, and ignore them in crash statistics.

---

### S5.4 — Interface

**Depends on:** S5.2, Stage 2  **Size:** M  **Design refs:** `interface_01.png`

**Subtasks**
- [ ] S5.4.1 Window: **Play video inside main window**, **Resize window to fit video** (Never, …), **Remember window size and position**, **Keep window above other windows**, **Show window decorations**, **Show the file name in the title bar**, **Open fullscreen on** (Current screen, or a specific display).
- [ ] S5.4.2 Playback controls: **Start in compact mode** (the miniplayer, S8), **Auto-hide the controls** (In fullscreen only, …), **Hide after** (0–10 s in 0.5 s steps), **Show controls in fullscreen**.
- [ ] S5.4.3 Notifications and system tray: rows are cut off in the design, so define them (now-playing notification, minimize to tray, tray icon menu).
- [ ] S5.4.4 Skin: **Use a custom skin** and **Skin file**. A skinning system is out of scope until it's defined; ship the rows hidden behind a feature flag (see S8.14).

**Edge cases**
- "Keep window above other windows": GPUI has no always-on-top API for normal windows, so it needs a per-platform call. Combined with fullscreen, it must not trap the user above system dialogs.
- "Open fullscreen on" names a display that's no longer connected: fall back to the current screen.
- "Resize window to fit video" with 8K or portrait video: cap the size to the work area, and never resize while fullscreen or maximized.
- "Hide after = 0 s" means the controls never show on hover. Either disallow 0 or document it. The mock's thumb sits at 0.5 s, which is suspiciously short; confirm the default (3 s is the usual choice).
- "Show window decorations" off on Windows: keep resize borders and snap layouts working.

---

### S5.5 — Privacy

**Depends on:** S5.2, S1.7  **Size:** S  **Design refs:** `privacy.png`

**Subtasks**
- [ ] S5.5.1 History: **Save recently played items**, **Remember the playback position**, **Never remember items matching** (patterns), and **Clear history now** (with a confirmation that shows counts).
- [ ] S5.5.2 Network: **Fetch titles and cover art from the internet** (off by default), **Submit played tracks to last.fm**, plus username, password and submission URL. The second "Last.fm username" label in the mock is a typo for password.
- [ ] S5.5.3 Store the last.fm password or session key in the OS keychain (Windows Credential Manager, macOS Keychain, Secret Service on Linux), never in `settings.toml`.

**Edge cases**
- The "Never remember" pattern is invalid (a bad glob or regex): show the error inline and don't apply a partial pattern. Decide glob vs regex (glob is friendlier).
- Turning off "Remember the playback position" with existing positions: offer to delete them.
- Overlap with Playback → "Continue where you left off": if remembering positions is off, that dropdown is disabled with an explanation. One source of truth.
- Overlap with Library → "Fetch titles and artwork": the Privacy toggle is the master switch; Library's dropdown only chooses the source when it's allowed.
- No keychain available (a Linux session without Secret Service): disable saving the password and explain why. Never fall back to plaintext silently.
- last.fm offline, or the credentials expire: queue submissions locally (capped) and show the error once.

---

### S5.6 — Controls, mouse and the Keyboard Shortcuts editor

**Depends on:** S5.2, S0.5  **Size:** L  **Design refs:** `controls.png`, `docs/ui/dialogs/keyboard_shortcuts/keyboard_shortcuts.png`

**Subtasks**
- [ ] S5.6.1 **Skip amounts**: Very short 3 s, Short 10 s, Medium 60 s, Long 300 s, bound to the seek actions with modifiers (for example Shift, Ctrl and Alt with the arrow keys). Document which modifier maps to which amount.
- [ ] S5.6.2 **Mouse**: single click on the video (Play/Pause), double click (Toggle fullscreen), scroll wheel (Volume), horizontal scroll (Seek), **Reverse scroll direction**.
- [ ] S5.6.3 Keyboard Shortcuts dialog: search ("Search Keyboard Shortcuts…") and a field filter ("All Fields"); columns **Action | Hotkey | Global Hotkey**; a pencil to edit each cell; grouped sections (Basic, Playback, …); Cancel and Save.
- [ ] S5.6.4 Recording a key: press the combination to capture it, Escape cancels capture, Backspace or Delete clears the binding.
- [ ] S5.6.5 Conflict detection: when a combination is already bound in the same context, show "Already used by *Stop playback*" with **Replace** or **Cancel**.
- [ ] S5.6.6 **Reset all shortcuts** (with a confirmation), plus a per-row reset.
- [ ] S5.6.7 **Global hotkeys** (they work while the app isn't focused): Windows `RegisterHotKey`, macOS a Carbon or event tap (needs Accessibility permission), Linux X11 grabs. Wayland has no general global hotkeys, so use the XDG GlobalShortcuts portal or show "not supported".
- [ ] S5.6.8 **Remotes**: Web remote control and Port, implemented in S8.15.

**Decision to resolve before S0.5 ships:** onboarding teaches `<`/`>` for Previous/Next item, but the shortcuts dialog design shows **N/P** for Next/Previous track (plus S Stop, T Show time, I Show controls in fullscreen, X Next program). Pick one default set and make onboarding step 3 read it from the keymap (S0.5.3).

**Edge cases**
- Binding a key the OS reserves (Win+L, Cmd+Q, Cmd+Tab, Alt+F4, PrintScreen): refuse it with an explanation.
- A global hotkey is already registered by another app: registration fails, so show "In use by another application" in that cell.
- The keyboard layout changes after binding: show bindings in the current layout's labels.
- Modifier-only bindings (just Shift) and Fn combinations aren't allowed.
- Cancel after many edits discards everything. Closing the dialog with ✕ while there are unsaved edits asks to discard them.
- An imported or hand-edited keymap with duplicate or invalid entries: load the valid ones and report the rest.
- The scroll wheel set to "Volume" on a trackpad with high-resolution and inertial scrolling: accumulate deltas into steps so one flick doesn't jump 0 → 200%.
- A single click set to Play/Pause together with double click set to fullscreen: a single click must wait for the double-click interval or play/pause toggles twice. Common approach: toggle immediately, and undo the toggle if a double click arrives.

---

### S5.7 — Playback preferences

**Depends on:** S5.2, S1.7, Stage 3  **Size:** S  **Design refs:** `playback_01.png`

**Subtasks**
- [ ] S5.7.1 Startup: **Continue where you left off** (Always, Ask, Never), **Start playing as soon as a file opens**, **Default playback speed** (0.25–2 in 0.25 steps), **Pause when another app uses audio**.
- [ ] S5.7.2 Tracks: **Preferred video quality** (for adaptive streams), **Preferred menu language** (DVD and Blu-ray menus), **Default audio track**, **Default subtitle track**.
- [ ] S5.7.3 Order: **Repeat** (Off, One, All), **Shuffle**, **Stop after each item**, **When playback finishes** (Stop, Close, Quit, Sleep or shut down the computer?). These must be the same state as the queue's own repeat and shuffle buttons (Stage 3).
- [ ] S5.7.4 Seeking: **Seek precisely** (exact frame seeking versus faster keyframe seeking).

**Edge cases**
- The mock shows Shuffle ON and Stop after each item ON. Confirm the real defaults (both are normally OFF).
- "Pause when another app uses audio" (audio focus): Windows has communications ducking only, macOS has no general API, Linux depends on PulseAudio or PipeWire roles. Define what triggers it per OS, or ship it only where detectable.
- "When playback finishes → Shut down the computer", if offered: show a 60 s cancellable countdown and never do it without one.
- Default audio or subtitle track given as an index ("1") is fragile. Prefer language-based selection and use the index only as a fallback; an index past the track count is ignored.

---

### S5.8 — Video, Audio and Subtitle defaults pages

**Depends on:** S5.2, and Stage 6 and Stage 7 for the effects they configure  **Size:** M  **Design refs:** `video_01.png`, `audio_01.png`, `audio_02.png`, `subtitles_01.png`

**Subtasks**
- [ ] S5.8.1 **Video Defaults**: Output (Enable video, Start videos in fullscreen, Stop the screensaver during playback, Play video as desktop wallpaper), Picture (Hue, Brightness, Contrast, Saturation, Gamma, and Reset picture), Deinterlacing (mode and method), Framing (Aspect ratio, Crop).
- [ ] S5.8.2 **Audio Defaults**: Output (Enable audio, Output device, Speaker setup); Volume (**Maximum volume** 100–300% with ticks every 25%, **Startup volume**: Remember last or Always start at N%, **Volume step**); Loudness (Normalize loudness, Target level, Night mode, Replay gain mode and preamp); Effects (Equalizer on/off, preset, the inline 11-slider panel, Two-pass, Spatial audio and its mode, Stereo widening).
- [ ] S5.8.3 **Subtitle Defaults**: Tracks & Languages (Enable subtitles, Preferred language, Text encoding); Loading Subtitles (Load subtitle files automatically, File name matching); Appearance (Font, Size values Relative or Pixels, Font size, Bold, Text color, Text opacity, Outline thickness, Outline color, Shadow, …).
- [ ] S5.8.4 Define how these pages relate to the in-player panels (Stage 7). The recommendation: these pages are the **defaults for new media**; the in-player panels change the **current session**, and each panel has "Save as default", which writes here.

**Edge cases**
- "Always start at" is greater than "Maximum volume": clamp it to the maximum and show a hint.
- Lowering Maximum volume below the current volume: clamp the live volume immediately.
- The hex color fields accept `#RGB`, `#RRGGBB` and color names, and reject anything else. The swatch opens a color picker (not designed, so design it).
- A font that isn't installed on this machine (settings synced from another machine): fall back to the system sans-serif and show the warning in the row.
- Text encoding "Automatic" guesses wrong (Windows-1255 Hebrew or Windows-1251 Cyrillic shown as mojibake): the encoding can be changed per file from the subtitle panel and is remembered per file.
- "Play video as desktop wallpaper" is Windows-only (behind desktop icons), and macOS/Linux support differs per desktop. Hide it where unsupported.
- "Enable video" off means audio-only mode: video tracks aren't decoded, which saves CPU. Make sure the UI says why the screen is black.

---

### S5.9 — Library settings, including the media folders onboarding picked

**Depends on:** S5.2, Stage 4  **Size:** S  **Design refs:** `library.png`

**Subtasks**
- [ ] S5.9.1 Opening Files: **Only allow one VLC window** (S0.6), and its dependents **Apply to files opened from the file manager** and **Add to the playlist instead of replacing it**; **When opening a folder** (Include subfolders, …); **Show hidden files**.
- [ ] S5.9.2 Metadata & Artwork: **Fetch titles and artwork** (From the file only, …) and **Use the media library**.
- [ ] S5.9.3 **Missing from the design:** a **Media folders** list (add, remove, rescan, show status), because there's currently no way to change the folder picked in onboarding after Finish. Add it at the top of this page, or under Advanced Options, and get design sign-off.
- [ ] S5.9.4 Turning off "Use the media library" hides the library UI and stops indexing, and asks whether to keep the index.

**Edge cases**
- Removing the last media folder: the library shows its empty state, not an error.
- Adding a folder that's inside another folder (nested), or the same folder twice through different paths (a symlink, a mapped drive versus UNC): de-duplicate by canonical path.
- Adding a whole drive root or the home folder: warn about indexing time and exclude system folders.

---

### S5.10 — Network, Performance, Recording and Extensions pages

**Depends on:** S5.2, and the features they configure  **Size:** M  **Design refs:** `network.png`, `performance.png`, `recording.png`, `extensions.png`

**Subtasks**
- [ ] S5.10.1 **Network**: Buffering preset (Normal, …, Custom). Custom enables the per-source fields (Local files, Discs, Network streams, Live capture, in ms). Proxy: HTTP proxy ("System proxy" by default) and proxy password (in the keychain). Security: **Verify certificates against the system trust store**, **Cipher preference**.
- [ ] S5.10.2 **Performance**: Hardware-accelerated decoding, Video post-processing, Skip the H.264 loop filter, Decoding threads (0 = auto), Drop frames when the system can't keep up, Give VLC real-time priority, Process priority, **Rescan plugins**. Advanced "Developer": Encoder, Decoder, Clock synchronisation, Clock jitter, Clock reference averaging.
- [ ] S5.10.3 **Recording, Casting & Streaming Out**: Save recordings to (path), Record without re-encoding when possible; Discover cast devices and Cast quality; Play locally while streaming, include audio, video or subtitles, Encoding preset, Tuning. The features themselves are S8.6, S8.11 and S8.12.
- [ ] S5.10.4 **Add-ons & Scripts**: the add-ons list with search, Find more extensions, Add and Remove; Load add-ons on startup. The extension system itself is S8.14.

**Edge cases**
- Turning off certificate verification: show a red warning and require confirmation. It can never be turned off for update checks.
- A proxy URL without a scheme, with credentials embedded in the URL, or with an unreachable host: validate the format and test the connection with a "Test" button.
- Hardware decoding "Automatic" crashes or shows green frames on buggy drivers: fall back to software for that file and remember the GPU and driver combination.
- "Real-time priority" needs administrator or root rights on some OSes: show the failure instead of silently ignoring it.
- Decoding threads larger than the core count: allow it, but hint.
- The recordings folder is on a full or removable disk (S8.6).

---

### S5.11 — Setting presets ("Default Preset", "Anime", "Loud and then Quiet Movies")

**Depends on:** S5.8  **Size:** M  **Design refs:** `docs/ui/settings/components/setting_preset-*`

**Subtasks**
- [ ] S5.11.1 A preset is a named overlay over the **Media** pages (Playback, Video, Audio, Subtitles). Confirm the scope with design; the up-chevron suggests the dropdown sits at the bottom of the window and opens upward.
- [ ] S5.11.2 The dropdown lists **+ New Preset**, then custom presets each with a `···` menu (Rename, Duplicate, Delete, Export), then **Default Preset** (read-only, with a checkmark when active).
- [ ] S5.11.3 Changing a setting while a custom preset is active updates that preset. Changing a setting while Default is active changes the defaults. Show which preset is being edited.
- [ ] S5.11.4 Import and export presets as files (sharing an "Anime" preset).

**Edge cases**
- Deleting the active preset: switch to Default and confirm.
- Duplicate names: add " (2)". Empty names aren't allowed. Very long names truncate.
- A preset references a setting a newer version removed, or one an older version doesn't know: ignore it and keep it (preserve unknown keys, as in S0.1).
- Relationship to player **Profiles** (`docs/ui/player/menus/profiles/*`, S8.8): decide whether profiles are the same concept as presets (D12). They shouldn't be two systems.

---

## Stage 6 — Subtitles

> **Goal:** subtitles load automatically when they exist, look right, can be synced by ear, and can be added in bulk for a whole season.
> **Exit criteria:** SRT, ASS, VTT and PGS render correctly; the Add Subtitles dialog pairs a 15-episode folder automatically; subtitle delay is adjustable to the millisecond; the subtitle timeline mode works.

### S6.1 — Subtitle detection and loading

**Depends on:** S1.6  **Size:** M  **Design refs:** `controls-subtitles.png`, `panels/subtitles/sub_filename-*`, Settings → Subtitles "Loading Subtitles"

**Subtasks**
- [ ] S6.1.1 Auto-load sidecar files next to the media (and in `Subs/`, `Subtitles/` subfolders) using the **File name matching** levels: Off · Any title · Contains title · **Title with extra characters** (default) · Exact title match.
- [ ] S6.1.2 Formats: SRT, ASS/SSA, WebVTT, MicroDVD SUB, VobSub (IDX+SUB), PGS (SUP), DVB and TTML, as far as the backend supports them.
- [ ] S6.1.3 Guess the language from the file name (`.en.srt`, `_English.srt`, `_EN.srt`) for display and auto-selection.
- [ ] S6.1.4 Add a subtitle file by dropping it on the player, with "+" in the track popover, and from Subtitle ▸ Add file in the context menu.
- [ ] S6.1.5 Subtitle rows: a file icon and the file name, an orange check when selected, and on hover a swap icon. **The swap action isn't defined** (switch to this track? set as secondary subtitle?). Propose: swap = make this the active track.
- [ ] S6.1.6 Text encoding detection (BOM, then a statistical guess) with the Settings → Subtitles "Text encoding" override and a per-file override.

**Edge cases**
- Several matching files (`movie.en.srt`, `movie.he.srt`, `movie.srt`): load them all as tracks and auto-select by preferred language.
- Sidecar files that are huge (a 50 MB "SRT" that's actually something else) or binary: cap the size and validate the format before parsing.
- Malformed SRT (missing blank lines, wrong numbering, `,` versus `.` in timestamps, BOM in the middle of the file, overlapping cues, negative or out-of-order times): parse leniently, sort the cues, skip unparseable ones, and log.
- An ASS file with embedded fonts or ones that reference fonts that aren't installed: use the embedded fonts, else fall back to a default font and keep the layout as close as possible.
- Subtitles for a file on a read-only or network location: load them read-only.
- Sidecar files that appear after playback started (the user downloads them mid-movie): offer a "Load new subtitle file found" toast, driven by the watcher.
- A subtitle file whose duration far exceeds the video (wrong file): load it anyway, and the sync tools are the remedy.

---

### S6.2 — Subtitle rendering and appearance

**Depends on:** S6.1, S1.4  **Size:** M  **Design refs:** `controls-subtitle_settings.png` (Appearance), Settings → Subtitles "Appearance"

**Subtasks**
- [ ] S6.2.1 Font, size (Relative or Pixels; "Automatic"), bold, text color (hex plus swatch), text opacity, outline thickness and color, and shadow. Changes apply live.
- [ ] S6.2.2 **Embedded Sub Format** toggle: on uses the ASS/SSA styles from the file, off forces the user's style.
- [ ] S6.2.3 Subtitles render on the video layer, inside the video rectangle (or in the letterbox bars, as an option), and are scaled with the video resolution.
- [ ] S6.2.4 A color picker for the swatches (not designed; design it).
- [ ] S6.2.5 Subtitles stay visible above the control bar when it's shown (shift them up) instead of being covered.

**Edge cases**
- Right-to-left subtitles (Hebrew, Arabic) with punctuation at the line ends, and mixed RTL/LTR lines: correct bidi rendering.
- CJK, Thai and Devanagari need proper shaping and line breaking. Emoji in subtitles.
- Vertical text and karaoke effects in ASS: as far as libass supports them.
- Very long lines: wrap within the safe area, never outside the video rectangle.
- Overlapping cues at the same time: stack them without overlapping.
- Text color equal to the outline color, or opacity 0%: allowed, but show a hint in the settings ("Subtitles will be invisible").
- Image-based subtitles (PGS, VobSub) ignore font and color settings: disable those controls with a note while an image track is active.
- Resizing the window or entering fullscreen: subtitles re-layout without flicker.

---

### S6.3 — Subtitle synchronization (and the subtitle tab defaults)

**Depends on:** S6.1, S2.7  **Size:** S  **Design refs:** `controls-subtitle_settings.png` (Defaults, Synchronization)

**Subtasks**
- [ ] S6.3.1 Defaults section: **Subtitle Track** and **Subtitle Language** (the preferred default for new media) — distinct from the track picked in the popover (current media). Make that distinction visible.
- [ ] S6.3.2 **Subtitle Track Delay**: a field (`-4.000 s`, millisecond precision) plus a −10 s … +10 s bipolar slider filled from the 0 s center. Shortcuts for ±50 ms (VLC uses G and H) with an OSD message.
- [ ] S6.3.3 **Subtitle Speed** (fps, for files made for a different frame rate, for example 23.976 versus 25) and **Subtitle Duration Factor**. Ranges and units aren't in the design: propose speed 0.5–2.0 relative or explicit fps choices, duration factor 0–20 with 0 meaning off.
- [ ] S6.3.4 Remember the delay per media file and subtitle file (resume it next time). Reset per file.
- [ ] S6.3.5 Audio delay (S7.6) uses the same widgets.

**Edge cases**
- A delay beyond the slider's range typed in the field (−30 s): allow it in the field (up to ±10 min) and pin the slider to its end.
- A delay applied while paused: preview the subtitle at the current frame immediately.
- The sync settings for an external subtitle must not apply to an embedded track picked later; delay is per track.
- An input like "-4,5" (decimal comma), "4500ms" or "-4.5s": parse it.

---

### S6.4 — Add Subtitles dialog (file, folder or playlist; computer, OpenSubtitles or link)

**Depends on:** S6.1, S4.8, S3.5  **Size:** L  **Design refs:** `docs/ui/dialogs/add_subtitles/**`

**Subtasks**
- [ ] S6.4.1 Header row: **Target** (To This File · To This Folder · To This Playlist) and **Origin** (From Computer · From OpenSubtitles · From Link …), plus a context field (a source folder path for Computer, a search field and a language dropdown for OpenSubtitles, a URL field for Link). Cancel and Save.
- [ ] S6.4.2 **Folder + Computer:** auto-pair each video with a subtitle by episode number (`S01E05`, `1x05`, `105`) and then file-name similarity, shown as two columns with a dashed connector. **Unmatched videos and editable pairings aren't designed**: add "No match" rows with a picker.
- [ ] S6.4.3 **File + Computer:** list candidate subtitle files from the source folder, with **multi-select** (several languages at once).
- [ ] S6.4.4 **File + OpenSubtitles:** search pre-filled from the parsed title ("The Wand S01E01"), a language filter, a results list with multi-select, and download on Save (S6.5).
- [ ] S6.4.5 **Playlist** target and **From Link** origin: no screens exist; design them, or hide them for v1.
- [ ] S6.4.6 Attachment model (**decision**): link the subtitle in the DB (non-destructive, the default) **or** copy it next to the media as `<video name>.<lang>.srt` (works in other players, needs write access). Recommendation: link by default, with a "Copy next to video" checkbox.
- [ ] S6.4.7 Opened from the file menu ("Add Subtitle Tracks", target File), the folder menu ("Edit Subtitle Tracks", target Folder), the playlist menu (target Playlist), and the track popover pencil. "Edit" also lists existing external tracks with a remove action.

**Edge cases**
- One video matches two subtitle files (`E05.en.srt`, `E05.he.srt`): pair both or pick by preferred language, and show it.
- Episode numbering mismatches (absolute numbering in subtitles, S01E05 in videos; specials as S00) and multi-episode files (`S01E05E06`): best-effort matching, and let the user fix it.
- The source folder holds subtitles for a different season: zero confident matches, so say so instead of pairing wrongly. Require a confidence threshold.
- The copy target is read-only, the disk is full, or a file with that name already exists: show an error per row (Overwrite / Keep both / Skip).
- The dialog is opened for a folder with 500 videos: list virtualization and async matching.
- Saving while OpenSubtitles downloads are in flight: wait with progress, and cancel them on Cancel.
- The target file is deleted while the dialog is open: show an error on Save for that row.

---

### S6.5 — OpenSubtitles integration

**Depends on:** S6.4  **Size:** M

**Subtasks**
- [ ] S6.5.1 OpenSubtitles REST API client with the app's API key (shipped with the app), and optional user login for higher download quotas (the token goes in the keychain).
- [ ] S6.5.2 Search by file hash (the OpenSubtitles "moviehash": file size plus the first and last 64 KB), falling back to the title or episode query.
- [ ] S6.5.3 Results show language, release name, rating and download count; sort by best match.
- [ ] S6.5.4 It's gated by Privacy: the first use shows a consent note that the file hash and name are sent to OpenSubtitles.

**Edge cases**
- Offline, an API outage, a rate limit (HTTP 429) or an exhausted daily download quota: show a clear message and the reset time.
- The API key is revoked or rotated: fail gracefully with "Update VLC to continue using OpenSubtitles".
- A downloaded file is zipped, gzipped, in an unexpected encoding, or empty: handle each case.
- A file hash for a file on a slow network share: read only 128 KB, with a timeout.
- Malicious subtitle content (absurd sizes, parser exploits): the size limits from S6.1 apply, and the parsers are fuzzed (S9.7).

---

### S6.6 — Subtitle timeline on the seek bar

**Depends on:** S6.1, S2.3  **Size:** M  **Design refs:** `controls-sub_timeline.png`, Playback tab toggle "Show Subtitle Timeline on Seekbar"

**Subtasks**
- [ ] S6.6.1 When enabled, the seek bar shows a **zoomed window** (50 s in the mock) around the playhead, with subtitle cue blocks (played cues tinted brown, upcoming cues grey).
- [ ] S6.6.2 The `−` and `+` buttons zoom out and in, and the `<` and `>` buttons pan. The time labels show the window bounds with a `*` marker. **Define what `*` means** (proposal: "window bounds, not media bounds"), and define where the current time is shown in this mode.
- [ ] S6.6.3 Drag a cue block to shift the subtitle delay visually (stretch goal), and click a cue to seek to its start.

**Edge cases**
- No subtitle track is active: the toggle is disabled with a hint.
- Image-based subtitles: cue times are known but text isn't, which is fine for blocks.
- Thousands of cues: render only the cues in the visible window.
- A window extending before 0 or past the end: clamp it, never show negative times.
- The unexplained 17 px gap before the playhead in the mock: treat it as an artifact unless design says otherwise.

---

## Stage 7 — Audio and video effects

> **Goal:** the Video, Audio and Subtitles tabs of the in-player settings panel and their expandable effect sections work live, and the matching Settings pages (S5.8) set the defaults.
> **Exit criteria:** every control in `docs/ui/player/panels/**`, `controls-video_settings.png` and `controls-audio_settings.png` changes the output live; presets apply; reset works; the persistence model (D13) is implemented.

### S7.1 — Effects architecture: live apply, persistence, presets and reset

**Depends on:** S2.7, S5.1, S5.11  **Size:** M

**Subtasks**
- [ ] S7.1.1 An effect state model: `session` (the current media) → `defaults` (the Settings pages) → `preset` (S5.11) → optionally `per-file` overrides. Decide (D13) which layers ship in v1. Recommendation: session plus defaults plus presets; per-file only for subtitle and audio delay.
- [ ] S7.1.2 Collapsible sections: "Expand Settings ›" and "Collapse Settings ⌄", with the expanded state remembered per section.
- [ ] S7.1.3 **No section in the design has an on/off switch or a name in its collapsed state** (they all just say "Expand Settings"). Add a section title and an enable toggle to each header, otherwise users can't tell what is active. Needs design.
- [ ] S7.1.4 Per-control reset (double-click the slider) and a "Reset section" action.
- [ ] S7.1.5 Panel footer: the **Default Preset** dropdown (which opens upward) and **Open Advanced Settings** (opens the matching Settings category).
- [ ] S7.1.6 Apply changes live with throttling (about 30 Hz while dragging) to avoid pops and clicks in the audio and stutter in the video.

**Edge cases**
- Effects that the backend can only apply by restarting the video output (some video filters in libVLC 3): apply on release instead of while dragging, and show a brief re-init.
- An effect isn't available in this build or on this platform (a missing plugin): hide or disable it with a reason.
- Turning on a filter that's too heavy for the CPU (an 8K source): monitor dropped frames and offer to turn the filter off.
- Effects plus hardware decoding: some filters force a copy back from the GPU, which costs performance. Warn in the Performance settings.

---

### S7.2 — Video tab: Defaults (flip, rotate, zoom, aspect ratio, crop, resolution, render) and the Crop panel

**Depends on:** S7.1, S1.4  **Size:** M  **Design refs:** `controls-video_settings.png`, `panels/video/crop/*`

**Subtasks**
- [ ] S7.2.1 **Flip**: a segmented control, None | Horizontal | Vertical.
- [ ] S7.2.2 **Rotate**: degrees ("0 deg"). Snap to 90° steps; arbitrary angles are optional.
- [ ] S7.2.3 **Zoom** ("1:1 Original", ½, 2×, fit, fill), **Aspect Ratio** (Default, 16:9, 4:3, 21:9, 1:1, 16:10, 2.35:1, custom), **Crop** (Default, plus presets).
- [ ] S7.2.4 **Resolution** (Best Available, for adaptive streams) and **Render** ("Local"; it lists cast targets once S8.11 exists).
- [ ] S7.2.5 Crop panel: **Top, Bottom, Left and Right in px**, with link toggles (link top and bottom, link left and right). The linked state isn't designed; design it.
- [ ] S7.2.6 Deinterlacing mode and method (Settings → Video; also in the Video ▸ submenu).

**Edge cases**
- A crop that exceeds the frame (Left + Right ≥ width): clamp so at least a 16 px picture remains, and show validation.
- Crop units: source pixels, not screen pixels; say so in a tooltip. For anamorphic sources, the crop applies before the aspect correction.
- Rotating 90° swaps the dimensions: the window's "Resize to fit video" logic and the aspect ratio must use the rotated size.
- Rotation combined with metadata rotation: they add up; show the effective rotation.
- A zoom larger than the window: pan by dragging (or don't support it; decide).

---

### S7.3 — Video "Essential" filters

**Depends on:** S7.1  **Size:** S  **Design refs:** `panels/video/essential/*`, Settings → Video "Picture"

**Subtasks**
- [ ] S7.3.1 Hue, Brightness, Contrast, Saturation and Gamma (the libVLC "adjust" filter), plus Sharpen Sigma, Banding Removal Radius (gradfun) and Film Grain Variance.
- [ ] S7.3.2 Define ranges and defaults; the design has no value readouts. Proposal (from the libVLC defaults): Hue −180…180 (0), Brightness 0–2 (1), Contrast 0–2 (1), Saturation 0–3 (1), Gamma 0.01–10 (1), Sharpen 0–2 (0.05), Banding radius 4–32 (16), Grain 0–10 (2).
- [ ] S7.3.3 Show the value while dragging (tooltip) even though the design has no readout, so changes are reversible.
- [ ] S7.3.4 "Reset picture" in Settings → Video resets this section.

**Edge cases**
- Filters on HDR content: apply them after tone-mapping, or disable them while HDR passthrough is active.
- Some filters need a CPU format conversion: performance warning, as in S7.1.

---

### S7.4 — Video "Colors" filters

**Depends on:** S7.1  **Size:** S  **Design refs:** `panels/video/colors/video_colors-expanded.png`

**Subtasks**
- [ ] S7.4.1 Color Extraction (hex color plus toggle), Color Threshold (hex color plus toggle, then Saturation and Similarity sliders that are disabled while the toggle is off), Negate Colors, Posterize, Gradient (toggle, mode dropdown, Color toggle, Cartoon toggle) and Sepia Intensity.
- [ ] S7.4.2 Missing details to settle with design: the Posterize level control (VLC has 2–256, default 6), the Gradient mode options (VLC: gradient, edge, hough), and whether Sepia has an on/off toggle (the mock slider is a copy of Saturation).
- [ ] S7.4.3 Children depend on their parent toggle, disabled styling as in S5.1.3.

**Edge cases:** invalid hex input; several color filters stacked (order matters, so fix a documented order); filters that change the output format.

---

### S7.5 — HDR options and tone mapping

**Depends on:** S1.1 (spike results), S7.1  **Size:** M  **Design refs:** `panels/video/hdr/*`, `docs/icons/settings_categories/HDR/*`

**Subtasks**
- [ ] S7.5.1 Options: **Auto** (follow the source and display), Off (always tone-map to SDR), HDR10, HDR10+, Dolby Vision, HLG.
- [ ] S7.5.2 Define what forcing a format means (interpreting the source's metadata versus the output mode), or drop the forced options if the backend can't honor them.
- [ ] S7.5.3 Detect display HDR capability per monitor and react when the window moves between an HDR and an SDR monitor.

**Edge cases**
- Windows "Use HDR" toggled while playing: re-initialize the output.
- Dolby Vision profile 5 (IPTPQc2) without proper support shows purple and green colors: detect it and tone-map, or warn.
- HDR on an SDR display with no tone-mapping available: dim, washed-out colors, so warn the user once.
- Screenshots and snapshots of HDR content: tone-map before saving the PNG.

---

### S7.6 — Audio tab: Defaults (track, language, stereo mode with Test, audio delay)

**Depends on:** S7.1, S1.5  **Size:** S  **Design refs:** `controls-audio_settings.png`

**Subtasks**
- [ ] S7.6.1 **Audio Track** and **Audio Language** defaults (the same distinction as in S6.3.1).
- [ ] S7.6.2 **Stereo Mode** (Stereo, Mono, Left, Right, Reverse stereo, Dolby Surround, Headphones) and a **Test** button that plays a left/right channel test tone.
- [ ] S7.6.3 **Audio Track Delay**: a field (`0.000 s`) and a −10 s … +10 s bipolar slider. **The design's right label reads "-10 s"; that's a typo for "+10 s".** Shortcuts (VLC: J and K) with an OSD message.

**Edge cases**
- The Test tone at the current volume, with boost possibly at 200%: play it at a fixed safe level instead.
- Test pressed while media is playing: duck or pause the media during the test.
- A delay set on an audio track that's then switched: delay is per track (as in S6.3).

---

### S7.7 — Equalizer and presets

**Depends on:** S7.1  **Size:** M  **Design refs:** `panels/audio/equalizer/*`, `docs/icons/eq/*`, Settings → Audio "Effects"

**Subtasks**
- [ ] S7.7.1 Preamp plus 10 bands (60 Hz, 170, 310, 600, 1 k, 3 k, 6 k, 12 k, 14 k, 16 kHz), ±20 dB, with numeric value boxes you can type into.
- [ ] S7.7.2 Two layouts: a **stacked horizontal** layout in the side panel (`audio_equalizer-expanded`) and a **wide drawer with vertical sliders** in Settings (`eq-variant2`, with "Open Equalizer" and "Close Equalizer").
- [ ] S7.7.3 Presets dropdown with curve glyphs: None, Flat, Classical, Club, Dance, Full bass, Full bass & treble, Full treble, Headphones, Large Hall, Live, Party, Pop, Reggae, Rock, Ska, Soft, Soft rock, Techno (`docs/icons/eq` has 18 glyphs). **Fix the design's typos "Full base" (should be bass) and "KHz" (should be kHz).** Define how "None" (EQ off) differs from "Flat" (EQ on, 0 dB).
- [ ] S7.7.4 Editing a band while a preset is selected switches the dropdown to "Custom". "Save as preset…" creates a named EQ preset.
- [ ] S7.7.5 **2 Pass** toggle (apply the EQ twice for a stronger effect).

**Edge cases**
- A high preamp plus band boosts causes clipping: show a clip indicator, or auto-reduce the preamp.
- An EQ preset with values out of range (an imported or hand-edited file): clamp it.
- Switching presets while dragging a band: the drag wins until release.

---

### S7.8 — Audio visualizer and the waveform seek bar

**Depends on:** S7.1, S2.3  **Size:** M  **Design refs:** `panels/audio/visualizer/*`, `controls-waveform.png`, `controls-music.png`, `docs/icons/audio_visualizer/*`

**Subtasks**
- [ ] S7.8.1 Visualizer options: Disabled · Spectrometer · Scope · Spectrum · Waveform. **Fix the design's typo "Spectometer".** Visualizers draw in the video area for audio-only media (or over video, as an option).
- [ ] S7.8.2 Waveform seek bar: precompute a peak profile (about 250–1,000 buckets) on a background thread, cache it per media identity, and draw played bars orange and upcoming bars grey.
- [ ] S7.8.3 **Decide when the waveform bar replaces the normal seek bar** (D14): for audio media in music mode (S8.1), when the visualizer is set to "Waveform", or as a user toggle.

**Edge cases**
- A long file (a 10 h podcast): profile computation is decoder-bound, so show the plain bar until it's ready and update progressively.
- A network stream or live media: no full profile is possible, so use the plain bar.
- A file changes after its profile was cached: invalidate by identity (S1.7.2).
- Visualizers at 120 Hz monitors: cap at the refresh rate and pause when the window is hidden.

---

### S7.9 — Compressor, spatializer, stereo widener and loudness

**Depends on:** S7.1  **Size:** M  **Design refs:** `panels/audio/compressor/*`, `spatializer/*`, `stereo_widener/*`, Settings → Audio "Loudness" and "Effects"

**Subtasks**
- [ ] S7.9.1 **Compressor**: RMS / Peak (the design says "FM5 / Peak", a typo), Attack (ms), Release (ms), Threshold (dB), Ratio (n : 1), Knee Radius (dB), Makeup Gain (dB). Use the libVLC compressor's ranges.
- [ ] S7.9.2 **Spatializer**: Size, Width, Wet, Dry, Damp (on a 0–10 display scale that maps to the backend's 0–1). Spatial audio mode (Headphone virtualization, …).
- [ ] S7.9.3 **Stereo widener**: Delay Time (ms), Feedback Gain, Crossfeed, Dry Mix. **The design shows "%" on values like 0.2/0.3/0.8; decide between fractions and real percentages.**
- [ ] S7.9.4 **Loudness**: Normalize loudness and Target level, Night mode (a dynamic range compression preset), Replay gain (Off, Track, Album) with preamp.

**Edge cases**
- Combining effects: the processing order is fixed and documented (EQ → compressor → spatializer → widener → normalizer → volume).
- Mono sources through the stereo widener: no effect, so disable it with a hint.
- Effects with passthrough audio (S/PDIF or HDMI bitstream): effects can't apply, so disable the audio effects with a note while passthrough is active.
- Replay gain tags missing: fall back to no gain, and never boost into clipping.

---

## Stage 8 — Advanced features

> **Goal:** the remaining designed features, in priority order. Each story is independent once Stages 0–7 are done, so they can be parallelized or reordered by priority.
> **Priority key:** **P1** = designed and expected by VLC users for 1.0 · **P2** = designed, can follow 1.0 · **P3** = settings rows exist but no feature design; needs a product decision before work starts.

### S8.1 — Music mode (audio-only player layout) · P1

**Depends on:** S4.3, S7.8  **Size:** M  **Design refs:** `screens/music/*`, `controls-music.png`

**Subtasks**
- [ ] S8.1.1 For audio-only media: large centered artwork over a blurred, enlarged copy of the same artwork, with a "title - artist" caption.
- [ ] S8.1.2 Control bar variant: Previous · **Back 15 s** · Play · **Forward 15 s** · Next · time · **waveform seek bar** · time · volume · Audio · List. No Subtitles button. **The two designs disagree about Settings** (`controls-music.png` has it, `player_music_01.png` doesn't); decide.
- [ ] S8.1.3 Placeholder art (the green music-note tile) when there's no artwork. The blur uses a cheap downscaled copy, not a live blur.

**Edge cases**
- Video files with only an audio track (or with video disabled through "Enable video" off): use music mode.
- Audio with an attached-picture video stream (MP3 cover art reported as a video track): treat it as audio with artwork, not a 1-frame video.
- Artwork with an extreme aspect ratio (a 3000×300 banner): letterbox it inside the square.
- A queue that mixes audio and video: the layout switches per item without a jarring window resize.
- "Is music mode chosen by media type or by the Music profile?" (D15): the recommendation is media type, and profiles can override the layout.

---

### S8.2 — Chapters · P1

**Depends on:** S1.2, S3.2  **Size:** S  **Design refs:** `controls-chapters.png`

**Subtasks**
- [ ] S8.2.1 The List panel's **Chapters** tab: number (orange), title and start time; click to seek. Highlight the current chapter (not designed).
- [ ] S8.2.2 Chapter markers on the seek bar, and the chapter name in the hover preview (not designed; propose).
- [ ] S8.2.3 Next and previous chapter shortcuts.

**Edge cases**
- Chapters without titles: "Chapter N". Chapters starting after 0 (the mock's chapter 1 starts at 00:02:15): the region before it is "Intro" or simply no chapter.
- Chapter times past the duration (as in the mock, where chapters reach 01:44:10 in a 25 min file): ignore or clamp the bad entries.
- DVD titles versus chapters: discs have titles containing chapters (S8.9).
- Ordered chapters or segment linking (MKV): follow what the backend supports.

---

### S8.3 — Bookmarks · P1

**Depends on:** S1.7, S3.2  **Size:** S  **Design refs:** `controls-bookmarks.png`, `docs/icons/library/bookmark*`

**Subtasks**
- [ ] S8.3.1 The List panel's **Bookmarks** tab: "N Bookmarks", "−" and "+". Rows show a name and a timestamp (`00:00:42:023`); click to seek; rename inline.
- [ ] S8.3.2 "+" adds a bookmark at the current position, auto-named "<title> N". Add a shortcut.
- [ ] S8.3.3 Bookmarks are stored per media identity (S1.7.2), so they survive renames.
- [ ] S8.3.4 Timestamp format: the design uses `HH:MM:SS:mmm` with a colon before the milliseconds. Confirm, since `HH:MM:SS.mmm` is standard.

**Edge cases:** bookmarks past the end of a re-encoded file; duplicate timestamps; bookmarks on a live stream (not allowed); 1,000 bookmarks (virtualize the list).

---

### S8.4 — Snapshots · P1

**Depends on:** S1.4  **Size:** S  **Design refs:** onboarding "Take a Snapshot SHIFT+S", customize palette "Snapshot"

**Subtasks**
- [ ] S8.4.1 Save the current frame at **source resolution** (not window resolution) as PNG (or JPG, a setting) to the snapshot folder (default: OS Pictures/VLC), named `<title>_<hh-mm-ss.mmm>.png`.
- [ ] S8.4.2 A toast with a thumbnail and actions: open, show in folder, copy to clipboard.

**Edge cases**
- The snapshot folder is missing or read-only: create it, or fall back and say where it was saved.
- File name collisions: add a suffix. Titles with characters illegal in file names (`:` `?` `*` `/` `\` `|` `"` `<` `>`): sanitize them, and also avoid reserved Windows names (`CON`, `NUL`).
- An HDR frame: tone-map it (S7.5). Applied filters, crop and rotation: decide whether the snapshot includes them (VLC does).
- A snapshot while paused works. A snapshot with no video (audio-only) is disabled.
- DRM or hardware-protected surfaces can't be read back: show an error.

---

### S8.5 — Miniplayer window, compact mode and Picture in Picture · P1

**Depends on:** S4.10  **Size:** M  **Design refs:** `miniplayer/*`, customize palette "Picture in Picture", Settings → Interface "Start in compact mode"

**Subtasks**
- [ ] S8.5.1 Pop the miniplayer out into its own small, borderless, **always-on-top** window, using the in-library card visuals (hover controls, ✕, pop-in).
- [ ] S8.5.2 Resizable with the video's aspect ratio locked; draggable anywhere; position remembered per display.
- [ ] S8.5.3 "Start in compact mode" launches straight into this window.

**Edge cases**
- Always-on-top needs a per-platform call (no GPUI API for normal windows). On Wayland it's not possible for clients; use a floating-window hint or document the limitation.
- The PiP window is positioned on a display that's then disconnected: move it onto an existing display.
- Fullscreen apps (games, presentations) on the same display: PiP stays on top only where the OS allows it.
- Closing PiP with ✕: stop playback, or return to the main window (decide; the design's ✕ suggests closing).

---

### S8.6 — Recording · P2

**Depends on:** S1.2, S5.10  **Size:** M  **Design refs:** right-click "Record R", onboarding "Record SHIFT+R", Settings → Recording

**Subtasks**
- [ ] S8.6.1 Toggle recording of the current stream to "Save recordings to" (default `~/Videos`), remuxing without re-encoding when possible.
- [ ] S8.6.2 A visible recording indicator (red dot and duration) in the title bar or OSD.

**Edge cases**
- The disk fills up during recording: stop cleanly, keep a playable file, and show an error.
- Recording a protected or unsupported stream: refuse with a reason.
- The stream ends or the app quits while recording: finalize the file (write the moov atom / MKV cues). A crash leaves a recoverable file (prefer MKV or fragmented MP4).
- The recordings folder is the library root: the new file appears in the library after finalization, not while it's being written (S4.2 "files still being written").

---

### S8.7 — Customize Player (drag-and-drop layout editor) · P2

**Depends on:** S2.1, S2.2 (declarative layout)  **Size:** L  **Design refs:** `screens/customize/player_customize_01..09.png`

**Subtasks**
- [ ] S8.7.1 Entry point: Main menu → **Customize Player**. Edit mode outlines every movable element with a dashed border. The logo is fixed.
- [ ] S8.7.2 A modal palette ("Drag items around and out of the box below to re-order them. Drag items into the box to remove them from the player.") with 31 items: Spacer, Divider, Playback Speed, Forward/Backward 10 s, Forward/Backward 15 s, Stop, Repeat, Shuffle, Record, Snapshot, Picture in Picture, HDR, Zoom, Right Sidebar, Quit VLC, VLC Settings, Go Home, Thumbnail, File Name, Artist Name, Folder Name, Playlist Name, Properties, Replay, Chapters, Favorite, Bookmark, Bookmarks, Playlists.
- [ ] S8.7.3 Drag between the title bar and the control bar, **into a second control-bar row**, out of the palette (add) and into the palette (remove). Spacer and Divider are unlimited; other items are single-instance and leave the palette once placed.
- [ ] S8.7.4 Groups can be split (pull Fullscreen out of the window-controls group). The time + seek + duration widget moves as one unit.
- [ ] S8.7.5 Save (enabled after the first change) and Cancel. Persist the layout (per profile, S8.8). **Add "Reset to default"** (not designed).
- [ ] S8.7.6 Define the behavior of items that have no design elsewhere: Right Sidebar, Thumbnail, Properties, Playlist Name, Artist Name, HDR, Zoom.

**Edge cases**
- The user removes Close/Minimize/Maximize from the title bar: allow it but keep the OS shortcuts working, or forbid removing Close (recommended).
- The user removes Play/Pause or the seek bar: allow it (Space still works), with a warning.
- A layout too wide for the current window: overflow rules from S2.2.5 apply.
- A layout from a newer version with unknown item ids: skip the unknown items and keep them in the saved file.
- Keyboard-only layout editing (accessibility): an alternative list-based editor with move up and down.
- Music mode has its own default layout (S8.1): define whether customization applies per mode.
- `customize_09` lost the heart (Favorite) that `customize_08` had: ignore it as a mock inconsistency.

---

### S8.8 — Profiles (Movies, Shows, Music, Work) · P2

**Depends on:** S8.7, S5.11  **Size:** M  **Design refs:** `menus/profiles/profiles.png`, main menu "Profile (Movies) ▸"

**Subtasks**
- [ ] S8.8.1 **Decision (D12):** a profile is a bundle of {player layout (S8.7) + settings preset (S5.11)}. Don't build two parallel systems.
- [ ] S8.8.2 Switch from Main menu → Profile ▸ (the current one is checked), and create one with "+ New". Rename and delete aren't designed.
- [ ] S8.8.3 Optional automatic profile by media type or library section (Music section → Music profile).

**Edge cases:** deleting the active profile; switching mid-playback (apply live, without restarting playback); the last profile can't be deleted.

---

### S8.9 — Discs, capture devices and broadcast programs · P2

**Depends on:** S1.2  **Size:** L  **Design refs:** Main menu "Open Disc Ctrl+D", "Open Capture Device Ctrl+C"; shortcut "Next program / Service ID X"; `docs/icons/library/disc*`, `capture_device*`

**Subtasks**
- [ ] S8.9.1 Open Disc: DVD (with menus), Blu-ray (unencrypted, or with user-provided libaacs keys), Audio CD (with CD-Text and track list), and VCD. Needs a dialog design.
- [ ] S8.9.2 Open Capture Device: webcams, capture cards and screen capture, with device and format selection. Needs a dialog design.
- [ ] S8.9.3 DVB and MPEG-TS programs: cycle programs with **X** and list services.

**Edge cases**
- Disc ejected during playback; region-locked DVDs; copy-protected Blu-ray (show a clear legal message, ship no circumvention); a scratched disc causing read stalls (timeouts, skip bad sectors).
- Camera permission denied (macOS TCC, Windows privacy settings): show how to grant it.
- A capture device in use by another app.
- **Ctrl+C for Capture conflicts with Copy** (see D3).

---

### S8.10 — Convert / transcode · P2

**Depends on:** S1.1  **Size:** L  **Design refs:** file context menu "Convert", main menu "Convert Ctrl+R" (no dialog design)

**Subtasks**
- [ ] S8.10.1 A conversion dialog (needs design): presets (MP4 H.264/AAC, MP3, WebM, …), destination, and a queue with progress.
- [ ] S8.10.2 Runs in the background with a cancel button, keeps playback usable, and sends a notification when done.

**Edge cases:** the output path equals the input path (refuse); disk full; a source with unsupported streams; the app is quit mid-conversion (ask whether to cancel); converting 100 files (a queue with limited concurrency).

---

### S8.11 — Casting (Chromecast and similar) · P3

**Depends on:** S1.1, S5.10  **Size:** L  **Design refs:** Settings → Recording "Casting", video tab "Render: Local"

**Subtasks**
- [ ] S8.11.1 Discover devices over mDNS, pick one in **Render**, and cast with transcoding when the device can't play the format ("Cast quality").

**Edge cases:** the device disappears mid-cast; a firewall blocks discovery; multiple network interfaces (VPN); local subtitles burned in versus sent separately; the device's own volume versus the app volume.

---

### S8.12 — Stream out · P3

**Depends on:** S1.1, S5.10  **Size:** L  **Design refs:** file context menu "Stream", Settings → Recording "Streaming"

**Subtasks**
- [ ] S8.12.1 Stream the current media to HTTP, RTSP or UDP with the encoding preset and tuning; optionally play locally while streaming.

**Edge cases:** port already in use; binding to all interfaces exposes content on public networks (default to localhost and warn); bandwidth shortfall.

---

### S8.13 — Playback extras: sleep timer, statistics, reverse, A-B loop · P2

**Depends on:** S2.7  **Size:** M  **Design refs:** `controls-playback.png`

**Subtasks**
- [ ] S8.13.1 **Sleep Timer**: Off, 15/30/45/60/90 min, End of current item, Custom. Fade the audio out over the last 10 s, then pause or stop.
- [ ] S8.13.2 **Show Statistics**: an overlay with the codec, resolution, bitrate, dropped frames, buffer and hardware decoding status.
- [ ] S8.13.3 **Reverse** playback. libVLC has **no native reverse playback**; implementing it means decoding GOPs backwards, which is expensive. Spike it; if it's infeasible, remove the toggle from the design.
- [ ] S8.13.4 A-B loop (in VLC, not designed; optional).

**Edge cases:** the sleep timer fires while paused (it still stops); the system sleeps before the timer fires (on resume, the timer is past due, so stop); statistics on a live stream.

---

### S8.14 — Extensions, add-ons and skins · P3

**Depends on:** S5.10  **Size:** XL  **Design refs:** Settings → Extensions, Settings → Interface "Skin"

**Subtasks**
- [ ] S8.14.1 **Product decision first:** what's the extension model (WASM plugins? Lua like VLC? none in v1?) and what's a "skin" given the custom-layout system (S8.7)? The recommendation is to defer both to after 1.0 and hide the Settings rows behind a feature flag.

**Edge cases to keep in mind:** sandboxing untrusted code, API versioning, crash isolation, and the security of a "Find more extensions" catalog.

---

### S8.15 — Web remote control · P3

**Depends on:** S5.6  **Size:** M  **Design refs:** Settings → Controls "Remotes"

**Subtasks**
- [ ] S8.15.1 An HTTP server with a small web UI for play, pause, volume and queue control from a phone, configurable port, and a **required password** (or pairing code).

**Edge cases:** port in use; binding to localhost versus the LAN (default to LAN only with a password); CSRF and DNS rebinding protection; never reachable from the internet by default; OS firewall prompts on first start.

---

### S8.16 — System tray, notifications and desktop wallpaper mode · P3

**Depends on:** S5.4, S5.8  **Size:** M

**Subtasks**
- [ ] S8.16.1 Tray or menu-bar icon with a mini menu (play/pause, next, show, quit), and minimize to tray (the Interface "Notifications and System Tray" section; its rows need definitions).
- [ ] S8.16.2 Now-playing notifications on track change (respect OS Do Not Disturb).
- [ ] S8.16.3 "Play video as desktop wallpaper": Windows only (render behind the desktop icons, using the WorkerW technique); hide it elsewhere.

**Edge cases:** the tray isn't available (GNOME without the AppIndicator extension), so hide the option; wallpaper mode with multiple monitors; wallpaper mode survives the explorer.exe restarting.

---

### S8.17 — last.fm scrobbling and podcasts · P3

**Depends on:** S5.5, S4.11  **Size:** M

**Subtasks**
- [ ] S8.17.1 Scrobble tracks played more than 50% or more than 4 min, with an offline queue (S5.5 edge cases).
- [ ] S8.17.2 Podcasts: subscribe to RSS feeds in Network → Podcasts, list episodes, mark them played (needs design).

---

## Stage 9 — Release hardening

> **Goal:** ship it. Accessible, localized, fast, signed, updatable, legally clean.
> Run S9.1–S9.3 continuously from Stage 2 onward (as checks on every story), then finish them here.

### S9.1 — Accessibility

**Subtasks**
- [ ] S9.1.1 Full keyboard operation of every screen (S0.4.14), with visible focus everywhere.
- [ ] S9.1.2 Screen reader support: check the current state of GPUI's accessibility tree (it's limited), and contribute upstream or plan around it. At minimum, announce the play state, track changes and errors.
- [ ] S9.1.3 Contrast: all text ≥ 4.5:1 (S0.3.3), and focus rings and icons ≥ 3:1, in both themes and with all 9 accents.
- [ ] S9.1.4 Reduced motion, larger text (respect the OS text scaling), and hit targets of at least 24×24 px (the design's 12 px preview icons are fine only inside the onboarding mock).

### S9.2 — Internationalization

**Subtasks**
- [ ] S9.2.1 Externalize every string (Fluent or a similar format) with plural rules ("1 item" / "N items"), and use locale date formats in the library.
- [ ] S9.2.2 **Right-to-left UI mirroring** (Hebrew, Arabic): the layout mirrors, but media transport icons and the seek bar direction stay left-to-right by convention. Decide and test.
- [ ] S9.2.3 Text expansion: German and Finnish labels are about 30–40% longer; check the settings label column and menus.
- [ ] S9.2.4 Interface language switch (S5.3.2), defaulting to the OS language.

### S9.3 — Performance budgets

| Metric | Budget |
|---|---|
| Cold start to interactive main window | < 1.0 s (no library scan blocking) |
| Open a local file to first frame | < 400 ms (SSD, 1080p H.264) |
| UI frame time during 4K playback | < 8 ms at 120 Hz, < 16 ms at 60 Hz |
| Idle CPU (paused, window visible) | ≈ 0% (no redraw loop) |
| Memory, 20k-item library open | < 400 MB |
| Library first index | ≥ 200 files/s on SSD (metadata only) |

**Subtasks**
- [ ] S9.3.1 Profile with GPUI's frame timing and track the budgets in CI on reference hardware.
- [ ] S9.3.2 Fix redraw loops: there should be no repaint while nothing changes (watch out for the 15 Hz position tick while paused).

### S9.4 — Packaging and distribution

**Subtasks**
- [ ] S9.4.1 Windows: an MSIX or MSI installer, Authenticode signing, file associations, Start menu entry, and uninstall that leaves user data unless asked to remove it.
- [ ] S9.4.2 macOS: a universal (arm64 + x86_64) `.app` (the `cargo-bundle` metadata already exists), hardened runtime, notarization, `Info.plist` document types, and a DMG.
- [ ] S9.4.3 Linux: Flatpak (with portals for file access, which affects S4.2 scanning outside the sandbox), AppImage, and `.deb`/`.rpm`. Handle the libVLC dependency in each format.
- [ ] S9.4.4 Ship the backend runtime per the S1.1 decision.

**Edge cases:** Windows SmartScreen reputation for a new certificate; macOS Gatekeeper quarantine on unsigned dev builds; Flatpak sandbox restrictions on media folders outside home (`--filesystem` permissions).

### S9.5 — Updates

**Subtasks**
- [ ] S9.5.1 "Check for updates automatically" (off by default, per the design): a signed update manifest over HTTPS, release notes, download, verify the signature, install on quit.
- [ ] S9.5.2 Rollback when the new version fails to start twice.

**Edge cases:** no internet; proxy (S5.10.1); an update while playing (never interrupt); the install dir isn't writable (per-machine install without admin rights); the user skips a version.

### S9.6 — Crash reporting and telemetry (opt-in only)

**Subtasks**
- [ ] S9.6.1 Local crash reports (S0.7.2) with an **opt-in** upload. "Collect playback statistics" (General → Advanced) is opt-in and documented.
- [ ] S9.6.2 Never include file names or paths unless the user explicitly allows it.

### S9.7 — Security review

**Subtasks**
- [ ] S9.7.1 Media parsing is attack surface: keep the backend updated, track VLC security advisories, and fuzz our own parsers (subtitles, playlists, settings, NFO and cue files).
- [ ] S9.7.2 URL handling: refuse `javascript:`, `data:` and other non-media schemes; sanitize OSD and title text.
- [ ] S9.7.3 Credentials only in the OS keychain (S5.5, S5.10, S6.5).
- [ ] S9.7.4 Review network listeners (web remote, stream out): authentication on, localhost by default.

### S9.8 — QA matrix and test media corpus

**Subtasks**
- [ ] S9.8.1 A test media corpus (see S1.1.3, plus subtitles in 10 encodings, broken files, huge files, odd names) in a separate LFS repo or artifact store, not in this repo.
- [ ] S9.8.2 OS matrix: Windows 10 22H2 and 11 (x64, arm64), macOS 13–15+ (arm64, x64), Ubuntu LTS (X11 and Wayland), Fedora (Wayland, PipeWire).
- [ ] S9.8.3 Display matrix: 100/125/150/200% scaling, mixed-DPI multi-monitor, an HDR monitor, a 1366×768 laptop.

### S9.9 — Legal and branding

**Subtasks**
- [ ] S9.9.1 **"VLC" and the cone logo are trademarks of VideoLAN.** The app ships as "VLC" (`Cargo.toml` bundle name, `build.rs` ProductName) with the cone icon. Get permission from VideoLAN or choose a distinct name and icon **before any public release**.
- [ ] S9.9.2 License compliance: libVLC (LGPL-2.1+) dynamically linked with its notices, plus the GPUI/Zed crate licenses and every dependency's license (`cargo-about`), shown in **About VLC**.
- [ ] S9.9.3 Codec patents and distribution in different regions: review with counsel if distributing commercially.
- [ ] S9.9.4 OpenSubtitles, last.fm and cover-art providers: comply with their API terms and attribution requirements.

---

## Appendix A — Cross-cutting edge-case catalog

Check every story against this list before closing it. The story-level lists only cover what's specific to each story.

### A.1 Files, paths and storage
- Unicode in names: emoji, right-to-left text, combining marks, **NFC versus NFD** (macOS), and full-width characters.
- Non-UTF-8 names (Linux bytes, unpaired Windows UTF-16 surrogates): display them lossily, but open them with the raw `OsString`.
- Long paths (> 260 characters on Windows need `\\?\`), UNC paths (`\\server\share`), mapped drives, `file://` URLs with percent-encoding.
- Reserved Windows names (`CON`, `NUL`, `COM1`) and illegal characters (`<>:"/\|?*`) when **writing** files (snapshots, exports, copied subtitles).
- Case-insensitive versus case-sensitive filesystems on the same machine (APFS volumes, WSL mounts).
- Symlinks, junctions, aliases and `.lnk` files; loops; broken targets; hard links.
- Removable drives unplugged mid-read; drive letters changing; network shares disconnecting or sleeping.
- Cloud placeholders (OneDrive, iCloud, Dropbox) that download when read; never bulk-read them.
- Read-only locations, a full disk, quota exceeded, a file locked by another process, antivirus holding a handle.
- Files that change while open: growing (a download or recording), truncated, replaced, renamed, deleted.
- Permission prompts: macOS TCC for Desktop, Documents, Downloads, removable volumes, camera and microphone; Flatpak portals.

### A.2 Media content
- Corrupt, truncated or zero-byte files; wrong or missing extensions; a directory or device passed as a file.
- Unsupported codec for one stream (play the rest); DRM; zero playable streams.
- Extreme dimensions (8K, larger than the GPU texture limit, 1×1, odd sizes); non-square pixels; rotation metadata; interlacing; VFR; 240 fps; 10-bit, HDR10, HLG, Dolby Vision.
- Duration that's unknown, wrong, changing, longer than 24 h, or shorter than 1 s; live streams; non-seekable streams.
- Many tracks (50+ subtitles), tracks appearing late, tracks without language tags, forced and default flags.
- Attached pictures reported as video tracks; still images opened as media; playlists, cue sheets and archives opened as media.
- Chapters out of order, past the end, untitled, or not starting at 0.

### A.3 Time and numbers
- Time format grows past 99 h; tabular digits; never show NaN, infinity or negative values from the engine.
- Rounding: the displayed duration and the end position must agree (don't show `00:25:39` at the end of a `00:25:40` file).
- Decimal comma versus point in numeric inputs (`1,5`); unit suffixes (`ms`, `s`, `x`, `%`, `dB`).
- Playback rates other than 1× when showing remaining time.
- Wall-clock changes (DST, manual clock changes, NTP jumps) must not affect timers (sleep timer, auto-hide); use monotonic clocks.

### A.4 Input
- Keyboard layouts (US, UK, German, AZERTY, Hebrew, Russian), dead keys, AltGr, Caps Lock, IME composition, numpad.
- Key repeat; chorded shortcuts; shortcuts reserved by the OS; a focused text field swallowing single-key shortcuts.
- Mouse buttons 4 and 5 (back and forward), high-resolution wheels, trackpad inertia, horizontal scrolling.
- Double-click timing versus single-click actions; drags that leave the window; touch and pen input (Windows).
- Drag-and-drop from other apps: files, folders, text, URLs, mixed payloads, huge payloads.

### A.5 Windows and displays
- Multi-monitor setups with mixed DPI and refresh rates; a display unplugged while a window or fullscreen is on it.
- Small screens (1366×768 at 125%), very large screens (5K, ultrawide), portrait monitors.
- The window resized to its minimum; maximized; snapped (Windows snap layouts); fullscreen; minimized; occluded.
- macOS: notch, native fullscreen Spaces, traffic-light inset, the app staying open without windows.
- Linux: Wayland can't set window positions or always-on-top; X11 window managers differ in decorations.
- Remote desktop and VMs (software rendering), GPU driver resets, GPU switching on laptops, HDR on/off toggles.

### A.6 Audio devices
- No device; a device removed or added mid-playback; the system default changes; exclusive mode held by another app.
- Bluetooth latency; multichannel sources on stereo devices; sample rate changes mid-stream; passthrough.

### A.7 Power and system state
- Sleep or hibernate during playback, indexing, conversion or recording; screen lock; user switching.
- Battery saver and thermal throttling (GPUI exposes `on_thermal_state_change`): lower background work.
- OS shutdown or logout while playing: save the position and queue on the quit signal (`WM_QUERYENDSESSION` on Windows, `applicationShouldTerminate` on macOS, SIGTERM).
- Low memory and low disk.

### A.8 Network
- Offline, captive portal, proxy (system or manual, with authentication), IPv6-only, very slow, flaky.
- TLS errors (self-signed, expired, wrong host), redirects (http → https), signed URLs that expire.
- Rate limits and quotas (OpenSubtitles, metadata providers); API keys revoked.

### A.9 Concurrency and lifecycle
- Two instances; rapid repeated commands; late events from a previous media (generation ids).
- Quitting during a scan, save, download, conversion or recording: cancel or finish cleanly, never corrupt data.
- Crash recovery: settings, the DB and the queue are consistent after `kill -9`. Detect a crash loop caused by a specific file.
- The same state changed from several places (popover, Settings, menu, shortcut): every view updates.

### A.10 Persistence and upgrades
- Schema migrations forward; running an **older** build on newer data (preserve unknown keys and tables).
- Corrupt `settings.toml` or `library.db`: back up, recover and inform the user; never crash.
- Config synced between machines (roaming profiles, dotfiles): keep machine-specific paths separate.

### A.11 Localization and text
- Right-to-left layout and bidi text; CJK line breaking; complex scripts; font fallback; emoji.
- Long translations (German +35%); pluralization; locale date and number formats (keep ASCII digits for time codes).

### A.12 Accessibility
- Keyboard-only use; screen readers; WCAG contrast in both themes and all 9 accents; reduced motion; OS text scaling.

### A.13 Security and privacy
- Untrusted input everywhere: media, subtitles, playlists, `.cue`/`.nfo` files, URLs, tags. Use size limits, time limits, fuzzing, and never execute anything.
- Credentials only in the OS keychain; network listeners off and on localhost by default.
- Privacy toggles (history, positions, online metadata) are honored everywhere, including logs, the OS recent documents list and crash reports.

### A.14 Platform specifics
- **Windows:** Known Folders (FOLDERID_Videos, FOLDERID_Music, FOLDERID_Pictures), `MAX_PATH`, sharing violations, Explorer launching N processes for a multi-select, SmartScreen, High Contrast mode, snap layouts.
- **macOS:** TCC prompts, notarization and Gatekeeper, `on_open_urls` and `on_reopen`, NFD file names, Cmd versus Ctrl shortcuts, menu bar conventions (Settings is Cmd+`,`).
- **Linux:** X11 versus Wayland, Flatpak sandbox paths, no system tray on GNOME, XDG directories, PipeWire and PulseAudio, MPRIS.

---

## Appendix B — Decisions to make

Each decision blocks the stories listed with it. The "Recommendation" column is a proposal, not a decision.

| ID | Decision | Blocks | Recommendation |
|---|---|---|---|
| D1 | Playback backend | S1.1 → everything after it | libVLC (it covers every designed panel) |
| D2 | Video presentation path per OS | S1.1, S1.4 | macOS: `CVPixelBuffer` → `surface()`; Windows and Linux: measure CPU upload versus GPU sharing in the spike |
| D3 | Default keymap (the designs conflict; table below) | S0.5, S2.10, S5.6 | **Decided (S0.5):** N/P for next/previous, Shift+R record, Shift+S snapshot; no Ctrl+C or Ctrl+F for menu items |
| D4 | First launch was triggered by opening a file | S0.2.6 | **Decided:** play the file now, run onboarding on the next plain launch |
| D5 | Settings as a separate window or an in-app modal | S5.2.1 | Separate window |
| D6 | Theme option order (Light/Dark/System versus Dark/Light/System) | S5.3.1 | One order everywhere (Light, Dark, System matches the code) |
| D7 | Music source, given that onboarding only asks for one folder | S4.2.7 | Auto-add the OS Music folder if it exists |
| D8 | Window title rule while playing | S2.1.2, S4.10 | The media title while media is loaded, otherwise "VLC Media Player" |
| D9 | Meaning of dimmed text and the watched bar | S4.6 | Dim = watched only; the bar shows real progress, full width when watched |
| D10 | Sort persistence (per folder or per section) and which sorts each view offers | S4.7 | Per folder; hide sorts that don't apply |
| D11 | Where media folders are managed in Settings (not designed) | S5.9.3 | A "Media folders" list at the top of Library settings |
| D12 | Profiles versus setting presets | S5.11, S8.8 | A profile = layout + preset; one system |
| D13 | Effect persistence layers | S7.1 | Session + defaults + presets; per-file only for delays |
| D14 | When the waveform seek bar appears | S7.8.3 | Audio media in music mode; optional for video |
| D15 | What triggers music mode | S8.1 | Media type (audio-only); profiles may override |
| D16 | Subtitles added via the dialog: link in the DB or copy next to the media | S6.4.6 | Link by default, with an opt-in copy |
| D17 | Miniplayer: an in-window card only, or also a pop-out OS window | S4.10, S8.5 | Both: card in library mode, pop-out as PiP |
| D18 | Reverse playback: feasible, or remove from the design | S8.13.3 | Spike it; remove it if it can't be made smooth |
| D19 | Extensions and skins scope | S8.14, S5.4.4, S5.10.4 | Defer past 1.0; hide the rows |
| D20 | Product name and logo (VideoLAN trademarks) | S9.9.1 — **before any public build** | Get permission, or rebrand |
| D21 | Restoring volume above 100% at launch | S1.5.2 | Cap the restored value at 100% |
| D22 | How the Video tab of the track popover is opened | S2.6.1 | **Decided (S2.6):** reachable inside the popover; shown only with 2+ video tracks |
| D23 | Effect sections have no title or enable toggle in the design | S7.1.3 | Add a title and toggle to each section header |

### D3 — shortcut conflicts found in the designs

| Action | Onboarding step 3 | Shortcuts dialog | Right-click menu | Main menu | Proposed default |
|---|---|---|---|---|---|
| Previous / Next item | `<` / `>` | P / N | P / N | — | **P / N** (no Shift, layout-safe); `<`/`>` as aliases |
| Record | Shift+R | — | R | — | **Shift+R** (avoid accidental recording) |
| Snapshot | Shift+S | — | — | — | Shift+S |
| Stop | — | S | S | — | S (Caps Lock must not turn it into Shift+S) |
| Open Folder | — | — | — | Ctrl+F | **Ctrl+Shift+O**; Ctrl+F stays "Search" in the library |
| Open Capture Device | — | — | — | Ctrl+C | **Ctrl+Shift+C** or none; Ctrl+C must stay Copy |
| Convert | — | — | — | Ctrl+R | Ctrl+R |
| Settings | — | — | — | Ctrl+P | Ctrl+P on Windows/Linux; **Cmd+`,`** on macOS |
| Hide UI | — | — | Ctrl+\ | — | Ctrl+\ (on German and other layouts `\` needs AltGr; test it) |
| Quit | — | — | Ctrl+Q | — | Ctrl+Q / Cmd+Q |
| Open File(s), Network, Disc | — | — | — | Ctrl+O, Ctrl+N, Ctrl+D | Keep |
| About | — | — | — | Shift+F1 | Keep |
| File browser, queue | L, Q | — | — | — | Keep |
| Show time, controls in fullscreen, next program | — | T, I, X | — | — | Keep |

---

## Appendix C — Design gaps and typos

### C.1 Not designed yet (needed before or during the story)
- **States:** empty library, first-run Home, indexing progress, no search results, folder unavailable, "can't play this file", buffering, live-stream seek bar, recording indicator, muted volume, current queue item, current chapter, Repeat and Shuffle ON states, linked crop fields.
- **Menus:** every submenu (Open Recent, Profile actions, Playback, Video, Audio, Subtitle, More Settings, Playlist, Open Media); the library header `•••` menu; the rail's "+" and History items.
- **Dialogs:** About, Convert, Stream, Open Disc, Capture Device, Add to Playlist picker, Properties & Metadata, Add Audio Tracks, Export Playlist options, Add Subtitles "To This Playlist" and "From Link", color picker, restart-required banner.
- **Views:** grid view in the sidebar overlay (built as two columns, S4.6.4), folder tiles (built as the folder icon on a plain tile), watched playlist card, playlist detail view, Favorites, History, Podcasts.
- **Settings:** Notifications and System Tray rows, several "Advanced Options" sections, the light-theme Settings window, the search no-results state, tooltip copy for every `(i)` icon, **media folders management**.
- **Effects:** section titles and enable toggles, the Posterize level, Gradient modes, Sepia on/off, ranges for every slider.
- **Other:** OSD messages, toasts and notifications, the drag-and-drop highlight, the update dialog.

### C.2 Typos and inconsistencies in the design files
| Where | As drawn | Should be |
|---|---|---|
| `panels/audio/compressor` | "FM5 / Peak" | "RMS / Peak" |
| `panels/audio/visualizer` | "Spectometer" | "Spectrometer" |
| `panels/audio/equalizer` presets | "Full base", "Full base & treble" | "Full bass", "Full bass & treble" |
| Equalizer band labels | "1 KHz" … "16 KHz" | "1 kHz" … "16 kHz" |
| `controls-audio_settings.png` | delay slider right label "-10 s" | "+10 s" |
| `settings/screens/recording` | "include subtitles" | "Include subtitles" |
| `settings/screens/privacy` | "Last.fm username" twice | second one is "Last.fm password" |
| Several screens (sample data) | "The_Wand_S01E13mp4" | "The_Wand_S01E13.mp4" (sample data only) |
| Queue header (sample data) | "4 items - 2h 3m" | the rows sum to about 1h 35m; compute it |
| Chapters (sample data) | chapters up to 01:44:10 in a 00:25:40 file | sample data only |
| Settings | the sidebar says "General", the page title says "Global Settings" | intentional? confirm |
| Theme order | Light/Dark/System (onboarding) versus Dark/Light/System (Settings) | D6 |
| Shortcuts | onboarding versus menus versus dialog | D3 |
| Duplicates | `interface_02`, `subtitles_02`, `video_02`, `add_subtitles_…_04`, `controls-variant10` are byte-identical to their siblings | the scrolled or alternate states are missing |
| Light theme | `player/screens/light` has a gray title bar and rail (#ABABAA) and a #C2C2C1 panel; onboarding's light preview uses #E6E6E6 and #F3F3F3 | confirm which light palette the player uses; S0.3.2 took the player design's values for `sidebar` and `panel` only |
| `shared/components/button-variant2` | the same button on `#191919` instead of `#090909` | read as a second style (`Filled`), not a hover state; confirm |
| Menus, dropdowns, popovers | about 95% opaque | the kit draws them opaque, since text underneath showed through; panels over video keep 95%. Confirm |

---

## Appendix D — Design reference index

| Design folder | Stories |
|---|---|
| `docs/onboarding_scenes/` | Implemented; S0.2, S0.3 |
| `docs/ui/shared/components/` | S0.4 |
| `docs/ui/player/screens/default/` | S2.1, S4.10 |
| `docs/ui/player/screens/hide_ui/` | S2.8 |
| `docs/ui/player/screens/light/` | S0.3, S2.1 |
| `docs/ui/player/screens/music/` | S8.1 |
| `docs/ui/player/screens/customize/` | S8.7 |
| `docs/ui/player/controls/control_bar/` | S2.2, S2.6, S2.7, S3.2, S6.6, S7.2, S7.6, S7.8, S8.1, S8.2, S8.3 |
| `docs/ui/player/controls/current_time/`, `duration/` | S2.4 |
| `docs/ui/player/controls/play_button/` | S2.2 |
| `docs/ui/player/controls/seek_thumbnail/` | S2.3 |
| `docs/ui/player/controls/volume/` | S2.5 |
| `docs/ui/player/menus/` | S2.9, S8.8 |
| `docs/ui/player/miniplayer/` | S4.10, S8.5 |
| `docs/ui/player/panels/audio/` | S7.7, S7.8, S7.9 |
| `docs/ui/player/panels/video/` | S7.2, S7.3, S7.4, S7.5 |
| `docs/ui/player/panels/subtitles/` | S6.1 |
| `docs/ui/library/sidebar/` | S4.5, S4.6, S4.9, S4.10, S4.11 |
| `docs/ui/library/media_cards/` | S4.6, S3.5 |
| `docs/ui/library/menus/` | S4.7, S4.8 |
| `docs/ui/dialogs/add_subtitles/` | S6.4 |
| `docs/ui/dialogs/keyboard_shortcuts/` | S2.10.4, S5.6 |
| `docs/ui/dialogs/new_playlist/` | S3.5 |
| `docs/ui/dialogs/open_network_stream/` | S4.11 |
| `docs/ui/settings/` | Stage 5 |
| `docs/icons/` | S0.4.13 (import as needed per story) |

