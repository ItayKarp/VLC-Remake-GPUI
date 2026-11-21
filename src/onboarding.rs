//! First-run onboarding, following `docs/onboarding_scenes`:
//!
//! 1. Welcome + choose a media folder
//! 2. Theme (light / dark / system) and accent color, with a live player preview
//! 3. Basic keyboard shortcuts

use std::path::PathBuf;

use gpui::{
    AnyElement, ClickEvent, Context, FocusHandle, FontWeight, Hsla, KeyDownEvent,
    PathPromptOptions, SharedString, Window, div, prelude::*, px, relative, svg,
};

use crate::assets::icons;
use crate::settings::{self, OnboardingResult};
use crate::theme::{Accent, Palette, ThemeMode, blend};
use crate::window_chrome::{self, TITLE_BAR_HEIGHT};

/// Window size: the design's 1088×760 content area plus our title strip.
pub const WINDOW_WIDTH: f32 = 1088.;
pub const WINDOW_HEIGHT: f32 = 760. + TITLE_BAR_HEIGHT;

/// Space between the bottom-row buttons and the window edges.
const EDGE_RIGHT: f32 = 62.;
const EDGE_BOTTOM: f32 = 56.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Welcome,
    Appearance,
    Shortcuts,
}

pub struct Onboarding {
    step: Step,
    media_folder: PathBuf,
    theme: ThemeMode,
    accent: Accent,
    focus_handle: FocusHandle,
}

impl Onboarding {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        // Re-render when the OS switches light/dark, for the "System" theme.
        cx.observe_window_appearance(window, |_, _, cx| cx.notify())
            .detach();

