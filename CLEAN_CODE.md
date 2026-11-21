# Clean code

Code in this repository is **done** only when all of these are true:

1. it builds and `cargo test` passes,
2. `cargo clippy --all-targets` reports **no warnings**,
3. `cargo fmt --check` reports nothing,
4. the structure rules below find nothing, and
5. it follows the Google style rules below.

Every mechanical rule is enforced by a tool: clippy is configured in [`Cargo.toml`](Cargo.toml)
(`[lints]`) with thresholds in [`clippy.toml`](clippy.toml), and rustfmt in
[`rustfmt.toml`](rustfmt.toml). These rules are adapted from the `learnrust` course's
`CLEAN_CODE.md`.

## clippy

`Cargo.toml` turns on clippy's default lints **and the pedantic group**, plus a few from the
restriction and nursery groups. Thresholds live in `clippy.toml`:

| Rule | Limit | Why |
|---|---|---|
| `too_many_lines` | 40 lines per function | A function that doesn't fit on a screen does more than one thing. Extract the steps and name them. |
| `too_many_arguments` | 5 | More inputs than that is a concept without a name: give it a struct. |
| `cognitive_complexity` | 15 | Branches and loops a reader can hold in their head at once. |
| `excessive_nesting` | 4 blocks deep | Return early (`let … else`, `?`), extract a function, use iterator adapters. |
| `fn_params_excessive_bools` | 1 | `draw(true, false)` means nothing at the call site. An enum does. |
| `struct_excessive_bools` | 2 | Several flags are usually one state machine: an enum. |
| `unwrap_used` | none outside tests | A bare `unwrap()` says "can't fail" without saying why. Use `?`, handle it, or `expect("why this can't fail")`. |
| `todo`, `dbg_macro` | none | Leftovers. |
| `mod_module_files` | no `mod.rs` | The modern layout: `shapes.rs` next to a `shapes/` folder. |
| `needless_collect`, `redundant_clone` | none | Work the computer does for nothing. |

A few pedantic lints are switched off in `Cargo.toml` (with the reason next to them): prose style
in doc comments, long hex colours, and the pixel/float `as` casts layout code is made of.

## rustfmt

`rustfmt.toml` holds Google's Rust formatting (as used by Chromium and Fuchsia): rustfmt
defaults, 100 columns, `use_small_heuristics = "Max"`. Run `cargo fmt` before every commit;
formatting is never worth arguing about.

## Structure: Rust's file conventions

| Rule | What it wants |
|---|---|
| `file-too-long` | At most **300 lines of code** per file (comments, blank lines and an inline `#[cfg(test)]` module don't count; a separate `tests.rs` file does). Past that, a file holds several concepts: give each one a module in its own file. Re-export with `pub use` so callers' paths don't change. |
| `thick-main` | When the crate has a `lib.rs`, `main.rs` only *starts* the program: at most 25 lines, no types, no helper functions. |
| `inline-module` | An inline `mod name { … }` (other than `mod tests`) holds at most 30 lines. Bigger modules get their own file. |
| `file-name` | File names are module names: `snake_case.rs`. |
| `todo-comment` | No `// TODO`, `FIXME`, `XXX`, `HACK` left behind. Finish it or delete the note. |
| `commented-out-code` | No switched-off code. Git remembers it. |

Unit tests sit at the bottom of the file they test in `#[cfg(test)] mod tests`, or in a sibling
`tests.rs` (or `tests/` folder of topic files) when they are large.

## Google style

Google has no separate Rust guide: its Rust code (Chromium, Fuchsia, Android) follows rustfmt,
the Rust API Guidelines and Google's general style rules. In practice:

- **Formatting** is whatever `cargo fmt` produces with `rustfmt.toml`. Lines are at most 100
  columns, including comments.
- **Naming**: `snake_case` for functions, variables, modules and files; `UpperCamelCase` for types
  and traits; `SCREAMING_SNAKE_CASE` for constants and statics. Acronyms are words
  (`HttpClient`, not `HTTPClient`). Conversions are `as_` (cheap borrow), `to_` (costly),
  `into_` (consumes). Getters drop `get_`. Names are descriptive; no abbreviations that aren't
  universal.
- **Documentation**: every module has a `//!` comment saying what it is for. Every `pub` and
  `pub(crate)` item has a `///` comment saying what it is for and what it promises. Doc comments
  start with a one-line summary sentence, then details.
- **Comments** are full sentences with capitalization and punctuation, and explain *why*; the
  code already says what.
- **Imports**: grouped `std`, then external crates, then `crate::`/`super::`, separated by blank
  lines; no wildcard imports outside tests and preludes.
- **Errors**: return `Result` and propagate with `?`; `panic!`/`expect` only for broken
  invariants, with a message that says why it can't happen.
- **Visibility**: private by default, `pub(crate)` inside the crate, `pub` only for API you mean
  to support.
- **No magic numbers**: a named `const`, used by name everywhere.

## Clean code in Rust

- **Small functions, one level of abstraction.** A function either decides *what* happens or
  does *one* of the steps, not both.
- **Make illegal states unrepresentable.** An enum instead of a `String` status or two `bool`s; a
  newtype instead of a bare number; a constructor that validates instead of public fields.
- **Errors are values.** Let the caller decide.
- **Don't repeat yourself, but don't abstract too early.** Three copies is a pattern; two might be
  a coincidence.

## SOLID, the Rust way

| Principle | In Rust |
|---|---|
| **S**ingle responsibility | A module, a type and a function each have one reason to change. |
| **O**pen/closed | Add behaviour by adding a type that implements a trait, not by editing a growing `match` (unless the set of cases is closed: then an enum and `match` is right). |
| **L**iskov substitution | Every implementation honours the trait's documented contract. |
| **I**nterface segregation | Small traits that each do one thing. Callers ask for exactly what they use. |
| **D**ependency inversion | Depend on traits where a second implementation (often a test fake) exists. |

## When a rule is wrong

Sometimes it is. Then say so, in the code, on the smallest item possible:

```rust
#[allow(clippy::too_many_lines, reason = "one declarative element tree; splitting hides the layout")]
```

An `allow` without a `reason` is not allowed. Prefer fixing the code over allowing the lint.
