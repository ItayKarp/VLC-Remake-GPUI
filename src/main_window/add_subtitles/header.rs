//! The Add Subtitles dialog's header row: the Target and Origin dropdowns, then the origin's
//! own field: the source folder (From Computer), a search and a language (From OpenSubtitles),
//! or a link (From Link).

use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, Entity, Stateful, accesskit, div, prelude::*, px,
};

use super::AddSubtitlesDialog;
use super::model::Origin;
use super::render::HEADER_INSET;
use crate::assets::icons;
use crate::settings;
use crate::theme::Palette;
use crate::ui::icon::icon;
use crate::ui::{focus, metrics};

/// The header row's controls' height and the dropdowns' width.
pub(super) const CONTROL_HEIGHT: f32 = 34.;
const DROPDOWN_WIDTH: f32 = 200.;
/// The space between the header row's controls.
const GAP: f32 = 10.;
/// What separates a source folder's parts.
const SEPARATOR: &str = " / ";

impl AddSubtitlesDialog {
    /// Target, Origin, and the origin's field.
    pub(super) fn header_row(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let fields: Vec<AnyElement> = match self.origin {
            Origin::Computer => vec![self.source_field(p, cx).into_any_element()],
            Origin::OpenSubtitles => vec![
                framed(icons::SEARCH, p)
                    .child(self.controls.search_field.clone())
                    .into_any_element(),
                dropdown(&self.controls.language_menu).into_any_element(),
            ],
            Origin::Link => {
                vec![
                    framed(icons::STREAMING, p)
                        .child(self.controls.link_field.clone())
                        .into_any_element(),
                ]
            }
        };
        div()
            .px(px(HEADER_INSET))
            .flex()
            .items_center()
            .gap(px(GAP))
            .child(dropdown(&self.controls.target_menu))
            .child(dropdown(&self.controls.origin_menu))
            .children(fields)
    }

    /// From Computer's source folder; a click picks another.
    fn source_field(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let shown = self.source.as_deref().map_or_else(
            || "Choose a folder with subtitles".to_owned(),
            |folder| source_text(folder, cx),
        );
        let field = framed(icons::FOLDER, p)
            .id("add-subtitles-source")
            .tab_index(0)
            .key_context(focus::CONTROL)
            .role(accesskit::Role::Button)
            .aria_label("Source folder")
            .cursor_pointer()
            .hover(|style| style.bg(p.button_hover))
            .on_click(cx.listener(|_, _: &ClickEvent, window, cx| Self::pick_source(window, cx)))
            .child(div().min_w_0().truncate().child(shown));
        focus::ring(field, p)
    }
}

/// A header field's frame, holding `icon_path` and whatever is added.
fn framed(icon_path: &'static str, p: &Palette) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .h(px(CONTROL_HEIGHT))
        .flex()
        .items_center()
        .gap(px(14.))
        .px(px(12.))
        .rounded(px(metrics::RADIUS + 2.))
        .bg(p.control)
        .text_size(px(metrics::TEXT))
        .text_color(p.text_muted)
        .child(icon(icon_path, metrics::ICON + 2., p.icon))
}

fn dropdown<V: Render>(menu: &Entity<V>) -> Div {
    div().flex_none().w(px(DROPDOWN_WIDTH)).child(menu.clone())
}

/// How the source field shows `folder`: from the media folder holding it, named as Videos
/// shows it ("Videos / Shows / The Wand / S01"), else from the home folder.
fn source_text(folder: &Path, cx: &App) -> String {
    let roots: &[PathBuf] = &settings::get(cx).local.media_folders;
    let inside = roots.iter().find_map(|root| Some((root, folder.strip_prefix(root).ok()?)));
    let Some((root, rest)) = inside else {
        let shown = settings::display_path(folder);
        let parts: Vec<&str> =
            shown.split(std::path::MAIN_SEPARATOR).filter(|part| !part.is_empty()).collect();
        return parts.join(SEPARATOR);
    };
    let first =
        root.file_name().map_or_else(|| "Videos".to_owned(), |n| n.to_string_lossy().into());
    let parts = rest.components().map(|part| part.as_os_str().to_string_lossy().into_owned());
    std::iter::once(first).chain(parts).collect::<Vec<_>>().join(SEPARATOR)
}
