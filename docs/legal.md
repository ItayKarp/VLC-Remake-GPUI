# Legal and branding (PRD S9.9)

What the app owes others before it is released: VideoLAN's trademarks, the licenses of what
it ships, codec patents, and the terms of the online services it can use. This page records
the facts and what was done; it is not legal advice, and the items marked **counsel** need a
lawyer's review before a commercial or wide public release.

## Release blockers

Until each is settled, no public release (a GitHub release from `main`, a store listing or a
package in a distribution):

1. **The name and icon** (S9.9.1): written permission from VideoLAN to call the app "VLC" and
   use the cone, or a distinct name and icon.
2. **GPL plugins** (S9.9.2): counsel confirms how the app and libVLC's GPL plugins are
   distributed together, or the GPL-only plugins are left out.
3. **Codec patents** (S9.9.3), if the app is distributed commercially or bundled with
   hardware: counsel's review.

## S9.9.1 Trademarks: "VLC" and the cone

"VLC", "VideoLAN" and the cone logo are trademarks of VideoLAN. This app ships as **VLC**: the
window and menu titles, About VLC, 114 messages in `assets/i18n/messages.pot`, the macOS
bundle name (`packaging/macos/build-app.sh`), the Windows ProductName (`build.rs`), the MSI,
the `.desktop` file and the Flatpak; the icon in `assets/app-icon/` is the cone. The crate,
bundle id (`org.redesignedvlc.app`), data folders (`RedesignedVLC`) and the OpenSubtitles and
last.fm User-Agents already use "RedesignedVLC".

VideoLAN's legal page (<https://www.videolan.org/legal.html>) sets how its name and logo may be
used. This app is a new interface on libVLC, not VLC itself, so shipping it as "VLC" with the
cone needs VideoLAN's written permission.

**Decided (2026-11-25):** keep the name and icon for now and ask VideoLAN. S9.9.1 stays open
until the answer comes; if it is no, the app takes a distinct name and icon before release
(every user-visible "VLC" goes through `tr!`, so the strings change in one pass, and the
packaging files listed above).

Draft request, to the trademark contact given on VideoLAN's legal page:

