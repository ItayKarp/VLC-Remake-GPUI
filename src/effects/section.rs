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

/// The Video tab's Essential filters (`panels/video/essential`): the picture's adjustments, then
/// sharpening, banding removal and film grain (S7.3).
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
        "video.picture.sharpen",
        "video.picture.deband_radius",
        "video.picture.grain",
    ],
};

/// The Video tab's Colors (`panels/video/colors`): the color filters (S7.4), each with its own
/// switch under the section's, Sepia's off at 0.
pub const VIDEO_COLORS: EffectSection = EffectSection {
    id: "video.colors",
    title: "Colors",
    toggle: "video.colors.enabled",
    controls: &[
        "video.colors.extract",
        "video.colors.extract_color",
        "video.colors.threshold",
        "video.colors.threshold_color",
        "video.colors.threshold_saturation",
        "video.colors.threshold_similarity",
        "video.colors.negate",
        "video.colors.posterize",
        "video.colors.posterize_levels",
        "video.colors.gradient",
        "video.colors.gradient_mode",
        "video.colors.gradient_color",
        "video.colors.cartoon",
        "video.colors.sepia",
    ],
};

/// The Video tab's Crop (`panels/video/crop`): pixels trimmed off each edge (S7.2.5).
pub const VIDEO_CROP: EffectSection = EffectSection {
    id: "video.crop",
    title: "Crop",
    toggle: "video.crop_edges.enabled",
    controls: &[
        "video.crop_edges.top",
        "video.crop_edges.bottom",
        "video.crop_edges.left",
        "video.crop_edges.right",
    ],
};

/// The Video tab's Defaults rows (S7.2, `controls-video_settings.png`), each a setting the panel
/// holds for the session like a section's controls, but with no toggle of their own.
pub const VIDEO_DEFAULTS: &[&str] = &[
    "video.flip",
    "video.rotation",
    "video.zoom",
    "player.video_quality",
    "video.aspect_ratio",
    "video.custom_aspect_ratio",
    "video.crop",
];

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
        assert_eq!(VIDEO_ESSENTIAL.keys().count(), 9);
    }

    #[test]
    fn the_crop_section_and_the_defaults_are_settings() {
        for key in VIDEO_CROP.controls {
            let def = registry::find(key).expect("each edge is a setting");
            assert!(matches!(def.control, Control::Number { .. }), "{key}");
        }
        for key in VIDEO_DEFAULTS.iter().chain(VIDEO_COLORS.controls) {
            assert!(registry::find(key).is_some(), "{key}");
        }
    }
}
