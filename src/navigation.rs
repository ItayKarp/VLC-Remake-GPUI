//! The library's navigation model (S4.5): where it is ([`route`]), where it was and can go back
//! to ([`history`]), the breadcrumb ([`breadcrumb`]), how the rail draws its icons ([`rail`]),
//! the "N items" count, the last route of each presentation, kept across launches
//! ([`stored`]), and whether each section shows a list or a grid ([`view`]).

pub mod breadcrumb;
pub mod history;
pub mod rail;
pub mod route;
pub mod stored;
pub mod view;

use crate::library::progress::group_thousands;
use crate::settings::LibraryRoutes;
use route::Route;

/// How the library is shown. Each remembers its own last route (S4.5.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presentation {
    /// The rail and panel over the player (`player_02`).
    Overlay,
    /// Full library mode (`player_03`–`07`, S4.10.2).
    #[allow(dead_code, reason = "full library mode arrives with S4.10.2")]
    Full,
}

impl Presentation {
    /// The route `presentation` was last at, or Home.
    pub fn last_route(self, routes: &LibraryRoutes) -> Route {
        stored::decode(self.slot(routes)).unwrap_or(Route::Home)
    }

    /// Remembers `route` as this presentation's last.
    pub fn remember(self, route: &Route, routes: &mut LibraryRoutes) {
        let slot = match self {
            Self::Overlay => &mut routes.overlay,
            Self::Full => &mut routes.full,
        };
        *slot = stored::encode(route);
    }

    fn slot(self, routes: &LibraryRoutes) -> &str {
        match self {
            Self::Overlay => &routes.overlay,
            Self::Full => &routes.full,
        }
    }
}

/// The count under the breadcrumb (S4.5.5): "1 item" or "N items". Action rows ("+ Add") are
/// not items, so callers leave them out of `count`.
pub fn item_count(count: usize) -> String {
    match count {
        1 => "1 item".to_owned(),
        _ => format!("{} items", group_thousands(count)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_item_is_singular() {
        assert_eq!(item_count(0), "0 items");
        assert_eq!(item_count(1), "1 item");
        assert_eq!(item_count(110), "110 items");
        assert_eq!(item_count(1234), "1,234 items");
    }

    #[test]
    fn each_presentation_keeps_its_own_route() {
        let mut routes = LibraryRoutes::default();
        Presentation::Overlay.remember(&Route::History, &mut routes);
        Presentation::Full.remember(&Route::Videos(None), &mut routes);
        assert_eq!(Presentation::Overlay.last_route(&routes), Route::History);
        assert_eq!(Presentation::Full.last_route(&routes), Route::Videos(None));
    }

    #[test]
    fn nothing_saved_opens_home() {
        let routes = LibraryRoutes { overlay: "nonsense".into(), full: String::new() };
        assert_eq!(Presentation::Overlay.last_route(&routes), Route::Home);
        assert_eq!(Presentation::Full.last_route(&routes), Route::Home);
    }
}
