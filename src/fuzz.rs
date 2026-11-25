//! A small mutation fuzzer for VLC's own parsers (S9.7.1): subtitles, playlists, podcast feeds,
//! settings, update manifests and the web remote's requests all read text someone else wrote.
//! libVLC's parsers are fuzzed upstream (oss-fuzz); these are ours.
//!
//! Each target starts from a few real inputs and mutates them (flipped bits, special bytes,
//! inserted tokens, deleted, repeated and spliced runs), checking that the parser neither
//! panics nor takes longer than [`TIME_BUDGET`] on one input. It runs a short, fixed
//! sequence in `cargo test`; for a longer run,
//! `RVLC_FUZZ_ITERATIONS=1000000 RVLC_FUZZ_SEED=$RANDOM cargo test --release fuzz` (see
//! `docs/security.md`). An input that fails is saved to the temp folder, and its path is in the
//! failure message; `RVLC_FUZZ_REPLAY=<that file>` runs it again. A long run also keeps the
//! input being tried in `rvlc-fuzz-<target>-last.bin`, for a crash that aborts the process (a
//! stack overflow, as the subtitle timestamps had).

mod targets;

use std::panic::{self, AssertUnwindSafe};
use std::time::{Duration, Instant};

/// Names how many inputs each target tries, for a long run.
const ITERATIONS_VAR: &str = "RVLC_FUZZ_ITERATIONS";
/// Inputs each target tries in a plain `cargo test`.
const DEFAULT_ITERATIONS: u64 = 1_500;
/// The longest one input may take, in a debug build: a parser that slows down this much on a
/// few kilobytes has a quadratic (or worse) path a hostile file can stretch.
const TIME_BUDGET: Duration = Duration::from_secs(2);
/// The largest input tried.
const MAX_INPUT: usize = 16 * 1024;
/// The seed used without `RVLC_FUZZ_SEED`, so a plain test run is repeatable.
const DEFAULT_SEED: u64 = 0x005e_ed0f_57c0_17e5;
/// Bytes that parsers treat specially.
const SPECIAL_BYTES: &[u8] = b"\0\xff\n\r\t :,.<>{}[]=\"'\\/-#%&;0123456789";
/// Tokens that start structures in the formats fuzzed.
const TOKENS: &[&[u8]] = &[
    b"-->",
    b"\r\n\r\n",
    b"Dialogue: ",
    b"{\\",
    b"<",
    b"</",
    b"<![CDATA[",
    b"&#x",
    b"#EXT",
    b"[",
    b"]\n",
    b"=",
    b"99999999999999999999",
    b"-1",
    b"\xef\xbb\xbf",
    b"\xe2\x80\xae",
    b"\":",
    b"{\"",
    b"Content-Length: ",
];

/// Fuzzes `check` from `seeds`: each round mutates a seed and calls `check` with it, which
/// panics if the parser misbehaved (a panic inside the parser counts too).
pub fn run(target: &str, seeds: &[&[u8]], mut check: impl FnMut(&[u8])) {
    if let Some(saved) = replayed(target) {
        check(&saved);
        return;
    }
    let mut random = Random::new(base_seed() ^ fnv(target.as_bytes()));
    // A stack overflow aborts the process before anything can be saved, so a long run keeps
    // each input until the next replaces it.
    let keep_last = std::env::var_os(ITERATIONS_VAR).is_some();
    for round in 0..iterations() {
        let input = mutate(&mut random, seeds);
        if keep_last {
            save(target, "last", &input);
        }
        let started = Instant::now();
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| check(&input)));
        let slow = started.elapsed() > TIME_BUDGET;
        if outcome.is_err() || slow {
            let saved = save(target, &round.to_string(), &input);
            panic!("fuzz {target}: round {round} {} on the input in {saved}", verdict(slow));
        }
    }
}

fn verdict(slow: bool) -> &'static str {
    if slow { "took too long" } else { "panicked" }
}

