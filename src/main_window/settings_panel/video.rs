//! The settings panel's Video tab (`controls-video_settings.png`): **Filters**, the effect
//! sections that change what plays for the rest of the session (S7.1), now the Essential picture
//! adjustments. The designed Defaults rows (flip, rotate, zoom, aspect ratio, crop, resolution,
//! render) come with S7.2, and the other filters with S7.3 to S7.5.

use gpui::{Context, Div, Window, div, prelude::*};

use super::{PanelTab, section};
use crate::main_window::MainWindow;
use crate::theme::Palette;

/// What the Filters heading says its sections change.
const FILTERS_NOTE: &str = "For what plays until VLC quits. Double-click a slider to put it \
                            back; Save as Default keeps them for later sessions.";

impl MainWindow {
    /// The Video tab: Filters and its sections.
    pub(super) fn render_video_tab(
        &mut self,
        p: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let shown = PanelTab::Video.sections();
        let sections = self.settings_panel.effects.iter().filter(|c| shown.contains(&c.section));
        for controls in sections.clone() {
            controls.sync(window, cx);
        }
        let filters = section("Filters", FILTERS_NOTE, p)
            .children(sections.map(|controls| controls.render(p, cx)));
        div().flex().flex_col().child(filters)
    }
}
