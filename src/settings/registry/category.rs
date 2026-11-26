//! The Settings window's 13 categories and the three sidebar groups they sit in
//! (`docs/ui/settings/screens/*`). The sidebar label and the page title differ. Extensions is
//! only listed in builds with the `extensions` feature, until an extension system exists
//! (S8.14, D19, `docs/adr/0003-extensions-and-skins.md`).

use serde::{Deserialize, Serialize};

/// A sidebar group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    /// General, Interface, Privacy, Controls.
    App,
    /// Playback, Video, Audio, Subtitles, Library.
    Media,
    /// Network, Performance, Recording, and Extensions with the `extensions` feature.
    System,
}

impl Group {
    /// The group's heading in the sidebar.
    pub fn label(self) -> &'static str {
        match self {
            Self::App => tr!("App"),
            Self::Media => tr!("Media"),
            Self::System => tr!("System"),
        }
    }
}

/// How a category's page is drawn.
#[allow(dead_code, reason = "the Settings window (S5.2) picks hand-built pages")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Built from the registry's rows.
    Rows,
    /// Hand-built, because the design needs more than rows (the extensions list).
    HandBuilt,
}

/// One page of the Settings window. Saved by name as the page Settings last showed (S5.2.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    /// "Global Settings".
    #[default]
    General,
    /// "Interface Settings".
    Interface,
    /// "Privacy Settings".
    Privacy,
    /// "Mouse, Keyboard & Remote Controls".
    Controls,
    /// "Playback Preferences".
    Playback,
    /// "Video Defaults".
    Video,
    /// "Audio Defaults".
    Audio,
    /// "Subtitle Defaults".
    Subtitles,
    /// "Playlists, Folders & Files".
    Library,
    /// "Network Connections & Security".
    Network,
    /// "Performance Settings".
    Performance,
    /// "Recording, Casting & Streaming Out".
    Recording,
    /// "Add-ons & Scripts". Kept without the `extensions` feature so a saved page still reads.
    Extensions,
}

impl Category {
    /// Every category the Settings window lists, in sidebar order.
    #[cfg(feature = "extensions")]
    pub const ALL: [Self; 13] = [
        Self::General,
        Self::Interface,
        Self::Privacy,
        Self::Controls,
        Self::Playback,
        Self::Video,
        Self::Audio,
        Self::Subtitles,
        Self::Library,
        Self::Network,
        Self::Performance,
        Self::Recording,
        Self::Extensions,
    ];

    /// Every category the Settings window lists, in sidebar order: Extensions waits for an
    /// extension system (D19).
    #[cfg(not(feature = "extensions"))]
    pub const ALL: [Self; 12] = [
        Self::General,
        Self::Interface,
        Self::Privacy,
        Self::Controls,
        Self::Playback,
        Self::Video,
        Self::Audio,
        Self::Subtitles,
        Self::Library,
        Self::Network,
        Self::Performance,
        Self::Recording,
    ];

    /// Whether the Settings window lists the page.
    pub fn is_listed(self) -> bool {
        Self::ALL.contains(&self)
    }

    /// The sidebar group the category is listed under.
    pub fn group(self) -> Group {
        match self {
            Self::General | Self::Interface | Self::Privacy | Self::Controls => Group::App,
            Self::Playback | Self::Video | Self::Audio | Self::Subtitles | Self::Library => {
                Group::Media
            }
            Self::Network | Self::Performance | Self::Recording | Self::Extensions => Group::System,
        }
    }

    /// The category's name in the sidebar.
    pub fn label(self) -> &'static str {
        match self {
            Self::General => tr!("General"),
            Self::Interface => tr!("Interface"),
            Self::Privacy => tr!("Privacy"),
            Self::Controls => tr!("Controls"),
            Self::Playback => tr!("Playback"),
            Self::Video => tr!("Video"),
            Self::Audio => tr!("Audio"),
            Self::Subtitles => tr!("Subtitles"),
            Self::Library => tr!("Library"),
            Self::Network => tr!("Network"),
            Self::Performance => tr!("Performance"),
            Self::Recording => tr!("Recording"),
            Self::Extensions => tr!("Extensions"),
        }
    }

    /// The title over the category's page.
    pub fn title(self) -> &'static str {
        match self {
            Self::General => tr!("Global Settings"),
            Self::Interface => tr!("Interface Settings"),
            Self::Privacy => tr!("Privacy Settings"),
            Self::Controls => tr!("Mouse, Keyboard & Remote Controls"),
            Self::Playback => tr!("Playback Preferences"),
            Self::Video => tr!("Video Defaults"),
            Self::Audio => tr!("Audio Defaults"),
            Self::Subtitles => tr!("Subtitle Defaults"),
            Self::Library => tr!("Playlists, Folders & Files"),
            Self::Network => tr!("Network Connections & Security"),
            Self::Performance => tr!("Performance Settings"),
            Self::Recording => tr!("Recording, Casting & Streaming Out"),
            Self::Extensions => tr!("Add-ons & Scripts"),
        }
    }

    /// Whether setting presets cover the page (S5.11), so it ends with the preset picker.
    pub fn has_presets(self) -> bool {
        matches!(self, Self::Playback | Self::Video | Self::Audio | Self::Subtitles)
    }

    /// How the page is drawn.
    #[allow(dead_code, reason = "the Settings window (S5.2) picks hand-built pages")]
    pub fn layout(self) -> Layout {
        match self {
            Self::Extensions => Layout::HandBuilt,
            _ => Layout::Rows,
        }
    }
}