/// A saved failing input to try again: `RVLC_FUZZ_REPLAY=<file>` for the target whose name the
/// file's starts with (`rvlc-fuzz-<target>-<round>.bin`).
fn replayed(target: &str) -> Option<Vec<u8>> {
    let path = std::path::PathBuf::from(std::env::var_os("RVLC_FUZZ_REPLAY")?);
    let name = path.file_name()?.to_str()?;
    name.starts_with(&format!("rvlc-fuzz-{target}-")).then(|| std::fs::read(&path).ok())?
}

fn iterations() -> u64 {
    env_number(ITERATIONS_VAR).unwrap_or(DEFAULT_ITERATIONS)
}

fn base_seed() -> u64 {
    env_number("RVLC_FUZZ_SEED").unwrap_or(DEFAULT_SEED)
}

fn env_number(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.parse().ok()
}

/// Saves a failing input where it can be read back, and says where.
fn save(target: &str, round: &str, input: &[u8]) -> String {
    let path = std::env::temp_dir().join(format!("rvlc-fuzz-{target}-{round}.bin"));
    match std::fs::write(&path, input) {
        Ok(()) => path.display().to_string(),
        Err(error) => format!("(not saved: {error}) {input:?}"),
    }
}

/// One seed, mutated one to eight times.
fn mutate(random: &mut Random, seeds: &[&[u8]]) -> Vec<u8> {
    let mut input = seeds[random.below(seeds.len())].to_vec();
    for _ in 0..=random.below(8) {
        mutate_once(random, &mut input, seeds);
    }
    input.truncate(MAX_INPUT);
    input
}

fn mutate_once(random: &mut Random, input: &mut Vec<u8>, seeds: &[&[u8]]) {
    let at = random.below(input.len() + 1);
    match random.below(6) {
        0 if at < input.len() => input[at] ^= 1 << random.below(8),
        1 if at < input.len() => input[at] = SPECIAL_BYTES[random.below(SPECIAL_BYTES.len())],
        2 => {
            let token = TOKENS[random.below(TOKENS.len())];
            input.splice(at..at, token.iter().copied());
        }
        3 => {
            let end = (at + random.below(64)).min(input.len());
            input.drain(at..end);
        }
        4 => repeat_run(random, input, at),
        _ => {
            let other = seeds[random.below(seeds.len())];
            let from = random.below(other.len() + 1);
            let end = (from + random.below(256)).min(other.len());
            input.splice(at..at, other[from..end].iter().copied());
        }
    }
}

/// Repeats a short run at `at` many times: deep nesting and long lines find slow paths.
fn repeat_run(random: &mut Random, input: &mut Vec<u8>, at: usize) {
    let end = (at + 1 + random.below(16)).min(input.len());
    let run = input.get(at..end).unwrap_or_default().to_vec();
    let times = 1 + random.below(MAX_INPUT / run.len().max(1)).min(2_000);
    let repeated: Vec<u8> = run.iter().copied().cycle().take(run.len() * times).collect();
    input.splice(at..at, repeated);
}

/// A 64-bit FNV-1a hash, to give each target its own sequence from one seed.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// xorshift64*: fast, repeatable and good enough to pick mutations.
struct Random(u64);

impl Random {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// A number below `bound`, or 0 when `bound` is 0.
    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 { 0 } else { (self.next() % bound as u64) as usize }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutations_stay_within_the_size_limit_and_vary() {
        let mut random = Random::new(7);
        let inputs: Vec<Vec<u8>> = (0..200).map(|_| mutate(&mut random, &[b"abc", b""])).collect();
        assert!(inputs.iter().all(|input| input.len() <= MAX_INPUT));
        assert!(inputs.iter().any(|input| input.as_slice() != b"abc"));
    }

    #[test]
    #[should_panic(expected = "panicked on the input in")]
    fn a_panicking_parser_is_reported() {
        run("self-test", &[b"x"], |input| assert!(input.len() < 2));
    }
}
