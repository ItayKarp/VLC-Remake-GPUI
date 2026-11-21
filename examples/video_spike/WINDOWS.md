# Running the S1.1 spike on Windows

These steps measure the video spike on a Windows PC and give us the numbers the decision
record (`docs/adr/0001-playback-backend.md`) is missing. On Windows only path (a), the CPU
path, exists. The question to answer: **does a 4K60 file play smoothly with GPUI drawn on
top?**

Plan on about an hour the first time, most of it installing tools.

## 1. What you need

- **A real Windows 10 or 11 PC, 64-bit (x64).** Not a virtual machine: VMs have slow
  virtual graphics, and their numbers would mean nothing. Note the PC's CPU, GPU, RAM and
  display (resolution and refresh rate); you'll report them.
- **About 15 GB of free disk space** (build output, VLC and the test files).
- **A GPU that decodes HEVC 10-bit** for the 4K60 file. Most GPUs from 2017 on do (Intel
  7th generation, NVIDIA GTX 10-series, AMD RX 400-series or newer). Older ones still work,
  but decode on the CPU.

## 2. Install the tools

1. **Visual Studio Build Tools**: <https://visualstudio.microsoft.com/visual-cpp-build-tools/>.
   In the installer, tick the **Desktop development with C++** workload. Make sure the
   right-hand panel includes **MSVC (x64/x86) build tools** and a **Windows 11 SDK** (or
   Windows 10 SDK 10.0.20348 or newer). GPUI needs the SDK's `fxc.exe` to compile its
   shaders.
2. **Rust**: <https://rustup.rs>. Run `rustup-init.exe` and accept the defaults
   (`x86_64-pc-windows-msvc`).
3. **Git for Windows**: <https://git-scm.com/download/win>. It includes **Git Bash**, which
   runs the spike's `.sh` scripts.

Open a new terminal afterwards, so the tools are on your `PATH`.

## 3. Get the code

The repository is private, so you need a GitHub account that has access to it. In Git
Bash:

```bash
git clone https://github.com/ItayKarp/VLC-Remake-GPUI.git
cd VLC-Remake-GPUI
```

The first time, Git opens a browser window to sign in to GitHub.

## 4. Get libVLC

The spike loads VLC's libraries from an unpacked VLC folder. Use the official 64-bit
**zip** (not the installer), version **3.0.23**, the same version as the macOS numbers:

- Download: <https://download.videolan.org/pub/videolan/vlc/3.0.23/win64/vlc-3.0.23-win64.zip>
  (about 80 MB).
- Optional check, in Git Bash: `sha256sum vlc-3.0.23-win64.zip` should print
  `992d19dbd0b8a7cde9167d2f7780b1ef6f92acc8a71acfa736101a21f35181e1`.
- Unzip it. Inside is one folder (`vlc-3.0.23`) that holds `libvlc.dll`, `libvlccore.dll`
  and a `plugins` folder. Move that folder to `vendor\libvlc\windows-x64` inside the
  repository, so you have `vendor\libvlc\windows-x64\libvlc.dll` and
  `vendor\libvlc\windows-x64\plugins\`. `vendor/` is git-ignored.

## 5. Get the test files

The fastest way is to **copy them from the Mac**: the `target/spike-media` folder (about
960 MB) into `target\spike-media` in the Windows checkout, by USB stick or cloud drive.

To make them on Windows instead, you also need FFmpeg with libx264 and libx265 (for example
the "full" build from <https://www.gyan.dev/ffmpeg/builds/>, with its `bin` folder on your
`PATH`). Then run `examples/video_spike/make_test_media.sh` in Git Bash. Expect it to take
a long time: without the Mac's hardware encoder, the 4K60 and 8K files are encoded on the
CPU.

## 6. Build

From the repository folder, in Git Bash:

```bash
cargo build --release --example video_spike
```

The first build downloads and compiles GPUI and takes a while (10–30 minutes). If it fails
with `Failed to find fxc.exe`, the Windows SDK is missing (step 2). If it fails with
linker errors (`link.exe` not found), the MSVC build tools are missing.

## 7. Check that it works

```bash
target/release/examples/video_spike --libvlc vendor/libvlc/windows-x64 --seconds 10 target/spike-media/1080p60-h264.mp4
```

What should happen:

- The terminal prints `libVLC 3.0.23 Vetinari`.
- A window opens with the moving test picture and a translucent bar with a **Hover me**
  button on top. Move your own mouse over the button: it should light up.
- A line like `video format: libVLC proposed ... got NV12 1920x1080 ...`.
- After 10 seconds the window closes, and the last line starts with `RESULT`. It must show
  numbers for `cpu=` and `rss=`, not `n/a`.

If no picture appears, see [Troubleshooting](#troubleshooting).

## 8. Measure

Before measuring:

- Plug the laptop in and use a high-performance power mode.
- Close other heavy programs (browsers with video, games, builds).
- **Keep the spike's window visible and uncovered.** Windows that are covered or minimised
  stop drawing, which shows up as dropped frames.

Then, in Git Bash:

```bash
LIBVLC=vendor/libvlc/windows-x64 examples/video_spike/measure.sh throughput
```

```bash
LIBVLC=vendor/libvlc/windows-x64 examples/video_spike/measure.sh edges
```

`throughput` plays the three acceptance files for 60 seconds each (about 3½ minutes).
`edges` plays the rest of the test files for 10 seconds each (about 2½ minutes). Each run
adds one `RESULT` line to `target/spike-results/results.txt`, and keeps its full output in
a `.log` file next to it.

Also measure the 4K60 file with hardware decoding turned off, which shows what happens on a
PC whose GPU can't decode it:

```bash
target/release/examples/video_spike --libvlc vendor/libvlc/windows-x64 --vlc-arg --avcodec-hw=none target/spike-media/2160p60-hevc10.mp4 | tee target/spike-results/2160p60-software.log
```

## 9. What to send back

1. `target/spike-results/results.txt`.
2. The `target/spike-results` `.log` files (zip the folder).
3. The PC: CPU, GPU (and driver version), RAM, display resolution and refresh rate, and the
   Windows version (`winver`).
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

- **`loading ...libvlccore.dll` fails:** `--libvlc` doesn't point at the folder that holds
  `libvlc.dll`, or the VLC download is the 32-bit one. Use the `win64` zip.
- **The window stays black, or the log has no `video format` line:** add
  `--vlc-arg --avcodec-hw=none` to rule out the GPU decoder. If that fixes it, say so in
  your report: it's a finding.
- **`measure.sh` says `No such file or directory`:** run it from the repository folder, in
  Git Bash (not PowerShell or cmd), after the build in step 6.