        let mut this = Self {
            step: Step::Welcome,
            media_folder: settings::default_media_folder(),
            theme: ThemeMode::default(),
            accent: Accent::default(),
            focus_handle,
        };
        this.apply_dev_overrides();
        this
    }

    /// Debug builds only: `RVLC_STEP=1..3`, `RVLC_THEME=light|dark|system` and
    /// `RVLC_ACCENT=<name>` open onboarding in a given state, for design checks.
    fn apply_dev_overrides(&mut self) {
        if !cfg!(debug_assertions) {
            return;
        }
        let var = |name| std::env::var(name).ok();
        if let Some(step) = var("RVLC_STEP") {
            self.step = match step.as_str() {
                "2" => Step::Appearance,
                "3" => Step::Shortcuts,
                _ => Step::Welcome,
            };
        }
        if let Some(theme) = var("RVLC_THEME") {
            self.theme = ThemeMode::ALL
                .into_iter()
                .find(|m| m.key() == theme)
                .unwrap_or_default();
        }
        if let Some(accent) = var("RVLC_ACCENT") {
            self.accent = Accent::ALL
                .into_iter()
                .find(|a| a.key() == accent)
                .unwrap_or_default();
        }
    }

    fn palette(&self, window: &Window) -> Palette {
        Palette::new(self.theme.is_dark(window.appearance()), self.accent)
    }

    fn go_next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.step {
            Step::Welcome => self.step = Step::Appearance,
            Step::Appearance => self.step = Step::Shortcuts,
            Step::Shortcuts => return self.finish(window, cx),
        }
        cx.notify();
    }

    fn go_back(&mut self, cx: &mut Context<Self>) {
        self.step = match self.step {
            Step::Welcome | Step::Appearance => Step::Welcome,
            Step::Shortcuts => Step::Appearance,
        };
        cx.notify();
    }

    fn finish(&mut self, window: &mut Window, _cx: &mut Context<Self>) {
        let result = OnboardingResult {
            media_folder: self.media_folder.clone(),
            theme: self.theme,
            accent: self.accent,
        };
        if let Err(err) = result.save() {
            eprintln!("failed to save onboarding settings: {err}");
        }
        // TODO: open the library window here once it exists.
        window.remove_window();
    }

    fn browse(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose Media Folder".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(folder) = paths.into_iter().next()
            {
                this.update(cx, |this, cx| {
                    this.media_folder = folder;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn cycle_accent(&mut self, delta: isize, cx: &mut Context<Self>) {
        let len = Accent::ALL.len() as isize;
        let index = Accent::ALL.iter().position(|a| *a == self.accent).unwrap_or(0) as isize;
        self.accent = Accent::ALL[(index + delta).rem_euclid(len) as usize];
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "enter" => self.go_next(window, cx),
            "escape" | "backspace" if self.step != Step::Welcome => self.go_back(cx),
            "left" if self.step == Step::Appearance => self.cycle_accent(-1, cx),
            "right" if self.step == Step::Appearance => self.cycle_accent(1, cx),
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Render for Onboarding {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette(window);

        let content = match self.step {
            Step::Welcome => self.render_welcome(&palette, cx),
            Step::Appearance => self.render_appearance(&palette, cx),
            Step::Shortcuts => self.render_shortcuts(&palette, cx),
        };

        div()
            .id("onboarding")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .relative()
            .size_full()
            .bg(palette.background)
            .text_color(palette.text)
            .child(
                div()
                    .absolute()
                    .top(px(TITLE_BAR_HEIGHT))
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .child(content),
            )
            .child(window_chrome::title_bar(&palette))
    }
}

// ---------------------------------------------------------------------------
// Step 1 — Welcome
// ---------------------------------------------------------------------------

impl Onboarding {
    fn render_welcome(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let folder = SharedString::from(settings::display_path(&self.media_folder));

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(118.))
            .child(svg().path(icons::LOGO).size(px(140.)).text_color(p.accent))
            .child(
                div()
                    .mt(px(18.))
                    .text_size(px(36.))
                    .line_height(px(44.))
                    .font_weight(FontWeight::LIGHT)
                    .text_color(p.text_strong)
                    .child("Welcome to VLC!"),
            )
            .child(
                div()
                    .w(px(594.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .mt(px(34.))
                            .text_size(px(16.))
                            .line_height(px(28.))
                            .text_color(p.text_muted)
                            .child(
                                "VLC is a free and open source cross-platform multimedia player \
                                 and framework that plays most multimedia files as well as DVDs, \
                                 Audio CDs, VCDs, and various streaming protocols.",
                            ),
                    )
                    .child(
                        div()
                            .mt(px(50.))
                            .text_size(px(15.))
                            .line_height(px(20.))
                            .text_color(p.text_muted)
                            .child("Choose a Media Folder to get started with"),
                    )
                    .child(
                        div()
                            .mt(px(14.))
                            .flex()
                            .flex_row()
                            .gap(px(11.))
                            .h(px(35.))
                            .child(
                                div()
                                    .id("media-folder")
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .px(px(13.))
                                    .rounded(px(4.))
                                    .bg(p.sunken)
                                    .border_2()
                                    .border_color(p.border)
                                    .text_size(px(16.))
                                    .text_color(p.text_strong)
                                    .overflow_hidden()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.browse(cx)
                                    }))
                                    .child(div().truncate().child(folder)),
                            )
                            .child(
                                button("browse", "Browse", p.button, p.button_hover, p.text_muted)
                                    .w(px(114.))
                                    .font_weight(FontWeight::NORMAL)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.browse(cx)
                                    })),
                            ),
                    ),
            )
            .child(
                bottom_bar()
                    .justify_end()
                    .child(self.nav_buttons(p, false, "Next", cx)),
            )
            .into_any_element()
    }

    /// Back / Next (or Finish) buttons in the bottom-right corner.
    fn nav_buttons(
        &self,
        p: &Palette,
        show_back: bool,
        next_label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .gap(px(8.))
            .when(show_back, |this| {
                this.child(
                    button("back", "Back", p.button, p.button_hover, p.text_muted)
                        .w(px(100.))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.go_back(cx))),
                )
            })
            .child(
                button("next", next_label, p.accent, p.accent.opacity(0.85), p.on_accent)
                    .w(px(100.))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.go_next(window, cx)
                    })),
            )
    }
}

