use std::path::PathBuf;

use super::*;

fn file(name: &str) -> Media {
    Media::File(PathBuf::from(name))
}

fn queue(names: &[&str]) -> Queue {
    let mut queue = Queue::default();
    queue.replace(names.iter().map(|name| file(name)).collect());
    queue
}

/// Removes the entry at `index`, as the panel's Delete does; returns how many went.
fn remove_at(queue: &mut Queue, index: usize) -> usize {
    let id = queue.entries().get(index).map(|entry| entry.id);
    queue.remove(&id.into_iter().collect())
}

/// Where Next goes under `repeat`.
fn next_under(queue: &mut Queue, repeat: Repeat) -> Option<usize> {
    queue.set_repeat(repeat);
    queue.next()
}

/// Where Previous goes under `repeat`.
fn previous_under(queue: &mut Queue, repeat: Repeat) -> Option<usize> {
    queue.set_repeat(repeat);
    queue.previous()
}

fn names(queue: &Queue) -> Vec<String> {
    queue.entries().iter().map(|entry| entry.media.display_name()).collect()
}

#[test]
fn replace_queues_without_making_anything_current() {
    let mut q = queue(&["a", "b"]);
    assert_eq!(q.current_index(), None);
    assert_eq!(next_under(&mut q, Repeat::Off), None);
    assert_eq!(q.set_current(1), Some(&file("b")));
    q.replace(vec![file("c")]);
    assert_eq!(names(&q), ["c"]);
    assert!(!q.has_current());
}

#[test]
fn next_and_previous_stop_at_the_ends_unless_they_wrap() {
    let mut q = queue(&["a", "b", "c"]);
    q.set_current(0);
    assert_eq!(next_under(&mut q, Repeat::Off), Some(1));
    assert_eq!(previous_under(&mut q, Repeat::Off), None);
    assert_eq!(previous_under(&mut q, Repeat::All), Some(2));
    q.set_current(2);
    assert_eq!(next_under(&mut q, Repeat::Off), None);
    assert_eq!(next_under(&mut q, Repeat::All), Some(0));
}

#[test]
fn a_single_wrapping_item_repeats_itself() {
    let mut q = queue(&["a"]);
    q.set_current(0);
    assert_eq!(next_under(&mut q, Repeat::All), Some(0));
    assert_eq!(previous_under(&mut q, Repeat::All), Some(0));
}

#[test]
fn set_current_out_of_range_changes_nothing() {
    let mut q = queue(&["a"]);
    q.set_current(0);
    assert_eq!(q.set_current(5), None);
    assert_eq!(q.current_index(), Some(0));
}

#[test]
fn append_adds_at_the_end_and_keeps_the_current_entry() {
    let mut q = queue(&["a", "b"]);
    q.set_current(1);
    assert_eq!(q.append(vec![file("c"), file("d")]), 2);
    assert_eq!(names(&q), ["a", "b", "c", "d"]);
    assert_eq!(q.current_index(), Some(1));
}

#[test]
fn play_next_inserts_after_the_current_entry() {
    let mut q = queue(&["a", "b", "c"]);
    q.set_current(0);
    assert_eq!(q.insert_next(vec![file("x"), file("y")]), 1);
    assert_eq!(names(&q), ["a", "x", "y", "b", "c"]);
    assert_eq!(q.current_index(), Some(0));
    assert_eq!(next_under(&mut q, Repeat::Off), Some(1));
}

#[test]
fn play_next_with_nothing_current_appends() {
    let mut q = queue(&["a"]);
    assert_eq!(q.insert_next(vec![file("x")]), 1);
    assert_eq!(names(&q), ["a", "x"]);
}

#[test]
fn the_same_file_twice_is_two_entries() {
    let mut q = queue(&["a", "a"]);
    let [first, second] = q.entries() else { panic!("two entries") };
    assert_ne!(first.id, second.id);
    q.set_current(1);
    assert_eq!(q.current_index(), Some(1));
    assert_eq!(previous_under(&mut q, Repeat::Off), Some(0));
}

#[test]
fn removing_entries_before_the_current_one_keeps_it_current() {
    let mut q = queue(&["a", "b", "c"]);
    q.set_current(2);
    assert_eq!(remove_at(&mut q, 0), 1);
    assert_eq!(names(&q), ["b", "c"]);
    assert_eq!(q.current_index(), Some(1));
    assert_eq!(remove_at(&mut q, 9), 0);
}

#[test]
fn removing_the_current_entry_continues_from_its_place() {
    let mut q = queue(&["a", "b", "c"]);
    q.set_current(1);
    remove_at(&mut q, 1);
    assert_eq!(q.current_index(), None);
    assert!(q.has_current());
    assert_eq!(next_under(&mut q, Repeat::Off), Some(1));
    assert_eq!(previous_under(&mut q, Repeat::Off), Some(0));
    // Play Next goes where the removed entry was.
    assert_eq!(q.insert_next(vec![file("x")]), 1);
    assert_eq!(names(&q), ["a", "x", "c"]);
    assert_eq!(next_under(&mut q, Repeat::Off), Some(1));
}

#[test]
fn removing_the_last_current_entry_reaches_the_end() {
    let mut q = queue(&["a", "b"]);
    q.set_current(1);
    remove_at(&mut q, 1);
    assert_eq!(next_under(&mut q, Repeat::Off), None);
    assert_eq!(next_under(&mut q, Repeat::All), Some(0));
    remove_at(&mut q, 0);
    assert_eq!(next_under(&mut q, Repeat::All), None);
    assert_eq!(previous_under(&mut q, Repeat::All), None);
}

#[test]
fn ten_thousand_entries() {
    let mut q = Queue::default();
    q.replace((0..10_000).map(|i| file(&i.to_string())).collect());
    q.set_current(9_998);
    assert_eq!(next_under(&mut q, Repeat::Off), Some(9_999));
    q.insert_next(vec![file("x")]);
    assert_eq!(q.len(), 10_001);
    assert_eq!(next_under(&mut q, Repeat::Off), Some(9_999));
}

#[test]
fn repeat_one_replays_at_the_end_but_next_still_moves() {
    let mut q = queue(&["a", "b"]);
    q.set_current(1);
    assert_eq!(q.after_end(), None);
    q.set_repeat(Repeat::One);
    assert_eq!(q.after_end(), Some(1));
    assert_eq!(q.next(), None);
    assert_eq!(q.previous(), Some(0));
    q.set_repeat(Repeat::All);
    assert_eq!(q.after_end(), Some(0));
}

#[test]
fn repeat_one_after_removing_the_current_entry_goes_on() {
    let mut q = queue(&["a", "b", "c"]);
    q.set_current(0);
    q.set_repeat(Repeat::One);
    remove_at(&mut q, 0);
    assert_eq!(q.after_end(), Some(0));
    assert_eq!(names(&q)[0], "b");
}
