//! Kernel facts — the canonical typed assertion unit.
//!
//! A [`Fact`] is a subject–predicate–object assertion pinned to a
//! [`SnapshotId`] and carried with a [`ProvenanceRecord`]. The record WRAPS
//! the legacy `domain::value_objects::Provenance` enum — that enum is
//! bincode-format-sensitive and MUST NOT gain variants (design D3,
//! extend-never-mutate).
//!
//! [`Fact::new`] rejects `ProducerKind::LlmAgent`: LLM output stays a
//! Hypothesis or AgentEvidence and is never persisted as an extracted Fact
//! (umbrella scenario "LLM output stays a hypothesis").

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::domain::value_objects::Provenance;

use super::ids::{EntityId, FactId, SnapshotId};
use super::relation::RelationKind;

/// Typed value of a fact's object position.
///
/// Deliberately `PartialEq` but not `Eq`/`Hash`: the `Float` variant makes
/// total equality/hash impossible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FactValue {
    /// Free-text value.
    Text(String),
    /// Integral value.
    Int(i64),
    /// Floating-point value.
    Float(f64),
    /// Boolean value.
    Bool(bool),
    /// Reference to another entity in the same kernel.
    Ref(EntityId),
}

/// Who or what produced a kernel artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProducerKind {
    /// A deterministic adapter (e.g. a format importer).
    DeterministicAdapter,
    /// A deterministic analyzer (e.g. a tree-sitter extractor).
    DeterministicAnalyzer,
    /// A runtime observer (e.g. a tracer or profiler).
    RuntimeObserver,
    /// An LLM agent. NEVER valid for an extracted [`Fact`] (design D3 /
    /// ADR-040); legitimate for hypotheses and agent evidence.
    LlmAgent,
    /// A human curator.
    Human,
}

impl fmt::Display for ProducerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ProducerKind::DeterministicAdapter => "DeterministicAdapter",
            ProducerKind::DeterministicAnalyzer => "DeterministicAnalyzer",
            ProducerKind::RuntimeObserver => "RuntimeObserver",
            ProducerKind::LlmAgent => "LlmAgent",
            ProducerKind::Human => "Human",
        };
        f.write_str(s)
    }
}

/// Provenance for kernel artifacts — composition over the legacy
/// `Provenance` enum (design D3, extend-never-mutate).
///
/// The legacy enum is bincode-format-sensitive: this record adds producer
/// and detail WITHOUT adding variants to it, so existing blobs stay
/// loadable and the class identity round-trips untouched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceRecord {
    /// Legacy provenance class (the unchanged `Provenance` enum).
    pub class: Provenance,
    /// Which kind of producer created the artifact.
    pub producer: ProducerKind,
    /// Optional free-form detail (e.g. extractor version, source location).
    pub detail: Option<String>,
}

impl ProvenanceRecord {
    /// Constructs a provenance record.
    pub fn new(class: Provenance, producer: ProducerKind, detail: Option<String>) -> Self {
        Self {
            class,
            producer,
            detail,
        }
    }
}

/// Error returned by [`Fact::new`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FactError {
    /// LLM-agent output cannot back an extracted fact.
    #[error(
        "LLM-agent output cannot back an extracted Fact; it stays a Hypothesis or AgentEvidence"
    )]
    LlmProvenance,
}

/// One canonical assertion: subject −predicate→ object, pinned to a snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fact {
    /// Producer-assigned fact id.
    pub id: FactId,
    /// The entity the assertion is about.
    pub subject: EntityId,
    /// The namespaced relation predicate.
    pub predicate: RelationKind,
    /// The typed object value.
    pub object: FactValue,
    /// The snapshot this assertion belongs to (design D5 pinning).
    pub snapshot: SnapshotId,
    /// How this fact was obtained.
    pub provenance: ProvenanceRecord,
}

