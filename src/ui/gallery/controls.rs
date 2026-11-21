//! The gallery's input sections: toggles, value pickers, dropdowns, tabs, sliders, the
//! collapsible panel and the virtualized grid.

use gpui::{Context, Div, div, prelude::*, px};

use super::basics::{row, section};
use super::{Gallery, updating};
use crate::assets::icons;
use crate::theme::Palette;
use crate::ui::collapsible::Collapsible;
use crate::ui::segmented::{Choice, Segmented, StackedChoice};
use crate::ui::tabs::{Tab, Tabs};
use crate::ui::toggle::{Checkbox, Toggle};

/// The width of each segmented value picker.
const VALUE_WIDTH: f32 = 213.;
/// The width of the dropdowns and the text input.
const PICKER_WIDTH: f32 = 300.;

impl Gallery {
    /// Toggles, checkboxes and value pickers in their on, off and disabled states.
    pub(super) fn toggles(&self, p: &Palette, cx: &Context<Self>) -> Div {
        section("Toggles, checkbox and values", p)
            .child(self.toggle_row(cx))
            .child(self.value_row(cx))
    }

    fn toggle_row(&self, cx: &Context<Self>) -> Div {
        let values = &self.values;
        row()
            .child(
                Toggle::new("t-on", values.toggles.on, "Negate colors")
                    .on_change(updating(cx, |this, on: &bool| this.values.toggles.on = *on)),
            )
            .child(
                Toggle::new("t-off", values.toggles.off, "Posterize")
                    .on_change(updating(cx, |this, on: &bool| this.values.toggles.off = *on)),
            )
            .child(Toggle::new("t-dis", true, "Disabled").disabled(true))
            .child(
                Checkbox::new("c", values.remember_position, "Remember last position")
                    .on_change(updating(cx, |this, on: &bool| this.values.remember_position = *on)),
            )
            .child(Checkbox::new("c-dis", false, "Unavailable").disabled(true))
    }

    fn value_row(&self, cx: &Context<Self>) -> Div {
        let values = &self.values;
        row()
            .child(
                div().w(px(VALUE_WIDTH)).child(
                    Segmented::new("v2", "Unit", ["Relative", "Pixels"], values.unit)
                        .on_select(updating(cx, |this, i: &usize| this.values.unit = *i)),
                ),
            )
            .child(
                div().w(px(VALUE_WIDTH)).child(
                    Segmented::new("v3", "Value", ["1", "2", "3"], values.value)
                        .on_select(updating(cx, |this, i: &usize| this.values.value = *i)),
                ),
            )
            .child(div().w(px(320.)).child(self.starting_volume(cx)))
    }

    /// A stacked choice whose second option carries its own text field.
    fn starting_volume(&self, cx: &Context<Self>) -> StackedChoice {
        let choices = vec![
            Choice::new("Remember last volume"),
            Choice::new("Always starts at:").field(self.widgets.volume_field.clone()),
        ];
        StackedChoice::new("stacked", "Starting volume", choices, self.values.starting_volume)
            .on_select(updating(cx, |this, i: &usize| this.values.starting_volume = *i))
    }

    /// Dropdowns (short and scrolling), a validated text input and tabs.
    pub(super) fn pickers(&self, p: &Palette, cx: &Context<Self>) -> Div {
        let widgets = &self.widgets;
        section("Dropdowns, text input, tabs", p)
            .child(
                row()
                    .child(div().w(px(PICKER_WIDTH)).child(widgets.dropdown.clone()))
                    .child(div().w(px(PICKER_WIDTH)).child(widgets.long_dropdown.clone())),
            )
            .child(row().child(div().w(px(PICKER_WIDTH)).child(widgets.name.clone())))
            .child(self.tabs(cx))
    }

    fn tabs(&self, cx: &Context<Self>) -> Tabs {
        let tabs = vec![
            Tab::new(Some(icons::PLAY), "Playback"),
            Tab::new(Some(icons::FILE), "Video"),
            Tab::new(Some(icons::AUDIO), "Audio"),
            Tab::new(Some(icons::SUBTITLES), "Subtitles"),
        ];
        Tabs::new("tabs", tabs, self.values.tab)
            .on_select(updating(cx, |this, i: &usize| this.values.tab = *i))
    }

    /// Horizontal sliders (plain, with a field, centred) and a vertical equalizer.
    pub(super) fn sliders(&self, p: &Palette) -> Div {
        let sliders = &self.widgets.sliders;
        section("Sliders", p)
            .child(div().text_color(p.text_muted).child("Saturation"))
            .child(sliders.saturation.clone())
            .child(div().text_color(p.text_muted).child("Sepia Intensity"))
            .child(sliders.sepia.clone())
            .child(div().text_color(p.text_muted).child("Audio Track Delay"))
            .child(sliders.delay.clone())
            .child(
                div()
                    .h(px(180.))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .children(sliders.eq.iter().cloned()),
            )
    }

    /// A settings panel made of collapsible sections, like the player's side panels.
    pub(super) fn panel(&self, p: &Palette, cx: &Context<Self>) -> Div {
        div()
            .w(px(360.))
            .rounded(px(10.))
            .bg(p.popover)
            .border_1()
            .border_color(p.divider)
            .child(self.open_section(cx))
            .child(Collapsible::new("section-2", "Sharpen", false))
    }

    fn open_section(&self, cx: &Context<Self>) -> Collapsible {
        Collapsible::new("section", "Collapse Settings", self.section_open)
            .on_toggle(updating(cx, |this, open: &bool| this.section_open = *open))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child("Negate Colors")
                    .child(Toggle::new("t-panel", self.values.toggles.off, "Negate colors")),
            )
            .child(div().child("Saturation"))
            .child(div().h(px(28.)).child(self.widgets.sliders.panel_saturation.clone()))
    }

    /// The virtualized media grid.
    pub(super) fn grid(&self, p: &Palette) -> Div {
        section("Virtualized grid (10,000 items)", p)
            .child(div().h(px(260.)).child(self.widgets.grid.clone()))
    }
}
