//! The breadcrumb (S4.5.3): the way from a route's section down to it, one clickable segment
//! per level, collapsed to `… ▸ parent ▸ current` from three levels deep.
//!
//! A folder outside every media folder (a file opened from Downloads) shows its real path
//! instead, and is marked as not in the library.

use std::path::{Path, PathBuf};

use super::route::{MusicCategory, MusicRoute, Route, Section};
use crate::app::media::Media;
use crate::db::playlists::Summary;

/// How many segments show before the rest fold into `…`.
const VISIBLE: usize = 2;

/// What the breadcrumb names besides the routes themselves.
#[derive(Clone, Copy, Debug)]
pub struct Places<'a> {
    /// The video media folders.
    pub video_roots: &'a [PathBuf],
    /// The music folders.
    pub music_roots: &'a [PathBuf],
    /// The saved playlists, for their titles.
    pub playlists: &'a [Summary],
}

/// One segment: what it says and where clicking it goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crumb {
    /// The segment's text.
    pub label: String,
    /// The route it opens.
    pub route: Route,
}

/// A route's breadcrumb.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trail {
    /// Every segment, the section first and the route itself last.
    pub crumbs: Vec<Crumb>,
    /// Whether the route is a folder outside every media folder ("Not in library").
    pub outside_library: bool,
}

impl Trail {
    /// The breadcrumb of `route`.
    pub fn of(route: &Route, places: Places) -> Self {
        if let Some(folder) = route.folder() {
            return folder_trail(route, folder, places);
        }
        let crumbs = match route {
            Route::Music(music) => music_crumbs(music),
            Route::HomeSection(home) => {
                vec![section(Section::Home), crumb(home.label(), route.clone())]
            }
            Route::Network(Some(category)) => {
                vec![section(Section::Network), crumb(category.label(), route.clone())]
            }
            Route::Playlists(Some(id)) => {
                let title = places.playlists.iter().find(|summary| summary.id == *id);
                let title = title.map_or("Playlist", |summary| summary.title.as_str());
                vec![section(Section::Playlists), crumb(title, route.clone())]
            }
            Route::Search(search) => {
                let results = crumb(&format!("Results for “{}”", search.query), route.clone());
                match search.scope {
                    Section::Home => vec![results],
                    scope => vec![section(scope), results],
                }
            }
            _ => vec![section(route.section())],
        };
        Self { crumbs, outside_library: false }
    }

    /// The segments `…` hides (its menu lists them) and the ones shown after it. Nothing is
    /// hidden up to two levels deep.
    pub fn collapsed(&self) -> (&[Crumb], &[Crumb]) {
        self.crumbs.split_at(self.crumbs.len().saturating_sub(VISIBLE))
    }
}

fn crumb(label: &str, route: Route) -> Crumb {
    Crumb { label: label.to_owned(), route }
}

fn section(section: Section) -> Crumb {
    crumb(section.label(), section.root())
}

fn music_crumbs(music: &MusicRoute) -> Vec<Crumb> {
    let mut crumbs = vec![section(Section::Music)];
    let Some(category) = music.category() else { return crumbs };
    crumbs.push(crumb(category.label(), category.route()));
    let named = match music {
        MusicRoute::Albums(name) | MusicRoute::Artists(name) | MusicRoute::Genres(name) => name,
        MusicRoute::Categories | MusicRoute::Songs(_) => &None,
    };
    if let Some(name) = named {
        crumbs.push(crumb(name, Route::Music(music.clone())));
    }
    crumbs
}

/// A folder's breadcrumb: from its section through its media folder, or its real path when no
/// media folder holds it.
fn folder_trail(route: &Route, folder: &Path, places: Places) -> Trail {
    let songs = MusicCategory::Songs;
    let (mut crumbs, roots) = if let Route::Videos(_) = route {
        (vec![section(Section::Videos)], places.video_roots)
    } else {
        (vec![section(Section::Music), crumb(songs.label(), songs.route())], places.music_roots)
    };
    let root = roots.iter().filter(|root| folder.starts_with(root)).max_by_key(|root| depth(root));
    let Some(root) = root else {
        let crumbs = folder.ancestors().collect::<Vec<_>>().into_iter().rev();
        let crumbs = crumbs.filter_map(|ancestor| folder_crumb(route, ancestor)).collect();
        return Trail { crumbs, outside_library: true };
    };
    let below = folder.strip_prefix(root).map_or(0, |rest| rest.components().count());
    let levels = folder.ancestors().take(below + 1).collect::<Vec<_>>().into_iter().rev();
    crumbs.extend(levels.filter_map(|level| folder_crumb(route, level)));
    Trail { crumbs, outside_library: false }
}

/// A segment for `folder`, named as the title bar's folder chip names it.
fn folder_crumb(route: &Route, folder: &Path) -> Option<Crumb> {
    let label = Media::File(folder.to_path_buf()).display_name();
    Some(Crumb { label, route: route.with_folder(folder.to_path_buf())? })
}

fn depth(path: &Path) -> usize {
    path.components().count()
}

#[cfg(test)]
mod tests;
