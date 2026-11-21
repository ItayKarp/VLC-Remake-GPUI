//! Finishing onboarding: saving the choices, then handing off to the main window
//! (S0.2.2, S0.2.3).

use gpui::{Context, SharedString, Window};

use super::Onboarding;
use crate::app;
use crate::settings::{self, Settings};

/// Progress of the Finish button (S0.2.3).
#[derive(Clone, PartialEq, Eq)]
pub(super) enum FinishState {
    /// Nothing is being saved.
    Idle,
    /// The save is in flight; Back and Finish are disabled.
    Saving,
    /// Saving failed: stay on step 3 and offer Retry / Continue without saving.
    Failed(SharedString),
}

impl Onboarding {
    fn apply_choices(&self, settings: &mut Settings, complete: bool) {
        settings.prefs.theme = self.theme;
        settings.prefs.accent = self.accent;
        settings.local.media_folders = vec![self.media_folder.clone()];
        settings.local.onboarding_complete = complete;
    }

    /// Saves, then hands off to the main window. Also used by Retry.
    pub(super) fn finish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.finish == FinishState::Saving {
            return;
        }
        self.finish = FinishState::Saving;
        cx.notify();

        let before = settings::get(cx).clone();
        settings::update(cx, |s| self.apply_choices(s, true));
        let save = settings::save(cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = save.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(()) => this.hand_off(window, cx),
                Err(error) => this.save_failed(before, error.into(), cx),
            })
            .ok();
        })
        .detach();
    }

    /// Records a failed save and restores the settings from before Finish.
    fn save_failed(&mut self, before: Settings, error: SharedString, cx: &mut Context<Self>) {
        tracing::warn!("onboarding: couldn't save settings: {error}");
        // Undo, so quitting from here (which flushes settings) doesn't record
        // onboarding as done.
        settings::update(cx, |s| *s = before);
        self.finish = FinishState::Failed(error);
        cx.notify();
    }

    /// "Continue without saving": use the choices for this session only. Onboarding
    /// stays incomplete, so it runs again next launch.
    pub(super) fn continue_without_saving(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        settings::update(cx, |s| self.apply_choices(s, false));
        self.hand_off(window, cx);
    }

    /// Opens the main window, then closes onboarding. The other order would quit the
    /// app, since closing the last window quits.
    fn hand_off(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let display = window.display(cx).map(|d| d.id());
        match app::open_main_window(display, app::open::take_pending(cx), cx) {
            Ok(_) => window.remove_window(),
            Err(err) => {
                tracing::error!("onboarding: couldn't open the main window: {err:#}");
                self.finish = FinishState::Failed("Couldn't open the main window.".into());
                cx.notify();
            }
        }
    }

    /// The save error to show, if the last Finish failed.
    pub(super) fn save_error(&self) -> Option<SharedString> {
        match &self.finish {
            FinishState::Failed(error) => Some(error.clone()),
            _ => None,
        }
    }
}
