//! e69 WU2 — `EvidenceProducer` trait + local producers.
//!
//! Producers convert one work-result stream (a cargo test run, a
//! `just` recipe, a `cogh` invocation, an existing analysis run)
//! into [`BundleEntry`] values. The aggregator runs every registered
//! producer; a single producer's failure NEVER aborts the bundle —
//! it is recorded as a `ProducerFailed` / `ProducerMissing` /
//! `ProducerUnknown` entry and the aggregator moves on.
//!
//! ## Architecture invariants
//!
//! - Producers PROPOSE evidence. They do not decide.
//! - The bundle aggregator is fail-soft: it absorbs local failures
//!   as typed entries. The gate (WU3) is the only authority that
//!   turns a bundle into a `Pass`/`Warn`/`Block`/`InsufficientEvidence`.
//! - Unknown/incomplete never becomes "safe": see the
//!   `ProducerUnknown` translation rule below.
//!
//! ## Determinism
//!
//! The aggregator produces entries in the order the producers are
//! registered. The `EvidenceBundle::new` constructor re-sorts by
//! `(source, slot_id)`, so any producer order yields the same bundle.

use crate::application::evidence_bundle::{
    BundleEntry, EvidenceBundle, EvidenceBundleId, ProducerSlot, ProducerSource, next_bundle_id,
};
use crate::domain::evidence_kernel::ids::SnapshotId;
use crate::domain::findings::ports::EvidenceDescriptor;
use crate::domain::kernel_ids::ExecutionId;

/// The outcome a producer returns for one of its slots.
///
/// Distinguishes the three non-Evidence cases explicitly:
/// - [`Outcome::Failed`]: the producer ran and returned a hard failure.
/// - [`Outcome::Missing`]: the producer could not be reached at all.
/// - [`Outcome::Unknown`]: the producer ran but returned ambiguous /
///   incomplete / unparseable output.
/// - [`Outcome::Evidence`]: the producer ran successfully and produced
///   graded evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Failed { reason: String, raw: Option<String> },
    Missing { why_unreachable: String },
    Unknown { detail: String },
    Evidence { descriptor: EvidenceDescriptor },
}

/// One piece of work a producer commits to. The aggregator does not
/// care about the producer's internal shape — only about the
/// [`Outcome`] for each [`ProducerSlot`] it claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducerOutput {
    pub slot: ProducerSlot,
    pub outcome: Outcome,
}

impl ProducerOutput {
    /// Translate this output into a [`BundleEntry`]. The translation
    /// is total: every `Outcome` variant maps to exactly one
    /// `BundleEntry` variant.
    pub fn into_entry(self) -> BundleEntry {
        let ProducerOutput { slot, outcome } = self;
        match outcome {
            Outcome::Failed { reason, raw } => BundleEntry::ProducerFailed { slot, reason, raw },
            Outcome::Missing { why_unreachable } => BundleEntry::ProducerMissing {
                slot,
                why_unreachable,
            },
            Outcome::Unknown { detail } => BundleEntry::ProducerUnknown { slot, detail },
            Outcome::Evidence { descriptor } => BundleEntry::Evidence { slot, descriptor },
        }
    }
}

/// A producer converts a piece of work into one or more
/// [`ProducerOutput`]s. Implementors MUST be total: every slot they
/// claim resolves to exactly one `Outcome`. They MUST NOT panic on
/// invalid inputs (the aggregator runs every producer and would
/// abort the bundle otherwise).
pub trait EvidenceProducer: Send + Sync {
    /// The producer's source identity.
    fn source(&self) -> ProducerSource;

    /// Produce all outputs for this work item. May return an empty
    /// `Vec` if the producer has nothing to report for the work.
    fn produce(
        &self,
        work: &crate::application::change_tracking::planner::WorkId,
    ) -> Vec<ProducerOutput>;
}

/// Aggregate a list of producers into a single bundle.
///
/// The aggregator is pure & total: it runs every producer, translates
/// each [`ProducerOutput`] into a [`BundleEntry`], and constructs the
/// bundle (which re-sorts entries for determinism). Producer panics
/// would propagate; producers are expected to translate their own
/// errors into `Outcome::Failed`/`Missing`/`Unknown`.
pub fn aggregate(
    id: EvidenceBundleId,
    work: crate::application::change_tracking::planner::WorkId,
    execution: ExecutionId,
    snapshot: SnapshotId,
    producers: &[Box<dyn EvidenceProducer>],
) -> EvidenceBundle {
    let mut entries: Vec<BundleEntry> = Vec::new();
    for p in producers {
        for out in p.produce(&work) {
            entries.push(out.into_entry());
        }
    }
    EvidenceBundle::new(id, work, execution, snapshot, entries)
}

