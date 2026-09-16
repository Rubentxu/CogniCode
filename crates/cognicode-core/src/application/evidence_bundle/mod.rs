//! e69 WU1 — `EvidenceBundle` data types.
//!
//! The bundle is a **derived operational aggregation** of work results.
//! It is NOT a canonical Fact; nothing here writes to the Evidence
//! Kernel. Per the e69 proposal (shape B, not A or C), we model
//! FAILED / MISSING / UNKNOWN / Evidence as first-class citizens of
//! [`BundleEntry`] — none collapses into another.
//!
//! ## Architecture invariants
//!
//! - Facts are canonical — the bundle is NOT a Fact.
//! - Evidence carries proof — producers propose; the gate decides.
//! - Derived operational decisions are not Facts — the bundle is
//!   derived.
//! - Unknown/incomplete never becomes "safe" — see the
//!   [`ready_for_gate`] helper, which is **advisory only**: the gate is
//!   the only authority that may pass.
//!
//! ## Ordering
//!
//! Entries are stored sorted by `(source, slot_id)`. The aggregator in
//! WU2 inserts in deterministic order; this module enforces the sort
//! invariant so consumers can rely on it.

pub mod producer;

use crate::domain::findings::ports::EvidenceDescriptor;

/// Opaque per-bundle identifier. Allocated in process for e69;
/// persistence is e70+.
///
/// Not `Copy`: the id is meaningful inside the bundle and downstream
/// consumers may compare it for auditability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EvidenceBundleId(pub u64);

impl std::fmt::Display for EvidenceBundleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bundle#{}", self.0)
    }
}

/// Identifies where a [`BundleEntry`] came from.
///
/// `slot_id` is producer-defined and is what makes the entry sortable
/// alongside the `source` (e.g. for a "cargo test" producer,
/// `slot_id = "unit::math::sum"`). Producers MUST keep slot ids stable
/// across runs for the same logical check.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProducerSlot {
    pub source: ProducerSource,
    pub slot_id: String,
}

/// What produced a `BundleEntry`.
///
/// In e69 the initial set is local-only: shell-out to existing tools
/// (cargo, just, cogh) and to existing analyses. e70 may grow this set
/// with adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProducerSource {
    /// `cargo test` (or `cargo test --test <name>`).
    CargoTest,
    /// A `just` recipe.
    JustRecipe,
    /// The `cogh` CLI.
    Cogh,
    /// An existing analysis run (e.g. e67 grounded finding flow).
    Analysis,
    /// Reserved for producers added in e70 or later.
    Other,
}

impl std::fmt::Display for ProducerSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ProducerSource::CargoTest => "cargo_test",
            ProducerSource::JustRecipe => "just_recipe",
            ProducerSource::Cogh => "cogh",
            ProducerSource::Analysis => "analysis",
            ProducerSource::Other => "other",
        };
        f.write_str(s)
    }
}

/// One element of an [`EvidenceBundle`].
///
/// The four variants are deliberately disjoint. Producers MUST NOT
/// translate FAILED into MISSING or vice versa; the gate (e69 WU3) is
/// the only authority that may downgrade any of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleEntry {
    /// The producer ran and returned a hard failure (non-zero exit,
    /// parse error, assertion failure). The bundle records the reason
    /// and (when available) a tail of raw output for auditing.
    ProducerFailed {
        slot: ProducerSlot,
        reason: String,
        raw: Option<String>,
    },
    /// The producer could not be reached at all (tool missing, I/O
    /// error before execution). Distinct from Failed: the producer
    /// never ran.
    ProducerMissing {
        slot: ProducerSlot,
        why_unreachable: String,
    },
    /// The producer ran but returned ambiguous / unparseable /
    /// incomplete output. Distinct from Failed (it ran) and from
    /// Missing (it executed). The gate MUST treat this as
    /// "insufficient evidence" — never as success.
    ProducerUnknown { slot: ProducerSlot, detail: String },
    /// The producer ran successfully and produced graded evidence.
    Evidence {
        slot: ProducerSlot,
        descriptor: EvidenceDescriptor,
    },
}

impl BundleEntry {
    /// The slot this entry belongs to. Used for ordering and for the
    /// gate to look up the matching rule.
    pub fn slot(&self) -> &ProducerSlot {
        match self {
            BundleEntry::ProducerFailed { slot, .. }
            | BundleEntry::ProducerMissing { slot, .. }
            | BundleEntry::ProducerUnknown { slot, .. }
            | BundleEntry::Evidence { slot, .. } => slot,
        }
    }

