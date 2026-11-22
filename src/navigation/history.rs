//! Back and forward (S4.5.2): the routes visited before and after the current one.
//!
//! Going somewhere new forgets the forward routes, as browsers do. Routes that no longer exist
//! (a deleted playlist) are skipped when going back or forward, and don't keep the buttons
//! enabled; the caller says which routes exist.

use super::route::Route;

/// The most routes each direction remembers.
const MAX_STEPS: usize = 100;

/// The current route, with the ones behind and ahead of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavHistory {
    /// Older routes, the most recent last.
    back: Vec<Route>,
    current: Route,
    /// Routes gone back from, the nearest last.
    forward: Vec<Route>,
}

/// A direction through the history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// To the route before.
    Back,
    /// To the route after.
    Forward,
}

impl NavHistory {
    /// A history that starts at `current`, with nothing behind or ahead.
    pub fn new(current: Route) -> Self {
        Self { back: Vec::new(), current, forward: Vec::new() }
    }

    /// Where the library is.
    pub fn current(&self) -> &Route {
        &self.current
    }

    /// Goes to `route`. Returns `false`, changing nothing, when it is already there.
    pub fn go(&mut self, route: Route) -> bool {
        if route == self.current {
            return false;
        }
        let previous = std::mem::replace(&mut self.current, route);
        push(&mut self.back, previous);
        self.forward.clear();
        true
    }

    /// Puts `route` in place of the current one, leaving no step back to it: a search being
    /// typed refines itself rather than stacking every keystroke. Returns `false`, changing
    /// nothing, when it is already there.
    pub fn replace(&mut self, route: Route) -> bool {
        if route == self.current {
            return false;
        }
        self.current = route;
        self.forward.clear();
        true
    }

    /// Takes one `step`, past the routes `exists` rejects, which are forgotten. Returns `false`,
    /// changing nothing, when no route that exists lies that way.
    pub fn step(&mut self, step: Step, exists: impl Fn(&Route) -> bool) -> bool {
        if !self.can_step(step, &exists) {
            return false;
        }
        let (from, to) = match step {
            Step::Back => (&mut self.back, &mut self.forward),
            Step::Forward => (&mut self.forward, &mut self.back),
        };
        while let Some(route) = from.pop() {
            if exists(&route) {
                push(to, std::mem::replace(&mut self.current, route));
                return true;
            }
        }
        false
    }

    /// Whether a route that exists lies one `step` away (the Back and Forward buttons'
    /// enabled states).
    pub fn can_step(&self, step: Step, exists: impl Fn(&Route) -> bool) -> bool {
        let routes = match step {
            Step::Back => &self.back,
            Step::Forward => &self.forward,
        };
        routes.iter().any(exists)
    }
}

/// Adds `route` to `stack`, forgetting the oldest past [`MAX_STEPS`].
fn push(stack: &mut Vec<Route>, route: Route) {
    stack.push(route);
    if stack.len() > MAX_STEPS {
        stack.remove(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::playlists::PlaylistId;

    fn all(_: &Route) -> bool {
        true
    }

    fn visited(routes: &[Route]) -> NavHistory {
        let mut history = NavHistory::new(Route::Home);
        for route in routes {
            history.go(route.clone());
        }
        history
    }

    #[test]
    fn a_new_history_goes_nowhere() {
        let history = NavHistory::new(Route::Home);
        assert!(!history.can_step(Step::Back, all));
        assert!(!history.can_step(Step::Forward, all));
    }

    #[test]
    fn back_then_forward_returns() {
        let mut history = visited(&[Route::History]);
        assert!(history.step(Step::Back, all));
        assert_eq!(history.current(), &Route::Home);
        assert!(history.can_step(Step::Forward, all));
        assert!(history.step(Step::Forward, all));
        assert_eq!(history.current(), &Route::History);
        assert!(!history.can_step(Step::Forward, all));
    }

    #[test]
    fn going_somewhere_new_forgets_forward() {
        let mut history = visited(&[Route::History]);
        history.step(Step::Back, all);
        history.go(Route::Videos(None));
        assert!(!history.can_step(Step::Forward, all));
    }

    #[test]
    fn replacing_leaves_no_step_back_to_the_old_route() {
        let mut history = visited(&[Route::History]);
        assert!(history.replace(Route::Videos(None)));
        assert!(history.step(Step::Back, all));
        assert_eq!(history.current(), &Route::Home);
    }

    #[test]
    fn going_to_the_current_route_changes_nothing() {
        let mut history = visited(&[Route::History]);
        assert!(!history.go(Route::History));
        history.step(Step::Back, all);
        assert_eq!(history.current(), &Route::Home);
    }

    #[test]
    fn routes_that_no_longer_exist_are_skipped() {
        let deleted = Route::Playlists(Some(PlaylistId(7)));
        let mut history = visited(&[Route::History, deleted.clone(), Route::Videos(None)]);
        let exists = |route: &Route| *route != deleted;
        assert!(history.step(Step::Back, exists));
        assert_eq!(history.current(), &Route::History);
        assert!(history.step(Step::Forward, exists));
        assert_eq!(history.current(), &Route::Videos(None), "the dead route is forgotten");
    }

    #[test]
    fn only_dead_routes_behind_disable_back() {
        let deleted = Route::Playlists(Some(PlaylistId(7)));
        let mut history = NavHistory::new(deleted.clone());
        history.go(Route::Home);
        let exists = |route: &Route| *route != deleted;
        assert!(!history.can_step(Step::Back, exists));
        assert!(!history.step(Step::Back, exists));
        assert_eq!(history.current(), &Route::Home);
    }

    #[test]
    fn the_oldest_steps_are_forgotten() {
        let mut history = NavHistory::new(Route::Home);
        for index in 0..=MAX_STEPS {
            history.go(Route::Network(Some(index.to_string())));
        }
        let mut steps = 0;
        while history.step(Step::Back, all) {
            steps += 1;
        }
        assert_eq!(steps, MAX_STEPS);
    }
}
