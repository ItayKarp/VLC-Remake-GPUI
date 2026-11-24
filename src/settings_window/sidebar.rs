//! The sidebar (S5.2.2, `docs/ui/settings/components/setting_category-*`): the App, Media and
//! System groups and their categories. The selected one has a raised background and an accent
//! icon. While a search is on, each category shows how many of its rows match, and those without
//! a match are dimmed and skipped. In a narrow window the sidebar shows icons only, with the name
//! on hover.

use gpui::{Context, Div, SharedString, Stateful, div, prelude::*, px};

use super::SettingsWindow;
use super::search::Search;
use crate::assets::icons;
use crate::settings::registry::{Category, Group};
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::icon::icon;
use crate::ui::{metrics, tooltip};

/// The sidebar's width, and its width with icons only.
pub(super) const WIDTH: f32 = 185.;
pub(super) const COMPACT_WIDTH: f32 = 56.;
/// A category row's height, its icon, and the space between icon and name.
const ROW_HEIGHT: f32 = 44.;
const ICON_SIZE: f32 = 20.;
const ICON_GAP: f32 = 16.;
/// The space inside the sidebar's panel and around a row's content.
const INSET: f32 = 6.;
const ROW_PADDING: f32 = 10.;
/// A group heading's height.
const GROUP_HEIGHT: f32 = 36.;
/// How much a category without search matches shows through.
const DIMMED_OPACITY: f32 = 0.4;

/// What the sidebar shows.
pub(super) struct Sidebar<'a> {
    /// The selected category.
    pub current: Category,
    /// The search, for match counts.
    pub search: &'a Search,
    /// Icons only.
    pub compact: bool,
}

impl Sidebar<'_> {
    /// The sidebar's panel, its groups and categories.
    pub(super) fn render(&self, p: &Palette, cx: &mut Context<SettingsWindow>) -> Stateful<Div> {
        let groups = [Group::App, Group::Media, Group::System];
        let mut panel = div()
            .id("settings-sidebar")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .p(px(INSET))
            .rounded(px(metrics::RADIUS_LARGE))
            .bg(p.sidebar);
        for group in groups {
            panel = panel.child(self.heading(group, p));
            for category in Category::ALL.into_iter().filter(|c| c.group() == group) {
                panel = panel.child(self.row(category, p, cx));
            }
        }
        panel
    }

    /// A group's heading; a thin rule when compact.
    fn heading(&self, group: Group, p: &Palette) -> Div {
        let heading = div().flex_none().h(px(GROUP_HEIGHT)).flex().items_center();
        if self.compact {
            return heading.child(div().w_full().h(px(1.)).bg(p.divider));
        }
        heading
            .px(px(ROW_PADDING + 4.))
            .text_size(text_px(metrics::TEXT))
            .text_color(p.text_muted)
            .child(group.label())
    }

    fn row(&self, category: Category, p: &Palette, cx: &mut Context<SettingsWindow>) -> Div {
        let selected = category == self.current;
        let offered = self.search.offers(category);
        let (icon_color, text_color) =
            if selected { (p.accent_fg, p.text_strong) } else { (p.icon, p.text) };
        let hover = p.hover;
        let row = div()
            .id(SharedString::from(format!("category-{}", category.label())))
            .h(px(ROW_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(ICON_GAP))
            .px(px(ROW_PADDING))
            .rounded(px(metrics::RADIUS))
            .aria_label(category.label())
            .aria_selected(selected)
            .text_size(text_px(metrics::TEXT))
            .text_color(text_color)
            .when(selected, |row| row.bg(p.selected))
            .when(!offered, |row| row.opacity(DIMMED_OPACITY))
            .when(offered && !selected, |row| row.cursor_pointer().hover(move |s| s.bg(hover)))
            .when(offered, |row| {
                row.on_click(cx.listener(move |this, _, window, cx| {
                    this.choose(category, window, cx);
                }))
            })
            .when(self.compact, |row| {
                row.justify_center().tooltip(tooltip::text(category.label(), None::<SharedString>))
            })
            .child(icon(category_icon(category), ICON_SIZE, icon_color))
            .when(!self.compact, |row| {
                row.child(div().flex_1().min_w_0().truncate().child(category.label()))
                    .children(self.count(category, p))
            });
        div().flex_none().child(row)
    }

    /// The number of matches on `category`'s page, while a search finds some there.
    fn count(&self, category: Category, p: &Palette) -> Option<Div> {
        let count = self.search.count(category);
        (count > 0).then(|| {
            div()
                .text_size(text_px(metrics::TEXT_SMALL))
                .text_color(p.text_muted)
                .child(count.to_string())
        })
    }
}

/// The icon beside `category` in the sidebar.
pub(super) fn category_icon(category: Category) -> &'static str {
    match category {
        Category::General | Category::Performance => icons::PREFERENCES,
        Category::Interface => icons::INTERFACE,
        Category::Privacy => icons::PRIVACY,
        Category::Controls => icons::HOTKEYS,
        Category::Playback => icons::PLAYBACK,
        Category::Video => icons::VIDEO,
        Category::Audio => icons::AUDIO,
        Category::Subtitles => icons::SUBTITLES,
        Category::Library => icons::FOLDER,
        Category::Network => icons::STREAMING,
        Category::Recording => icons::RECORD,
        Category::Extensions => icons::PLUGINS,
    }
}
