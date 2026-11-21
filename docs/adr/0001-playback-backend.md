# ADR 0001 — Playback backend and video presentation in GPUI

- **Status:** Proposed. macOS (Apple silicon) is measured; Windows and Linux are not yet, so
  D2 is decided for macOS only. Accept this record once the Windows and Linux rows of the
  measurement table are filled in.
- **Story:** S1.1 (decides D1 and D2 in PRD Appendix B)
- **Date:** 2026-11-21
- **Prototype:** [`examples/video_spike`](../../examples/video_spike/main.rs)

## Context

GPUI has no video element. The only zero-copy path, `gpui::surface()`, takes a macOS
`CVPixelBuffer`. Windows and Linux have nothing comparable. The designs draw controls,
popovers and auto-hiding overlays **on top of** the video, so a native child window that
covers part of the GPUI window (the "airspace" problem) does not work on its own.

## Decision

1. **Backend (D1): libVLC 3.0.x** (3.0.23 today), loaded at run time and wrapped behind
   the engine's backend trait (S1.2.7). Move to libVLC 4 when it ships a stable release.
   Its GPU output callbacks remove the copies and limits listed below.
2. **Presentation (D2), macOS:** path (b). libVLC's `vmem` callbacks write NV12 straight into
   IOSurface-backed `CVPixelBuffer`s from a `CVPixelBufferPool`, and GPUI draws them with
   `surface()`.
3. **Presentation (D2), Windows and Linux:** path (a), BGRA frames as GPUI `RenderImage`s,
   is the **working fallback** while we measure there. libVLC hands it NV12 and we convert to
   BGRA ourselves, with the right YCbCr matrix. On this Mac it passes the 4K60 acceptance
   criterion (0.45% dropped), but at twice path (b)'s CPU, and it cannot sustain 8K. If it
   fails 4K60 on Windows or Linux, we need path (d) there: an NV12/P010 texture primitive in
   GPUI's wgpu and DirectX renderers, upstreamed or carried as a patch. Decide after the
   Windows and Linux numbers are in.
4. Path (c), a native child window, is **rejected**. GPUI cannot draw over a foreign child
   view, and the designs need overlays on top of the video.

## S1.1.1 — Backend comparison

| Criterion | libVLC 3.0.x | libVLC 4 | libmpv (render API) | FFmpeg + own pipeline |
|---|---|---|---|---|
| Release status | Stable, maintained | Not released: nightlies and iOS alphas only; libvlcpp still lists its libvlc 4 branch as "in development" (July 2026) | Stable | Stable |
| Codecs and containers | Everything VLC plays | Same | FFmpeg-based, very broad | Broad, but we write demux, A/V sync and clock |
| Network (HTTP, HLS, DASH, RTSP, SMB, UPnP) | All, as modules (SMB and UPnP included) | Same | HTTP, HLS, DASH, RTSP through FFmpeg; no SMB browsing or UPnP discovery | Same as libmpv, minus the player logic |
| DVD, Blu-ray, audio CD | Yes (dvdnav, libbluray, cdda) | Yes | DVD and Blu-ray through libdvdnav/libbluray builds; menus are limited | Write it ourselves |
| SSA/ASS subtitles (libass) | Yes | Yes | Yes, best in class | Integrate libass ourselves |
| Designed filters (EQ, compressor, spatializer, stereo widener, video adjust, crop, deinterlace) | Every panel maps to an existing module | Same | Most, through FFmpeg filters | Through libavfilter, wired ourselves |
| HDR tone-mapping | Only in VLC's own OpenGL/libplacebo outputs, **not** through `vmem` (see limits) | libplacebo in its GPU outputs | libplacebo (excellent), but only through the OpenGL render API | Ours to write |
| Recording, snapshots, chapters, thumbnails | Yes | Yes | Yes (thumbnails are slower) | Ours to write |
| Getting frames to the host | `vmem` CPU callbacks (always one CPU copy) | D3D11/OpenGL/Metal output callbacks, no copy | Render API offers OpenGL and software only | Full control |
| License | LGPL-2.1+, fine with dynamic linking | Same | LGPL build possible (`-Dgpl=false`), GPL by default | LGPL build possible |
| Redistributable size | 82 MB on disk, 32 MB compressed (macOS arm64, 340 plugins) | Similar | About 30–60 MB with FFmpeg | About 20–40 MB |
| Rust bindings | Old (`vlc-rs`); we keep our own small run-time-loaded FFI (`libvlc.rs`, about 20 calls) | None | `libmpv2` is maintained | `ffmpeg-next`, `rsmpeg` |