    /// True iff this entry is graded `Evidence` — i.e. a producer
    /// successfully produced a [`EvidenceDescriptor`]. The other three
    /// variants all return false.
    pub fn is_evidence(&self) -> bool {
        matches!(self, BundleEntry::Evidence { .. })
    }
}

/// A derived operational aggregation of work results for one piece of
/// logical work. Sorted by `(source, slot_id)` on construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceBundle {
    pub id: EvidenceBundleId,
    pub work: crate::application::change_tracking::planner::WorkId,
    pub execution: crate::domain::kernel_ids::ExecutionId,
    pub snapshot: crate::domain::evidence_kernel::ids::SnapshotId,
    pub entries: Vec<BundleEntry>,
}

impl EvidenceBundle {
    /// Construct a bundle, sorting entries by `(source, slot_id)` so
    /// downstream consumers can rely on a stable order.
    pub fn new(
        id: EvidenceBundleId,
        work: crate::application::change_tracking::planner::WorkId,
        execution: crate::domain::kernel_ids::ExecutionId,
        snapshot: crate::domain::evidence_kernel::ids::SnapshotId,
        mut entries: Vec<BundleEntry>,
    ) -> Self {
        entries.sort_by(|a, b| a.slot().cmp(b.slot()));
        Self {
            id,
            work,
            execution,
            snapshot,
            entries,
        }
    }

    /// True iff there is at least one entry of any kind.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of entries (across all four variants).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True iff every entry is graded `Evidence` (no FAILED, MISSING,
    /// UNKNOWN). This is **advisory** — the gate may still decline to
    /// Pass based on which slots are required and which grades they
    /// carry.
    pub fn ready_for_gate(&self) -> bool {
        self.entries.iter().all(BundleEntry::is_evidence)
    }

    /// Count entries of a given kind (for diagnostics and tests).
    pub fn count_failed(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, BundleEntry::ProducerFailed { .. }))
            .count()
    }

    /// Count entries of a given kind (for diagnostics and tests).
    pub fn count_missing(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, BundleEntry::ProducerMissing { .. }))
            .count()
    }

    /// Count entries of a given kind (for diagnostics and tests).
    pub fn count_unknown(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, BundleEntry::ProducerUnknown { .. }))
            .count()
    }

    /// Count entries of a given kind (for diagnostics and tests).
    pub fn count_evidence(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, BundleEntry::Evidence { .. }))
            .count()
    }
}

