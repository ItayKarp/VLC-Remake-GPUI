# ADR 0005 — Updates

- **Status:** Accepted.
- **Story:** S9.5
- **Date:** 2026-11-25

## Context

Settings → General has "Check for updates automatically", off by default as the design shows.
With it on, the app should find a new version, show its release notes, download it, check it
wasn't tampered with, and install it without interrupting playback, then come back to the
previous version if the new one doesn't start. Releases are GitHub releases that CI publishes
from `main` (`ci/release.sh`), in the formats S9.4 builds: a DMG on macOS, an MSI on Windows,
an AppImage, `.deb`, `.rpm` and a Flatpak on Linux.

## Decision

1. **A signed manifest.** Each release carries `update-manifest.json`: the version, the release
   page (its notes) and one package per installable format, each with its size and SHA-256.
   `update-manifest.json.sig` beside it is an Ed25519 signature of the manifest's exact bytes,
   in hex. The app reads `releases/latest/download/update-manifest.json` over HTTPS (through the
   proxy in Settings → Network), checks the signature against the public key it was built with
   (`RVLC_UPDATE_PUBLIC_KEY`, hex), then trusts the hashes it lists. Every link must be `https`
   (debug builds also take `http://localhost`, to try it out). `ring`, which rustls already
   builds, does the signature and the hash, so no crypto is added. A build without a key trusts
   nothing and finds no update. A manifest replayed from an older release can't downgrade: only
   newer versions are offered.
2. **Keys and signing.** `cargo run --example update_manifest -- keygen <file>` makes the key
   pair; the private key stays out of the repository (a CI secret). `… -- sign dist` writes and
   signs the manifest for the packages in `dist/`; `ci/release.sh` runs it when
   `RVLC_UPDATE_SIGNING_KEY` (or `RVLC_UPDATE_SIGNING_KEY_FILE`) is set.
3. **What installs how** depends on how the running app was installed:
   - a macOS app bundle: `packaging/macos/build-app.sh` also makes `…-macos-<arch>.app.zip`
     (`ditto`, which keeps the signature). The update is unzipped beside the app, hidden
     (`.VLC.app.update`), and swapped in with two renames when the app quits; the previous
     version is kept as `.VLC.app.previous` until the update has started;
   - an AppImage (`$APPIMAGE`): downloaded beside it and swapped in the same way;
   - the MSI (an optimized build under Program Files): `msiexec /i … /passive` starts as the app
     quits; Windows asks for elevation;
   - anything else (`.deb`, `.rpm`, Flatpak, a development build) is updated by its package
     manager or developer: the app only says the version is out and links its release page.
   An install folder the app can't write to (an app in `/Applications` of another user) is
   treated like the last case.
4. **Never interrupt.** Checks and downloads run off the UI thread, 30 s after the start and
   then once a day (and as soon as the setting is turned on); installing happens only at quit.
   No internet, a proxy refusing, a bad signature or a corrupt download are logged and left to
   the next check. The main window says "VLC x is ready. It installs when you quit." (or "is
   available") with **What's New** and **Skip This Version**; a skipped version, and older
   ones, aren't offered again.
5. **Rollback (S9.5.2).** `updates.toml` (next to the local settings) records the update
   installed at quit as on trial. Each launch of it counts a start before the app starts; five
   seconds of running, or a clean quit, ends the trial and removes the previous version. A
   launch that finds two starts counted puts the previous version back (renaming it into
   place, or on Windows uninstalling the update and installing the kept MSI of the previous
   version), skips the failed version, starts the previous version and exits; that one then
   says "VLC x didn't start, so this version was put back."

## Consequences

- Releases need the signing key in CI and the packaging scripts run for each platform; until
  then a release has no manifest (or one with no packages) and apps find nothing to install.
- A failure before `main` runs (a missing dynamic library) can't count a start, so it can't be
  rolled back; a launcher process would be needed for that.
- Windows can only go back if the previous version was itself installed by the updater (its MSI
  is kept); a first update from a manual install can't be rolled back.
- `.deb`, `.rpm` and Flatpak installs never update themselves, which is what those formats
  expect.
