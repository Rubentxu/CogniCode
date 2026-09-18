//! Canonical evidence write bridge and read model (M6, cycle e62.4, U42).
//!
//! This is where the findings domain meets the evidence kernel. It is an
//! **application**-layer adapter on purpose:
//!
//! - the kernel stores are async, and the domain must not `block_on`;
//! - the domain declares what it needs through the sync [`EvidenceSink`] and
//!   [`EvidenceLookup`] ports, and this module implements them over the kernel.
//!
//! ```text
//! DetectorExecutor::prepare ──► PreparedExecution ──┐
//!                                                   │ produced evidence (raw)
//!                        CanonicalEvidenceWriter ◄──┘
//!                             │  1. FactStore::get  (the fact is the authority)
//!                             │  2. entity/fact coherence
//!                             │  3. EvidenceStore::append_batch (atomic)
//!                             ▼
//!                        EvidenceBindings (index-aligned)
//!                             │
//!                        PreparedExecution::finalize ──► Finding
//!                             │
//!                        KernelEvidenceReadModel::load ──► FindingVerifier (sync)
//! ```
//!
//! Two rules are enforced here rather than anywhere else, because this is the
//! only layer that can see both sides:
//!
//! - **the fact is the authority.** A projection may name an entity, but the
//!   canonical fact decides; a hint that contradicts it is refused, never
//!   silently preferred;
//! - **nothing is fabricated.** An item the kernel cannot ground is reported
//!   [`Ungrounded`](EvidenceBinding::Ungrounded) with a reason. It is still
//!   persisted and still explainable — it simply cannot open the gate.
//!
//! Feature-gated with the rest of the kernel (`evidence-kernel`).

use std::collections::HashMap;

use crate::domain::evidence_kernel::evidence::EvidenceGrade;
use crate::domain::evidence_kernel::fact::ProvenanceRecord;
use crate::domain::evidence_kernel::ports::{EvidenceStore, FactStore, KernelError, NewEvidence};
use crate::domain::findings::binding::{EvidenceBinding, EvidenceBindings, GroundingFailure};
use crate::domain::findings::outcome::ProducedEvidence;
use crate::domain::findings::ports::{
    EvidenceDescriptor, EvidenceLookup, EvidenceResolution, FactDescriptor, FactSlot,
};
use crate::domain::findings::scope::AnalysisScope;
use crate::domain::kernel_ids::{EntityId, EvidenceId, FactId, SnapshotId};

/// Persists a run's produced evidence into the kernel, refusing to invent
/// truth.
pub struct CanonicalEvidenceWriter<'a> {
    facts: &'a dyn FactStore,
    evidence: &'a dyn EvidenceStore,
}

impl<'a> CanonicalEvidenceWriter<'a> {
    /// Construct a writer over the kernel stores.
    pub fn new(facts: &'a dyn FactStore, evidence: &'a dyn EvidenceStore) -> Self {
        Self { facts, evidence }
    }

    /// Persist `produced` into `scope`, returning bindings index-aligned with
    /// `produced`.
    ///
    /// Grounded items are validated against the canonical fact *before*
    /// anything is written, and then committed in one atomic batch. An item
    /// that cannot be validated is not an error: it is reported ungrounded.
    /// A **store** error is an error, and is never degraded into "ungrounded".
    pub async fn persist(
        &self,
        scope: &AnalysisScope,
        produced: &[ProducedEvidence],
        provenance: &ProvenanceRecord,
    ) -> Result<EvidenceBindings, KernelError> {
        let mut entries: Vec<Option<EvidenceBinding>> = vec![None; produced.len()];
        // Items that passed validation, in produced order: `(index, fact)`.
        let mut pending: Vec<(usize, FactId)> = Vec::new();

        for (index, item) in produced.iter().enumerate() {
            let Some(grounding) = item.grounding else {
                entries[index] = Some(EvidenceBinding::ungrounded(GroundingFailure::NoFact));
                continue;
            };

            let fact = self
                .facts
                .get(&scope.workspace, &scope.snapshot, grounding.fact)
                .await?;
            let Some(fact) = fact else {
                entries[index] = Some(EvidenceBinding::ungrounded(GroundingFailure::MissingFact {
                    fact: grounding.fact,
                }));
                continue;
            };

            // The fact is the authority: a projection may *hint* at an entity,
            // but a hint that contradicts the canonical subject is refused.
            if let Some(hint) = grounding.entity
                && hint != fact.subject
            {
                entries[index] = Some(EvidenceBinding::ungrounded(
                    GroundingFailure::EntityFactMismatch {
                        entity: hint,
                        subject: Some(fact.subject),
                    },
                ));
                continue;
            }

            pending.push((index, fact.id));
        }

        // One atomic batch: either every validated item lands or none does, so
        // a failure cannot leave orphaned evidence no finding will cite.
        if !pending.is_empty() {
            let batch: Vec<NewEvidence> = pending
                .iter()
                .map(|(_, fact)| NewEvidence {
                    fact: *fact,
                    grade: EvidenceGrade::Supports,
                    provenance: provenance.clone(),
                })
                .collect();
            let ids = self
                .evidence
                .append_batch(&scope.workspace, &scope.snapshot, batch)
                .await?;
            for ((index, fact), id) in pending.into_iter().zip(ids) {
                entries[index] = Some(EvidenceBinding::grounded(id, fact));
            }
        }

        Ok(EvidenceBindings::new(
            entries
                .into_iter()
                .map(|entry| entry.expect("every index is decided above"))
                .collect(),
        ))
    }
}

