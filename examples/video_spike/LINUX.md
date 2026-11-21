# Running the S1.1 spike on Linux

These steps measure the video spike on a Linux PC and give us the numbers the decision
record (`docs/adr/0001-playback-backend.md`) is missing. On Linux only path (a), the CPU
path, exists, and GPUI draws with Vulkan. The question to answer, **once under Wayland and
once under X11**: does a 4K60 file play smoothly with GPUI drawn on top?

Plan on about an hour the first time, most of it the first build.

## 1. What you need

- **A real Linux PC, 64-bit (x86-64 or ARM64).** Not a virtual machine: VMs have slow
  virtual graphics, and their numbers would mean nothing. Ubuntu 24.04 or newer is the
  easiest; Fedora and Arch notes are below. Note the PC's CPU, GPU, RAM and display
  (resolution and refresh rate); you'll report them.
- **A desktop session with Wayland and XWayland**, which is the default on GNOME and KDE.
  `echo $XDG_SESSION_TYPE` prints `wayland`. On an X11-only session you can measure X11
  only.
- **A GPU with Vulkan drivers** (GPUI needs Vulkan) and, for the 4K60 file, **video
  decoding that handles HEVC 10-bit**. Intel and AMD use VA-API; check with `vainfo` (step
  2). NVIDIA's proprietary driver uses VDPAU.
- **About 15 GB of free disk space** (build output and the test files).

## 2. Install the tools

**Ubuntu or Debian:**

```bash
sudo apt install build-essential pkg-config git curl libfontconfig-dev libvulkan1 mesa-vulkan-drivers libwayland-dev libx11-xcb-dev libxkbcommon-x11-dev libzstd-dev vlc vainfo
```

**Fedora:** VLC with its H.264 and HEVC decoders comes from RPM Fusion.

```bash
sudo dnf install https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-$(rpm -E %fedora).noarch.rpm
```

```bash
sudo dnf swap ffmpeg-free ffmpeg --allowerasing
```

```bash
sudo dnf install gcc pkgconf-pkg-config git curl fontconfig-devel vulkan-loader mesa-vulkan-drivers wayland-devel libX11-devel libxkbcommon-x11-devel libzstd-devel vlc libva-utils
```

**Arch:**

```bash
sudo pacman -S base-devel git fontconfig vulkan-icd-loader wayland libxkbcommon-x11 zstd vlc vlc-plugins-all libva-utils
```

Then **Rust**: <https://rustup.rs>, and accept the defaults. Open a new terminal afterwards,
so `cargo` is on your `PATH`.

Install VLC **from the distribution's packages**, as above, not as a Snap or Flatpak. The
spike loads `libvlc.so.5` from the system, and sandboxed packages don't put it there.
Check the version with `vlc --version`: it should be 3.0.x, and any 3.0.20 or newer is
fine.

If you have an Intel or AMD GPU, run `vainfo`. For the 4K60 file, its list should include
a line with `VAProfileHEVCMain10` and `VAEntrypointVLD`. If it doesn't, the file still
plays, but decodes on the CPU: note it in your report. Intel GPUs may need
`intel-media-va-driver-non-free` (Ubuntu) or `intel-media-driver` (Fedora, Arch).

## 3. Get the code

The repository is private, so you need a GitHub account that has access to it, and a way
to sign in from Git (for example `gh auth login` from the GitHub CLI, or an SSH key).

```bash
git clone https://github.com/ItayKarp/VLC-Remake-GPUI.git
```

```bash
cd VLC-Remake-GPUI
```

## 4. Get the test files

The fastest way is to **copy them from the Mac**: the `target/spike-media` folder (about
960 MB) into `target/spike-media` in the Linux checkout, by USB stick, cloud drive or
`scp`.

To make them on Linux instead, install FFmpeg (`sudo apt install ffmpeg`; on Fedora it
came with the `dnf swap` above), then run `examples/video_spike/make_test_media.sh`.
Expect it to take a long time: without the Mac's hardware encoder, the 4K60 and 8K files
are encoded on the CPU.

## 5. Build

From the repository folder:

```bash
cargo build --release --example video_spike
```

The first build downloads and compiles GPUI and takes a while (10–30 minutes). If it stops
with a message that pkg-config can't find a library (`fontconfig`, `xkbcommon`,
`wayland-client`, `x11-xcb`), a package from step 2 is missing.

## 6. Check that it works

