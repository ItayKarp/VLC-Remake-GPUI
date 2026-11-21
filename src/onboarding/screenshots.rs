//! Screenshot tests for onboarding against `docs/onboarding_scenes`: step 1, step 2 in
//! dark with each of the nine accents (scenes 2.0 to 2.8), and step 3.

use gpui::AppContext;

use crate::screenshot_test::{self, Shot};
use crate::theme::{Accent, ThemeMode};
use crate::window_chrome::TITLE_BAR_HEIGHT;

use super::{DevOverrides, Onboarding, WINDOW_HEIGHT, WINDOW_WIDTH};

/// Where the window's content starts in every onboarding scene (past the drop shadow).
const SCENE_ORIGIN: (u32, u32) = (24, 16);

/// The share of pixels allowed to differ. Every scene measured 1.7% to 2.1% on macOS
/// (November 2026): text sits a few pixels off throughout, and step 2 has a Back button
/// and larger theme tiles than its design. Lower it as the screens get closer.
const MAX_MISMATCH: f64 = 0.025;

/// One onboarding scene: its file and the state that shows it.
struct Scene {
    file: &'static str,
    step: u8,
    accent: Accent,
}

/// Every scene.
fn scenes() -> Vec<Scene> {
    let mut scenes = vec![
        Scene { file: "onboarding-1", step: 1, accent: Accent::Orange },
        Scene { file: "onboarding-3", step: 3, accent: Accent::Orange },
    ];
    scenes.extend(Accent::ALL.into_iter().zip(STEP_2_FILES).map(|(accent, file)| Scene {
        file,
        step: 2,
        accent,
    }));
    scenes
}

/// Scene 2.N shows `Accent::ALL[N]`.
const STEP_2_FILES: [&str; 9] = [
    "onboarding-2.0",
    "onboarding-2.1",
    "onboarding-2.2",
    "onboarding-2.3",
    "onboarding-2.4",
    "onboarding-2.5",
    "onboarding-2.6",
    "onboarding-2.7",
    "onboarding-2.8",
];

#[test]
fn onboarding_matches_the_design_scenes() {
    let failures: Vec<String> = scenes().iter().filter_map(check_scene).collect();
    assert!(failures.is_empty(), "screens differ from the design:\n{}", failures.join("\n"));
}

/// Renders `scene` and returns a failure message if it differs too much.
fn check_scene(scene: &Scene) -> Option<String> {
    let shot = Shot {
        name: scene.file,
        design: format!("docs/onboarding_scenes/{}.png", scene.file),
        design_origin: SCENE_ORIGIN,
        window: (WINDOW_WIDTH, WINDOW_HEIGHT),
        skip_top: TITLE_BAR_HEIGHT,
        max_mismatch: MAX_MISMATCH,
    };
    let overrides = DevOverrides {
        step: Some(scene.step),
        theme: Some(ThemeMode::Dark),
        accent: Some(scene.accent),
    };
    screenshot_test::check(&shot, |window, cx| {
        cx.new(|cx| {
            let mut onboarding = Onboarding::with_overrides(overrides, window, cx);
            // The scenes show `~/Videos`, whatever this machine's video folder is.
            onboarding.media_folder = dirs::home_dir().unwrap_or_default().join("Videos");
            onboarding
        })
    })
}
