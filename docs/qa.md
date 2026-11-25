# QA: the test media corpus and the release matrix (S9.8)

Automated tests run on one Linux arm64 agent with no display and no libVLC (see the
`Jenkinsfile`). Everything that needs real media, a real OS or a real screen is checked here:
against the **test media corpus** (S9.8.1), on each **OS** of the matrix (S9.8.2) and on each
**display** setup (S9.8.3). A release is ready when its QA pass (below) is recorded.

## The test media corpus (S9.8.1)

The corpus is media made to break a player: the S1.1.3 videos, subtitles in fourteen
encodings, broken files, odd names and very long or large files. It is generated from
synthetic sources only (FFmpeg's test patterns and tones, and dialogue written for it), so
nothing in it is copyrighted, and it is **kept out of this repository**: the archive lives in
an artifact store and this repository pins it.

| Script | Does |
|---|---|
| `tools/corpus/make.sh [--huge] [folder]` | Makes the corpus (in `target/corpus` by default) with `manifest.toml` and `SHA256SUMS`. Files already made are kept. Needs ffmpeg (libx264, libx265, libopus), iconv and shasum. |
| `tools/corpus/verify.sh [folder]` | Checks every file against `SHA256SUMS`. |
| `tools/corpus/pack.sh [folder]` | Archives it as `dist/corpus/rvlc-corpus-<date>.tar.gz`, with its SHA-256, for the artifact store. |
| `tools/corpus/fetch.sh [folder]` | Downloads the archive `tools/corpus/corpus.lock` pins, checks it, unpacks it and verifies it. |

**Where it lives.** Releases of
[ItayKarp/VLC-Remake-GPUI-corpus](https://github.com/ItayKarp/VLC-Remake-GPUI-corpus), one
archive each (under GitHub's 2 GiB asset limit); the first is `corpus-2026-11-25` (976 MB).
To publish a new corpus: run `make.sh` and `pack.sh`, make a release `corpus-<date>` there
with the archive, and put its link and SHA-256 in `tools/corpus/corpus.lock` in the same pull
request as any test that needs the new files.

### What is in it

| Folder | Files | The app should |
|---|---|---|
| `video/` | The S1.1.3 corpus: 8K and 4K60 HEVC 10-bit, HDR10, HLG, VFR with rotation, anamorphic DVD, 1080i, 1919×1079, 240 fps, audio only, one picture, colour bars, tracks, two programs. | Play each at its length. |
| `reference/` | Clean 6 s clips (MP4 with its index at the end and at the start, MKV) and a 2 s clip, which the broken and named files are cut from. | Play them. |
| `broken/` | An empty file, random bytes, text named `.mp4`, a Matroska header alone, MP4s cut in half (index at the end: unreadable; index first: half plays), 32 KiB of noise mid-file, Matroska named `.mp4`; binary, empty and nonsense subtitles; playlists of missing files, one listing itself, unclosed XML, a BOM with Windows line ends. | Refuse or play each without hanging or crashing. |
| `subtitles/` | One SRT each in UTF-8, UTF-8 with a BOM, UTF-16LE and UTF-16BE (with BOMs), and the code pages windows-1250 (Czech), -1251 (Russian), -1252 (French), -1253 (Greek), -1255 (Hebrew), -1256 (Arabic), Shift_JIS, GBK, Big5 and EUC-KR. | Guess each encoding (S6.1.6) and show the text right. |
| `names/` | The 2 s clip named with spaces, a leading dash, `#`, `%20`, `&`, `;`, quotes, brackets, many dots, an upper-case extension, no extension, NFC and NFD accents, Hebrew, Arabic, Japanese, Korean, emoji, zero-width and direction marks, 200 characters, a path past 260 characters, and a film with its English and French subtitles beside it. | Open, list, store and show each; find the film's subtitles. |
| `names-unix/` | Names Windows refuses: `? : * " < > \|`, a backslash, `CON`, a trailing dot, a tab. Left out on Windows by `fetch.sh`. | The same, on macOS and Linux. |
| `long/` | Ten hours of Opus, a 39 MB subtitle file (past the app's 32 MiB reading limit), playlists of 100,000 and 100,001 entries. | Play and seek the ten hours; leave the big subtitle to libVLC; import both, the second stopping at the 100,000-item limit. |
| `huge/` | With `--huge` only: a 4.7 GB, 24-minute video, for 32-bit sizes and offsets. Not packed (too big for a release asset); made where it is used. | Play and seek it. |
| `real/` | Samples that can't be generated, added by hand with their entries in `real/manifest.toml` (which `make.sh` appends): only files that may be redistributed. **Missing:** a Dolby Vision profile 5 sample (S1.1.3). | As each entry says. |

`manifest.toml` lists each file with its kind (`media`, `subtitle`, `playlist`), what the app
should do (`plays`, `fails`, `reads`, `skipped`) and what tests check: a media file's
`seconds`, a subtitle's `encoding` and a line of its `text`, a playlist's `items` and
`missing`, a video's `subtitles`.

### Running the corpus tests

```sh
tools/corpus/make.sh            # or tools/corpus/fetch.sh
cargo test --features runtime_shaders corpus -- --ignored --test-threads 1
```

`RVLC_CORPUS_DIR` names a corpus elsewhere. The tests (`src/corpus/tests.rs` and
`src/engine/libvlc/corpus_tests.rs`) check that each subtitle is read in its encoding and
shows its text; that each playlist imports with its item and missing counts, or is refused;
that each media file is taken for media whatever its name; that each name survives being
stored (history, session, playlists) and is shown without control characters; that the film's
subtitles are found; and that libVLC opens each media file, or fails on it, as listed, with
its length, within 20 s. They report every failure at once. libVLC's part opens files paused
and muted, so the run makes no sound.

## The OS matrix (S9.8.2)

Each cell installs the package its users would, then runs the corpus tests (from a checkout,
where there is a toolchain) and the smoke pass below. ✅ passed, ⚠️ passed with notes,
❌ failed, — not run yet.

| OS | Arch | Package | Display / audio | Watch for | Last pass |
|---|---|---|---|---|---|
| Windows 10 22H2 | x64 | MSI | — / WASAPI | SmartScreen on an unsigned or new certificate; D3D11 hardware decoding; per-monitor DPI. | — |
| Windows 11 | x64 | MSI | — / WASAPI | Snap layouts, the Windows 11 title bar, "Default apps" listing VLC. | — |
| Windows 11 | arm64 | MSI (x64, emulated) | — / WASAPI | libVLC 3 has no arm64 Windows build, so it runs under emulation: speed and hardware decoding. | — |
| macOS 13 Ventura | arm64, x64 | DMG | — / CoreAudio | The oldest release in the matrix: the menu bar, the camera prompt (Open Capture Device). | — |
| macOS 14 Sonoma | arm64 | DMG | — / CoreAudio | | — |
| macOS 15 Sequoia and later | arm64, x64 | DMG | — / CoreAudio | Gatekeeper on a notarized build; the local network prompt (S9.7.4 sharing). | ⚠️ macOS 27.0.1, arm64, from a checkout: the corpus tests only (below), not the smoke pass |
| Ubuntu LTS | x64, arm64 | .deb, AppImage, Flatpak | X11 and Wayland (GNOME) / PulseAudio or PipeWire | Fractional scaling on Wayland; the FileChooser portal in the Flatpak; the distribution's libVLC for the .deb. | — |
| Fedora (current) | x64 | .rpm, Flatpak | Wayland (GNOME) / PipeWire | libVLC from RPM Fusion for the .rpm; PipeWire device switching; SELinux on the single-instance socket. | — |

### The smoke pass (each OS cell)

1. Install the package; first launch shows onboarding; quit and launch again skips it.
2. Open a file by double-clicking it (file association), by dragging it on the window, from
   File → Open and from the command line; each corpus folder once.
3. Play, pause, seek, frame step, change speed; the ten-hour file's time display and seeking.
4. Subtitles: a video with `subtitles/` files dropped on it shows each language right;
   `names/Movie – Director's Cut …` finds both of its subtitles.
5. Add the corpus folder to the library: it scans without hanging on `broken/`, and odd names
   show as named.
6. Hardware decoding on and off with `video/2160p60-hevc10.mp4`; HDR files on an SDR screen.
7. Audio: switch output device while playing; media keys; sleep is prevented while a video
   plays.
8. Picture in picture, fullscreen on each monitor, the tray icon (Windows, Linux).
9. Updates against a local server (`AGENTS.md`), where the package updates itself.
10. Uninstall: settings and the library stay unless asked to remove them.

## The display matrix (S9.8.3)

| Setup | Check | Last pass |
|---|---|---|
| 100 % scaling | Text and icons crisp, nothing blurry from being scaled up. | — |
| 125 % and 150 % scaling (Windows, Linux fractional) | No blurry text, no one-pixel gaps or doubled lines in borders, the seek bar's thumb centred. | — |
| 200 % scaling (Retina) | Icons drawn at 2×, video drawn at the screen's pixels. | — |
| Mixed DPI, two monitors | Moving the main window, the video window and PiP between monitors redraws each at the new scale, keeps their size, and fullscreen fills the monitor it is on. | — |
| An HDR monitor | `video/1080p-hdr10.mkv` and `1080p-hlg.mkv` play in HDR where the OS allows, the UI isn't dimmed or blown out beside them, and an SDR video next to them looks right. | — |
| A 1366×768 laptop | The main window, Settings, onboarding and every dialog fit; the player's controls don't overlap at the smallest window size. | — |

## Results so far

**2026-11-25, macOS 27.0.1, M-series Mac, from a checkout (libVLC 3.0.23 from `vendor/libvlc`):**
the corpus tests pass, all 81 files of the manifest, and with `--huge` the 4.7 GB file too (its 1,420 s length right). Every subtitle
encoding is guessed right; libVLC plays every file listed as playing within 1 s of its length
(MPEG-PS and transport streams report theirs only once playing, 9.98–10.68 s for 10 s), and
fails the rest without hanging: the empty file as an empty file, random bytes, text and a cut
MP4 without its index as having no playable streams, the Matroska header alone by ending at
once. The run found one bug, fixed: a file name with a tab or a bidirectional override was
shown as it was; `Media::display_name` now cleans it like a tag (S9.7.2). Recorded as found,
not changed: an XSPF cut short gives no items (the XML reader stops at the error), and a
playlist past 100,000 entries imports its first 100,000.

## Recording a pass

Open an issue from the **QA pass** template (`.github/ISSUE_TEMPLATE/qa-pass.md`) for the
release, tick each cell as it is run with what was found, and update the **Last pass** columns
above in the release's pull request. A release goes out with every row run or with the rows
not run named in its notes.