```bash
target/release/examples/video_spike --seconds 10 target/spike-media/1080p60-h264.mp4
```

No `--libvlc` is needed: the spike uses the system's VLC.

What should happen:

- The terminal prints `libVLC 3.0.x ...` with your VLC version.
- A window opens with the moving test picture and a translucent bar with a **Hover me**
  button on top. Move your own mouse over the button: it should light up.
- A line like `video format: libVLC proposed ... got NV12 1920x1080 ...`.
- After 10 seconds the window closes, and the last line starts with `RESULT`, with numbers
  for `cpu=` and `rss=`.

Then do the same under X11. An empty `WAYLAND_DISPLAY` makes GPUI use X11 (through
XWayland on a Wayland desktop):

```bash
WAYLAND_DISPLAY= target/release/examples/video_spike --seconds 10 target/spike-media/1080p60-h264.mp4
```

If no picture appears, see [Troubleshooting](#troubleshooting).

## 7. Measure

Before measuring:

- Plug the laptop in and use a performance power mode.
- Close other heavy programs (browsers with video, games, builds).
- **Keep the spike's window visible and uncovered.** Windows that are covered or minimised
  stop drawing, which shows up as dropped frames.

**Wayland** (skip on an X11-only session):

```bash
examples/video_spike/measure.sh throughput
```

```bash
examples/video_spike/measure.sh edges
```

```bash
target/release/examples/video_spike --vlc-arg --avcodec-hw=none target/spike-media/2160p60-hevc10.mp4 | tee target/spike-results/2160p60-software.log
```

```bash
mv target/spike-results target/spike-results-wayland
```

**X11:** the same three runs with `WAYLAND_DISPLAY=` in front, then rename the folder:

```bash
WAYLAND_DISPLAY= examples/video_spike/measure.sh throughput
```

```bash
WAYLAND_DISPLAY= examples/video_spike/measure.sh edges
```

```bash
mkdir -p target/spike-results && WAYLAND_DISPLAY= target/release/examples/video_spike --vlc-arg --avcodec-hw=none target/spike-media/2160p60-hevc10.mp4 | tee target/spike-results/2160p60-software.log
```

```bash
mv target/spike-results target/spike-results-x11
```

`throughput` plays the three acceptance files for 60 seconds each (about 3½ minutes).
`edges` plays the rest of the test files for 10 seconds each (about 2½ minutes). The
third run plays the 4K60 file with hardware decoding turned off, which shows what happens
on a PC whose GPU can't decode it. Each run adds one `RESULT` line to `results.txt` and
keeps its full output in a `.log` file next to it.

## 8. What to send back

1. The `target/spike-results-wayland` and `target/spike-results-x11` folders (zip them).
2. The PC: CPU, GPU and driver (`glxinfo -B` or `vulkaninfo --summary` shows it), RAM,
   display resolution and refresh rate, the distribution and version, the desktop (GNOME,
   KDE, ...), and the VLC version.
3. Whether `vainfo` listed HEVC Main10 decoding (Intel and AMD).
4. What you saw: did the video look right (colours, no tearing or stutter), and did the
   **Hover me** button keep responding during the 4K files?

## Reading the results

Each `RESULT` line has:

- `fps`: frames shown per second.
- `dropped`: the share of frames that never reached the screen. **Under 1% on the 4K60
  file passes.**
- `cpu`: the whole process's CPU time, where 100% is one core.
- `rss`: the most memory the process used, in MB.
- `input_to_paint`: from a mouse move to the next drawn frame, as median, 95th percentile
  and worst. The spike sends these mouse moves itself, 10 times a second. **The 95th
  percentile should stay under about 16 ms (one frame at 60 Hz).**

For comparison, on an Apple M1 path (a) played the 4K60 file with 0.45% dropped and 105%
CPU.

## Troubleshooting

- **`loading libvlccore.so.9` fails:** VLC isn't installed from the distribution's
  packages (step 2), or it's a Snap or Flatpak.
- **No window opens, or GPUI reports no Vulkan device:** the Vulkan driver is missing.
  `vulkaninfo --summary` (package `vulkan-tools`) should list your GPU.
- **The window stays black, or the log has no `video format` line:** add
  `--vlc-arg --avcodec-hw=none` to rule out the GPU decoder. If that fixes it, say so in
  your report: it's a finding.
- **The X11 run opens no window:** your desktop has no XWayland. Measure X11 from an X11
  session instead, if your login screen offers one.
