# ADR 0003 — Extensions and skins

- **Status:** Accepted.
- **Story:** S8.14 (decides D19 in PRD Appendix B; affects S5.4.4 and S5.10.4)
- **Date:** 2026-11-24

## Context

The Settings designs have two things with nothing behind them: Settings → Extensions
(`extensions.png`, "Add-ons & Scripts": an add-ons list with search, Find more extensions, Add,
Remove and Load add-ons on startup) and Settings → Interface's Skin section (Use a custom skin,
Skin file). The PRD asked for a product decision before any of it is built: what an extension
is, what a skin is given the custom layouts (S8.7), or neither in 1.0.

## Options considered

1. **Lua extensions, like VLC.** VLC's Lua scripts (playlist parsers, service discovery,
   extensions with dialogs) run inside libVLC 3 through its `lua` module, against VLC's own
   Lua API and Qt-era dialog calls. Ours draws its interface in GPUI, so libVLC can host the
   parsers and discovery scripts but not the extensions that build dialogs or read the
   playlist, which is the app's own queue (S3.1). Supporting them means a second API surface,
   kept in step with VLC's, for scripts that run unsandboxed with the user's rights.
2. **WASM plugins of our own.** A sandboxed runtime (Wasmtime or Wasmer) with a versioned host
   API: what a plugin may read and change in the queue, the library and the player, and how
   it draws anything. Sandboxing, API versioning and crash isolation all come with it, but the
   API is a product of its own to design, document and keep stable, and nothing exists to
   write plugins against yet.
3. **Skins.** VLC's skins2 format describes a fixed bitmap layout for the old interface and
   doesn't map onto GPUI views. A skin of ours would be the theme (S5.3: light, dark, accent)
   plus a layout from the custom-layout system (S8.7), which isn't built.
4. **Neither in 1.0.** Keep the settings, hide the rows.

Whatever is chosen, a "Find more extensions" catalog means a hosted service, signing and
review, which no story covers.

## Decision

**Defer extensions and skins past 1.0 (option 4).** Neither is in the release; the rows that
would configure them are hidden behind Cargo features, off by default, as every unbuilt row is:

- `skins` adds Interface's Skin section (S5.4.4, `prefs.interface.use_custom_skin` and
  `local.skin_file`). Nothing reads them.
- `extensions` lists the Extensions page (S5.10.4) in the Settings window's sidebar, where it
  says "Nothing to set here yet." Without it the page isn't listed, searched or stepped to,
  and a Settings window saved on it opens on General.

## Consequences

- The Settings sidebar's System group has three pages, one fewer than the designs; the
  screenshot tests allow for it.
- Nothing in 1.0 runs third-party code, so there is no sandbox, API version or catalog to
  secure.
- When extensions come back, WASM (option 2) is the starting point: it gives sandboxing and
  crash isolation that Lua inside libVLC doesn't, and a versioned API of our own instead of
  VLC's. A skin should be a saved theme plus a custom layout once S8.7 exists, not VLC's
  skins2 files. Each is a story of its own that turns its feature on.
