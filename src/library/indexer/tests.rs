//! The indexer thread on real folders and an in-memory database: first scans, live updates,
//! missing folders and Resync.

use std::fs::{self, File};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, SystemTime};

use super::store::{InPlace, Store};
use super::*;
use crate::db::roots::{self, RootKind, RootStatus};
use crate::history::DatabaseWork;
use crate::library::extract::Extractor;
use crate::opening::Cancel;
use crate::opening::scratch::scratch_dir;

/// How long a test waits for the indexer to report what it expects.
const PATIENCE: Duration = Duration::from_secs(30);

struct Shared(Arc<InPlace>);

impl Store for Shared {
    fn run(&self, work: DatabaseWork) -> bool {
        self.0.run(work)
    }
}

struct Running {
    commands: mpsc::Sender<Command>,
    reports: Receiver<Vec<RootState>>,
    store: Arc<InPlace>,
    cancel: Cancel,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

fn start() -> Running {
    let store = Arc::new(InPlace::new());
    let (sender, reports) = mpsc::channel();
    let report = Box::new(move |roots| {
        let _ = sender.send(roots);
    });
    let cancel = Cancel::default();
    let extractor = Extractor::new(Box::new(crate::engine::tags::NoTags), None);
    let commands = spawn(Box::new(Shared(store.clone())), extractor, report, cancel.clone());
    Running { commands, reports, store, cancel }
}

impl Running {
    fn send(&self, command: Command) {
        self.commands.send(command).expect("the indexer runs");
    }

    /// Waits for a report that `done` accepts.
    fn wait_for(&self, what: &str, done: impl Fn(&[RootState]) -> bool) -> Vec<RootState> {
        let deadline = std::time::Instant::now() + PATIENCE;
        let mut last = Vec::new();
        while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
            let Ok(roots) = self.reports.recv_timeout(left) else { break };
            if done(&roots) {
                return roots;
            }
            last = roots;
        }
        panic!("never reported {what}; last: {last:?}");
    }

    fn identified(&self) -> usize {
        let mut database = self.store.0.lock().expect("lock");
        let count: i64 = database
            .connection()
            .query_row("SELECT COUNT(*) FROM files WHERE media_id IS NOT NULL", [], |row| {
                row.get(0)
            })
            .expect("count");
        count as usize
    }
}

/// Writes `content` at `path`, last modified a minute ago so it counts as settled.
fn write_settled(path: &Path, content: &[u8]) {
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(path, content).expect("write");
    let past = SystemTime::now() - Duration::from_secs(60);
    File::options().write(true).open(path).and_then(|f| f.set_modified(past)).expect("backdate");
}

fn video(path: &Path) -> Command {
    Command::SetRoots(vec![Wanted { path: path.to_path_buf(), kind: RootKind::Video }])
}

fn idle_with(files: usize) -> impl Fn(&[RootState]) -> bool {
    move |roots| roots.first().is_some_and(|r| r.files == files && r.indexing.is_none())
}

#[test]
fn a_new_media_folder_is_indexed_and_identified() {
    let top = scratch_dir();
    write_settled(&top.join("a.mkv"), b"one");
    write_settled(&top.join("Season 1/e1.mp4"), b"two");
    write_settled(&top.join("notes.txt"), b"three");
    let indexer = start();
    indexer.send(video(&top));
    let roots = indexer.wait_for("two files", idle_with(2));
    assert_eq!(roots[0].status, RootStatus::Ok);
    assert_eq!(indexer.identified(), 2);
}

#[test]
fn changes_on_disk_show_up_without_a_rescan() {
    let top = scratch_dir();
    write_settled(&top.join("a.mkv"), b"one");
    let indexer = start();
    indexer.send(video(&top));
    indexer.wait_for("one file", idle_with(1));
    write_settled(&top.join("new/b.mkv"), b"two");
    indexer.wait_for("the new file", idle_with(2));
    fs::remove_file(top.join("a.mkv")).expect("remove");
    indexer.wait_for("the removal", idle_with(1));
}

#[test]
fn a_missing_folder_is_marked_and_comes_back() {
    let top = scratch_dir().join("Videos");
    let indexer = start();
    indexer.send(video(&top));
    indexer.wait_for("missing", |roots| {
        roots.first().is_some_and(|r| r.status == RootStatus::Missing)
    });
    write_settled(&top.join("a.mkv"), b"one");
    indexer.send(Command::Resync(top));
    let roots = indexer.wait_for("found again", idle_with(1));
    assert_eq!(roots[0].status, RootStatus::Ok);
}

#[test]
fn roots_that_are_no_longer_wanted_are_removed() {
    let (one, two) = (scratch_dir(), scratch_dir());
    write_settled(&one.join("a.mkv"), b"one");
    let indexer = start();
    let both = vec![
        Wanted { path: one, kind: RootKind::Video },
        Wanted { path: two.clone(), kind: RootKind::Music },
    ];
    indexer.send(Command::SetRoots(both));
    indexer.wait_for("both", |roots| roots.len() == 2 && roots[0].files == 1);
    indexer.send(video(&two));
    let roots = indexer.wait_for("one left", |roots| roots.len() == 1);
    assert_eq!(roots[0].path, two);
    let mut database = indexer.store.0.lock().expect("lock");
    assert_eq!(roots::list(database.connection()).expect("list").len(), 1);
}

#[test]
fn hidden_files_follow_the_setting() {
    let top = scratch_dir();
    write_settled(&top.join(".hidden.mkv"), b"one");
    write_settled(&top.join("shown.mkv"), b"two");
    let indexer = start();
    indexer.send(video(&top));
    indexer.wait_for("the visible file", idle_with(1));
    indexer.send(Command::ShowHidden(true));
    indexer.wait_for("both files", idle_with(2));
}

#[test]
fn suspending_keeps_the_index_and_stops_following_the_folder() {
    let top = scratch_dir();
    write_settled(&top.join("a.mkv"), b"one");
    let indexer = start();
    indexer.send(video(&top));
    indexer.wait_for("one file", idle_with(1));
    indexer.send(Command::Suspend);
    indexer.wait_for("no folders", <[RootState]>::is_empty);
    {
        let mut database = indexer.store.0.lock().expect("lock");
        assert_eq!(roots::list(database.connection()).expect("list").len(), 1);
    }
    write_settled(&top.join("b.mkv"), b"two");
    indexer.send(video(&top));
    indexer.wait_for("both files", idle_with(2));
}
