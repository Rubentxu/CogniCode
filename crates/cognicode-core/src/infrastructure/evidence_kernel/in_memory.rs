//! In-memory adapters for the evidence-kernel ports (design D7).
//!
//! Facts are stored keyed by `(WorkspaceId, SnapshotId)`, so a pinned read
//! is snapshot-isolated BY CONSTRUCTION — it cannot observe rows belonging
//! to another snapshot (design D5, umbrella scenario "Historical read
//! remains stable"). Commit validation (design D6): LLM provenance,
//! snapshot mismatch, and unregistered predicates are rejected before any
//! state changes (atomic batches).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::domain::evidence_kernel::evidence::Evidence;
use crate::domain::evidence_kernel::fact::{Fact, ProducerKind};
use crate::domain::evidence_kernel::ids::{EntityId, EvidenceId, FactId, SnapshotId};
use crate::domain::evidence_kernel::ports::{
    EvidenceStore, FactStore, KernelError, SchemaError, SchemaRegistry, SnapshotStore,
};
use crate::domain::evidence_kernel::relation::{RelationKind, RelationSpec};
use crate::domain::evidence_kernel::snapshot::SnapshotDescriptor;
use crate::domain::value_objects::{RevisionId, WorkspaceId};

// ============================================================================
// InMemorySchemaRegistry
// ============================================================================

/// In-memory [`SchemaRegistry`] backed by a `BTreeMap` so `list()` is
/// deterministically ordered by kind.
#[derive(Debug, Default)]
pub struct InMemorySchemaRegistry {
    vocabulary: Mutex<BTreeMap<RelationKind, RelationSpec>>,
}

impl InMemorySchemaRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }
}

impl SchemaRegistry for InMemorySchemaRegistry {
    fn register(&self, k: RelationKind, s: RelationSpec) -> Result<(), SchemaError> {
        let mut vocabulary = self.vocabulary.lock().expect("schema registry lock");
        if vocabulary.contains_key(&k) {
            return Err(SchemaError::AlreadyRegistered(k));
        }
        vocabulary.insert(k, s);
        Ok(())
    }

    fn lookup(&self, k: &RelationKind) -> Option<RelationSpec> {
        self.vocabulary
            .lock()
            .expect("schema registry lock")
            .get(k)
            .cloned()
    }

    fn list(&self) -> Vec<(RelationKind, RelationSpec)> {
        self.vocabulary
            .lock()
            .expect("schema registry lock")
            .iter()
            .map(|(k, s)| (k.clone(), s.clone()))
            .collect()
    }
}

// ============================================================================
// InMemoryFactStore
// ============================================================================

/// In-memory [`FactStore`]: facts keyed by `(WorkspaceId, SnapshotId)`.
///
/// Snapshot isolation is structural: rows live under exactly one
/// `(workspace, snapshot)` key, so a pinned read can never observe another
/// snapshot's facts.
pub struct InMemoryFactStore {
    registry: Arc<dyn SchemaRegistry>,
    facts: Mutex<HashMap<(WorkspaceId, SnapshotId), Vec<Fact>>>,
}

impl InMemoryFactStore {
    /// Creates a store that validates predicates against `registry` at
    /// commit (design D6).
    pub fn new(registry: Arc<dyn SchemaRegistry>) -> Self {
        Self {
            registry,
            facts: Mutex::new(HashMap::new()),
        }
    }

    /// Commit-time validation, in rejection order: LLM provenance, snapshot
    /// mismatch, unregistered predicate.
    fn validate(&self, snap: &SnapshotId, fact: &Fact) -> Result<(), KernelError> {
        if fact.provenance.producer == ProducerKind::LlmAgent {
            return Err(KernelError::LlmProvenance);
        }
        if fact.snapshot != *snap {
            return Err(KernelError::SnapshotMismatch(fact.id, fact.snapshot, *snap));
        }
        if self.registry.lookup(&fact.predicate).is_none() {
            return Err(KernelError::UnregisteredPredicate(fact.predicate.clone()));
        }
        Ok(())
    }
}

#[async_trait]
impl FactStore for InMemoryFactStore {
    async fn commit(
        &self,
        ws: &WorkspaceId,
        snap: &SnapshotId,
        batch: Vec<Fact>,
    ) -> Result<Vec<FactId>, KernelError> {
        // Atomic batch: validate every fact before touching the table.
        for fact in &batch {
            self.validate(snap, fact)?;
        }
        let mut facts = self.facts.lock().expect("fact store lock");
        let rows = facts.entry((ws.clone(), *snap)).or_default();
        let ids = batch.iter().map(|f| f.id).collect();
        rows.extend(batch);
        Ok(ids)
    }

