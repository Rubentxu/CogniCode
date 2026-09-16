//! Read-set: ordered, deduplicated, bounded dependency recorder for executions.
//!
//! This module is the M7.5 (cycle e66) foundation. An [`InMemoryReadSetRecorder`]
//! admits [`FactId`] references during execution and yields a [`ReadSet`] whose
//! [`ReadSet::contains`] lookup is the only input to the UAT-U61
//! "unrelated change does not invalidate" expectation (see
//! `openspec/changes/e66-lsi-m7-5-read-sets/proposal.md`).
//!
//! ## Architecture notes
//!
//! - Pure domain: no I/O, no clocks, no allocation beyond the buffered facts.
//! - Insertion order is preserved via [`Vec`]; dedup uses a parallel
//!   [`HashSet`] for O(1) `contains` lookups (invalidations iterate `changed_facts`
//!   and ask `contains` once per change — read-set size dominates work).
//! - Truncation: when `max_records` is set and the bound is reached, subsequent
//!   inserts set `truncated = true` but **still attempt dedup** so post-bound
//!   references can resolve an existing fact (matters for invalidation accuracy).
//! - No external persistence — finalization hands the owned [`ReadSet`] to
//!   the caller; persistence adapter is a future-cycle concern.

use crate::domain::kernel_ids::FactId;
use std::collections::HashSet;
use std::num::NonZeroUsize;

/// Configuration for an [`InMemoryReadSetRecorder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadSetConfig {
    /// Soft upper bound on admitted facts. When `None`, the recorder is
    /// unbounded; when `Some(n)`, the (n+1)-th distinct fact sets the
    /// truncation marker without growing the stored set.
    pub max_records: Option<NonZeroUsize>,
}

/// Errors returned by the recorder.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReadSetError {
    /// Caller passed an empty `FactId` (would corrupt dedup invariants).
    #[error("FactId payload cannot be empty")]
    EmptyFactId,
}

/// Immutable, deduplicated, ordered dependency set.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReadSet {
    ordered: Vec<FactId>,
    seen: HashSet<FactId>,
    truncated: bool,
}

impl ReadSet {
    /// Iterate in insertion order, first occurrence preserved.
    pub fn iter(&self) -> impl Iterator<Item = &FactId> {
        self.ordered.iter()
    }

    /// True iff at least one distinct fact was dropped due to the bound.
    pub fn is_truncated(&self) -> bool {
        self.truncated
    }

    /// O(1) containment check (the invalidation primitive).
    pub fn contains(&self, fact: &FactId) -> bool {
        self.seen.contains(fact)
    }

    /// Distinct facts recorded.
    pub fn len(&self) -> usize {
        self.ordered.len()
    }

    /// True iff no facts were recorded.
    pub fn is_empty(&self) -> bool {
        self.ordered.is_empty()
    }
}

impl std::iter::FromIterator<FactId> for ReadSet {
    fn from_iter<I: IntoIterator<Item = FactId>>(iter: I) -> Self {
        let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
        for f in iter {
            // Bounded-by-construction: from_iter implies no bound.
            let _ = rec.record(f);
        }
        rec.finalize()
    }
}

/// In-memory recorder. Admit facts by reference, finalize to a [`ReadSet`].
pub struct InMemoryReadSetRecorder {
    config: ReadSetConfig,
    ordered: Vec<FactId>,
    seen: HashSet<FactId>,
    truncated: bool,
    finalized: bool,
}

impl InMemoryReadSetRecorder {
    /// Construct a new recorder with the given config.
    pub fn new(config: ReadSetConfig) -> Self {
        Self {
            config,
            ordered: Vec::new(),
            seen: HashSet::new(),
            truncated: false,
            finalized: false,
        }
    }

    /// Record a fact. Returns `Ok(())` regardless of whether the fact was
    /// distinct (dedup is silent) or admitted past the bound (sets
    /// `is_truncated` but **still tracks the fact for invalidation** —
    /// a recording cannot discard a dependency once observed).
    pub fn record(&mut self, fact_id: FactId) -> Result<(), ReadSetError> {
        if self.finalized {
            // Defensive: a recorder must not be reused post-finalize.
            return Err(ReadSetError::EmptyFactId);
        }
        if !self.seen.insert(fact_id) {
            // Dedup: silently drop repeats regardless of bound state.
            return Ok(());
        }
        match self.config.max_records {
            Some(limit) if self.ordered.len() >= limit.get() => {
                // Past the bound. The fact is tracked for invalidation
                // (`seen`) but not added to the iteration-ordered list
                // (so `len()` reports the bounded count, not the
                // observed count). Truncation is observable via
                // `ReadSet::is_truncated`.
                self.truncated = true;
                Ok(())
            }
            _ => {
                self.ordered.push(fact_id);
                Ok(())
            }
        }
    }