> **Subject:** Permission to use the VLC name and cone for a libVLC-based player
>
> Hello,
>
> I am building an open-source media player (MIT license,
> https://github.com/ItayKarp/VLC-Remake-GPUI) that plays media with an unmodified libVLC
> 3.0.23 or later, dynamically linked, with a new interface written in Rust. It is not
> affiliated with VideoLAN.
>
> I would like to release it as "VLC" with the cone icon, on Windows, macOS and Linux. Would
> VideoLAN allow that, and under what conditions (for example a different name such as
> "<name> for VLC", a modified cone, or a statement that it is not an official VideoLAN
> product)? If not, I will choose a different name and icon before any release.
>
> The app credits VideoLAN and the VLC authors in its About dialog, ships libVLC's LGPL notice
> and links to libVLC's source.
>
> Thank you,
> Itay Karp

## S9.9.2 Licenses

**This app** is MIT (`LICENSE`, `license = "MIT"` in `Cargo.toml`).

**Rust crates.** `tools/licenses/generate.sh` lists every crate in `Cargo.lock` that the app is
built from (dev-dependencies left out, every OS included) under its license with the license
text, using cargo-about (`cargo install cargo-about --locked --features cli`) and
`about.toml`, into `assets/licenses/third-party.html`. `about.toml` lists the licenses allowed,
in the order one is picked from a choice (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode, MPL-2.0 and
other permissive ones); a crate under any other license stops the generation until it is
reviewed. In use today: MIT, Apache-2.0 (also with the LLVM exception), BSD-2/3-Clause, ISC,
Zlib, 0BSD, Unicode-3.0, BSL-1.0, CC0-1.0, CDLA-Permissive-2.0 and MPL-2.0 (three crates: their
own files stay under MPL and their source is on crates.io). GPUI is Apache-2.0. Run the script
after changing dependencies; `licenses::tests` fails while the page lists a crate at a version
`Cargo.lock` doesn't have, or misses a direct dependency.

**libVLC.** libVLC and libVLCcore are LGPL-2.1-or-later and loaded as separate libraries (S1.1),
which a user can replace. Some plugins shipped with them, and the libraries inside them, are
GPL-2.0-or-later; on macOS these include `x264`, `x26410b`, `x265`, `postproc` (FFmpeg's), `dvdnav`,
`dvdread`, `faad`, `mad` and `sid` (libsidplay2). The licenses page starts with libVLC's notice, the
LGPL-2.1 and GPL-2.0 texts (`packaging/libvlc/licenses/`, from the SPDX list), where VideoLAN
publishes the source of the version shipped, and where this app's source is.

**Shown in About VLC.** Its **Licenses** button writes the page (built into the app with
`include_str!`) to the cache folder and opens it in the browser, so every package (MSI, DMG,
AppImage, Flatpak, `.deb`, `.rpm`) carries it.

**Counsel:**

- The GPL plugins. The app's own code is MIT and GPUI Apache-2.0; distributed together with
  GPL-2.0-or-later plugins, the whole may have to be offered under the GPL (Apache-2.0 is
  compatible with GPLv3, not GPLv2-only; the plugins listed are "or later"). The alternatives
  are to confirm that, or to add the GPL-only plugins to
  `packaging/libvlc/excluded-plugins.txt`, which loses H.264/H.265 encoding for Convert and
  Stream, DVD menus, and the FAAD, MAD and SID decoders where FFmpeg doesn't cover them.
- The LGPL's source offer. A link to VideoLAN's tarball of the same version is what is done
  now; the Flatpak builds that tarball unchanged. Packages built from VideoLAN's binaries
  (macOS, Windows) should keep the exact version they ship, which About VLC reports.

## S9.9.3 Codec patents (counsel)

libVLC decodes and encodes formats covered by patent pools in some countries: H.264/AVC,
H.265/HEVC, AAC, AC-3 and E-AC-3, DTS, MPEG-2, VC-1, and others. VideoLAN distributes VLC from
France and explains on its legal page why it holds no patent licenses there; that reasoning
doesn't carry over to other distributors or countries. Some points for the review:

- Free distribution from the project's GitHub releases is the case VideoLAN itself is in;
  selling the app, bundling it with hardware, or distributing it from a company in the US,
  Japan or Korea changes that.
- Hardware decoding through the OS (VideoToolbox, D3D11/DXVA, VA-API) uses the OS vendor's
  licensed decoders; libVLC's software decoders (FFmpeg) are not licensed by the app.
- Leaving out the encoders (`x264`, `x265`) and patent-encumbered decoders per region is
  possible with `excluded-plugins.txt`, per package; nothing is left out for this reason today.
- Linux distributions (Fedora, and Debian's main archive for some codecs) have their own
  rules; the `.deb` and `.rpm` use the distribution's libVLC, which follows them.

Nothing is decided here; S9.9.3 stays open until a release is planned that needs it.

## S9.9.4 Online services

| Service | Terms | What the app does |
|---|---|---|
| **OpenSubtitles** ([terms for integrators](https://opensubtitles.stoplight.io/docs/opensubtitles-api/branches/master/f43c0749230b3-terms-of-use-for-integrators), read 2026-11-25) | One API key per app, never one per user; no username or password shipped; a `User-Agent` of the form `App v1.0`; on a free key, 5 downloads per IP a day without signing in, then the user's own account; apps on a free key show "Subtitles provided by OpenSubtitles.com", linked, where subtitles are searched or picked; downloads kept only for the user who asked; no bulk downloading. Commercial use allowed. | One key built in (`RVLC_OPENSUBTITLES_API_KEY`); `User-Agent: RedesignedVLC v<version>`; searches and downloads only for the video the user is adding subtitles to, saved next to it or in the app's folder; signing in is the user's own account, its password in the keychain. **Added:** the credit line under the search in Add Subtitles, linking to opensubtitles.com, a tab stop. A store listing must carry the same credit. |
| **last.fm** ([API terms](https://www.last.fm/api/tos), [scrobbling](https://www.last.fm/api/scrobbling), read 2026-11-25) | Non-commercial use only without a commercial agreement (clause 3); credit and a link to last.fm, with an approved "powered by AudioScrobbler" button, where Last.fm Data is used (2.7, 4.2.2); last.fm's marks only with written approval (7); scrobble tracks over 30 s after half their length or 4 minutes; tags, never file names; batches of at most 50; no automatic corrections. | Scrobbling only: the app sends plays and shows no Last.fm Data, so no credit is shown and no logo used (the setting names last.fm in words). Scrobbles follow the rules (`scrobble/listening.rs`, `api.rs`): tags only, 30 s, half or 4 minutes, 50 per batch, retried only while last.fm is down or limiting (codes 11, 16 and 29, server errors). The API key and secret are built in (`RVLC_LASTFM_API_KEY`, `RVLC_LASTFM_API_SECRET`), as for every desktop scrobbler. **Counsel/last.fm:** a commercial release needs last.fm's agreement (`partners@last.fm`). |
| **Libre.fm** and other last.fm-compatible services | Their own terms; the user types the address. | The same scrobbles. |
| **Cover art** | — | None is contacted. libVLC 3 fetches art online only through Lua scripts; none ship, and packages leave the Lua plugin out (`excluded-plugins.txt`), so "Fetch titles and cover art from the internet" finds art only in the file and its folder; the settings say so ("VLC has no online source
yet"). Choosing a provider (MusicBrainz and the Cover Art Archive, say) would bring their terms here. |
| **GitHub** (updates, Report on GitHub) | GitHub's terms; nothing beyond them for reading a release or opening an issue page. | See `docs/privacy.md`. |