fn button(
    id: &'static str,
    label: &'static str,
    bg: Hsla,
    hover_bg: Hsla,
    fg: Hsla,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .bg(bg)
        .hover(move |style| style.bg(hover_bg))
        .cursor_pointer()
        .text_size(px(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(fg)
        .child(label)
}

/// Row pinned to the bottom of the content area, aligned with the nav buttons.
fn bottom_bar() -> gpui::Div {
    div()
        .absolute()
        .left_0()
        .right(px(EDGE_RIGHT))
        .bottom(px(EDGE_BOTTOM))
        .h(px(32.))
        .flex()
        .flex_row()
        .items_center()
}

// ---------------------------------------------------------------------------
// Step 2 — Theme and accent color
// ---------------------------------------------------------------------------

const PREVIEW_INSET: f32 = 17.;
const PREVIEW_HEIGHT: f32 = 594.;
const SWATCH_SIZE: f32 = 40.;
const SWATCH_GAP: f32 = 5.;
const SWATCH_RING: f32 = 4.;

impl Onboarding {
    fn render_appearance(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        div()
            .relative()
            .size_full()
            .child(
                div()
                    .absolute()
                    .top(px(PREVIEW_INSET))
                    .left(px(PREVIEW_INSET))
                    .right(px(PREVIEW_INSET))
                    .h(px(PREVIEW_HEIGHT))
                    .child(self.render_preview(p, cx)),
            )
            .child(
                bottom_bar()
                    .left(px(43.))
                    .justify_between()
                    .child(self.render_swatches(p, cx))
                    .child(self.nav_buttons(p, true, "Next", cx)),
            )
            .into_any_element()
    }

    /// A miniature player window that reflects the chosen theme and accent.
    fn render_preview(&self, p: &Palette, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.sunken)
            .overflow_hidden()
            // Mock title bar
            .child(
                div()
                    .h(px(22.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(8.))
                    .bg(p.title_bar)
                    .child(svg().path(icons::LOGO_SMALL).size(px(14.)).text_color(p.accent))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.))
                            .children(
                                [icons::MINIMIZE, icons::MAXIMIZE, icons::FULLSCREEN, icons::CLOSE]
                                    .map(|icon| svg().path(icon).size(px(12.)).text_color(p.icon)),
                            ),
                    ),
            )
            // Theme picker
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .pt(px(136.))
                    .child(
                        div()
                            .text_size(px(26.))
                            .line_height(px(34.))
                            .font_weight(FontWeight::LIGHT)
                            .text_color(p.text)
                            .child("Select your preferred theme and color."),
                    )
                    .child(
                        div()
                            .mt(px(66.))
                            .flex()
                            .flex_row()
                            .gap(px(34.))
                            .children(ThemeMode::ALL.map(|mode| self.theme_tile(mode, p, cx))),
                    ),
            )
            .child(self.render_control_bar(p))
    }

    fn theme_tile(
        &self,
        mode: ThemeMode,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let selected = self.theme == mode;
        let light: Hsla = gpui::rgb(0xAFAFAF).into();
        let dark: Hsla = gpui::rgb(0x212121).into();

        let swatch = div()
            .relative()
            .size(px(100.))
            .rounded(px(12.))
            .overflow_hidden()
            .bg(if mode == ThemeMode::Light { light } else { dark })
            .when(mode == ThemeMode::System, |this| {
                this.child(
                    svg()
                        .absolute()
                        .top_0()
                        .left_0()
                        .path(icons::THEME_SYSTEM)
                        .size(px(100.))
                        .text_color(light),
                )
            });

        div()
            .id(SharedString::from(format!("theme-{}", mode.key())))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(16.))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.theme = mode;
                cx.notify();
            }))
            .child(
                div()
                    .p(px(8.))
                    .rounded(px(22.))
                    .border(px(3.))
                    .border_color(if selected { p.accent } else { gpui::transparent_black() })
                    .child(swatch),
            )
            .child(
                div()
                    .text_size(px(16.))
                    .text_color(if selected { p.text_strong } else { p.text })
                    .child(mode.label()),
            )
    }

    fn render_control_bar(&self, p: &Palette) -> impl IntoElement {
        let icon = |path: &'static str| svg().path(path).size(px(12.)).text_color(p.icon);
        let time = |label: &'static str| {
            div()
                .text_size(px(8.))
                .text_color(p.text_muted)
                .child(label)
        };

        div()
            .h(px(29.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .px(px(14.))
            .bg(p.control_bar)
            .child(icon(icons::PREVIOUS))
            .child(
                div()
                    .w(px(29.))
                    .h(px(14.))
                    .rounded(px(3.))
                    .bg(p.accent)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(svg().path(icons::PLAY).size(px(11.)).text_color(p.on_accent)),
            )
            .child(icon(icons::NEXT))
            .child(time("00:02:10"))
            // Seek bar
            .child(
                div()
                    .flex_1()
                    .h(px(13.))
                    .rounded(px(2.))
                    .bg(p.sunken)
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .w(relative(0.075))
                            .h_full()
                            .rounded_l(px(2.))
                            .bg(blend(p.accent, p.sunken, 0.23)),
                    )
                    .child(div().w(px(3.)).h(px(11.)).rounded(px(1.)).bg(p.accent)),
            )
            .child(time("00:25:40"))
            .child(icon(icons::VOLUME))
            // Volume bar
            .child(
                div()
                    .w(px(48.))
                    .h(px(9.))
                    .rounded(px(2.))
                    .bg(p.sunken)
                    .child(div().w(relative(0.35)).h_full().rounded(px(2.)).bg(p.volume)),
            )
            .child(icon(icons::AUDIO))
            .child(icon(icons::SUBTITLES))
            .child(icon(icons::SETTINGS))
            .child(icon(icons::LIST))
    }

    fn render_swatches(&self, p: &Palette, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(80. - SWATCH_SIZE - 2. * (SWATCH_GAP + SWATCH_RING)))
            .children(Accent::ALL.map(|accent| {
                let selected = self.accent == accent;
                let color = accent.color();
                div()
                    .id(SharedString::from(format!("accent-{}", accent.key())))
                    .p(px(SWATCH_GAP))
                    .rounded_full()
                    .border(px(SWATCH_RING))
                    .border_color(if selected {
                        blend(color, p.background, 0.3)
                    } else {
                        gpui::transparent_black()
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.accent = accent;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .size(px(SWATCH_SIZE))
                            .rounded_full()
                            .bg(if selected {
                                color
                            } else {
                                blend(color, p.background, 0.5)
                            }),
                    )
            }))
    }
}

