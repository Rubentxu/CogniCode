//! `SymbolCount` / `SymbolDelta` — how many symbols a file holds, and how
//! that number changed.
//!
//! A count is either **measured** — somebody counted the symbols in a file
//! and can say how many there are — or **unmeasured**, which is the absence
//! of that count. Unmeasured is *not* zero: a file whose symbols were never
//! counted does not have zero symbols. Publishing an unmeasured count as
//! zero is the same lie as publishing a guess as a measurement, and it is
//! the failure this type exists to make impossible.
//!
//! The change between two counts is measured only when both sides are. One
//! missing side makes the difference unknown, and the unknown is carried as
//! [`SymbolDelta::Unmeasured`] rather than silently collapsed into a zero.
//!
//! ```text
//! SymbolCount = Measured(usize)
//!             | Unmeasured
//!
//! SymbolDelta = Measured { added: usize, removed: usize }
//!             | Unmeasured
//! ```

use serde::{Deserialize, Serialize};

/// The number of symbols in one file, as far as it is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolCount {
    /// The symbols were counted, and this is how many there are.
    Measured(usize),
    /// Nobody counted them. This is the absence of a count, not zero.
    Unmeasured,
}

impl SymbolCount {
    /// The counted number of symbols, or `None` when they were not counted.
    pub fn measured(self) -> Option<usize> {
        match self {
            SymbolCount::Measured(count) => Some(count),
            SymbolCount::Unmeasured => None,
        }
    }

    /// Whether this count was actually measured.
    pub fn is_measured(self) -> bool {
        self.measured().is_some()
    }
}

/// The change in the number of symbols between two points in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolDelta {
    /// Both sides were counted, so the difference is a measurement.
    Measured {
        /// Symbols present afterwards that were not present before.
        added: usize,
        /// Symbols present before that are not present afterwards.
        removed: usize,
    },
    /// At least one side was not counted, so the difference is unknown.
    Unmeasured,
}

impl SymbolDelta {
    /// The measured number of symbols added.
    ///
    /// Zero when the change is unknown. A caller that sums deltas must
    /// also report how many of them were unknown; the sum alone does not
    /// say whether it describes the whole change.
    pub fn added(self) -> usize {
        match self {
            SymbolDelta::Measured { added, .. } => added,
            SymbolDelta::Unmeasured => 0,
        }
    }

    /// The measured number of symbols removed. See [`SymbolDelta::added`].
    pub fn removed(self) -> usize {
        match self {
            SymbolDelta::Measured { removed, .. } => removed,
            SymbolDelta::Unmeasured => 0,
        }
    }

    /// Whether the change was actually measured.
    pub fn is_measured(self) -> bool {
        matches!(self, SymbolDelta::Measured { .. })
    }
}

/// The change between two symbol counts.
///
/// Both sides are required to be measured for the result to be measured: the
/// difference between a number and an absence of a number is not a number.
pub fn symbol_delta(previous: SymbolCount, current: SymbolCount) -> SymbolDelta {
    match (previous.measured(), current.measured()) {
        (Some(before), Some(after)) => SymbolDelta::Measured {
            added: after.saturating_sub(before),
            removed: before.saturating_sub(after),
        },
        _ => SymbolDelta::Unmeasured,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The partitions of a symbol change, one row each. Every row states
    /// what the two counts are and what the change therefore is.
    #[test]
    fn symbol_delta_measures_every_partition() {
        let rows: &[(SymbolCount, SymbolCount, SymbolDelta)] = &[
            // Nothing before, nothing after: nothing changed.
            (
                SymbolCount::Measured(0),
                SymbolCount::Measured(0),
                SymbolDelta::Measured {
                    added: 0,
                    removed: 0,
                },
            ),
            // Nothing before, three after: three arrived.
            (
                SymbolCount::Measured(0),
                SymbolCount::Measured(3),
                SymbolDelta::Measured {
                    added: 3,
                    removed: 0,
                },
            ),
            // Three before, nothing after: three left.
            (
                SymbolCount::Measured(3),
                SymbolCount::Measured(0),
                SymbolDelta::Measured {
                    added: 0,
                    removed: 3,
                },
            ),
            // Three before, five after: the file grew.
            (
                SymbolCount::Measured(3),
                SymbolCount::Measured(5),
                SymbolDelta::Measured {
                    added: 2,
                    removed: 0,
                },
            ),
            // Five before, three after: the file shrank.
            (
                SymbolCount::Measured(5),
                SymbolCount::Measured(3),
                SymbolDelta::Measured {
                    added: 0,
                    removed: 2,
                },
            ),
            // Three before, three after: the file held the same number.
            (
                SymbolCount::Measured(3),
                SymbolCount::Measured(3),
                SymbolDelta::Measured {
                    added: 0,
                    removed: 0,
                },
            ),
            // The before side was never counted: the change is unknown.
            (
                SymbolCount::Unmeasured,
                SymbolCount::Measured(3),
                SymbolDelta::Unmeasured,
            ),
            // The after side was never counted: the change is unknown.
            (
                SymbolCount::Measured(3),
                SymbolCount::Unmeasured,
                SymbolDelta::Unmeasured,
            ),
            // Neither side was counted: the change is unknown.
            (
                SymbolCount::Unmeasured,
                SymbolCount::Unmeasured,
                SymbolDelta::Unmeasured,
            ),
        ];

        for (previous, current, expected) in rows {
            assert_eq!(
                symbol_delta(*previous, *current),
                *expected,
                "delta of {previous:?} -> {current:?}"
            );
        }
    }

    /// An unknown change contributes nothing to a sum, and says so.
    #[test]
    fn an_unmeasured_delta_is_not_a_zero_delta() {
        let unknown = symbol_delta(SymbolCount::Unmeasured, SymbolCount::Measured(4));
        assert!(!unknown.is_measured());
        assert_eq!(unknown.added(), 0);
        assert_eq!(unknown.removed(), 0);

        let measured = symbol_delta(SymbolCount::Measured(1), SymbolCount::Measured(4));
        assert!(measured.is_measured());
        assert_eq!(measured.added(), 3);
        assert_eq!(measured.removed(), 0);
    }

    /// An unmeasured count is not a count of zero.
    #[test]
    fn an_unmeasured_count_is_not_zero() {
        assert_eq!(SymbolCount::Unmeasured.measured(), None);
        assert!(!SymbolCount::Unmeasured.is_measured());
        assert_eq!(SymbolCount::Measured(0).measured(), Some(0));
        assert!(SymbolCount::Measured(0).is_measured());
    }
}
