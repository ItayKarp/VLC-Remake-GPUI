//! Effect sections (S7.1.2, S7.1.3, D23): a titled group of the panel's controls with an enable
//! toggle, which folds away under "Expand Settings". Each control is a setting in the registry,
//! so its label, range and format are the Settings page's.

/// One effect section of the player's settings panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectSection {
    /// A stable id, under which its expanded state is remembered.
    pub id: &'static str,
    /// The title over it ("Essential").
    pub title: &'static str,
    /// The key of the toggle that turns the effect on and off.
    pub toggle: &'static str,
    /// The keys of its controls, in order.
    pub controls: &'static [&'static str],
}

impl EffectSection {
    /// Its toggle's key, then its controls'.
    pub fn keys(&self) -> impl Iterator<Item = &'static str> + use<> {
        std::iter::once(self.toggle).chain(self.controls.iter().copied())
    }
}

/// The Video tab's Essential filters (`panels/video/essential`): the picture's adjustments.
/// Sharpen, banding removal and film grain join them with S7.3.
pub const VIDEO_ESSENTIAL: EffectSection = EffectSection {
    id: "video.essential",
    title: "Essential",
    toggle: "video.picture.enabled",
    controls: &[
        "video.picture.hue",
        "video.picture.brightness",
        "video.picture.contrast",
        "video.picture.saturation",
        "video.picture.gamma",
    ],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::registry::{self, Control};

    #[test]
    fn every_key_is_a_setting_and_controls_are_sliders() {
        let toggle = registry::find(VIDEO_ESSENTIAL.toggle).expect("the toggle is a setting");
        assert!(matches!(toggle.control, Control::Toggle));
        for key in VIDEO_ESSENTIAL.controls {
            let def = registry::find(key).expect("each control is a setting");
            assert!(matches!(def.control, Control::Slider { .. }), "{key}");
        }
        assert_eq!(VIDEO_ESSENTIAL.keys().count(), 6);
    }
}
