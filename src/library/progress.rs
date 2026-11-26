//! How far a scan has got, as "Indexing 1,234 of ~5,000" (S4.2.1).
//!
//! The total isn't known until the walk ends, so it is estimated: the files the last scan found,
//! or, on a first scan, the files found so far scaled by how many of the folders found so far
//! were listed. It never goes below what was found.

/// A scan's progress.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// Media files found so far.
    pub found: usize,
    /// About how many there are in all.
    pub estimate: usize,
}

/// What the estimate is made from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sample {
    /// Media files found so far.
    pub found: usize,
    /// Folders listed so far.
    pub listed: usize,
    /// Folders found so far (listed or waiting).
    pub folders: usize,
    /// Files the library had below the scanned folder before this scan.
    pub before: usize,
}

impl Progress {
    /// The progress `sample` shows.
    pub fn estimate(sample: Sample) -> Self {
        let scaled = sample
            .found
            .saturating_mul(sample.folders)
            .checked_div(sample.listed)
            .unwrap_or(sample.found);
        let estimate = if sample.before > 0 { sample.before } else { scaled };
        Self { found: sample.found, estimate: estimate.max(sample.found) }
    }

    /// "Indexing 1,234 of ~5,000".
    pub fn label(self) -> String {
        tr!(
            "Indexing {found} of ~{estimate}",
            found = crate::i18n::number(self.found),
            estimate = crate::i18n::number(self.estimate)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rescan_estimates_from_the_last_count() {
        let sample = Sample { found: 1_234, listed: 10, folders: 100, before: 5_000 };
        assert_eq!(Progress::estimate(sample).label(), "Indexing 1,234 of ~5,000");
    }

    #[test]
    fn a_first_scan_scales_by_the_folders_left() {
        let sample = Sample { found: 100, listed: 10, folders: 40, before: 0 };
        assert_eq!(Progress::estimate(sample), Progress { found: 100, estimate: 400 });
        let start = Sample { found: 3, listed: 0, folders: 1, before: 0 };
        assert_eq!(Progress::estimate(start).estimate, 3);
    }

    #[test]
    fn the_estimate_never_falls_below_what_was_found() {
        let sample = Sample { found: 6_000, listed: 10, folders: 10, before: 5_000 };
        assert_eq!(Progress::estimate(sample).estimate, 6_000);
    }
}
