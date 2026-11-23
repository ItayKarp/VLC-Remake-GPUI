//! The conversion queue: naming, turns, cancelling and batches.

use std::path::{Path, PathBuf};

use super::*;

fn file(path: &str) -> Media {
    Media::File(PathBuf::from(path))
}

fn queue_of(count: usize) -> Queue {
    let mut queue = Queue::default();
    for index in 0..count {
        queue.add(
            file(&format!("/m/{index}.mkv")),
            Preset::Mp4H264,
            &Destination::default(),
            |_| false,
        );
    }
    queue
}

#[test]
fn two_conversions_of_one_name_get_different_files() {
    let mut queue = Queue::default();
    let destination = Destination::Folder(PathBuf::from("/out"));
    queue.add(file("/a/film.mkv"), Preset::Mp4H264, &destination, |_| false);
    queue.add(file("/b/film.avi"), Preset::Mp4H264, &destination, |_| false);
    let outputs: Vec<&Path> = queue.jobs().iter().map(|job| job.output.as_path()).collect();
    assert_eq!(outputs, [Path::new("/out/film.mp4"), Path::new("/out/film (2).mp4")]);
}

#[test]
fn only_a_few_run_at_once_and_the_next_starts_when_one_ends() {
    let mut queue = queue_of(100);
    let started = queue.start_due();
    assert_eq!(started.len(), MAX_RUNNING);
    assert!(queue.start_due().is_empty(), "no more until one ends");
    queue.finish(started[0].id, Ok(()));
    let next = queue.start_due();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].id, MAX_RUNNING as JobId, "in the order asked for");
}

#[test]
fn cancelling_says_whether_work_must_stop() {
    let mut queue = queue_of(3);
    let running = queue.start_due()[0].id;
    let waiting = queue.jobs()[2].id;
    assert!(queue.cancel(running));
    assert!(!queue.cancel(waiting));
    assert!(!queue.cancel(running), "already cancelled");
    queue.finish(running, Err("stopped".into()));
    assert_eq!(queue.jobs()[0].status, Status::Cancelled, "a late ending doesn't undo it");
}

#[test]
fn progress_only_moves_running_conversions() {
    let mut queue = queue_of(1);
    queue.set_progress(0, 0.5);
    assert_eq!(queue.jobs()[0].status, Status::Waiting);
    queue.start_due();
    queue.set_progress(0, 1.5);
    assert_eq!(queue.jobs()[0].status, Status::Converting(1.));
}

#[test]
fn a_batch_is_summed_up_once_it_is_all_over() {
    let mut queue = queue_of(3);
    let started = queue.start_due();
    queue.finish(started[0].id, Ok(()));
    queue.finish(started[1].id, Err("no room".into()));
    assert_eq!(queue.take_summary(), None, "one still waits");
    queue.cancel(2);
    let summary = queue.take_summary().expect("the batch is over");
    assert_eq!(summary.done, [PathBuf::from("/m/0.mp4")]);
    assert_eq!(summary.failed, ["no room"]);
    assert_eq!(queue.take_summary(), None, "said once");
}

#[test]
fn a_batch_cancelled_whole_says_nothing() {
    let mut queue = queue_of(2);
    queue.start_due();
    queue.cancel(0);
    queue.cancel(1);
    assert!(!queue.is_busy());
    assert_eq!(queue.take_summary(), None);
}

#[test]
fn clearing_keeps_what_isnt_over() {
    let mut queue = queue_of(3);
    let started = queue.start_due();
    queue.finish(started[0].id, Ok(()));
    queue.clear_finished();
    assert_eq!(queue.jobs().len(), 2);
    assert_eq!(queue.unfinished(), 2);
}