**Why libVLC.** It is the only option where every designed panel (EQ, effects, SMB and UPnP
browsing, discs, recording, streaming, casting) maps to code that already exists. libmpv is
the strongest alternative: it has better HDR and subtitles, but no SMB or UPnP, and its
render API does not reach Metal or Direct3D. FFmpeg alone means writing a player.

## S1.1.2 — Presentation paths, measured

**Machine:** Apple M1 (8 CPU and 7 GPU cores, 16 GB), built-in 2560×1600 Retina display
at 60 Hz, macOS 27.0.1. **Setup:** libVLC 3.0.23 (official arm64 build), release build, a
1280×720 window with a translucent GPUI control bar drawn over the video. The spike moves the
mouse over a hover button every 100 ms through GPUI's event dispatch.

**Definitions**
- **Dropped:** frames libVLC dropped as late, plus frames we received but replaced before
  GPUI drew them, as a share of the frames libVLC offered.
- **CPU:** the whole process, decoder threads included; 100% = one core.
- **Paint:** main-thread time spent painting the video element, which includes the atlas
  upload on path (a).
- **Input→paint:** time from a synthetic mouse move to the next paint. It does not include
  the OS input queue.

| File (60 s) | Path | Shown fps | Dropped | CPU | Peak RSS | Paint p50/p95 | Input→paint p50/p95/max |
|---|---|---|---|---|---|---|---|
| 1080p60 H.264 | (a) CPU | 59.9 | 0.16% | 45% | 166 MB | 2.8/3.6 ms | 10.8/17.0/18.0 ms |
| 1080p60 H.264 | (b) surface | 59.9 | 0.16% | 24% | 132 MB | 0.0/0.0 ms | 8.7/16.3/16.6 ms |
| 2160p30 H.264 | (a) CPU | 30.0 | 0.05% | 41% | 402 MB | 6.1/6.8 ms | 11.2/22.6/24.0 ms |
| 2160p30 H.264 | (b) surface | 30.0 | 0.05% | 17% | 265 MB | 0.0/0.0 ms | 8.8/16.5/17.1 ms |
| **2160p60 HEVC 10-bit** | (a) CPU | 59.7 | **0.45%** ✓ | 105% | 930 MB | 5.7/6.2 ms | 11.8/16.7/23.9 ms |
| **2160p60 HEVC 10-bit** | (b) surface | 60.0 | **0.00%** ✓ | 48% | 788 MB | 0.0/0.0 ms | 8.4/16.2/16.6 ms |

**Acceptance (macOS):** path (b) plays the 4K60 sample for 60 s with 0.00% dropped frames.
The overlay's input→paint stays under 16.6 ms at p95 (one 60 Hz frame). This **passes**.
Path (a) passes too, at 0.45% dropped, but uses twice the CPU and has a 1-frame-longer
input→paint p95 at 4K30. Its rows were re-measured after it started converting NV12 to BGRA
itself (see the colour check). Before that, libVLC did the conversion, and path (a) dropped
4.16% at 4K60 with 128% CPU, and once stalled input for 892 ms at 4K30. The conversion
takes about 2.7 ms per 4K frame on one core (`yuvutils-rs`, NEON). One 1080p60 run, not in
the table, dropped 28%: nothing was painted for 14 s, which is what happens when the window
is covered. The two repeats dropped 0.19% and 0.16%.

**Paths (c) and (d)** were not prototyped. Path (c) is ruled out by the airspace problem
(decision 4). Path (d) needs GPUI changes, estimated below.