/// Convenience: allocate a fresh id and aggregate.
pub fn aggregate_fresh(
    work: crate::application::change_tracking::planner::WorkId,
    execution: ExecutionId,
    snapshot: SnapshotId,
    producers: &[Box<dyn EvidenceProducer>],
) -> EvidenceBundle {
    aggregate(next_bundle_id(), work, execution, snapshot, producers)
}

/// In-memory producer for testing. Records a fixed list of outputs.
#[derive(Clone)]
pub struct StaticProducer {
    pub source: ProducerSource,
    pub outputs: Vec<ProducerOutput>,
}

impl StaticProducer {
    pub fn new(source: ProducerSource, outputs: Vec<ProducerOutput>) -> Self {
        Self { source, outputs }
    }
}

impl EvidenceProducer for StaticProducer {
    fn source(&self) -> ProducerSource {
        self.source
    }

    fn produce(
        &self,
        _work: &crate::application::change_tracking::planner::WorkId,
    ) -> Vec<ProducerOutput> {
        self.outputs.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::change_tracking::planner::WorkId;
    use crate::domain::evidence_kernel::ids::{EvidenceId, FactId, SnapshotId};
    use crate::domain::findings::ports::{EvidenceDescriptor, FactDescriptor, FactSlot};
    use crate::domain::kernel_ids::{EvidenceGrade, ExecutionId};
    use crate::domain::naming::NamespacedName;

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

    fn slot(source: ProducerSource, id: &str) -> ProducerSlot {
        ProducerSlot {
            source,
            slot_id: id.to_string(),
        }
    }

    fn work(s: &str) -> WorkId {
        WorkId::new(NamespacedName::new(s).unwrap())
    }

    // --- UAT: producer outcomes translate to bundle entries ---

    #[test]
    fn ok_outcome_yields_evidence_entry() {
        let s = slot(ProducerSource::CargoTest, "ok");
        let p = StaticProducer::new(
            ProducerSource::CargoTest,
            vec![ProducerOutput {
                slot: s,
                outcome: Outcome::Evidence {
                    descriptor: ev_descriptor(1, 10, EvidenceGrade::Supports),
                },
            }],
        );
        let b = aggregate_fresh(work("ci.x"), ExecutionId(1), SnapshotId(1), &[Box::new(p)]);
        assert_eq!(b.count_evidence(), 1);
        assert_eq!(b.count_failed(), 0);
    }

    #[test]
    fn err_outcome_yields_failed_entry() {
        let s = slot(ProducerSource::CargoTest, "boom");
        let p = StaticProducer::new(
            ProducerSource::CargoTest,
            vec![ProducerOutput {
                slot: s,
                outcome: Outcome::Failed {
                    reason: "exit 1".into(),
                    raw: Some("...".into()),
                },
            }],
        );
        let b = aggregate_fresh(work("ci.x"), ExecutionId(1), SnapshotId(1), &[Box::new(p)]);
        assert_eq!(b.count_failed(), 1);
        assert_eq!(b.count_evidence(), 0);
    }

    #[test]
    fn unreachable_yields_missing_entry() {
        let s = slot(ProducerSource::JustRecipe, "missing");
        let p = StaticProducer::new(
            ProducerSource::JustRecipe,
            vec![ProducerOutput {
                slot: s,
                outcome: Outcome::Missing {
                    why_unreachable: "tool not found".into(),
                },
            }],
        );
        let b = aggregate_fresh(work("ci.x"), ExecutionId(1), SnapshotId(1), &[Box::new(p)]);
        assert_eq!(b.count_missing(), 1);
        assert_eq!(b.count_evidence(), 0);
    }

    #[test]
    fn garbled_yields_unknown_entry() {
        let s = slot(ProducerSource::Cogh, "weird");
        let p = StaticProducer::new(
            ProducerSource::Cogh,
            vec![ProducerOutput {
                slot: s,
                outcome: Outcome::Unknown {
                    detail: "garbled".into(),
                },
            }],
        );
        let b = aggregate_fresh(work("ci.x"), ExecutionId(1), SnapshotId(1), &[Box::new(p)]);
        assert_eq!(b.count_unknown(), 1);
        assert_eq!(b.count_evidence(), 0);
    }

    // --- aggregation invariants ---

    #[test]
    fn aggregation_across_n_producers_yields_n_or_fewer_entries() {
        let ps: Vec<Box<dyn EvidenceProducer>> = vec![
            Box::new(StaticProducer::new(
                ProducerSource::CargoTest,
                vec![ProducerOutput {
                    slot: slot(ProducerSource::CargoTest, "a"),
                    outcome: Outcome::Evidence {
                        descriptor: ev_descriptor(1, 1, EvidenceGrade::Supports),
                    },
                }],
            )),
            Box::new(StaticProducer::new(
                ProducerSource::JustRecipe,
                (0..3)
                    .map(|i| ProducerOutput {
                        slot: slot(ProducerSource::JustRecipe, &format!("r{i}")),
                        outcome: Outcome::Evidence {
                            descriptor: ev_descriptor(i + 10, i + 10, EvidenceGrade::Supports),
                        },
                    })
                    .collect(),
            )),
            Box::new(StaticProducer::new(
                ProducerSource::Cogh,
                vec![ProducerOutput {
                    slot: slot(ProducerSource::Cogh, "c"),
                    outcome: Outcome::Missing {
                        why_unreachable: "not installed".into(),
                    },
                }],
            )),
        ];
        let b = aggregate_fresh(work("ci.x"), ExecutionId(1), SnapshotId(1), &ps);
        assert_eq!(b.len(), 5, "1+3+1 = 5 entries across 3 producers");
        assert_eq!(b.count_evidence(), 4);
        assert_eq!(b.count_missing(), 1);
    }

    #[test]
    fn aggregation_is_deterministic_under_producer_reordering() {
        let p1_inner = StaticProducer::new(
            ProducerSource::CargoTest,
            vec![ProducerOutput {
                slot: slot(ProducerSource::CargoTest, "a"),
                outcome: Outcome::Evidence {
                    descriptor: ev_descriptor(1, 1, EvidenceGrade::Supports),
                },
            }],
        );
        let p2_inner = StaticProducer::new(
            ProducerSource::JustRecipe,
            vec![ProducerOutput {
                slot: slot(ProducerSource::JustRecipe, "b"),
                outcome: Outcome::Evidence {
                    descriptor: ev_descriptor(2, 2, EvidenceGrade::Supports),
                },
            }],
        );
        let b1 = aggregate(
            EvidenceBundleId(7),
            work("ci.x"),
            ExecutionId(1),
            SnapshotId(1),
            &[Box::new(p1_inner.clone()), Box::new(p2_inner.clone())],
        );
        let b2 = aggregate(
            EvidenceBundleId(7),
            work("ci.x"),
            ExecutionId(1),
            SnapshotId(1),
            &[Box::new(p2_inner), Box::new(p1_inner)],
        );
        assert_eq!(b1, b2);
    }

    #[test]
    fn one_producer_failure_does_not_abort_others() {
        let p1: Box<dyn EvidenceProducer> = Box::new(StaticProducer::new(
            ProducerSource::CargoTest,
            vec![ProducerOutput {
                slot: slot(ProducerSource::CargoTest, "a"),
                outcome: Outcome::Evidence {
                    descriptor: ev_descriptor(1, 1, EvidenceGrade::Supports),
                },
            }],
        ));
        // StaticProducer::produce cannot fail by construction. To
        // simulate "the producer had a hard failure", use a custom
        // producer that returns Failed outcomes for all its slots.
        struct AlwaysFailedProducer;
        impl EvidenceProducer for AlwaysFailedProducer {
            fn source(&self) -> ProducerSource {
                ProducerSource::JustRecipe
            }
            fn produce(&self, _w: &WorkId) -> Vec<ProducerOutput> {
                vec![ProducerOutput {
                    slot: slot(ProducerSource::JustRecipe, "boom"),
                    outcome: Outcome::Failed {
                        reason: "exit 2".into(),
                        raw: Some("oops".into()),
                    },
                }]
            }
        }
        let p2: Box<dyn EvidenceProducer> = Box::new(AlwaysFailedProducer);
        let p3: Box<dyn EvidenceProducer> = Box::new(StaticProducer::new(
            ProducerSource::Cogh,
            vec![ProducerOutput {
                slot: slot(ProducerSource::Cogh, "c"),
                outcome: Outcome::Evidence {
                    descriptor: ev_descriptor(3, 3, EvidenceGrade::Supports),
                },
            }],
        ));

        let b = aggregate_fresh(work("ci.x"), ExecutionId(1), SnapshotId(1), &[p1, p2, p3]);
        assert_eq!(b.count_evidence(), 2);
        assert_eq!(b.count_failed(), 1);
        assert_eq!(b.len(), 3);
    }
}