/// Allocate a fresh bundle id. In e69 this is process-local; e70+ may
/// back it with a persistent allocator.
///
/// The id is monotonic within a process so callers can order bundles
/// by allocation when they need to.
pub fn next_bundle_id() -> EvidenceBundleId {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    EvidenceBundleId(COUNTER.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::evidence_kernel::ids::{EvidenceId, FactId, SnapshotId};
    use crate::domain::findings::ports::{EvidenceDescriptor, FactDescriptor, FactSlot};
    use crate::domain::kernel_ids::{EvidenceGrade, ExecutionId};
    use crate::domain::naming::NamespacedName;

    fn slot(source: ProducerSource, slot_id: &str) -> ProducerSlot {
        ProducerSlot {
            source,
            slot_id: slot_id.to_string(),
        }
    }

    fn ev_descriptor(id: u64, fact: u64, grade: EvidenceGrade) -> EvidenceDescriptor {
        EvidenceDescriptor {
            id: EvidenceId(id),
            grade,
            fact: FactSlot::Resolved(FactDescriptor {
                id: FactId(fact),
                subject: None,
                snapshot: SnapshotId(1),
            }),
        }
    }

    fn bundle_with_id(id: u64, entries: Vec<BundleEntry>) -> EvidenceBundle {
        EvidenceBundle::new(
            EvidenceBundleId(id),
            crate::application::change_tracking::planner::WorkId::new(
                NamespacedName::new("ci.test-suite").unwrap(),
            ),
            ExecutionId(1),
            SnapshotId(1),
            entries,
        )
    }

    fn bundle_with(entries: Vec<BundleEntry>) -> EvidenceBundle {
        bundle_with_id(42, entries)
    }

    // --- shape UAT ---

    #[test]
    fn empty_bundle_is_well_formed() {
        let b = bundle_with(vec![]);
        assert!(b.is_empty());
        assert_eq!(b.len(), 0);
        // Empty is technically "all evidence" vacuously — but the
        // gate (WU3) is the only authority that may Pass.
        assert!(b.ready_for_gate());
    }

    #[test]
    fn four_kinds_coexist_without_collapsing() {
        let s1 = slot(ProducerSource::CargoTest, "alpha");
        let s2 = slot(ProducerSource::CargoTest, "beta");
        let s3 = slot(ProducerSource::CargoTest, "gamma");
        let s4 = slot(ProducerSource::CargoTest, "delta");
        let entries = vec![
            BundleEntry::ProducerFailed {
                slot: s1,
                reason: "exit 1".into(),
                raw: Some("...".into()),
            },
            BundleEntry::ProducerMissing {
                slot: s2,
                why_unreachable: "tool not found".into(),
            },
            BundleEntry::ProducerUnknown {
                slot: s3,
                detail: "garbled output".into(),
            },
            BundleEntry::Evidence {
                slot: s4,
                descriptor: ev_descriptor(1, 10, EvidenceGrade::Supports),
            },
        ];
        let b = bundle_with(entries);
        assert_eq!(b.len(), 4);
        assert_eq!(b.count_failed(), 1);
        assert_eq!(b.count_missing(), 1);
        assert_eq!(b.count_unknown(), 1);
        assert_eq!(b.count_evidence(), 1);
        assert!(!b.ready_for_gate());
    }

    #[test]
    fn ready_for_gate_requires_no_failed_missing_unknown() {
        let entries = vec![BundleEntry::Evidence {
            slot: slot(ProducerSource::CargoTest, "ok"),
            descriptor: ev_descriptor(1, 10, EvidenceGrade::Supports),
        }];
        let b = bundle_with(entries);
        assert!(b.ready_for_gate());
    }

    #[test]
    fn ready_for_gate_false_when_any_non_evidence_present() {
        let cases: Vec<BundleEntry> = vec![
            BundleEntry::ProducerFailed {
                slot: slot(ProducerSource::CargoTest, "x"),
                reason: "boom".into(),
                raw: None,
            },
            BundleEntry::ProducerMissing {
                slot: slot(ProducerSource::JustRecipe, "y"),
                why_unreachable: "missing".into(),
            },
            BundleEntry::ProducerUnknown {
                slot: slot(ProducerSource::Cogh, "z"),
                detail: "?".into(),
            },
        ];
        for c in cases {
            let b = bundle_with(vec![c]);
            assert!(!b.ready_for_gate(), "{:?}", b);
        }
    }

    #[test]
    fn ordering_is_stable_across_input_reordering() {
        let a = BundleEntry::Evidence {
            slot: slot(ProducerSource::CargoTest, "a"),
            descriptor: ev_descriptor(1, 1, EvidenceGrade::Supports),
        };
        let b = BundleEntry::Evidence {
            slot: slot(ProducerSource::CargoTest, "b"),
            descriptor: ev_descriptor(2, 2, EvidenceGrade::Supports),
        };
        let c = BundleEntry::Evidence {
            slot: slot(ProducerSource::CargoTest, "c"),
            descriptor: ev_descriptor(3, 3, EvidenceGrade::Supports),
        };

        let bundle_1 = bundle_with(vec![a.clone(), b.clone(), c.clone()]);
        let bundle_2 = bundle_with(vec![c, b, a.clone()]);
        assert_eq!(bundle_1, bundle_2);

        // Also test cross-source: CargoTest before JustRecipe before
        // Cogh (ordering by source then slot_id).
        let j = BundleEntry::Evidence {
            slot: slot(ProducerSource::JustRecipe, "a"),
            descriptor: ev_descriptor(4, 4, EvidenceGrade::Supports),
        };
        let bundle_3 = bundle_with(vec![j.clone(), a.clone()]);
        assert_eq!(bundle_3.entries[0].slot().source, ProducerSource::CargoTest);
        assert_eq!(
            bundle_3.entries[1].slot().source,
            ProducerSource::JustRecipe
        );
        assert_eq!(bundle_3, bundle_with(vec![a, j]));
    }

    #[test]
    fn bundle_ids_are_unique_within_a_process() {
        let a = next_bundle_id();
        let b = next_bundle_id();
        let c = next_bundle_id();
        assert!(a < b);
        assert!(b < c);
        assert_ne!(a, b);
        assert_ne!(b, c);
    }

    #[test]
    fn slot_lookup_is_consistent_across_variants() {
        let s = slot(ProducerSource::Cogh, "the-test");
        let entries = vec![
            BundleEntry::ProducerFailed {
                slot: s.clone(),
                reason: "x".into(),
                raw: None,
            },
            BundleEntry::ProducerMissing {
                slot: s.clone(),
                why_unreachable: "y".into(),
            },
            BundleEntry::ProducerUnknown {
                slot: s.clone(),
                detail: "z".into(),
            },
            BundleEntry::Evidence {
                slot: s.clone(),
                descriptor: ev_descriptor(1, 1, EvidenceGrade::Supports),
            },
        ];
        let b = bundle_with(entries);
        for e in &b.entries {
            assert_eq!(e.slot(), &s);
        }
    }
}
