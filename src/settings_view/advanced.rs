//! Which pages have Advanced Options open (S5.2.4). Remembered for the session, so closing and
//! reopening Settings keeps them, but a new launch starts with them closed.

use std::collections::HashSet;

use gpui::{App, Global};

use crate::settings::registry::Category;

/// The pages whose Advanced Options are open.
#[derive(Default)]
struct OpenPages(HashSet<Category>);

impl Global for OpenPages {}

/// Whether `category`'s Advanced Options were left open this session.
pub(super) fn is_open(category: Category, cx: &App) -> bool {
    cx.try_global::<OpenPages>().is_some_and(|pages| pages.0.contains(&category))
}

/// Remembers whether `category`'s Advanced Options are open.
pub(super) fn set_open(category: Category, open: bool, cx: &mut App) {
    let pages = &mut cx.default_global::<OpenPages>().0;
    if open {
        pages.insert(category);
    } else {
        pages.remove(&category);
    }
}
