//! How often something was used, fading with time (S4.9 edge case): Home's "Frequently …"
//! sections rank by a score that halves every [`HALF_LIFE_MS`], so a folder visited 100 times
//! last year doesn't stay above one visited every day this week.
//!
//! Each use adds 1 to the score as it stands at that moment. A score is stored with the time it
//! was last raised, and [`decayed`] brings it to the present.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::hash::Hash;

/// How long a use takes to count half as much: 30 days, in milliseconds.
pub const HALF_LIFE_MS: f64 = 30. * 24. * 60. * 60. * 1000.;

/// What `score`, last raised `elapsed_ms` ago, is worth now. Time that runs backwards (a clock
/// set back) doesn't raise it.
pub fn decayed(score: f64, elapsed_ms: i64) -> f64 {
    let elapsed = elapsed_ms.max(0) as f64;
    score * (-elapsed / HALF_LIFE_MS).exp2()
}

/// The score after one more use at `now_ms`, from `previous`: a score and when it was raised.
pub fn raised(previous: Option<(f64, i64)>, now_ms: i64) -> f64 {
    previous.map_or(0., |(score, at)| decayed(score, now_ms - at)) + 1.
}

/// Something with a score, and when it was last used.
#[derive(Clone, Debug, PartialEq)]
pub struct Scored<T> {
    /// What is scored.
    pub item: T,
    /// Its score now.
    pub score: f64,
    /// When it was last used, in milliseconds since the epoch.
    pub last_ms: i64,
}

/// The `limit` best of `scored`: the highest score first, the most recently used first among
/// equals.
pub fn top<T>(mut scored: Vec<Scored<T>>, limit: usize) -> Vec<T> {
    scored.sort_by(|a, b| {
        let by_score = b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal);
        by_score.then(b.last_ms.cmp(&a.last_ms))
    });
    scored.into_iter().take(limit).map(|scored| scored.item).collect()
}

/// The scores at `now_ms` of the items `uses` names, one use each at its time: every use adds
/// its own decayed 1.
pub fn sum_uses<T: Eq + Hash + Clone>(uses: Vec<(T, i64)>, now_ms: i64) -> Vec<Scored<T>> {
    let mut totals: HashMap<T, Scored<T>> = HashMap::new();
    for (item, at) in uses {
        let weight = decayed(1., now_ms - at);
        let entry = totals.entry(item.clone()).or_insert(Scored { item, score: 0., last_ms: at });
        entry.score += weight;
        entry.last_ms = entry.last_ms.max(at);
    }
    totals.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: i64 = 24 * 60 * 60 * 1000;

    #[test]
    fn a_score_halves_every_half_life() {
        assert!((decayed(8., 30 * DAY_MS) - 4.).abs() < 1e-9);
        assert!((decayed(8., 60 * DAY_MS) - 2.).abs() < 1e-9);
        assert_eq!(decayed(8., -DAY_MS), 8.);
    }

    #[test]
    fn a_use_adds_one_to_the_faded_score() {
        assert_eq!(raised(None, 5), 1.);
        assert!((raised(Some((4., 0)), 30 * DAY_MS) - 3.).abs() < 1e-9);
    }

    #[test]
    fn last_years_habit_falls_below_this_weeks() {
        let now = 400 * DAY_MS;
        let old = Scored { item: "old", score: decayed(100., 365 * DAY_MS), last_ms: 35 * DAY_MS };
        let mut week = None;
        for day in (0..7).rev() {
            week = Some((raised(week, now - day * DAY_MS), now - day * DAY_MS));
        }
        let (score, at) = week.expect("visited");
        let new = Scored { item: "new", score: decayed(score, now - at), last_ms: at };
        assert_eq!(top(vec![old, new], 2), ["new", "old"]);
    }

    #[test]
    fn equal_scores_put_the_latest_first() {
        let a = Scored { item: 'a', score: 1., last_ms: 1 };
        let b = Scored { item: 'b', score: 1., last_ms: 2 };
        assert_eq!(top(vec![a, b], 1), ['b']);
    }

    #[test]
    fn uses_add_up_per_item() {
        let scored = sum_uses(vec![("x", 10), ("y", 10), ("x", 10)], 10);
        assert_eq!(top(scored, 2), ["x", "y"]);
    }
}