impl Fact {
    /// Constructs a fact, rejecting LLM-agent provenance (design D3 /
    /// ADR-040: LLM claims stay Hypothesis or AgentEvidence).
    pub fn new(
        id: FactId,
        subject: EntityId,
        predicate: RelationKind,
        object: FactValue,
        snapshot: SnapshotId,
        provenance: ProvenanceRecord,
    ) -> Result<Self, FactError> {
        if provenance.producer == ProducerKind::LlmAgent {
            return Err(FactError::LlmProvenance);
        }
        Ok(Self {
            id,
            subject,
            predicate,
            object,
            snapshot,
            provenance,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::value_objects::Provenance;

    use super::super::ids::{EntityId, FactId, SnapshotId};
    use super::super::relation::RelationKind;
    use super::*;

    // -------------------------------------------------------------------------
    // Task 3.1 RED — exhaustive FactValue round-trip (JSON + bincode)
    // -------------------------------------------------------------------------

    /// Every `FactValue` variant must round-trip losslessly through both
    /// JSON and bincode (house pattern: exhaustive-variant loop, no proptest).
    #[test]
    fn fact_value_round_trip_exhaustive() {
        let values = [
            FactValue::Text("hello".to_string()),
            FactValue::Int(-42),
            FactValue::Float(3.5),
            FactValue::Bool(true),
            FactValue::Ref(EntityId::new(17)),
        ];
        for value in values {
            let json = serde_json::to_string(&value).expect("json serialize");
            let parsed: FactValue = serde_json::from_str(&json).expect("json deserialize");
            assert_eq!(parsed, value, "json round-trip lost identity for {value:?}");

            let bytes =
                bincode::serde::encode_to_vec(&value, bincode::config::standard()).expect("encode");
            let (decoded, _): (FactValue, usize) =
                bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                    .expect("decode");
            assert_eq!(
                decoded, value,
                "bincode round-trip lost identity for {value:?}"
            );
        }
    }

    // -------------------------------------------------------------------------
    // Task 3.1 RED — exhaustive ProvenanceRecord round-trip (design D3)
    // -------------------------------------------------------------------------

    /// Every (legacy class × producer × detail) combination must round-trip
    /// losslessly through bincode (the format-sensitive path, design D3) and
    /// JSON. The legacy `Provenance` enum is exercised over ALL its variants
    /// — the record extends it by composition, never by adding variants.
    #[test]
    fn provenance_record_round_trip_exhaustive() {
        let classes = [
            Provenance::Extracted,
            Provenance::Inferred,
            Provenance::Ambiguous,
            Provenance::Manual,
            Provenance::Tested,
        ];
        let producers = [
            ProducerKind::DeterministicAdapter,
            ProducerKind::DeterministicAnalyzer,
            ProducerKind::RuntimeObserver,
            ProducerKind::LlmAgent,
            ProducerKind::Human,
        ];
        for class in classes {
            for producer in producers {
                for detail in [None, Some("extractor v1.2".to_string())] {
                    let record = ProvenanceRecord::new(class, producer, detail);

                    let bytes = bincode::serde::encode_to_vec(&record, bincode::config::standard())
                        .expect("encode");
                    let (decoded, _): (ProvenanceRecord, usize) =
                        bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                            .expect("decode");
                    assert_eq!(decoded.class, record.class, "bincode lost provenance class");
                    assert_eq!(decoded.producer, record.producer, "bincode lost producer");
                    assert_eq!(decoded.detail, record.detail, "bincode lost detail");

                    let json = serde_json::to_string(&record).expect("json serialize");
                    let parsed: ProvenanceRecord =
                        serde_json::from_str(&json).expect("json deserialize");
                    assert_eq!(parsed, record, "json round-trip lost identity");
                }
            }
        }
    }

    /// A `ProvenanceRecord` MAY carry `ProducerKind::LlmAgent` — LLM output
    /// is legitimate for hypotheses and agent evidence. The exclusion is
    /// enforced at fact construction, not at provenance construction.
    #[test]
    fn provenance_record_allows_llm_agent_for_non_fact_use() {
        let record = ProvenanceRecord::new(Provenance::Manual, ProducerKind::LlmAgent, None);
        assert_eq!(record.class, Provenance::Manual);
        assert_eq!(record.producer, ProducerKind::LlmAgent);
    }

    // -------------------------------------------------------------------------
    // Task 3.1 RED — LlmAgent rejection (umbrella "LLM output stays a hypothesis")
    // -------------------------------------------------------------------------

    /// `Fact::new` MUST reject `ProducerKind::LlmAgent`: an LLM claim is
    /// stored as Hypothesis or AgentEvidence, never as an extracted Fact.
    #[test]
    fn fact_new_rejects_llm_agent_producer() {
        let err = Fact::new(
            FactId::new(1),
            EntityId::new(100),
            RelationKind::try_new("core:calls").expect("valid"),
            FactValue::Ref(EntityId::new(200)),
            SnapshotId::new(1),
            ProvenanceRecord::new(Provenance::Extracted, ProducerKind::LlmAgent, None),
        )
        .expect_err("LlmAgent provenance must be rejected");
        assert_eq!(err, FactError::LlmProvenance);
    }

    /// Every non-LLM producer kind must be accepted by `Fact::new`.
    #[test]
    fn fact_new_accepts_non_llm_producers() {
        for producer in [
            ProducerKind::DeterministicAdapter,
            ProducerKind::DeterministicAnalyzer,
            ProducerKind::RuntimeObserver,
            ProducerKind::Human,
        ] {
            let fact = Fact::new(
                FactId::new(1),
                EntityId::new(100),
                RelationKind::try_new("core:calls").expect("valid"),
                FactValue::Ref(EntityId::new(200)),
                SnapshotId::new(1),
                ProvenanceRecord::new(Provenance::Extracted, producer, None),
            )
            .expect("non-LLM producer must be accepted");
            assert_eq!(fact.provenance.producer, producer);
        }
    }

    // -------------------------------------------------------------------------
    // Task 3.1 RED — Fact round-trip preserves provenance
    // (umbrella scenario "Fact round-trip preserves provenance")
    // -------------------------------------------------------------------------

    /// After serialize + deserialize, subject, predicate, object, snapshot,
    /// producer and provenance class must be identical — exercised across
    /// every legacy provenance class crossed with every `FactValue` variant,
    /// through both bincode and JSON.
    #[test]
    fn fact_round_trip_preserves_everything() {
        let classes = [
            Provenance::Extracted,
            Provenance::Inferred,
            Provenance::Ambiguous,
            Provenance::Manual,
            Provenance::Tested,
        ];
        let objects = [
            FactValue::Text("fn main".to_string()),
            FactValue::Int(7),
            FactValue::Float(-1.5),
            FactValue::Bool(false),
            FactValue::Ref(EntityId::new(300)),
        ];
        for (i, class) in classes.into_iter().enumerate() {
            let object = objects[i].clone();
            let producer = [
                ProducerKind::DeterministicAdapter,
                ProducerKind::DeterministicAnalyzer,
                ProducerKind::RuntimeObserver,
                ProducerKind::Human,
                ProducerKind::DeterministicAnalyzer,
            ][i];
            let fact = Fact::new(
                FactId::new(i as u64 + 1),
                EntityId::new(100 + i as u64),
                RelationKind::try_new("core:calls").expect("valid"),
                object,
                SnapshotId::new(i as u64 + 1),
                ProvenanceRecord::new(class, producer, Some("src/main.rs".to_string())),
            )
            .expect("non-LLM fact");

            let bytes =
                bincode::serde::encode_to_vec(&fact, bincode::config::standard()).expect("encode");
            let (decoded, _): (Fact, usize) =
                bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                    .expect("decode");
            assert_eq!(decoded.id, fact.id, "bincode lost fact id");
            assert_eq!(decoded.subject, fact.subject, "bincode lost subject");
            assert_eq!(decoded.predicate, fact.predicate, "bincode lost predicate");
            assert_eq!(decoded.object, fact.object, "bincode lost object");
            assert_eq!(decoded.snapshot, fact.snapshot, "bincode lost snapshot");
            assert_eq!(
                decoded.provenance.class, fact.provenance.class,
                "bincode lost provenance class"
            );
            assert_eq!(
                decoded.provenance.producer, fact.provenance.producer,
                "bincode lost producer"
            );
            assert_eq!(decoded, fact, "bincode round-trip lost full identity");

            let json = serde_json::to_string(&fact).expect("json serialize");
            let parsed: Fact = serde_json::from_str(&json).expect("json deserialize");
            assert_eq!(parsed, fact, "json round-trip lost full identity");
        }
    }
}
