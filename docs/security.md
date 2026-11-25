# Security review (PRD S9.7)

VLC opens files and links other people made, reads text from servers it doesn't control, and
can listen on the network. This page records what was reviewed for S9.7, what each part relies
on, and what to do again before each release. What VLC keeps and sends is in
[`privacy.md`](privacy.md).

## What can attack VLC

| Input | Who writes it | Read by |
|---|---|---|
| Media files and streams | anyone | libVLC (demuxers, decoders) |
| Subtitles (`.srt`, `.vtt`, `.ass`, `.ssa`, `.sub`) | anyone | `engine/cues` (our parsers), libVLC for the rest |
| Playlists (`.m3u`, `.pls`, `.xspf`, `.cue`, `.asx`) | anyone | `opening/playlist` |
| Tags, chapter and track names, a stream's own title | whoever made the file or runs the server | shown in the window, lists and OSD |
| Podcast feeds | the feed's server | `podcasts/feed.rs` |
| Update manifests | GitHub, checked against a signature first | `updates/manifest.rs` |
| Settings files | the user (or anything that can write their files) | `settings/` |
| Web remote requests | anyone on the local network | `remote/http.rs` |
| A second launch's forwarded files | other processes of the same user | `app/instance` |

VLC reads no `.nfo` file: the library only recognizes the extension to leave the file out of
its lists.

## S9.7.1 Media parsing

**libVLC** does most of the parsing, so it must be current. `engine/libvlc/advisories.rs` holds
`SECURITY_FLOOR`, the first release with every published VideoLAN security bulletin fixed (3.0.23,
January 2026: 3.0.22's bulletin of December 2025 covered 3.0.21 and earlier, and 3.0.23 fixed
more). Loading an older libVLC logs a warning. The macOS, Windows, AppImage and Flatpak packages
carry their own libVLC; the `.deb` and `.rpm` use the distribution's, which may be older.

Before each release:

1. Read <https://www.videolan.org/security/> for bulletins since the last release.
2. If there is one, update `vendor/libvlc/<os>-<arch>`, the Flatpak manifest's VLC source
   (`packaging/linux/flatpak/org.redesignedvlc.app.yml`, which a test checks against the floor),
   and raise `SECURITY_FLOOR`.

**Our own parsers** are fuzzed by `src/fuzz.rs`, a small mutation fuzzer that runs in
`cargo test` (1,500 inputs per target): subtitles, playlists and their text decoding, podcast
feeds, settings files, update manifests and web remote requests. A target fails when its parser
panics or takes over 2 s on one input. For a longer run:

```bash
RVLC_FUZZ_ITERATIONS=200000 RVLC_FUZZ_SEED=$RANDOM cargo test --release --features runtime_shaders fuzz
```

A failing input is saved as `rvlc-fuzz-<target>-<round>.bin` in the temp folder (a long run
also keeps `rvlc-fuzz-<target>-last.bin`, for a crash that aborts the process), and
`RVLC_FUZZ_REPLAY=<file>` runs that target on it again. The first long run found a stack
overflow: a subtitle timestamp recursed once per leading `-`, so a file with a long run of them
crashed VLC; timestamps also overflowed on huge hour counts. Both are fixed and tested.

The parsers also have limits of their own: playlists are read up to 16 MB, 100,000 items, 8
levels of nesting and 10 s; web remote requests up to 8 KB of head and 4 KB of body.

## S9.7.2 Links and text from media

- **Only media schemes open.** `app/media/scheme.rs` lists them (`http`, `https`, `rtsp`,
  `rtsps`, `rtmp`, `rtmps`, `mms`, `mmsh`, `udp`, `rtp`, `smb`, `ftp`, `ftps`, `sftp`, `file`). The
  player refuses any other link before libVLC sees it ("VLC doesn't open this kind of link."), so
  `javascript:`, `data:` and libVLC's own access modules (`fd://`, `imem://`, `concat://`, …)
  can't be reached from a playlist, the clipboard, the command line or a forwarded launch. Open
  Network Stream already refused them.
- **Playlist files can't start a capture.** A playlist may list a disc, but entries naming a
  camera or the screen are left out, as are links that aren't media.
- **No libVLC options from outside.** VLC never passes a playlist's `#EXTVLCOPT` lines on.
  Discs and capture devices are stored with the options VLC opened them with; read back (from
  the session, a saved playlist or a forwarded launch), only VLC's own option names are kept,
  so `:sout=` (which writes files) can't be slipped in.
- **Text from media is one plain line.** Titles, artists, albums, chapter, title and track
  names pass through `engine/display_text.rs`: control characters and line breaks become
  spaces, the bidirectional embeddings, overrides and isolates are dropped (they make
  `exe.mp4` read as `4pm.exe`), and the text is cut at 512 characters. Everything shown on the
  OSD goes through it too. GPUI draws text as text, so markup in a title is never interpreted;
  the web remote's page sets its text with `textContent` under a strict content security policy.

## S9.7.3 Credentials

Passwords and tokens live only in the OS keychain (`privacy/keychain.rs`, the `keyring` crate):
the last.fm password, the HTTP proxy password, the OpenSubtitles password and sign-in token, and
the web remote password. Settings rows for them are masked fields that write to the keychain
and are never read back into the field.

A link may carry a user name and password (`https://me:secret@host/live`). It plays as typed,
is shown with the password masked, and is never written down with them:
`Media::to_stored` drops them for the history, the session file, saved playlists and the
hardware-decoding crash marker; Open Recent and Recent Streams leave such links out. Database
migration 15 (`db/scrub.rs`) takes them out of rows earlier builds wrote. For `udp://` and
`rtp://` what comes before `@` is a source address, not an account, and is kept.

The OpenSubtitles API key is compiled in from `RVLC_OPENSUBTITLES_API_KEY` and is not a user
secret; no key is in the repository.

## S9.7.4 Network listeners

VLC listens only when asked, and never without a way to keep strangers out:

| Listener | Default | Reach | Sign-in |
|---|---|---|---|
| Web remote (`remote/`) | off | local networks only: loopback, private and link-local addresses; the internet is refused even through a forwarded port | a password from the keychain is required (there is no way in without one); five wrong ones shut an address out for a minute; compared in constant time; a random 256-bit session cookie, `HttpOnly`, `SameSite=Strict` |
| Stream over HTTP or RTSP (`engine/stream_out.rs`) | only while streaming | **Only this computer** by default; **This network** listens on every interface | none on this computer alone; shared with the network, a random access code (about 70 bits) that the copied link carries (`http://vlc:<code>@…/stream`) |
| Stream over UDP | only while streaming | sends to one address; listens on nothing | — |
| Single-instance socket (`app/instance`) | always | this user: a per-user folder, and on Unix the socket file is `0600`; on Windows a pipe named per user and session | — |

The web remote listens on every interface because it is for a phone, but answers only local
networks, only by an IP address, `localhost` or a `.local` name (against DNS rebinding), and
takes changes only from its own page (`Origin` must match), with `X-Frame-Options: DENY`,
`nosniff`, `no-store` and a strict CSP. Its traffic is plain HTTP on the local network, so a
password typed there can be read by anyone able to watch that network; this is said here rather
than solved with a self-signed certificate phones would warn about.

## Not covered yet

- A user-visible notice for an old system libVLC (today only the log says so).
- Fuzzing in CI on a schedule; today it runs in `cargo test` with a short count, and long runs by
  hand before a release.
- Old pages of the library database can still hold scrubbed text until SQLite reuses them.