/// A canonical evidence read model, loaded once and then queried synchronously.
///
/// [`load`](Self::load) performs the only I/O; [`EvidenceLookup`] is pure, so
/// [`FindingVerifier`](crate::domain::findings::FindingVerifier) stays sync.
/// A missing id is **recorded**, not raised: an id that does not resolve is a
/// fact about the world, not a store failure.
pub struct KernelEvidenceReadModel {
    scope: AnalysisScope,
    descriptors: Vec<EvidenceDescriptor>,
    provenance: HashMap<EvidenceId, ProvenanceRecord>,
}

impl KernelEvidenceReadModel {
    /// Load the canonical truth for `ids` in `scope`.
    ///
    /// Store failures are `Err`; a missing id or a missing fact is recorded in
    /// the model so the verifier can report it precisely.
    pub async fn load(
        scope: &AnalysisScope,
        ids: &[EvidenceId],
        facts: &dyn FactStore,
        evidence: &dyn EvidenceStore,
    ) -> Result<Self, KernelError> {
        let mut descriptors = Vec::with_capacity(ids.len());
        let mut provenance = HashMap::new();

        let mut seen: Vec<EvidenceId> = Vec::with_capacity(ids.len());
        for id in ids {
            if seen.contains(id) {
                continue;
            }
            seen.push(*id);

            let Some(record) = evidence.get(&scope.workspace, &scope.snapshot, *id).await? else {
                continue;
            };

            // The fact may be gone even though the evidence is present: that is
            // a dangling fact, and it must stay representable.
            let slot = match facts
                .get(&scope.workspace, &scope.snapshot, record.fact)
                .await?
            {
                Some(fact) => FactSlot::Resolved(FactDescriptor {
                    id: fact.id,
                    subject: Some(fact.subject),
                    snapshot: fact.snapshot,
                }),
                None => FactSlot::Dangling { id: record.fact },
            };

            descriptors.push(EvidenceDescriptor {
                id: record.id,
                grade: record.grade,
                fact: slot,
            });
            provenance.insert(record.id, record.provenance);
        }

        Ok(Self {
            scope: scope.clone(),
            descriptors,
            provenance,
        })
    }

    /// Number of evidence atoms that resolved.
    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    /// Whether nothing resolved.
    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }

    /// The kernel provenance of a resolved atom, for explainability.
    ///
    /// Kept out of [`EvidenceDescriptor`] on purpose: verification does not
    /// reason about provenance, and the domain port must stay ungated.
    pub fn provenance(&self, id: EvidenceId) -> Option<&ProvenanceRecord> {
        self.provenance.get(&id)
    }
}

impl EvidenceLookup for KernelEvidenceReadModel {
    fn resolve(&self, id: EvidenceId) -> EvidenceResolution<'_> {
        self.descriptors
            .iter()
            .find(|d| d.id == id)
            .map(EvidenceResolution::Known)
            .unwrap_or(EvidenceResolution::Unknown)
    }

    fn scope(&self) -> Option<&AnalysisScope> {
        Some(&self.scope)
    }
}

/// The entity a fact descriptor concerns, if it can attest one.
pub fn fact_subject(descriptor: &FactDescriptor) -> Option<EntityId> {
    descriptor.subject
}

/// The snapshot a fact descriptor belongs to.
pub fn fact_snapshot(descriptor: &FactDescriptor) -> SnapshotId {
    descriptor.snapshot
}