    async fn facts_of(
        &self,
        ws: &WorkspaceId,
        snap: &SnapshotId,
        subject: &EntityId,
    ) -> Result<Vec<Fact>, KernelError> {
        let facts = self.facts.lock().expect("fact store lock");
        Ok(facts
            .get(&(ws.clone(), *snap))
            .map(|rows| {
                rows.iter()
                    .filter(|f| &f.subject == subject)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn facts_in_snapshot(
        &self,
        ws: &WorkspaceId,
        snap: &SnapshotId,
    ) -> Result<Vec<Fact>, KernelError> {
        // Snapshot isolation is structural (rows live under one
        // `(workspace, snapshot)` key); the Vec preserves commit order.
        let facts = self.facts.lock().expect("fact store lock");
        Ok(facts.get(&(ws.clone(), *snap)).cloned().unwrap_or_default())
    }
}

// ============================================================================
// InMemoryEvidenceStore
// ============================================================================

/// In-memory [`EvidenceStore`]: evidence indexed by `(WorkspaceId, FactId)`.
///
/// The snapshot pin of a read is transitive through the pinned fact: a fact
/// id belongs to exactly one snapshot's fact set, so `for_fact` returns the
/// evidence attached to that fact — identical before and after other
/// snapshots are published.
#[derive(Debug, Default)]
pub struct InMemoryEvidenceStore {
    evidence: Mutex<HashMap<(WorkspaceId, FactId), Vec<Evidence>>>,
}

impl InMemoryEvidenceStore {
    /// Creates an empty evidence store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl EvidenceStore for InMemoryEvidenceStore {
    async fn add(&self, ws: &WorkspaceId, e: Evidence) -> Result<EvidenceId, KernelError> {
        let id = e.id;
        self.evidence
            .lock()
            .expect("evidence store lock")
            .entry((ws.clone(), e.fact))
            .or_default()
            .push(e);
        Ok(id)
    }

    async fn for_fact(
        &self,
        ws: &WorkspaceId,
        _snap: &SnapshotId,
        fact: FactId,
    ) -> Result<Vec<Evidence>, KernelError> {
        let evidence = self.evidence.lock().expect("evidence store lock");
        Ok(evidence
            .get(&(ws.clone(), fact))
            .cloned()
            .unwrap_or_default())
    }
}

// ============================================================================
// InMemorySnapshotStore
// ============================================================================

/// In-memory [`SnapshotStore`]: descriptors keyed by `(WorkspaceId,
/// SnapshotId)`, mapping bijectively onto the revision model (design D4).
#[derive(Debug, Default)]
pub struct InMemorySnapshotStore {
    descriptors: Mutex<HashMap<(WorkspaceId, SnapshotId), SnapshotDescriptor>>,
}

impl InMemorySnapshotStore {
    /// Creates an empty snapshot store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SnapshotStore for InMemorySnapshotStore {
    async fn from_revision(
        &self,
        ws: &WorkspaceId,
        rev: RevisionId,
    ) -> Result<SnapshotDescriptor, KernelError> {
        if !rev.is_valid() {
            return Err(KernelError::InvalidRevision(rev));
        }
        let mut descriptors = self.descriptors.lock().expect("snapshot store lock");
        let key = (ws.clone(), SnapshotId::from_revision(rev));
        // First descriptor wins: a published snapshot never mutates.
        Ok(descriptors
            .entry(key)
            .or_insert_with(|| SnapshotDescriptor::from_revision(ws.clone(), rev, "", ""))
            .clone())
    }

    async fn descriptor(
        &self,
        ws: &WorkspaceId,
        id: &SnapshotId,
    ) -> Result<SnapshotDescriptor, KernelError> {
        self.descriptors
            .lock()
            .expect("snapshot store lock")
            .get(&(ws.clone(), *id))
            .cloned()
            .ok_or_else(|| KernelError::SnapshotNotFound(*id, ws.clone()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::domain::evidence_kernel::evidence::{Evidence, EvidenceGrade};
    use crate::domain::evidence_kernel::fact::{Fact, FactValue, ProducerKind, ProvenanceRecord};
    use crate::domain::evidence_kernel::ids::{EntityId, EvidenceId, FactId, SnapshotId};
    use crate::domain::evidence_kernel::ports::{
        EvidenceStore, FactStore, KernelError, SchemaError, SchemaRegistry, SnapshotStore,
    };
    use crate::domain::evidence_kernel::relation::{RelationKind, RelationSpec};
    use crate::domain::value_objects::{Provenance, RevisionId, WorkspaceId};

    use super::*;

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    /// A registry with the `core:calls` predicate registered.
    fn registry_with_calls() -> Arc<InMemorySchemaRegistry> {
        let registry = InMemorySchemaRegistry::new();
        registry
            .register(
                RelationKind::try_new("core:calls").expect("valid kind"),
                RelationSpec::new("direct call edge"),
            )
            .expect("first registration succeeds");
        Arc::new(registry)
    }

    /// A deterministic-analyzer fact for `(subject) core:calls (object)`
    /// pinned to `snapshot`.
    fn fact(id: u64, subject: u64, object: FactValue, snapshot: u64) -> Fact {
        Fact::new(
            FactId::new(id),
            EntityId::new(subject),
            RelationKind::try_new("core:calls").expect("valid kind"),
            object,
            SnapshotId::new(snapshot),
            ProvenanceRecord::new(
                Provenance::Extracted,
                ProducerKind::DeterministicAnalyzer,
                None,
            ),
        )
        .expect("non-LLM fact")
    }

    fn ws(name: &str) -> WorkspaceId {
        WorkspaceId::try_new(name).expect("valid workspace")
    }

    // -------------------------------------------------------------------------
    // Task 4.1 RED — snapshot pinning (umbrella "Historical read remains stable")
    // -------------------------------------------------------------------------

    /// Publishing snapshot B must never change what a read pinned to A
    /// returns: same facts as before B existed, zero B facts.
    #[tokio::test]
    async fn pinned_read_is_stable_after_newer_snapshot_is_published() {
        let store = InMemoryFactStore::new(registry_with_calls());
        let workspace = ws("ws-a");
        let snap_a = SnapshotId::new(1);
        let subject = EntityId::new(100);

        store
            .commit(
                &workspace,
                &snap_a,
                vec![fact(1, 100, FactValue::Ref(EntityId::new(200)), 1)],
            )
            .await
            .expect("snapshot A commit");
        let before = store
            .facts_of(&workspace, &snap_a, &subject)
            .await
            .expect("read A");
        assert_eq!(before.len(), 1, "precondition: one fact in snapshot A");

        // Publish snapshot B with a different object for the same subject.
        let snap_b = SnapshotId::new(2);
        store
            .commit(
                &workspace,
                &snap_b,
                vec![fact(2, 100, FactValue::Ref(EntityId::new(999)), 2)],
            )
            .await
            .expect("snapshot B commit");

        // Pinned read of A after B is published: identical result, zero B facts.
        let after = store
            .facts_of(&workspace, &snap_a, &subject)
            .await
            .expect("read A again");
        assert_eq!(
            after, before,
            "pinned read of A changed after B was published"
        );
        assert!(
            after.iter().all(|f| f.snapshot == snap_a),
            "snapshot mixing detected"
        );

        let pinned_b = store
            .facts_of(&workspace, &snap_b, &subject)
            .await
            .expect("read B");
        assert_eq!(pinned_b.len(), 1);
        assert!(pinned_b.iter().all(|f| f.snapshot == snap_b));
    }

    // -------------------------------------------------------------------------
    // Task 4.3 — commit validation (design D6): LlmAgent, unregistered
    // predicates, snapshot mismatch, atomic batches
    // -------------------------------------------------------------------------

    /// The store must re-check provenance at commit: a fact built through a
    /// struct literal (bypassing `Fact::new`) with LLM provenance is still
    /// rejected (umbrella "LLM output stays a hypothesis").
    #[tokio::test]
    async fn commit_rejects_llm_agent_provenance() {
        let store = InMemoryFactStore::new(registry_with_calls());
        let workspace = ws("ws-a");
        let snap = SnapshotId::new(1);
        let smuggled = Fact {
            id: FactId::new(9),
            subject: EntityId::new(100),
            predicate: RelationKind::try_new("core:calls").expect("valid kind"),
            object: FactValue::Ref(EntityId::new(200)),
            snapshot: snap,
            provenance: ProvenanceRecord::new(Provenance::Extracted, ProducerKind::LlmAgent, None),
        };

        let err = store
            .commit(&workspace, &snap, vec![smuggled])
            .await
            .expect_err("LlmAgent provenance must be rejected at commit");
        assert!(matches!(err, KernelError::LlmProvenance));
    }

    /// A fact whose predicate is not in the registry must be rejected.
    #[tokio::test]
    async fn commit_rejects_unregistered_predicate() {
        let store = InMemoryFactStore::new(registry_with_calls());
        let workspace = ws("ws-a");
        let snap = SnapshotId::new(1);
        let unregistered = Fact::new(
            FactId::new(1),
            EntityId::new(100),
            RelationKind::try_new("semantic:summarizes").expect("valid kind"),
            FactValue::Text("summary".to_string()),
            snap,
            ProvenanceRecord::new(
                Provenance::Extracted,
                ProducerKind::DeterministicAnalyzer,
                None,
            ),
        )
        .expect("non-LLM fact");

        let err = store
            .commit(&workspace, &snap, vec![unregistered])
            .await
            .expect_err("unregistered predicate must be rejected");
        assert!(matches!(err, KernelError::UnregisteredPredicate(_)));
    }

    /// A fact whose own snapshot pin disagrees with the commit target is a
    /// contract violation and must be rejected.
    #[tokio::test]
    async fn commit_rejects_snapshot_mismatch() {
        let store = InMemoryFactStore::new(registry_with_calls());
        let workspace = ws("ws-a");
        // The fact claims snapshot 1, but the commit targets snapshot 2.
        let mismatched = fact(1, 100, FactValue::Int(1), 1);

        let err = store
            .commit(&workspace, &SnapshotId::new(2), vec![mismatched])
            .await
            .expect_err("snapshot mismatch must be rejected");
        assert!(matches!(err, KernelError::SnapshotMismatch(_, _, _)));
    }

    /// A batch with one invalid fact fails wholesale: no partial state.
    #[tokio::test]
    async fn commit_rejects_batch_atomically() {
        let store = InMemoryFactStore::new(registry_with_calls());
        let workspace = ws("ws-a");
        let snap = SnapshotId::new(1);
        let valid = fact(1, 100, FactValue::Int(1), 1);
        let invalid = Fact::new(
            FactId::new(2),
            EntityId::new(100),
            RelationKind::try_new("core:unknown").expect("valid kind"),
            FactValue::Int(2),
            snap,
            ProvenanceRecord::new(
                Provenance::Extracted,
                ProducerKind::DeterministicAnalyzer,
                None,
            ),
        )
        .expect("non-LLM fact");

        let err = store
            .commit(&workspace, &snap, vec![valid, invalid])
            .await
            .expect_err("batch with one bad fact must fail");
        assert!(matches!(err, KernelError::UnregisteredPredicate(_)));

        let remaining = store
            .facts_of(&workspace, &snap, &EntityId::new(100))
            .await
            .expect("read after failed batch");
        assert!(
            remaining.is_empty(),
            "failed batch must not leave partial state"
        );
    }

    // -------------------------------------------------------------------------
    // E37 Task 1.1 RED — facts_in_snapshot returns ONLY its snapshot's facts,
    // in commit order (design D6; consumers sort)
    // -------------------------------------------------------------------------

    /// `facts_in_snapshot` must return exactly the facts committed to the
    /// requested `(workspace, snapshot)` key, in commit order — never facts
    /// from another snapshot, and never an id-sorted view. Unknown snapshots
    /// degrade gracefully like `facts_of`.
    #[tokio::test]
    async fn facts_in_snapshot_returns_only_its_snapshots_facts_in_commit_order() {
        let store = InMemoryFactStore::new(registry_with_calls());
        let workspace = ws("ws-a");
        let snap_a = SnapshotId::new(1);
        let snap_b = SnapshotId::new(2);

        // Commit ids out of order to pin down commit-order (not id-order) reads.
        store
            .commit(
                &workspace,
                &snap_a,
                vec![
                    fact(3, 103, FactValue::Int(3), 1),
                    fact(1, 101, FactValue::Int(1), 1),
                    fact(2, 102, FactValue::Int(2), 1),
                ],
            )
            .await
            .expect("snapshot A commit");
        store
            .commit(
                &workspace,
                &snap_b,
                vec![fact(9, 109, FactValue::Int(9), 2)],
            )
            .await
            .expect("snapshot B commit");

        let snapshot_facts = store
            .facts_in_snapshot(&workspace, &snap_a)
            .await
            .expect("pinned snapshot read");
        assert_eq!(
            snapshot_facts
                .iter()
                .map(|f| f.id.get())
                .collect::<Vec<_>>(),
            vec![3, 1, 2],
            "must return exactly this snapshot's facts in commit order"
        );
        assert!(
            snapshot_facts.iter().all(|f| f.snapshot == snap_a),
            "snapshot mixing detected"
        );

        let snapshot_b_facts = store
            .facts_in_snapshot(&workspace, &snap_b)
            .await
            .expect("pinned snapshot read B");
        assert_eq!(snapshot_b_facts.len(), 1);
        assert_eq!(snapshot_b_facts[0].id, FactId::new(9));

        // Unknown snapshots degrade gracefully (facts_of precedent).
        let empty = store
            .facts_in_snapshot(&workspace, &SnapshotId::new(42))
            .await
            .expect("graceful read");
        assert!(empty.is_empty());
    }

    /// Reads degrade gracefully: an unknown subject yields an empty vector.
    #[tokio::test]
    async fn facts_of_unknown_subject_returns_empty() {
        let store = InMemoryFactStore::new(registry_with_calls());
        let empty = store
            .facts_of(&ws("ws-a"), &SnapshotId::new(1), &EntityId::new(1))
            .await
            .expect("graceful read");
        assert!(empty.is_empty());
    }

    // -------------------------------------------------------------------------
    // SchemaRegistry (design D6, sync trait)
    // -------------------------------------------------------------------------

    /// Register/lookup/list must round-trip; list is deterministic.
    #[test]
    fn schema_registry_registers_looks_up_and_lists() {
        let registry = InMemorySchemaRegistry::new();
        let calls = RelationKind::try_new("core:calls").expect("valid");
        let imports = RelationKind::try_new("core:imports").expect("valid");
        registry
            .register(calls.clone(), RelationSpec::new("call edge"))
            .expect("register calls");
        registry
            .register(imports.clone(), RelationSpec::new("import edge"))
            .expect("register imports");

        assert_eq!(
            registry.lookup(&calls),
            Some(RelationSpec::new("call edge"))
        );
        assert_eq!(
            registry.lookup(&RelationKind::try_new("core:missing").expect("valid")),
            None
        );

        let listed = registry.list();
        assert_eq!(
            listed,
            vec![
                (calls, RelationSpec::new("call edge")),
                (imports, RelationSpec::new("import edge")),
            ],
            "list must be deterministic (BTreeMap ordering)"
        );
    }

    /// The vocabulary is append-only: re-registering a predicate fails and
    /// leaves the original spec intact.
    #[test]
    fn schema_registry_rejects_duplicates() {
        let registry = InMemorySchemaRegistry::new();
        let kind = RelationKind::try_new("core:calls").expect("valid");
        registry
            .register(kind.clone(), RelationSpec::new("first"))
            .expect("first registration");

        let err = registry
            .register(kind.clone(), RelationSpec::new("second"))
            .expect_err("duplicate registration must fail");
        assert_eq!(err, SchemaError::AlreadyRegistered(kind.clone()));
        assert_eq!(registry.lookup(&kind), Some(RelationSpec::new("first")));
    }

    // -------------------------------------------------------------------------
    // SnapshotStore (design D4 facade over the revision model)
    // -------------------------------------------------------------------------

    /// `from_revision` maps `rev:N` bijectively onto `snap:N`, and the
    /// descriptor is retrievable through `descriptor`.
    #[tokio::test]
    async fn snapshot_store_maps_revision_to_descriptor() {
        let store = InMemorySnapshotStore::new();
        let workspace = ws("ws-a");
        let rev = RevisionId::new(7);

        let descriptor = store
            .from_revision(&workspace, rev)
            .await
            .expect("descriptor");
        assert_eq!(descriptor.id, SnapshotId::new(7), "rev:7 maps to snap:7");
        assert_eq!(descriptor.revision, rev);
        assert_eq!(descriptor.workspace, workspace);
        assert_eq!(
            descriptor.source_state, "",
            "reserved field stays empty in M1"
        );
        assert_eq!(
            descriptor.config_digest, "",
            "reserved field stays empty in M1"
        );

        let fetched = store
            .descriptor(&workspace, &descriptor.id)
            .await
            .expect("descriptor lookup");
        assert_eq!(fetched, descriptor);
    }

    /// The `RevisionId::NONE` sentinel has no snapshot mapping.
    #[tokio::test]
    async fn snapshot_store_rejects_none_revision() {
        let store = InMemorySnapshotStore::new();
        let err = store
            .from_revision(&ws("ws-a"), RevisionId::NONE)
            .await
            .expect_err("NONE sentinel must be rejected");
        assert!(matches!(err, KernelError::InvalidRevision(_)));
    }

    /// An unknown snapshot id yields `SnapshotNotFound`.
    #[tokio::test]
    async fn snapshot_store_unknown_descriptor_is_not_found() {
        let store = InMemorySnapshotStore::new();
        let workspace = ws("ws-a");
        let err = store
            .descriptor(&workspace, &SnapshotId::new(42))
            .await
            .expect_err("unknown snapshot must be NotFound");
        assert!(matches!(err, KernelError::SnapshotNotFound(_, _)));
    }

    /// A published snapshot is immutable: a later `from_revision` for the
    /// same `(workspace, revision)` returns the first descriptor.
    #[tokio::test]
    async fn snapshot_store_first_descriptor_is_immutable() {
        let store = InMemorySnapshotStore::new();
        let workspace = ws("ws-a");
        let rev = RevisionId::new(3);
        let first = store.from_revision(&workspace, rev).await.expect("first");
        let again = store.from_revision(&workspace, rev).await.expect("again");
        assert_eq!(again, first, "published snapshots must not mutate");
    }

    // -------------------------------------------------------------------------
    // EvidenceStore (kernel-namespaced, design D2)
    // -------------------------------------------------------------------------

    /// `add` returns the evidence id and `for_fact` retrieves the record.
    #[tokio::test]
    async fn evidence_add_and_for_fact_round_trip() {
        let store = InMemoryEvidenceStore::new();
        let workspace = ws("ws-a");
        let snap = SnapshotId::new(1);
        let fact_id = FactId::new(5);
        let evidence = Evidence {
            id: EvidenceId::new(1),
            fact: fact_id,
            grade: EvidenceGrade::Supports,
            provenance: ProvenanceRecord::new(
                Provenance::Tested,
                ProducerKind::DeterministicAnalyzer,
                None,
            ),
        };

        let added = store.add(&workspace, evidence.clone()).await.expect("add");
        assert_eq!(added, evidence.id);

        let found = store
            .for_fact(&workspace, &snap, fact_id)
            .await
            .expect("for_fact");
        assert_eq!(found, vec![evidence]);
    }

    /// Evidence attached to a fact is stable under later activity: reads
    /// pinned earlier return the same records afterwards.
    #[tokio::test]
    async fn evidence_for_fact_is_stable_across_snapshots() {
        let store = InMemoryEvidenceStore::new();
        let workspace = ws("ws-a");
        let snap_a = SnapshotId::new(1);
        let fact_id = FactId::new(5);
        let evidence = Evidence {
            id: EvidenceId::new(1),
            fact: fact_id,
            grade: EvidenceGrade::Corroborates,
            provenance: ProvenanceRecord::new(Provenance::Manual, ProducerKind::Human, None),
        };
        store.add(&workspace, evidence.clone()).await.expect("add");

        let before = store
            .for_fact(&workspace, &snap_a, fact_id)
            .await
            .expect("before later activity");
        assert_eq!(before, vec![evidence]);

        // Later activity: evidence for a different fact "in a later snapshot".
        let later = Evidence {
            id: EvidenceId::new(2),
            fact: FactId::new(6),
            grade: EvidenceGrade::Refutes,
            provenance: ProvenanceRecord::new(Provenance::Manual, ProducerKind::Human, None),
        };
        store
            .add(&workspace, later)
            .await
            .expect("add later evidence");

        let after = store
            .for_fact(&workspace, &snap_a, fact_id)
            .await
            .expect("after later activity");
        assert_eq!(
            after, before,
            "pinned evidence read changed after later activity"
        );
    }

    /// An unknown fact yields an empty evidence vector.
    #[tokio::test]
    async fn evidence_unknown_fact_returns_empty() {
        let store = InMemoryEvidenceStore::new();
        let found = store
            .for_fact(&ws("ws-a"), &SnapshotId::new(1), FactId::new(1))
            .await
            .expect("graceful read");
        assert!(found.is_empty());
    }
}
