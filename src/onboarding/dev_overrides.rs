//! Dev overrides that open onboarding in a given state, for design checks: the
//! `RVLC_STEP`, `RVLC_THEME` and `RVLC_ACCENT` variables in debug builds, and the
//! screenshot tests, which set the same fields directly.

use crate::theme::{Accent, ThemeMode};

use super::{Onboarding, Step};

/// An onboarding state to start in instead of the saved choices and step 1.
#[derive(Clone, Copy, Default)]
pub struct DevOverrides {
    /// The step to open, `1..=3` like the design scenes.
    pub step: Option<u8>,
    /// The theme to preview.
    pub theme: Option<ThemeMode>,
    /// The accent to preview.
    pub accent: Option<Accent>,
}

impl DevOverrides {
    /// Reads `RVLC_STEP=1..3`, `RVLC_THEME=light|dark|system` and `RVLC_ACCENT=<name>`.
    /// Release builds ignore them.
    pub fn from_env() -> Self {
        if !cfg!(debug_assertions) {
            return Self::default();
        }
        let var = |name| std::env::var(name).ok();
        Self {
            step: var("RVLC_STEP").map(|step| step.parse().unwrap_or(1)),
            theme: var("RVLC_THEME").map(|theme| {
                ThemeMode::ALL.into_iter().find(|m| m.key() == theme).unwrap_or_default()
            }),
            accent: var("RVLC_ACCENT").map(|accent| {
                Accent::ALL.into_iter().find(|a| a.key() == accent).unwrap_or_default()
            }),
        }
    }

    /// Puts `onboarding` into the overridden state.
    pub(super) fn apply(self, onboarding: &mut Onboarding) {
        if let Some(step) = self.step {
            onboarding.step = match step {
                2 => Step::Appearance,
                3 => Step::Shortcuts,
                _ => Step::Welcome,
            };
        }
        if let Some(theme) = self.theme {
            onboarding.theme = theme;
        }
        if let Some(accent) = self.accent {
            onboarding.accent = accent;
        }
    }
}
