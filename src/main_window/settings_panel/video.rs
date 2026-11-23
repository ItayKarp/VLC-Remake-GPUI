//! The settings panel's Video tab (`controls-video_settings.png`): **Defaults** (flip, rotate,
//! zoom, resolution, aspect ratio and crop, S7.2; render, S8.11) with the **Crop** section's edges under them,
//! then **Filters**, the effect sections (S7.1): the Essential picture adjustments and
//! enhancements (S7.3), then the Colors (S7.4). All of them change what plays for the rest of the
//! session.

use gpui::{Context, Div, Window, div, prelude::*, px};

use super::{PanelTab, SECTION_PADDING, section};
use crate::main_window::MainWindow;
use crate::theme::Palette;

/// What the Filters heading says its sections change.
const FILTERS_NOTE: &str = "For what plays until VLC quits. Double-click a slider to put it \
                            back; Save as Default keeps them for later sessions.";

impl MainWindow {
    /// The Video tab: Defaults and the Crop section, then Filters and its sections.
    pub(super) fn render_video_tab(
        &mut self,
        p: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        self.settings_panel.video_defaults.sync(window, cx);
        let player = self.player.as_ref().map(|(player, _)| player.clone());
        self.settings_panel.video_defaults.render.sync(player.as_ref(), cx);
        self.settings_panel.crop.sync(window, cx);
        self.settings_panel.colors.sync(window, cx);
        let shown = PanelTab::Video.sections();
        let sections = self.settings_panel.effects.iter().filter(|c| shown.contains(&c.section));
        for controls in sections.clone() {
            controls.sync(window, cx);
        }
        let defaults = div()
            .flex()
            .flex_col()
            .p(px(SECTION_PADDING))
            .child(self.render_video_defaults(p, window, cx))
            .child(self.render_crop_section(p, cx));
        let filters = section("Filters", FILTERS_NOTE, p)
            .children(sections.map(|controls| controls.render(p, cx)))
            .child(self.settings_panel.colors.render(p, cx));
        div().flex().flex_col().child(defaults).child(div().h(px(1.)).bg(p.divider)).child(filters)
    }
}
