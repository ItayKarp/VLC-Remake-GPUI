//! Crash safety (acceptance criterion): kill a process mid-save 100 times; the file
//! must never be empty or half-written. Spawns this test binary as the writer.
//! Run with `cargo test kill_during_save -- --ignored`.

use std::fs;
use std::path::Path;

use super::support::TempDir;
use crate::settings::storage;

const CRASH_DIR_ENV: &str = "RVLC_TEST_CRASH_DIR";

fn crash_variant(n: u32) -> String {
    let filler = "x".repeat(64 * 1024);
    format!("generation = {n}\nfiller = \"{filler}\"\nend = true\n")
}

#[test]
#[ignore = "helper process for kill_during_save"]
fn crash_writer() {
    let Some(dir) = std::env::var_os(CRASH_DIR_ENV) else {
        return;
    };
    let path = Path::new(&dir).join("settings.toml");
    let mut n = 0;
    loop {
        storage::save(&path, &crash_variant(n)).unwrap();
        n += 1;
    }
}

#[test]
#[ignore = "slow: spawns 100 processes"]
fn kill_during_save() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, SystemTime};

    let dir = TempDir::new();
    let path = dir.path("settings.toml");
    storage::save(&path, &crash_variant(0)).unwrap();
    let mut progressed = false;
    for round in 0..100 {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "settings::tests::crash::crash_writer", "--ignored"])
            .env(CRASH_DIR_ENV, dir.root())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let jitter =
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().subsec_nanos() % 40;
        std::thread::sleep(Duration::from_millis(20 + u64::from(jitter)));
        child.kill().unwrap();
        child.wait().unwrap();

        let text = fs::read_to_string(&path).unwrap();
        let doc: toml::Table = toml::from_str(&text)
            .unwrap_or_else(|err| panic!("round {round}: settings file is broken: {err}"));
        assert_eq!(doc.get("end"), Some(&toml::Value::Boolean(true)), "round {round}");
        progressed |= doc.get("generation").and_then(toml::Value::as_integer) != Some(0);
    }
    assert!(progressed, "the writer never got to write; the test proved nothing");
}
