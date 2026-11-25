//! An effect section of the settings panel (S7.1.2–S7.1.4, `panels/video/essential/*`): its title
//! and enable toggle over a box that reads "Expand Settings ›" folded and "Collapse Settings ⌄"
//! open, with the section's sliders inside. The designs have neither title nor toggle (D23), so
//! users couldn't tell which effects are on: both sit on a row over the box, with a Reset that
//! puts the whole section back on its defaults, dimmed while nothing differs.
//!
//! Each slider is a setting from the registry, with its Settings page's label, range and format,
//! showing the value in effect ([`effects`]). A double-click puts one slider back on its default
//! and lets it follow Settings again (S7.1.4). A slider under one of the section's own switches is
//! disabled while that switch is off (S7.4.3); the section's toggle never disables one, as moving
//! it turns the section on. Whether a section is open is remembered across launches, per section.

use gpui::{
    App, Context, Div, Entity, SharedString, Stateful, Subscription, Window, div, prelude::*, px,
};

use crate::assets::icons;
use crate::effects::{self, EffectSection};
use crate::i18n::DirectionalStyle;
use crate::main_window::MainWindow;
use crate::settings::registry::{self, Control, Marks, Value};
use crate::settings::{self};
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::icon::icon;
use crate::ui::slider::{self, Slider, SliderModel};
use crate::ui::toggle::Toggle;
use crate::ui::{focus, metrics};

/// The title row's height, and the space between its controls.
const TITLE_HEIGHT: f32 = 44.;
const TITLE_GAP: f32 = 8.;
/// The text size of the title, the header and the slider labels.
pub(super) const TEXT: f32 = 14.;
/// The box's padding, and the space above each slider's label.
pub(super) const BOX_PADDING: f32 = 16.;
const ROW_GAP: f32 = 12.;
/// The chevron's size.
const CHEVRON: f32 = 20.;
/// How faded a disabled control's label is, as the kit's disabled controls (S5.1.3).
pub(super) const DISABLED_OPACITY: f32 = 0.4;

/// A section's sliders, made once and kept in step with the values in effect.
pub(super) struct EffectControls {
    pub(super) section: EffectSection,
    sliders: Vec<(&'static str, &'static str, Entity<Slider>)>,
    _events: Vec<Subscription>,
}

impl EffectControls {
    /// The sliders of `section`'s slider controls, changing the session when moved; its other
    /// controls are left to the caller.
    pub(super) fn new(
        section: EffectSection,
        window: &mut Window,
        cx: &mut Context<MainWindow>,
    ) -> Self {
        let mut sliders = Vec::new();
        let mut events = Vec::new();
        for key in section.controls.iter().filter(|key| is_slider(key)) {
            let Some((label, slider)) = make_slider(key, window, cx) else { continue };
            events.push(cx.subscribe(&slider, move |_, _, event: &slider::Changed, cx| {
                effects::change(cx, &section, key, Value::Number(event.0));
            }));
            events.push(cx.subscribe(&slider, move |_, _, _: &slider::Reset, cx| {
                effects::reset(cx, [*key]);
            }));
            sliders.push((*key, label, slider));
        }
        Self { section, sliders, _events: events }
    }

    /// The slider of the setting `key`.
    pub(super) fn slider(&self, key: &str) -> Option<(&'static str, &Entity<Slider>)> {
        self.sliders.iter().find(|(k, ..)| *k == key).map(|(_, label, slider)| (*label, slider))
    }

    /// Shows the values in effect, makes each default the one a double-click goes back to, and
    /// disables the sliders whose switch is off.
    pub(super) fn sync(&self, window: &mut Window, cx: &mut Context<MainWindow>) {
        for (key, _, slider) in &self.sliders {
            let live = is_live(cx, &self.section, key);
            let (Some(value), Some(default)) = (
                number(effects::value(cx, key).as_ref()),
                number(effects::default_value(cx, key).as_ref()),
            ) else {
                continue;
            };
            slider.update(cx, |slider, cx| {
                slider.set_disabled(!live, cx);
                slider.set_default(default);
                slider.set_value(value, window, cx);
            });
        }
    }

    /// The section: its title row, then its box.
    pub(super) fn render(&self, p: &Palette, cx: &App) -> Div {
        let content = is_expanded(cx, &self.section).then(|| {
            div().flex_row_start().flex_col().px(px(BOX_PADDING)).pb(px(BOX_PADDING)).children(
                self.sliders.iter().map(|(key, label, slider)| {
                    slider_row(label, slider, is_live(cx, &self.section, key), p)
                }),
            )
        });
        folding(&self.section, content, p, cx)
    }
}

/// A slider under its label, which is dimmed with it while the slider isn't `live`.
pub(super) fn slider_row(
    label: &'static str,
    slider: &Entity<Slider>,
    live: bool,
    p: &Palette,
) -> Div {
    div()
        .flex_row_start()
        .flex_col()
        .gap(px(ROW_GAP))
        .pt(px(ROW_GAP))
        .text_size(text_px(TEXT))
        .text_color(p.text_muted)
        .child(div().when(!live, |label| label.opacity(DISABLED_OPACITY)).child(label))
        .child(slider.clone())
}

/// Whether `key`'s control can be used: one depending on another of `section`'s switches only
/// while that switch is on (S7.4.3). The section's toggle never disables a control.
pub(super) fn is_live(cx: &App, section: &EffectSection, key: &str) -> bool {
    let Some(dependency) = registry::find(key).and_then(|def| def.dependency) else { return true };
    dependency.key == section.toggle
        || effects::value(cx, dependency.key)
            .is_some_and(|value| dependency.condition.holds(&value))
}

