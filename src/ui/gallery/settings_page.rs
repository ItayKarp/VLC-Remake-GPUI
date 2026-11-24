//! The gallery's Settings page (S5.1): any category's page as the registry draws it, for design
//! checks. Its controls change the real settings, like the theme switches.

use gpui::{Context, Div, Entity, SharedString, Subscription, Window, div, prelude::*, px};

use super::Gallery;
use super::basics::section;
use crate::settings::registry::{Category, Group};
use crate::settings_view::SettingsPage;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::dropdown::{self, Dropdown};

/// The page's height in the gallery: tall enough for most pages without scrolling.
const PAGE_HEIGHT: f32 = 640.;
/// The category picker's width.
const PICKER_WIDTH: f32 = 320.;

/// A category picker and the chosen category's page.
pub(super) struct SettingsDemo {
    picker: Entity<Dropdown>,
    page: Entity<SettingsPage>,
    category: Category,
    _picked: Subscription,
}

impl SettingsDemo {
    /// The General page, with a picker for the others.
    pub(super) fn new(window: &mut Window, cx: &mut Context<Gallery>) -> Self {
        let labels = Category::ALL.iter().map(|category| SharedString::from(label(*category)));
        let picker =
            cx.new(|cx| Dropdown::new("Settings category", labels.collect(), 0, window, cx));
        let on_pick =
            cx.subscribe_in(&picker, window, |this, _, event: &dropdown::Changed, window, cx| {
                let Some(category) = Category::ALL.get(event.0).copied() else { return };
                this.settings_demo.category = category;
                this.settings_demo.page = cx.new(|cx| SettingsPage::new(category, window, cx));
                cx.notify();
            });
        let category = Category::General;
        let page = cx.new(|cx| SettingsPage::new(category, window, cx));
        Self { picker, page, category, _picked: on_pick }
    }

    /// The section: the picker, the page's title, and the page.
    pub(super) fn render(&self, p: &Palette) -> Div {
        let picker = div().w(px(PICKER_WIDTH)).child(self.picker.clone());
        let title = div().text_size(text_px(20.)).text_color(p.text).child(self.category.title());
        let page = div()
            .h(px(PAGE_HEIGHT))
            .rounded(px(8.))
            .bg(p.background)
            .border_1()
            .border_color(p.divider)
            .child(self.page.clone());
        section("Settings page", p).child(picker).child(title).child(page)
    }
}

/// The category as the picker lists it: "Media › Playback".
fn label(category: Category) -> String {
    let group: Group = category.group();
    format!("{} › {}", group.label(), category.label())
}
