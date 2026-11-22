//! First-run onboarding, following `docs/onboarding_scenes`:
//!
//! 1. Welcome + choose a media folder
//! 2. Theme (light / dark / system) and accent color, with a live player preview
//! 3. Basic keyboard shortcuts
//!
//! Finish saves, then hands off to the main window (S0.2.2). Until then the choices
//! live only in this view, so closing early saves nothing and the next launch starts
//! again at step 1 (S0.2.4).
//!
//! Run again once setup was done ("Run setup again", S0.2.5, S5.3.5), it starts from the current
//! choices and offers **Cancel**: Cancel, Escape on step 1 and closing the window return to the
//! main window with nothing changed.

mod appearance;
mod dev_overrides;
mod finish;
mod layout;
mod preview;
mod save_error;
#[cfg(test)]
mod screenshots;
mod shortcuts;
mod welcome;

use std::path::PathBuf;

use gpui::{App, Context, ExternalPaths, FocusHandle, KeyDownEvent, Window, div, prelude::*, px};

use crate::app;
use crate::app::media::Media;
use crate::app::request::OpenRequest;
use crate::settings;
use crate::theme::{self, Accent, Palette, Theme, ThemeMode};
use crate::ui::{drop_target, focus};
use crate::window_chrome::{self, TITLE_BAR_HEIGHT};

use dev_overrides::DevOverrides;
use finish::FinishState;

/// Window width: the design's 1088-pixel content area.
pub const WINDOW_WIDTH: f32 = 1088.;
/// Window height: the design's 760-pixel content area plus our title strip.
pub const WINDOW_HEIGHT: f32 = 760. + TITLE_BAR_HEIGHT;

/// The onboarding step on screen.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Welcome,
    Appearance,
    Shortcuts,
}

/// The onboarding window's root view; it holds the choices until Finish saves them.
pub struct Onboarding {
    step: Step,
    media_folder: PathBuf,
    theme: ThemeMode,
    accent: Accent,
    finish: FinishState,
    /// Whether setup was done before, so Cancel returns to the main window.
    rerun: bool,
    focus_handle: FocusHandle,
}

impl Onboarding {
    /// Creates the view at step 1, starting from the saved choices, and focuses it.
    /// Debug builds start in the state the `RVLC_*` variables ask for.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_overrides(DevOverrides::from_env(), window, cx)
    }

    /// Like [`Onboarding::new`], but starts in the state `overrides` gives.
    fn with_overrides(
        overrides: DevOverrides,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        crate::diagnostics::system::remember_gpu(window);

        // Re-render when the OS switches light/dark (for the "System" theme) or
        // turns high contrast on or off.
        theme::observe(window, cx);

        // Start from whatever is saved, so running setup again keeps earlier choices.
        let saved = settings::get(cx);
        let rerun = saved.local.onboarding_complete;
        if rerun {
            window.on_window_should_close(cx, |window, cx| {
                finish::reopen_main_window(window, cx).is_ok()
            });
        }
        let mut this = Self {
            step: Step::Welcome,
            media_folder: saved
                .local
                .media_folders
                .first()
                .cloned()
                .unwrap_or_else(settings::default_media_folder),
            theme: saved.prefs.theme,
            accent: saved.prefs.accent,
            finish: FinishState::Idle,
            rerun,
            focus_handle,
        };
        overrides.apply(&mut this);
        this
    }

    /// The live preview of the choices on screen, not the saved theme.
    fn palette(&self, window: &Window, cx: &App) -> Palette {
        cx.global::<Theme>().palette_for(self.theme, self.accent, window.appearance())
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
        if self.finish == FinishState::Saving {
            return;
        }
        self.finish = FinishState::Idle;
        self.step = match self.step {
            Step::Welcome | Step::Appearance => Step::Welcome,
            Step::Shortcuts => Step::Appearance,
        };
        cx.notify();
    }

    /// Window-wide keys. They act only when no control has focus, so Enter on a
    /// focused button clicks that button, and arrows in a group move inside it.
    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus_handle.is_focused(window) {
            return;
        }
        match event.keystroke.key.as_str() {
            "enter" if self.finish != FinishState::Saving => self.go_next(window, cx),
            "escape" if self.step == Step::Welcome && self.rerun => self.cancel(window, cx),
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
        let palette = self.palette(window, cx);

        let content = match self.step {
            Step::Welcome => self.render_welcome(&palette, cx),
            Step::Appearance => self.render_appearance(&palette, cx),
            Step::Shortcuts => self.render_shortcuts(&palette, cx),
        };

        focus::tab_navigation(div())
            .id("onboarding")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            // Files dropped here open once onboarding hands off (S1.3.2, D4).
            .group(drop_target::GROUP)
            .on_drop(|paths: &ExternalPaths, _, cx| {
                let items = paths.paths().iter().cloned().map(Media::from).collect();
                app::open::queue(OpenRequest::replace(items), cx);
            })
            .relative()
            .size_full()
            .bg(palette.background)
            .text_color(palette.text)
            .child(scroll_body(content))
            .child(window_chrome::title_bar(&palette))
            .child(drop_target::highlight(&palette))
    }
}

/// The area under the title strip. The layout is designed for a fixed size; on
/// screens too small for it the window shrinks and this body scrolls (S0.2.7).
fn scroll_body(content: impl IntoElement) -> impl IntoElement {
    div()
        .id("onboarding-body")
        .absolute()
        .top(px(TITLE_BAR_HEIGHT))
        .left_0()
        .right_0()
        .bottom_0()
        .overflow_scroll()
        .child(
            div()
                .relative()
                .size_full()
                .min_w(px(WINDOW_WIDTH))
                .min_h(px(WINDOW_HEIGHT - TITLE_BAR_HEIGHT))
                .child(content),
        )
}
