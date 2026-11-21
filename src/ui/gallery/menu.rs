//! The gallery's demo context menu, with icons, shortcuts, checks, radios and submenus.

use gpui::{App, Context, DismissEvent, Pixels, Point, WeakEntity, Window};

use super::Gallery;
use crate::assets::icons;
use crate::ui::menu::{ContextMenu, MenuItem};

impl Gallery {
    /// Opens the demo context menu at `position`; it removes itself when dismissed.
    pub(super) fn open_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let menu = ContextMenu::build(menu_items(&cx.entity().downgrade()), window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.menu = None;
            cx.notify();
        });
        self.menu = Some((position, menu, subscription));
        cx.notify();
    }
}

/// A menu action that reports `text` in the gallery's status line.
fn report(
    gallery: &WeakEntity<Gallery>,
    text: &'static str,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let gallery = gallery.clone();
    move |_, cx| {
        gallery.update(cx, |this, cx| this.set_status(text, cx)).ok();
    }
}

fn menu_items(gallery: &WeakEntity<Gallery>) -> Vec<MenuItem> {
    let say = |text| report(gallery, text);
    vec![
        MenuItem::action("Play", say("Menu → Play")).icon(icons::PLAY).shortcut("Space"),
        MenuItem::action("Add to Queue", say("Menu → Add to Queue")).icon(icons::LIST),
        MenuItem::action("Show in File Explorer", say("Menu → Show")).icon(icons::FOLDER),
        MenuItem::Separator,
        MenuItem::check("Mark as Watched", true, say("Menu → Watched")),
        MenuItem::submenu("Sort By", sort_items(gallery)),
        MenuItem::action("Convert", say("Menu → Convert")).disabled(true),
        MenuItem::action(
            "Properties & Metadata of a file with a very long name that truncates",
            say("Menu → Properties"),
        )
        .icon(icons::INFO),
    ]
}

/// The "Sort By" submenu, which nests a submenu of its own.
fn sort_items(gallery: &WeakEntity<Gallery>) -> Vec<MenuItem> {
    let say = |text| report(gallery, text);
    vec![
        MenuItem::radio("Name", true, say("Sort → Name")),
        MenuItem::radio("Date Added", false, say("Sort → Date")),
        MenuItem::radio("Duration", false, say("Sort → Duration")),
        MenuItem::Separator,
        MenuItem::submenu(
            "Direction",
            vec![
                MenuItem::radio("Ascending", true, say("Sort → Asc")),
                MenuItem::radio("Descending", false, say("Sort → Desc")),
            ],
        ),
    ]
}
