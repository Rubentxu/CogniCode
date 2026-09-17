//! OPTIMIZE / CONFIRM dataset split (e81 — M13).
//!
//! ## The core invariant
//!
//! ```text
//! OPTIMIZE ∩ CONFIRM = ∅
//! ```
//!
//! and the split is immutable for the whole evaluation run. Overlap is a
//! **configuration error**, rejected at construction, so it can never reach an
//! execution stage. That is what makes "overlap => zero predictor executions"
//! true by construction rather than by discipline.
//!
//! ## The digest encodes the ROLE
//!
//! `OPTIMIZE=[A], CONFIRM=[B]` and `OPTIMIZE=[B], CONFIRM=[A]` must not collide:
//! swapping roles changes which cases a candidate was fitted on, so it must
//! produce a different digest.

use crate::application::historical_replay::case::HistoricalCaseId;
use crate::application::portable_execution::content_digest;

/// Which side of the held-out separation a case belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DatasetRole {
    /// Visible while iterating on the analyzer.
    Optimize,
    /// Held out: never used to construct a candidate.
    Confirm,
}

impl DatasetRole {
    /// Stable name for diagnostics and digests.
    pub fn name(self) -> &'static str {
        match self {
            Self::Optimize => "optimize",
            Self::Confirm => "confirm",
        }
    }
}

/// An immutable, disjoint assignment of case ids to roles.
///
/// Private fields, canonical id order, and no mutation API: once constructed,
/// role assignment cannot change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetSplit {
    optimize: Vec<HistoricalCaseId>,
    confirm: Vec<HistoricalCaseId>,
    digest: String,
}

impl DatasetSplit {
    /// Build a split, validating before anything can execute.
    ///
    /// Rejects a duplicate id within either role, and any id present in both
    /// roles (the configuration error the umbrella spec forbids).
    pub fn try_new(
        mut optimize: Vec<HistoricalCaseId>,
        mut confirm: Vec<HistoricalCaseId>,
    ) -> Result<Self, SplitError> {
        optimize.sort();
        confirm.sort();

        for pair in optimize.windows(2) {
            if pair[0] == pair[1] {
                return Err(SplitError::DuplicateInOptimize(pair[0].clone()));
            }
        }
        for pair in confirm.windows(2) {
            if pair[0] == pair[1] {
                return Err(SplitError::DuplicateInConfirm(pair[0].clone()));
            }
        }
        for id in &optimize {
            if confirm.binary_search(id).is_ok() {
                return Err(SplitError::Overlap(id.clone()));
            }
        }

        let digest = compute_split_digest(&optimize, &confirm);
        Ok(Self {
            optimize,
            confirm,
            digest,
        })
    }

    /// The OPTIMIZE ids, sorted.
    pub fn optimize(&self) -> &[HistoricalCaseId] {
        &self.optimize
    }

    /// The CONFIRM ids, sorted.
    pub fn confirm(&self) -> &[HistoricalCaseId] {
        &self.confirm
    }

    /// The ids for a role.
    pub fn ids_for(&self, role: DatasetRole) -> &[HistoricalCaseId] {
        match role {
            DatasetRole::Optimize => &self.optimize,
            DatasetRole::Confirm => &self.confirm,
        }
    }

    /// Which role an id has, if any.
    pub fn role_of(&self, id: &HistoricalCaseId) -> Option<DatasetRole> {
        if self.optimize.binary_search(id).is_ok() {
            Some(DatasetRole::Optimize)
        } else if self.confirm.binary_search(id).is_ok() {
            Some(DatasetRole::Confirm)
        } else {
            None
        }
    }

    /// The stable split digest (order-invariant, role-sensitive).
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Digest that encodes role assignment as well as ids.
fn compute_split_digest(optimize: &[HistoricalCaseId], confirm: &[HistoricalCaseId]) -> String {
    let mut canonical = String::new();
    canonical.push_str("historical-split.v1\n");
    canonical.push_str("role=optimize\n");
    for id in optimize {
        canonical.push_str(id.as_str());
        canonical.push('\n');
    }
    canonical.push_str("role=confirm\n");
    for id in confirm {
        canonical.push_str(id.as_str());
        canonical.push('\n');
    }
    content_digest(canonical.as_bytes()).as_str().to_string()
}

/// Why a split could not be constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitError {
    /// The same id appeared twice on the OPTIMIZE side.
    DuplicateInOptimize(HistoricalCaseId),
    /// The same id appeared twice on the CONFIRM side.
    DuplicateInConfirm(HistoricalCaseId),
    /// An id appeared on both sides.
    Overlap(HistoricalCaseId),
}

impl std::fmt::Display for SplitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateInOptimize(id) => {
                write!(f, "duplicate case id in OPTIMIZE: {id}")
            }
            Self::DuplicateInConfirm(id) => {
                write!(f, "duplicate case id in CONFIRM: {id}")
            }
            Self::Overlap(id) => write!(
                f,
                "case id {id} appears in both OPTIMIZE and CONFIRM (configuration error)"
            ),
        }
    }
}

impl std::error::Error for SplitError {}
