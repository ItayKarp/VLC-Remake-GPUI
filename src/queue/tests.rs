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

fn names(queue: &Queue) -> Vec<String> {
    queue.entries().iter().map(|entry| entry.media.display_name()).collect()
}

#[test]
fn replace_queues_without_making_anything_current() {
    let mut q = queue(&["a", "b"]);
    assert_eq!(q.current_index(), None);
    assert_eq!(q.next(Ends::Stop), None);
    assert_eq!(q.set_current(1), Some(&file("b")));
    q.replace(vec![file("c")]);
    assert_eq!(names(&q), ["c"]);
    assert!(!q.has_current());
}

#[test]
fn next_and_previous_stop_at_the_ends_unless_they_wrap() {
    let mut q = queue(&["a", "b", "c"]);
    q.set_current(0);
    assert_eq!(q.next(Ends::Stop), Some(1));
    assert_eq!(q.previous(Ends::Stop), None);
    assert_eq!(q.previous(Ends::Wrap), Some(2));
    q.set_current(2);
    assert_eq!(q.next(Ends::Stop), None);
    assert_eq!(q.next(Ends::Wrap), Some(0));
}

#[test]
fn a_single_wrapping_item_repeats_itself() {
    let mut q = queue(&["a"]);
    q.set_current(0);
    assert_eq!(q.next(Ends::Wrap), Some(0));
    assert_eq!(q.previous(Ends::Wrap), Some(0));
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
    assert_eq!(q.next(Ends::Stop), Some(1));
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
    assert_eq!(q.previous(Ends::Stop), Some(0));
}

#[test]
fn removing_entries_before_the_current_one_keeps_it_current() {
    let mut q = queue(&["a", "b", "c"]);
    q.set_current(2);
    assert_eq!(q.remove(0).map(|entry| entry.media), Some(file("a")));
    assert_eq!(q.current_index(), Some(1));
    assert_eq!(q.remove(9), None);
}

#[test]
fn removing_the_current_entry_continues_from_its_place() {
    let mut q = queue(&["a", "b", "c"]);
    q.set_current(1);
    q.remove(1);
    assert_eq!(q.current_index(), None);
    assert!(q.has_current());
    assert_eq!(q.next(Ends::Stop), Some(1));
    assert_eq!(q.previous(Ends::Stop), Some(0));
    // Play Next goes where the removed entry was.
    assert_eq!(q.insert_next(vec![file("x")]), 1);
    assert_eq!(names(&q), ["a", "x", "c"]);
    assert_eq!(q.next(Ends::Stop), Some(1));
}

#[test]
fn removing_the_last_current_entry_reaches_the_end() {
    let mut q = queue(&["a", "b"]);
    q.set_current(1);
    q.remove(1);
    assert_eq!(q.next(Ends::Stop), None);
    assert_eq!(q.next(Ends::Wrap), Some(0));
    q.remove(0);
    assert_eq!(q.next(Ends::Wrap), None);
    assert_eq!(q.previous(Ends::Wrap), None);
}

#[test]
fn history_follows_entries_as_they_move() {
    let mut q = queue(&["a", "b", "c"]);
    q.set_current(2);
    q.set_current(0);
    assert_eq!(q.previous_in_history(), Some(2));
    q.insert_next(vec![file("x")]);
    assert_eq!(q.previous_in_history(), Some(3));
    q.remove(3);
    assert_eq!(q.previous_in_history(), None);
}

#[test]
fn ten_thousand_entries() {
    let mut q = Queue::default();
    q.replace((0..10_000).map(|i| file(&i.to_string())).collect());
    q.set_current(9_998);
    assert_eq!(q.next(Ends::Stop), Some(9_999));
    q.insert_next(vec![file("x")]);
    assert_eq!(q.len(), 10_001);
    assert_eq!(q.next(Ends::Stop), Some(9_999));
}
