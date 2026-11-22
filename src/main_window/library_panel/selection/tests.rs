use super::*;

fn file(name: &str) -> Key {
    Key::File(PathBuf::from(format!("/v/{name}.mkv")))
}

const LIST: Layout = Layout::LIST;

fn keys() -> Vec<Key> {
    vec![Key::Add, Key::Media(MediaId(7)), file("b"), file("c"), file("d")]
}

#[test]
fn arrows_move_and_stop_at_the_ends() {
    let mut selection = Selection::default();
    assert_eq!(selection.step(&keys(), Move::Next, LIST, false), Some(0));
    assert_eq!(selection.step(&keys(), Move::Next, LIST, false), Some(1));
    assert_eq!(selection.step(&keys(), Move::Last, LIST, false), Some(4));
    assert_eq!(selection.step(&keys(), Move::Next, LIST, false), Some(4));
    assert_eq!(selection.indices(&keys()), [4]);
    assert_eq!(selection.step(&[], Move::Next, LIST, false), None);
}

#[test]
fn in_a_grid_up_and_down_jump_a_row() {
    let k = keys();
    let mut selection = Selection::default();
    selection.select(k[1].clone());
    let grid = Layout::grid(2, 3);
    assert_eq!(selection.step(&k, Move::Down, grid, false), Some(3));
    assert_eq!(selection.step(&k, Move::Next, grid, false), Some(4));
    assert_eq!(selection.step(&k, Move::Up, grid, true), Some(2));
    assert_eq!(selection.indices(&k), [2, 3, 4]);
}

#[test]
fn clicks_toggles_and_ranges() {
    let k = keys();
    let mut selection = Selection::default();
    selection.select(k[1].clone());
    selection.toggle(k[3].clone());
    assert_eq!(selection.indices(&k), [1, 3]);
    // The range grows from the last clicked item.
    selection.extend_to(&k, 4);
    assert_eq!(selection.indices(&k), [3, 4]);
    selection.extend_to(&k, 1);
    assert_eq!(selection.indices(&k), [1, 2, 3]);
    selection.toggle(k[2].clone());
    assert_eq!(selection.indices(&k), [1, 3]);
    selection.select_all(&k);
    assert_eq!(selection.indices(&k).len(), 5);
}

#[test]
fn shift_arrows_extend_from_the_anchor() {
    let k = keys();
    let mut selection = Selection::default();
    selection.select(k[2].clone());
    assert_eq!(selection.step(&k, Move::Next, LIST, true), Some(3));
    assert_eq!(selection.step(&k, Move::Next, LIST, true), Some(4));
    assert_eq!(selection.indices(&k), [2, 3, 4]);
    assert_eq!(selection.step(&k, Move::First, LIST, true), Some(0));
    assert_eq!(selection.indices(&k), [0, 1, 2]);
}

#[test]
fn the_selection_follows_its_items_not_their_places() {
    let mut selection = Selection::default();
    selection.select(Key::Media(MediaId(7)));
    selection.toggle(file("b"));
    // A reload added a row above, and a file's name changed: same media, same item.
    let reloaded = vec![Key::Add, file("a"), Key::Media(MediaId(7)), file("b")];
    assert_eq!(selection.indices(&reloaded), [2, 3]);
    assert_eq!(selection.cursor(&reloaded), Some(3));
    selection.clear();
    assert_eq!(selection.indices(&reloaded), Vec::<usize>::new());
    assert_eq!(selection.cursor(&reloaded), None);
}