**Windows and Linux:** not measured yet; nothing in this checkout can run there. The spike
builds on both. Path (a) is the only path available there. Its macOS numbers pass 4K60
with little headroom, so slower CPUs may not.

### Path (d) cost estimate (GPUI change)

The surface pipeline already exists in GPUI's Metal renderer: two plane textures, one
YCbCr→RGB shader. `gpui_wgpu` (Linux, and Windows if it moves to wgpu) has the bind-group
plumbing for surfaces but deliberately skips them as macOS-only. Adding an NV12/P010 surface
source backed by a wgpu texture we upload into (or import as a D3D11 or dmabuf texture
later) is roughly 400–800 lines across `gpui` (the `SurfaceSource` variant),
`gpui_wgpu` (pipeline and shader) and `gpui_windows`. Upstream is active but breaks API often
(S0.8.2), so carrying it as a patch costs a rebase on every GPUI bump. Try to upstream it.

## S1.1.3 — Test corpus results (10 s each)

`examples/video_spike/make_test_media.sh` generates every file from FFmpeg test sources
(nothing copyrighted). `examples/video_spike/measure.sh` reruns the whole matrix, and
`examples/video_spike/screenshot.sh` captures the spike's window, converted to sRGB, for
the pixel checks below.

| File | (a) CPU | (b) surface | Notes |
|---|---|---|---|
| 8K HEVC (7680×4320, 30 fps) | **25% dropped**, 173% CPU, 2.1 GB RSS | 0.36%, 27% CPU | Path (a) cannot sustain 8K: painting (the atlas upload) takes 36 ms per frame. Under the 8192 px limit, so no downscale. |
| HDR10 (PQ, BT.2020, 10-bit) | 0% | 0% | Plays, but **not tone-mapped** (see limits). |
| HLG (BT.2020, 10-bit) | 0% | 0% | Same. |
| Dolby Vision profile 5 | — | — | Not generated: FFmpeg cannot encode it. Needs a real sample (S9.8.1). |
| VFR phone video, 90° rotation | **no video** | **no video** | libVLC 3 cannot rotate hardware frames (see limits). With software decoding: 1.5% / 0.37%. |
| Anamorphic DVD (720×480, SAR 32:27) | 0% | 0% | Shape is found only after playback starts (MPEG-PS), then shown at 16:9. |
| Interlaced 1080i H.264 (TS) | 0% | 0% | VideoToolbox refused it, so libVLC decoded in software. Deinterlaced on both paths (see screenshot checks). |
| Odd size (1919×1079, 4:2:0) | 0.71% | 0.35% | Plays. Edges are clean on both paths (see screenshot checks). |
| 240 fps (720p) | 75% "dropped" | 75% "dropped" | Expected: a 60 Hz display shows 60 of 240. Count these as display-limited, not dropped. |
| Audio only (FLAC) | OK | OK | No video output is created. |
| Single frame (PNG) | OK | OK | libVLC plays an image as a 10 s still. |

### Screenshot checks (macOS)

Each check plays a still or synthetic picture whose correct pixels are known, captures the
window, and compares 10×10-pixel patch averages. The references are computed from the
decoded YCbCr values with the standard formulas, not from another player.

**Colour** (`bars-709.mp4`: SMPTE HD bars, BT.709 limited range). Each path's capture was
fitted against three conversions; the number is the worst patch's error in 8-bit levels.

| Path | BT.709 limited (correct) | BT.601 limited | BT.601 full |
|---|---|---|---|
| (a) CPU, VideoToolbox decode | 32 | **2** | 20 |
| (a) CPU, software decode | 32 | **2** | 20 |
| (b) surface | 26 | 20 | **2** |
| (a) CPU, **converting NV12 ourselves** | **3** | 31 | 28 |
| (b) surface, **after the GPUI fix** | **2** | 32 | 26 |

