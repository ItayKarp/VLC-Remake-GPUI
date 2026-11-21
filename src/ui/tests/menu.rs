//! Context menu: keyboard movement, submenus and dismissal.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Context, DismissEvent, Entity, Render, Subscription, TestAppContext, VisualTestContext, Window,
    div, prelude::*, px,
};

use super::support::{draw, host, setup};
use crate::ui::menu::{ContextMenu, MenuItem};

/// Choices made so far, by name.
type Chosen = Rc<RefCell<Vec<&'static str>>>;

struct MenuHost {
    /// The open menu, and the subscription that clears it when it's dismissed.
    menu: Option<(Entity<ContextMenu>, Subscription)>,
    chosen: Chosen,
}

impl Render for MenuHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().children(self.menu.as_ref().map(|(menu, _)| {
            crate::ui::popover::at_position(gpui::point(px(10.), px(10.)), 1, menu.clone())
        }))
    }
}

/// An action that records `name` as chosen.
fn pick(name: &'static str, chosen: &Chosen) -> impl Fn(&mut Window, &mut gpui::App) + 'static {
    let chosen = chosen.clone();
    move |_: &mut Window, _: &mut gpui::App| chosen.borrow_mut().push(name)
}

fn open_menu(cx: &mut TestAppContext) -> (Entity<MenuHost>, &mut VisualTestContext) {
    let (host, cx) = host(cx, |_, _| MenuHost { menu: None, chosen: Rc::default() });
    host.update_in(cx, |this, window, cx| {
        let items = vec![
            MenuItem::action("Play", pick("play", &this.chosen)),
            MenuItem::Separator,
            MenuItem::action("Convert", pick("convert", &this.chosen)).disabled(true),
            MenuItem::submenu(
                "Sort By",
                vec![
                    MenuItem::radio("Name", true, pick("name", &this.chosen)),
                    MenuItem::radio("Duration", false, pick("duration", &this.chosen)),
                ],
            ),
        ];
        let menu = ContextMenu::build(items, window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.menu = None;
            cx.notify();
        });
        this.menu = Some((menu, subscription));
        cx.notify();
    });
    draw(cx);
    (host, cx)
}

#[gpui::test]
fn menu_skips_separators_and_disabled_items_and_activates(cx: &mut TestAppContext) {
    setup(cx);
    let (host, cx) = open_menu(cx);
    // Down: Play. Down: skips the separator and disabled Convert to Sort By.
    // Down: wraps to Play.
    cx.simulate_keystrokes("down down down enter");
    draw(cx);
    let host = host.read_with(cx, |h, _| (h.menu.is_none(), h.chosen.borrow().clone()));
    assert_eq!(host, (true, vec!["play"]));
}

#[gpui::test]
fn escape_in_a_submenu_closes_only_the_submenu(cx: &mut TestAppContext) {
    setup(cx);
    let (host, cx) = open_menu(cx);
    cx.simulate_keystrokes("up right");
    draw(cx);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(
        host.read_with(cx, |h, _| h.menu.is_some()),
        "Escape in the submenu closed the whole menu"
    );
    // Back in the parent: open the submenu again and choose its second item.
    cx.simulate_keystrokes("right");
    draw(cx);
    cx.simulate_keystrokes("down enter");
    draw(cx);
    let chosen = host.read_with(cx, |h, _| (h.menu.is_none(), h.chosen.borrow().clone()));
    assert_eq!(chosen, (true, vec!["duration"]));
}

#[gpui::test]
fn escape_at_the_top_closes_the_menu(cx: &mut TestAppContext) {
    setup(cx);
    let (host, cx) = open_menu(cx);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(host.read_with(cx, |h, _| h.menu.is_none()));
}