    /// Consume the recorder, returning the recorded [`ReadSet`].
    pub fn finalize(mut self) -> ReadSet {
        self.finalized = true;
        ReadSet {
            ordered: self.ordered,
            seen: self.seen,
            truncated: self.truncated,
        }
    }
}

/// Invalidation primitive: did anything the execution actually read change?
///
/// Returns `true` if at least one changed fact appears in the read set
/// (the execution's recorded dependencies are stale). Returns `false`
/// for an empty `changed_facts` slice, an empty read set, or when no
/// intersection exists.
///
/// This is the implementation backing UAT-U61
/// ("Unrelated change does not invalidate the execution").
pub fn is_stale(read_set: &ReadSet, changed_facts: &[FactId]) -> bool {
    changed_facts.iter().any(|f| read_set.contains(f))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fid(n: u64) -> FactId {
        FactId(n)
    }

    #[test]
    fn dedup_keeps_first_insertion_order() {
        let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
        rec.record(fid(1)).unwrap();
        rec.record(fid(2)).unwrap();
        rec.record(fid(1)).unwrap(); // duplicate
        rec.record(fid(3)).unwrap();
        let rs = rec.finalize();

        let order: Vec<u64> = rs.iter().map(|f| f.0).collect();
        assert_eq!(order, vec![1, 2, 3]);
        assert_eq!(rs.len(), 3);
        assert!(!rs.is_truncated());
        // Dedup also works for `contains`.
        assert!(rs.contains(&fid(1)));
        assert!(!rs.contains(&fid(99)));
    }

    #[test]
    fn truncation_marker_at_bound() {
        // Bound = 3, record 4 distinct facts in order [1, 2, 3, 4].
        let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig {
            max_records: NonZeroUsize::new(3),
        });
        for n in [1u64, 2, 3, 4] {
            rec.record(fid(n)).unwrap();
        }
        let rs = rec.finalize();

        // The bounded ordered list holds the first 3 distinct facts.
        assert_eq!(rs.len(), 3);
        let order: Vec<u64> = rs.iter().map(|f| f.0).collect();
        assert_eq!(order, vec![1, 2, 3]);

        // Truncation marker is observable.
        assert!(rs.is_truncated());

        // Even though fact 4 is NOT in the iteration order (it was dropped
        // to preserve the bound), it IS still tracked as a recorded
        // dependency so invalidation propagates correctly: an execution
        // that read fact 4 must be invalidated when fact 4 changes.
        assert!(rs.contains(&fid(4)));
        // Conversely, an unrelated fact is not in either view.
        assert!(!rs.contains(&fid(99)));
    }

    #[test]
    fn truncation_preserves_invalidation_semantics() {
        // Companion to the bound test: confirm that an execution whose
        // recorded read set saturated at the bound is still invalidated
        // when a post-bound fact changes.
        let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig {
            max_records: NonZeroUsize::new(2),
        });
        rec.record(fid(1)).unwrap();
        rec.record(fid(2)).unwrap();
        rec.record(fid(3)).unwrap(); // post-bound, sets truncated
        let rs = rec.finalize();
        assert!(rs.is_truncated());

        // Changing fact 3 invalidates the execution even though fact 3
        // is not in the iteration order.
        assert!(is_stale(&rs, &[fid(3)]));
        // An unrelated change does not.
        assert!(!is_stale(&rs, &[fid(99)]));
    }

    #[test]
    fn uat_u61_unrelated_change_does_not_invalidate() {
        let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
        rec.record(fid(10)).unwrap(); // A
        rec.record(fid(20)).unwrap(); // B
        let rs = rec.finalize();

        // C is unrelated to A and B.
        assert!(!is_stale(&rs, &[fid(30)]));
        // And even mixed: one unrelated, none related.
        assert!(!is_stale(&rs, &[fid(30), fid(40)]));
        // A is in the read set, so the change IS relevant.
        assert!(is_stale(&rs, &[fid(10)]));
        // Mixed: one related, one not — still stale.
        assert!(is_stale(&rs, &[fid(30), fid(20)]));
    }

    #[test]
    fn empty_inputs_are_safe() {
        let rs = ReadSet::default();
        // Empty read set with any change is trivially not affected.
        assert!(!is_stale(&rs, &[fid(1)]));
        // Empty changed set is trivially not stale regardless of read set.
        let mut rec = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
        rec.record(fid(1)).unwrap();
        let rs = rec.finalize();
        assert!(!is_stale(&rs, &[]));
    }

    #[test]
    fn from_iterator_collects() {
        let rs: ReadSet = vec![fid(5), fid(6), fid(5)].into_iter().collect();
        assert_eq!(rs.len(), 2);
        let order: Vec<u64> = rs.iter().map(|f| f.0).collect();
        assert_eq!(order, vec![5, 6]);
    }
}