- **Path (b)** showed black at 16–18 instead of 0, and 100% white at 235 instead of 255.
  Saturated colours were off by up to 26 levels (75% green showed (40, 241, 23) instead of
  (14, 223, 5)). GPUI's surface shader read video-range BT.709 as full-range BT.601.
  **Fixed** in our Zed fork (see limit 4). After the fix, HD bars are within 2 levels of
  BT.709, and SD bars (`bars-601-sd.mp4`, 720×480) are within 2 levels of BT.601 (30 off
  if read as BT.709).
- **Path (a)** had the range right (black 0, white 255, greys exact), but libVLC 3's
  conversion to BGRA uses **BT.601 for HD video**, ignoring the BT.709 tag. Saturated colours
  were off by up to 32 levels; neutral tones were exact. **Fixed** in the spike: path (a) now
  asks libVLC for NV12 and converts it to BGRA itself with `yuvutils-rs`, choosing the matrix
  like path (b). HD bars are within 3 levels of BT.709, and SD bars within 2 of BT.601.

**Odd-size edges** (`edges-1919x1079.mkv`: a 1-pixel red frame on grey, FFV1 4:2:0). On
both paths, the red frame reaches the outermost pixel on all four sides. There are no green
or black lines and no missing row or column. The centre is neutral: 128 on path (a), and
126 on path (b) (the range error). Path (a) had a 2–3 pixel cyan halo inside the red line,
from libVLC's bicubic chroma upsampling (swscale), normal for 4:2:0. Since path (a) converts
NV12 itself, the halo is gone: the red line fades straight into the grey (centre 129).

**Deinterlacing** (`1080i-motion.ts`: 59.94p motion woven into 29.97i fields). libVLC's
automatic mode detects the interlaced stream and inserts its `x` deinterlacer on both paths.
Combing score: adjacent-row difference minus the difference two rows apart, over a moving
band (0 means no combing). It is 5.11 on the raw frame, −0.37 after FFmpeg's yadif,
−0.15 on path (b) and −0.04 on path (a). Choosing a better method (yadif2x, bob) is a
quality setting for later.

**Not checked:** HDR. The corpus's HDR10 and HLG files are SDR test cards *tagged* as PQ
and HLG, so they have no correct appearance to compare with. That check needs real HDR
samples (S9.8.1).

## S1.1.4 — Shipping the runtime

- **macOS:** ship `libvlc.dylib`, `libvlccore.dylib` and `plugins/` from the official arm64
  and x86_64 builds inside `Contents/Frameworks` (or `Contents/MacOS/lib`). That is 82 MB
  for one architecture, about 160 MB universal (`lipo` the dylibs). Set `VLC_PLUGIN_PATH`
  before `libvlc_new`, and sign every dylib for notarization.
- **Windows:** ship `libvlc.dll`, `libvlccore.dll` and `plugins/` next to the executable,
  from the official 64-bit ZIP. Expect a similar size.
- **Linux:** depend on the distribution's `libvlc5` / `vlc-plugin-base` for `.deb`/`.rpm`.
  Flatpak and AppImage bundle the VideoLAN runtime.
- **Copy plugins with their modification times** (`ditto`, `cp -p`), or regenerate
  `plugins.dat` with `vlc-cache-gen`. libVLC rejects a plugin cache whose file times
  changed, then scans all 340 plugins at every start and prints one error per plugin.
- **Strip unneeded plugins.** The VLC.app build includes `macosx`, `osx_notifications`
  (wants Growl.framework), Sparkle-linked UI, `lua` and others that only fail to load in
  our process. Removing them saves size and log noise; the exact list is S9.4.4.
- **Load libVLC at run time** (`libloading`), as the spike does. CI and contributors then
  build without it, and a missing runtime becomes a clear error instead of a launch crash.

## Known limits and what each later story must do

1. **`vmem` proposes the coded size, not the visible one.** For example, 1920×**1088** for
   1080p H.264, and 1920×1090 for the 1080i sample. Answering with the visible size makes
   libVLC crop for free: its `cvpx` converter copies only the visible rows, with no
   swscale. So the engine must know the visible size **before** playback (parse the track
   first). If it learns the size only later (MPEG-TS and MPEG-PS report it once decoding
   starts), it must restart the video output or crop in the presenter. Otherwise the
   padding rows show. → S1.2, S1.4.1.
