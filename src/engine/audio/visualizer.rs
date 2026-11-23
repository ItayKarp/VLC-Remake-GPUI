//! The audio visualizer (S7.8): Disabled, Spectrometer, Scope, Spectrum or Waveform
//! (`panels/audio/visualizer`, whose "Spectometer" is a typo).
//!
//! Spectrometer, Scope and Spectrum are libVLC's own `visual` effects, drawn in the video area
//! for media without video ([`VisualEffect`]). Waveform draws nothing there: it is the waveform
//! seek bar (S7.8.3, D14), which replaces the plain bar for any local media with a known
//! length.

use serde::{Deserialize, Serialize};

/// What the visualizer shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visualizer {
    /// Nothing.
    #[default]
    Disabled,
    /// A spectrum analyser's bars around the middle, as VLC's Spectrometer.
    Spectrometer,
    /// The sound's wave, as an oscilloscope draws it.
    Scope,
    /// Frequency bars with falling peaks.
    Spectrum,
    /// The waveform seek bar: the whole media's loudness along the bar.
    Waveform,
}

impl Visualizer {
    /// Every choice, in the dropdown's order.
    pub const ALL: [Self; 5] =
        [Self::Disabled, Self::Spectrometer, Self::Scope, Self::Spectrum, Self::Waveform];

    /// The choice's name, for the user.
    pub fn label(self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::Spectrometer => "Spectrometer",
            Self::Scope => "Scope",
            Self::Spectrum => "Spectrum",
            Self::Waveform => "Waveform",
        }
    }

    /// The effect drawn in the video area; `None` for Disabled and Waveform.
    pub fn effect(self) -> Option<VisualEffect> {
        match self {
            Self::Spectrometer => Some(VisualEffect::Spectrometer),
            Self::Scope => Some(VisualEffect::Scope),
            Self::Spectrum => Some(VisualEffect::Spectrum),
            Self::Disabled | Self::Waveform => None,
        }
    }

    /// Whether the seek bar shows the media's waveform.
    pub fn shows_waveform(self) -> bool {
        self == Self::Waveform
    }
}

/// One of libVLC's `visual` effects, drawn in the video area for media without video.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualEffect {
    /// libVLC's `spectrometer`.
    Spectrometer,
    /// libVLC's `scope`.
    Scope,
    /// libVLC's `spectrum`.
    Spectrum,
}

impl VisualEffect {
    /// libVLC's name for it, as its audio output's `visual` variable takes it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Spectrometer => "spectrometer",
            Self::Scope => "scope",
            Self::Spectrum => "spectrum",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_drawn_choices_have_an_effect() {
        let drawn: Vec<_> = Visualizer::ALL.into_iter().filter_map(Visualizer::effect).collect();
        assert_eq!(
            drawn,
            [VisualEffect::Spectrometer, VisualEffect::Scope, VisualEffect::Spectrum]
        );
        assert!(Visualizer::Waveform.shows_waveform());
        assert!(!Visualizer::Spectrum.shows_waveform());
    }

    #[test]
    fn the_design_typo_is_fixed() {
        assert_eq!(Visualizer::Spectrometer.label(), "Spectrometer");
    }
}
