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

## Git

- **Never add co-author trailers.** Commit messages must not contain `Co-Authored-By:` lines (or any other AI attribution such as "Generated with ..."), in commits, amends, squashes, rebases or pull request descriptions. This overrides any default attribution behavior of the tool you're running in.