2. **Anamorphic pixels:** answer `vmem` with square-pixel display sizes (libVLC scales), or
   stretch the element to the display aspect when the shape arrives late. The spike does
   both. → S1.4.1.
3. **Rotation:** libVLC 3's `transform` filter rotates only planar YUV in software. With
   VideoToolbox (CVPN) or NV12 output, video output creation fails and no picture appears.
   GPUI's `img` and `surface` cannot rotate either. Until a GPU path exists: when the track
   says 90° or 270°, open with software decoding (`--codec=avcodec --avcodec-hw=none`),
   which costs CPU (41–52% at 1080p30 here). → S1.4.1. libVLC 4 or path (d) removes this.
4. **Colour.** Path (b) is **fixed**. GPUI hard-coded a full-range BT.601 surface shader
   and accepted only '420f' buffers. Our fork (`github.com/ItayKarp/zed`, branch
   `gpui-surface-colour`, one commit on upstream 2f808b3c) accepts '420v' too and converts
   with each buffer's `kCVImageBufferYCbCrMatrixKey` (BT.601, BT.709 or BT.2020). All GPUI
   dependencies point at that commit. The spike now writes '420v' buffers tagged BT.709 for
   pictures larger than 1024×576 and BT.601 otherwise, because libVLC 3 does not report a
   track's matrix; S1.4 should read the real tag where the container has one. The same patch,
   rebased on Zed's main, is on the fork's `gpui-surface-ycbcr` branch, ready to propose
   upstream so the fork can go. Path (a) is **fixed** too: libVLC 3 converts to BGRA with
   BT.601 only, so path (a) now asks for NV12 and converts with the same matrix choice
   (`yuvutils-rs`, SIMD on every target). → S1.4.
5. **HDR:** through `vmem`, libVLC converts 10-bit PQ or HLG to 8-bit NV12 or BGRA **without
   tone-mapping**, so HDR looks washed out on SDR displays. Real HDR needs a GPU path
   (libVLC 4 with libplacebo, or our own tone-mapping shader in a path (d) primitive).
   → S7 HDR story.
6. **Copies:** VideoToolbox decodes into a `CVPixelBuffer`, libVLC's `cvpx` converter copies
   it to NV12, and `vmem` copies it again into our buffer. That is two CPU copies per frame
   (unified memory keeps them cheap on Apple silicon). libVLC 4's output callbacks could
   hand the decoder's buffer straight to `surface()`.
7. **Path (a) memory:** each frame becomes a new `RenderImage` and atlas texture, and must be
   freed with `Window::drop_image`, or the atlas grows without bound. Peak RSS still
   reached 933 MB at 4K60 and 2.1 GB at 8K.
8. **Threading:** the `vmem` callbacks run serialized on libVLC's video-output thread (lock,
   copy, unlock, display). Frames reach GPUI through a one-frame mailbox that wakes the
   window. The spike calls the blocking `libvlc_media_parse` on the UI thread; the engine
   must use `libvlc_media_parse_with_options` asynchronously. → S1.2.4.
9. **Display-limited frame rates:** on a 60 Hz display, sources above 60 fps lose frames by
   design. Counting those as drops would hide real problems, so report them apart. → S1.2.

## Remaining work before accepting

- [ ] Run `measure.sh` on Windows (path a) and Linux X11 and Wayland (path a), and fill in
      the table. Needs a libVLC there: `--libvlc DIR` pointing at an unpacked VLC.
- [ ] Prototype path (d) on wgpu if path (a) fails 4K60 there. On macOS it passes with
      little headroom.
- [x] Screenshot check of colours, odd-size edges and deinterlacing on macOS (see the
      screenshot checks). HDR still needs real samples.
- [ ] The edge cases not exercised: remote desktop and GPU-less VMs, dual-GPU laptops, GPU
      device loss, and textures above 8192 px (the downscale logic is unit-tested only).
