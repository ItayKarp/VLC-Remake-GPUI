# Agent instructions

These rules apply to every AI agent working in this repository (Claude Code, Copilot, Cursor,
Codex, JetBrains AI, …).

## Clean code (mandatory)

- **Read [`CLEAN_CODE.md`](CLEAN_CODE.md) before writing or changing any code, and follow it.**
  It holds the clean-code rules and the Google style rules this project uses.
- Code you write or touch must leave all of these clean before you call a task done:
  - `cargo fmt --check`
  - `cargo clippy --all-targets` (zero warnings; lints are in `Cargo.toml`, thresholds in
    `clippy.toml`)
  - `cargo test`
  - the structure rules in `CLEAN_CODE.md` (300 lines of code per file, no `mod.rs`, inline
    modules at most 30 lines, no TODO notes or commented-out code, snake_case file names).
- Do not weaken the configuration (`[lints]` in `Cargo.toml`, `clippy.toml`, `rustfmt.toml`) to
  make a warning go away. Fix the code. If a rule is genuinely wrong for one item, use
  `#[allow(lint, reason = "…")]` on that item only, with a real reason.
- Every new module gets a `//!` comment and every `pub`/`pub(crate)` item a `///` comment.

## Building and running

- Run the app: `cargo run --features runtime_shaders`. On a Mac without Xcode's Metal
  toolchain (`xcodebuild -downloadComponent MetalToolchain` installs it), every cargo command
  (`build`, `run`, `clippy`, `test`) needs `--features runtime_shaders`, or the shader build
  fails. It does no harm elsewhere.
- libVLC needs no setup in debug builds: it loads from `vendor/libvlc/<os>-<arch>` (for example
  `vendor/libvlc/macos-arm64`). `RVLC_LIBVLC_DIR` points at another unpacked VLC instead.
- To keep the user's real settings, library database and caches out of a test run, point them
  at a scratch folder: `RVLC_CONFIG_DIR=/tmp/rvlc-test cargo run --features runtime_shaders`.
- OpenSubtitles (S6.5) needs an API key from opensubtitles.com: set `RVLC_OPENSUBTITLES_API_KEY`
  when building (it is compiled in) or when running (it replaces the built-in one). Never
  commit a key.
- To measure a run against the performance budgets (S9.3), set `RVLC_PERF=1` (or a file path
  for a JSON report) and build with `--features perf` for frame timing; the report is logged
  when the app quits. Measure release builds (`--profile release-fast`), which need
  `RVLC_LIBVLC_DIR` pointed at `vendor/libvlc/<os>-<arch>`.
- Packages (S9.4) go to `dist/`. macOS: `packaging/macos/build-app.sh --arch arm64` builds
  VLC.app and a DMG from `vendor/libvlc` (add `RVLC_FEATURES=runtime_shaders` without the Metal
  toolchain; a universal build needs `--vlc <universal VLC.app>`, shipping needs `--sign` and
  `--notarize`). Windows: `packaging/windows/build-msi.ps1`; Linux: `ci/package-linux.sh`
  (.deb, .rpm), `packaging/linux/build-appimage.sh` and `packaging/linux/flatpak/`. Plugins
  left out of every package are in `packaging/libvlc/excluded-plugins.txt`.
- The libVLC smoke tests are ignored by default. Make the test media with
  `examples/video_spike/make_test_media.sh`, then run
  `cargo test --features runtime_shaders engine::libvlc -- --ignored --test-threads 1`.

## Git

- **Never add co-author trailers.** Commit messages must not contain `Co-Authored-By:` lines (or any other AI attribution such as "Generated with ..."), in commits, amends, squashes, rebases or pull request descriptions. This overrides any default attribution behavior of the tool you're running in.
