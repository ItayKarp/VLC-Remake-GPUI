//! The Settings window's 13 categories and the three sidebar groups they sit in
//! (`docs/ui/settings/screens/*`). The sidebar label and the page title differ.

use serde::{Deserialize, Serialize};

/// A sidebar group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    /// General, Interface, Privacy, Controls.
    App,
    /// Playback, Video, Audio, Subtitles, Library.
    Media,
    /// Network, Performance, Recording, Extensions.
    System,
}

impl Group {
    /// The group's heading in the sidebar.
    pub fn label(self) -> &'static str {
        match self {
            Self::App => "App",
            Self::Media => "Media",
            Self::System => "System",
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
    /// "Add-ons & Scripts".
    Extensions,
}

impl Category {
    /// Every category, in sidebar order.
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
            Self::General => "General",
            Self::Interface => "Interface",
            Self::Privacy => "Privacy",
            Self::Controls => "Controls",
            Self::Playback => "Playback",
            Self::Video => "Video",
            Self::Audio => "Audio",
            Self::Subtitles => "Subtitles",
            Self::Library => "Library",
            Self::Network => "Network",
            Self::Performance => "Performance",
            Self::Recording => "Recording",
            Self::Extensions => "Extensions",
        }
    }

    /// The title over the category's page.
    pub fn title(self) -> &'static str {
        match self {
            Self::General => "Global Settings",
            Self::Interface => "Interface Settings",
            Self::Privacy => "Privacy Settings",
            Self::Controls => "Mouse, Keyboard & Remote Controls",
            Self::Playback => "Playback Preferences",
            Self::Video => "Video Defaults",
            Self::Audio => "Audio Defaults",
            Self::Subtitles => "Subtitle Defaults",
            Self::Library => "Playlists, Folders & Files",
            Self::Network => "Network Connections & Security",
            Self::Performance => "Performance Settings",
            Self::Recording => "Recording, Casting & Streaming Out",
            Self::Extensions => "Add-ons & Scripts",
        }
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