/// Whether the setting `key` is shown with a slider.
fn is_slider(key: &str) -> bool {
    registry::find(key).is_some_and(|def| matches!(def.control, Control::Slider { .. }))
}

/// `section`'s title row over its box, which holds `content` while open (`None` folded).
pub(super) fn folding(section: &EffectSection, content: Option<Div>, p: &Palette, cx: &App) -> Div {
    let expanded = content.is_some();
    // Folded, the box is a well (`video_essential-collapsed`); open, its controls are wells
    // themselves, so it keeps the panel's color inside a border.
    let fold = div()
        .flex_row_start()
        .flex_col()
        .rounded(px(metrics::RADIUS_LARGE))
        .when(expanded, |fold| fold.border_1().border_color(p.divider))
        .when(!expanded, |fold| fold.bg(p.well))
        .child(header(section, expanded, p))
        .children(content);
    div().flex_row_start().flex_col().child(title_row(section, p, cx)).child(fold)
}

/// The section's title, its Reset and its toggle.
fn title_row(section: &EffectSection, p: &Palette, cx: &App) -> Div {
    let on = effects::value(cx, section.toggle) == Some(Value::Bool(true));
    let section = *section;
    let title = crate::i18n::text(section.title);
    let reset = Button::new(element_id("reset-effect", &section), tr!("Reset"))
        .style(ButtonStyle::Ghost)
        .disabled(!effects::any_changed(cx, &section))
        .tooltip(tr!("Put {title} back on its defaults", title), None::<&str>)
        .on_click(move |_, _, cx| effects::reset(cx, section.keys()));
    let toggle = Toggle::new(element_id("effect-toggle", &section), on, title)
        .on_change(move |on, _, cx| effects::set_enabled(cx, &section, *on));
    div()
        .h(px(TITLE_HEIGHT))
        .flex_row_start()
        .items_center()
        .justify_between()
        .text_size(text_px(TEXT))
        .text_color(p.text_muted)
        .child(title)
        .child(div().flex_row_start().items_center().gap(px(TITLE_GAP)).child(reset).child(toggle))
}

/// The box's header button: "Expand Settings ›" or "Collapse Settings ⌄".
fn header(section: &EffectSection, expanded: bool, p: &Palette) -> Stateful<Div> {
    let title = crate::i18n::text(section.title);
    let (text, label, chevron) = if expanded {
        let label = tr!("Collapse Settings: {title}", title);
        (tr!("Collapse Settings"), label, icons::CHEVRON_DOWN)
    } else {
        (
            tr!("Expand Settings"),
            tr!("Expand Settings: {title}", title),
            crate::i18n::mirrored(icons::CHEVRON_RIGHT, icons::CHEVRON_LEFT),
        )
    };
    let id = section.id;
    let hover = p.text;
    let header = div()
        .id(element_id("effect-fold", section))
        .h(px(metrics::SECTION_HEIGHT))
        .flex_row_start()
        .items_center()
        .justify_between()
        .px(px(BOX_PADDING))
        .rounded(px(metrics::RADIUS_LARGE))
        .border_2()
        .border_color(gpui::transparent_black())
        .text_size(text_px(metrics::TEXT))
        .text_color(p.text_muted)
        .tab_index(0)
        .cursor_pointer()
        .aria_expanded(expanded)
        .aria_label(label)
        .hover(move |s| s.text_color(hover))
        .on_click(move |_, _, cx| set_expanded(cx, id, !expanded))
        .child(text)
        .child(icon(chevron, CHEVRON, p.icon));
    focus::ring(header, p)
}

/// The slider for the setting `key`, and its label; `None` if it isn't a slider setting.
fn make_slider(
    key: &'static str,
    window: &mut Window,
    cx: &mut Context<MainWindow>,
) -> Option<(&'static str, Entity<Slider>)> {
    let def = registry::find(key)?;
    let (Control::Slider { marks, format }, Some(range)) = (def.control, def.range) else {
        tracing::warn!("effects: {key} isn't a slider setting");
        return None;
    };
    let label = crate::i18n::text(def.label);
    let default = number(Some(&def.default_value())).unwrap_or(range.min);
    let model = SliderModel::new(range.min, range.max, range.step, default);
    let slider = cx.new(|cx| {
        let slider = Slider::new(label, model, window, cx).accent().format(format);
        match marks {
            Marks::None => slider,
            Marks::Ticks(every) => slider.bare_marks(every),
            Marks::Labelled(every, label) => slider.marks(every, label),
        }
    });
    Some((label, slider))
}

/// An id for one of `section`'s elements, `what` it is.
pub(super) fn element_id(what: &str, section: &EffectSection) -> SharedString {
    format!("{what}-{}", section.id).into()
}

/// A value's number, if it is one.
fn number(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(number)) => Some(*number),
        _ => None,
    }
}

/// Whether `section` was left open.
pub(super) fn is_expanded(cx: &App, section: &EffectSection) -> bool {
    settings::get(cx).local.expanded_effects.iter().any(|id| id == section.id)
}

/// Opens or folds the section `id`, remembering it.
fn set_expanded(cx: &mut App, id: &str, expanded: bool) {
    settings::update(cx, |s| {
        let open = &mut s.local.expanded_effects;
        open.retain(|other| other != id);
        if expanded {
            open.push(id.to_owned());
        }
    });
}

#[cfg(test)]
mod tests;