// ---------------------------------------------------------------------------
// Step 3 — Keyboard shortcuts
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Key {
    Text(&'static str),
    Icon(&'static str),
}

/// A shortcut: alternatives are separated by "/", keys within one
/// alternative are pressed together (e.g. SHIFT + S).
struct Shortcut {
    label: &'static str,
    keys: &'static [&'static [Key]],
}

const LEFT_SHORTCUTS: &[Shortcut] = &[
    Shortcut { label: "Play / Pause", keys: &[&[Key::Text("Space")]] },
    Shortcut {
        label: "Seek Backward / Forward",
        keys: &[&[Key::Icon(icons::ARROW_LEFT)], &[Key::Icon(icons::ARROW_RIGHT)]],
    },
    Shortcut { label: "Previous / Next Item", keys: &[&[Key::Text("<")], &[Key::Text(">")]] },
    Shortcut { label: "Previous / Next Frame", keys: &[&[Key::Text(",")], &[Key::Text(".")]] },
    Shortcut { label: "Cycle Subtitle Track", keys: &[&[Key::Text("V")]] },
    Shortcut { label: "Cycle Audio Track", keys: &[&[Key::Text("B")]] },
];

const RIGHT_SHORTCUTS: &[Shortcut] = &[
    Shortcut { label: "Toggle Fullscreen", keys: &[&[Key::Text("F")]] },
    Shortcut {
        label: "Increase / Decrease Volume",
        keys: &[&[Key::Icon(icons::ARROW_UP)], &[Key::Icon(icons::ARROW_DOWN)]],
    },
    Shortcut { label: "Show / Hide Queue", keys: &[&[Key::Text("Q")]] },
    Shortcut { label: "Show / Hide File Browser", keys: &[&[Key::Text("L")]] },
    Shortcut { label: "Take a Snapshot", keys: &[&[Key::Text("SHIFT"), Key::Text("S")]] },
    Shortcut { label: "Record", keys: &[&[Key::Text("SHIFT"), Key::Text("R")]] },
];

impl Onboarding {
    fn render_shortcuts(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let column = |shortcuts: &'static [Shortcut]| {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(37.))
                .children(shortcuts.iter().map(|s| shortcut_row(s, p)))
        };

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .mt(px(64.))
                    .w_full()
                    .text_center()
                    .text_size(px(26.))
                    .line_height(px(34.))
                    .text_color(p.text)
                    .child("Learn the basic keyboard shortcuts."),
            )
            .child(
                div()
                    .mt(px(55.))
                    .px(px(62.))
                    .flex()
                    .flex_row()
                    .child(column(LEFT_SHORTCUTS))
                    .child(column(RIGHT_SHORTCUTS)),
            )
            .child(
                bottom_bar()
                    .left(px(62.))
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(16.))
                            .text_color(p.text_muted)
                            .child("Find the full list in the menu."),
                    )
                    .child(self.nav_buttons(p, true, "Finish", cx)),
            )
            .into_any_element()
    }
}

fn shortcut_row(shortcut: &Shortcut, p: &Palette) -> impl IntoElement {
    let mut keys = div().flex().flex_row().items_center().gap(px(10.));
    for (i, combo) in shortcut.keys.iter().enumerate() {
        if i > 0 {
            keys = keys.child(div().text_size(px(16.)).text_color(p.text_muted).child("/"));
        }
        keys = keys.child(
            div()
                .flex()
                .flex_row()
                .gap(px(6.))
                .children(combo.iter().map(|key| keycap(*key, p))),
        );
    }

    div()
        .h(px(45.))
        .flex()
        .flex_row()
        .items_center()
        .child(
            div()
                .w(px(240.))
                .text_size(px(16.))
                .text_color(p.text_muted)
                .child(shortcut.label),
        )
        .child(keys)
}

fn keycap(key: Key, p: &Palette) -> impl IntoElement {
    let face = div()
        .h(px(39.))
        .min_w(px(46.))
        .px(px(20.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(8.))
        .bg(p.key_face)
        .border_2()
        .border_color(p.key_border)
        .text_size(px(16.))
        .text_color(p.text_strong);

    let face = match key {
        Key::Text(text) => face.child(text),
        Key::Icon(path) => face.child(svg().path(path).size(px(20.)).text_color(p.text_strong)),
    };

    // The lip under the key cap gives it the raised look from the design.
    div()
        .pb(px(5.))
        .rounded(px(9.))
        .bg(p.key_edge)
        .child(face)
}
