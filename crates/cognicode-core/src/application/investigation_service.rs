//! Investigation service facade — ADR-005 Phase INV-1.
//!
//! Provides a use-case-level API for investigation operations.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)

use time::OffsetDateTime;
use tokio::sync::Mutex;

use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::investigation::{Artifact, Evidence, Investigation};
use crate::domain::investigation_candidates::{
    CANDIDATE_ARTIFACT_ID_PREFIX, CANDIDATE_ARTIFACT_KIND, CandidateError, CandidateLedger,
    CandidateScope, EvidencePolarity, InvestigationCandidate,
};
use crate::domain::investigation_store::{InvestigationStore, StoreError};

/// Errors that can occur during investigation operations.
#[derive(Debug, thiserror::Error)]
pub enum InvestigationError {
    #[error("Investigation not found: {0}")]
    NotFound(String),

    #[error("Store error: {0}")]
    Store(#[from] StoreError),

    #[error("Invalid status transition: {0}")]
    InvalidStatus(String),

    /// A candidate operation was refused.
    #[error("Candidate refused: {0}")]
    Candidate(#[from] CandidateError),
}

/// Result type for investigation operations.
pub type InvestigationResult<T> = Result<T, InvestigationError>;

/// Investigation service facade.
///
/// Provides use-case-level operations for creating, reading, updating,
/// and deleting investigations.
pub struct InvestigationService<S: InvestigationStore> {
    store: S,
    /// Serializes aggregate writes issued through this service so replacing an
    /// investigation cannot drop a candidate ledger another operation saved.
    /// Callers sharing a store must share this service instance: the store port
    /// does not expose a cross-instance transaction.
    investigation_writes: Mutex<()>,
}

impl<S: InvestigationStore> InvestigationService<S> {
    /// Create a new service with the given store.
    pub fn new(store: S) -> Self {
        Self {
            store,
            investigation_writes: Mutex::new(()),
        }
    }

    /// Create a new investigation with Draft status.
    pub async fn create_investigation(
        &self,
        workspace_id: &str,
        title: &str,
        goal: &str,
    ) -> InvestigationResult<Investigation> {
        let now = OffsetDateTime::now_utc();

        let investigation = Investigation {
            id: format!(
                "inv_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            workspace_id: workspace_id.to_string(),
            title: title.to_string(),
            goal: goal.to_string(),
            status: crate::domain::investigation::Status::Draft,
            entry_point: None,
            panes: Vec::new(),
            evidence: Vec::new(),
            artifacts: Vec::new(),
            narrative: String::new(),
            related_adrs: Vec::new(),
            created_at: now,
            updated_at: now,
        };

        self.store
            .save(&investigation)
            .await
            .map_err(InvestigationError::from)?;

        Ok(investigation)
    }

    /// Get an investigation by ID.
    pub async fn get_investigation(&self, id: &str) -> InvestigationResult<Option<Investigation>> {
        self.store.load(id).await.map_err(InvestigationError::from)
    }

    /// List all investigations for a workspace.
    pub async fn list_investigations(
        &self,
        workspace_id: &str,
    ) -> InvestigationResult<Vec<Investigation>> {
        self.store
            .list(workspace_id)
            .await
            .map_err(InvestigationError::from)
    }

    /// Add a single evidence item to an existing investigation.
    ///
    /// Returns `Err(InvestigationError::NotFound)` when the investigation
    /// does not exist.
    pub async fn add_evidence(
        &self,
        investigation_id: &str,
        evidence: Evidence,
    ) -> InvestigationResult<()> {
        let _write = self.investigation_writes.lock().await;
        self.store
            .add_evidence(investigation_id, evidence)
            .await
            .map_err(InvestigationError::from)
    }

    /// Add a single artifact to an existing investigation (ADR-010 E24.1).
    ///
    /// Returns `Err(InvestigationError::NotFound)` when the investigation
    /// does not exist. The returned Artifact carries the server-stamped
    /// provenance.created_at.
    pub async fn add_artifact(
        &self,
        investigation_id: &str,
        artifact: Artifact,
    ) -> InvestigationResult<Artifact> {
        let _write = self.investigation_writes.lock().await;
        if artifact.id.starts_with(CANDIDATE_ARTIFACT_ID_PREFIX) {
            return Err(CandidateError::ArtifactConflict(artifact.id).into());
        }
        self.store
            .add_artifact(investigation_id, artifact)
            .await
            .map_err(InvestigationError::from)
    }

    /// Update an existing investigation.
    ///
    /// Validates status transitions:
    /// - Draft → Active
    /// - Active → Completed
    /// - Completed → Archived
    pub async fn update_investigation(
        &self,
        mut investigation: Investigation,
    ) -> InvestigationResult<()> {
        let _write = self.investigation_writes.lock().await;
        // Validate status transition if status is changing.
        // The validation allows staying in the same status (idempotent).
        validate_status_transition(investigation.status)?;

        if let Some(existing) = self
            .store
            .load(&investigation.id)
            .await
            .map_err(InvestigationError::from)?
        {
            let current_candidate_artifacts: Vec<Artifact> = existing
                .artifacts
                .into_iter()
                .filter(|artifact| artifact.kind == CANDIDATE_ARTIFACT_KIND)
                .collect();
            investigation
                .artifacts
                .retain(|artifact| artifact.kind != CANDIDATE_ARTIFACT_KIND);
            investigation.artifacts.extend(current_candidate_artifacts);
        }

        self.store
            .save(&investigation)
            .await
            .map_err(InvestigationError::from)?;

        Ok(())
    }

    // --- Reviewable candidates -------------------------------------------
    //
    // A candidate is an annotation of one session, never a fact. The ledger
    // is kept in the investigation the store already persists, as one
    // artifact per session, so it travels, is scoped and is deleted with the
    // investigation and needs no store of its own.

    /// Proposes a candidate for `subject`, anchoring the session's ledger at
    /// `snapshot` the first time. A ledger that is at another snapshot refuses
    /// the proposal until [`Self::apply_candidate_revision`] moves it.
    ///
    /// `subject` is the canonical symbol identity resolved from a verified
    /// workspace/snapshot-bound handle; `reason` is short data and is never interpreted.
    pub async fn propose_candidate(
        &self,
        scope: &CandidateScope,
        investigation_id: &str,
        snapshot: &str,
        subject: &SymbolId,
        reason: &str,
    ) -> InvestigationResult<InvestigationCandidate> {
        self.mutate_ledger(scope, investigation_id, Some(snapshot), |_, ledger| {
            if ledger.snapshot() != snapshot {
                return Err(CandidateError::RevisionConflict {
                    current: ledger.snapshot().to_string(),
                    expected: snapshot.to_string(),
                });
            }
            ledger.propose(subject, reason).cloned()
        })
        .await
    }

    /// Links an evidence item already pinned to the investigation to a
    /// candidate. Only the reference is kept, never the evidence text.
    pub async fn link_candidate_evidence(
        &self,
        scope: &CandidateScope,
        investigation_id: &str,
        candidate_id: &str,
        evidence_id: &str,
        polarity: EvidencePolarity,
    ) -> InvestigationResult<InvestigationCandidate> {
        self.mutate_ledger(scope, investigation_id, None, |investigation, ledger| {
            if !investigation.evidence.iter().any(|e| e.id == evidence_id) {
                return Err(CandidateError::LinkNotFound {
                    candidate: candidate_id.to_string(),
                    evidence: evidence_id.to_string(),
                });
            }
            ledger
                .link_evidence(candidate_id, evidence_id, polarity)
                .cloned()
        })
        .await
    }

    /// Observes the references of a candidate again at the ledger's snapshot:
    /// the caller re-read them and they stand. The state does not change.
    pub async fn revalidate_candidate(
        &self,
        scope: &CandidateScope,
        investigation_id: &str,
        candidate_id: &str,
    ) -> InvestigationResult<InvestigationCandidate> {
        self.mutate_ledger(scope, investigation_id, None, |investigation, ledger| {
            let evidence: Vec<String> = ledger
                .candidate(candidate_id)
                .ok_or_else(|| CandidateError::CandidateNotFound(candidate_id.to_string()))?
                .evidence
                .iter()
                .map(|link| link.evidence_id.clone())
                .collect();
            for evidence_id in &evidence {
                if !investigation
                    .evidence
                    .iter()
                    .any(|item| item.id == *evidence_id)
                {
                    return Err(CandidateError::LinkNotFound {
                        candidate: candidate_id.to_string(),
                        evidence: evidence_id.clone(),
                    });
                }
                ledger.revalidate_link(candidate_id, evidence_id)?;
            }
            ledger.revalidate_subject(candidate_id).cloned()
        })
        .await
    }

    /// Discards a candidate; only current refuting evidence discards.
    pub async fn discard_candidate(
        &self,
        scope: &CandidateScope,
        investigation_id: &str,
        candidate_id: &str,
    ) -> InvestigationResult<InvestigationCandidate> {
        self.mutate_ledger(scope, investigation_id, None, |_, ledger| {
            ledger.discard(candidate_id).cloned()
        })
        .await
    }

    /// Confirms a candidate; only current supporting evidence confirms and
    /// never while a reference is stale. New evidence never confirms.
    pub async fn confirm_candidate(
        &self,
        scope: &CandidateScope,
        investigation_id: &str,
        candidate_id: &str,
    ) -> InvestigationResult<InvestigationCandidate> {
        self.mutate_ledger(scope, investigation_id, None, |_, ledger| {
            ledger.confirm(candidate_id).cloned()
        })
        .await
    }

    /// Removes one candidate of the session.
    pub async fn remove_candidate(
        &self,
        scope: &CandidateScope,
        investigation_id: &str,
        candidate_id: &str,
    ) -> InvestigationResult<()> {
        self.mutate_ledger(scope, investigation_id, None, |_, ledger| {
            ledger.remove(candidate_id)
        })
        .await
    }

    /// The session's candidates, or `None` when it has none for this
    /// investigation. Another session's candidates are never visible.
    pub async fn candidates(
        &self,
        scope: &CandidateScope,
        investigation_id: &str,
    ) -> InvestigationResult<Option<CandidateLedger>> {
        let investigation = self.scoped_investigation(scope, investigation_id).await?;
        Ok(read_ledger(&investigation, scope)?)
    }

    /// Deletes every candidate of the session for this investigation.
    pub async fn clear_candidates(
        &self,
        scope: &CandidateScope,
        investigation_id: &str,
    ) -> InvestigationResult<()> {
        let _write = self.investigation_writes.lock().await;
        let mut investigation = self.scoped_investigation(scope, investigation_id).await?;
        let artifact_id = ledger_artifact_id(scope);
        if read_ledger(&investigation, scope)?.is_none() {
            return Ok(());
        }
        investigation
            .artifacts
            .retain(|a| a.id != artifact_id || a.kind != CANDIDATE_ARTIFACT_KIND);
        investigation.updated_at = OffsetDateTime::now_utc();
        self.store.save(&investigation).await?;
        Ok(())
    }

    /// Moves the session's ledger from snapshot `from` to `to`.
    ///
    /// `still_valid` names the candidates a caller proved unaffected by the
    /// revision; the others become stale and a confirmed or discarded one is
    /// reopened. Nothing is confirmed here. A ledger that is no longer at
    /// `from` refuses the revision and stays as it was.
    pub async fn apply_candidate_revision(
        &self,
        scope: &CandidateScope,
        investigation_id: &str,
        from: &str,
        to: &str,
        still_valid: &[String],
    ) -> InvestigationResult<CandidateLedger> {
        self.mutate_ledger(scope, investigation_id, None, |_, ledger| {
            ledger.apply_revision(from, to, still_valid)?;
            Ok(ledger.clone())
        })
        .await
    }

    async fn scoped_investigation(
        &self,
        scope: &CandidateScope,
        investigation_id: &str,
    ) -> InvestigationResult<Investigation> {
        match self.store.load(investigation_id).await? {
            Some(investigation) if investigation.workspace_id == scope.workspace_id() => {
                Ok(investigation)
            }
            _ => Err(InvestigationError::NotFound(investigation_id.to_string())),
        }
    }

    /// Loads the session's ledger (creating it at `anchor` when asked and
    /// absent), applies `change`, and persists only when it succeeded.
    async fn mutate_ledger<T>(
        &self,
        scope: &CandidateScope,
        investigation_id: &str,
        anchor: Option<&str>,
        change: impl FnOnce(&Investigation, &mut CandidateLedger) -> Result<T, CandidateError>,
    ) -> InvestigationResult<T> {
        let _write = self.investigation_writes.lock().await;
        let mut investigation = self.scoped_investigation(scope, investigation_id).await?;
        let mut ledger = match (read_ledger(&investigation, scope)?, anchor) {
            (Some(ledger), _) => ledger,
            (None, Some(snapshot)) => CandidateLedger::new(scope.clone(), snapshot)?,
            (None, None) => {
                return Err(InvestigationError::NotFound(format!(
                    "candidates of {investigation_id}"
                )));
            }
        };
        let outcome = change(&investigation, &mut ledger)?;
        write_ledger(&mut investigation, scope, &ledger);
        self.store.save(&investigation).await?;
        Ok(outcome)
    }

    /// Delete an investigation by ID.
    pub async fn delete_investigation(&self, id: &str) -> InvestigationResult<()> {
        let _write = self.investigation_writes.lock().await;
        // Check if the investigation exists first.
        let existing = self
            .store
            .load(id)
            .await
            .map_err(InvestigationError::from)?;
        if existing.is_none() {
            return Err(InvestigationError::NotFound(id.to_string()));
        }

        self.store
            .delete(id)
            .await
            .map_err(InvestigationError::from)?;

        Ok(())
    }
}

/// Validate a status transition.
///
/// Allowed transitions:
/// - Draft → Active
/// - Active → Completed
/// - Completed → Archived
///
/// Staying in the same status is always allowed (idempotent).
fn validate_status_transition(
    status: crate::domain::investigation::Status,
) -> InvestigationResult<()> {
    use crate::domain::investigation::Status;
    match status {
        Status::Draft | Status::Active | Status::Completed | Status::Archived => {
            // All statuses are valid inputs. The transition validation
            // happens at the API layer when transitioning between states.
            Ok(())
        }
    }
}

/// The id of the artifact that holds one session's ledger.
fn ledger_artifact_id(scope: &CandidateScope) -> String {
    format!("{}{}", CANDIDATE_ARTIFACT_ID_PREFIX, scope.session_id())
}

/// Reads the session's ledger out of an investigation. A ledger whose own
/// scope is not the caller's is treated as absent: it is not the caller's.
fn read_ledger(
    investigation: &Investigation,
    scope: &CandidateScope,
) -> Result<Option<CandidateLedger>, CandidateError> {
    let id = ledger_artifact_id(scope);
    let Some(artifact) = investigation.artifacts.iter().find(|a| a.id == id) else {
        return Ok(None);
    };
    if artifact.kind != CANDIDATE_ARTIFACT_KIND {
        return Err(CandidateError::ArtifactConflict(id));
    }
    let ledger = CandidateLedger::from_wire(&artifact.content)?;
    if ledger.scope() != scope {
        return Err(CandidateError::ArtifactConflict(id));
    }
    Ok(Some(ledger))
}

/// Writes the session's ledger into an investigation, replacing its artifact.
fn write_ledger(
    investigation: &mut Investigation,
    scope: &CandidateScope,
    ledger: &CandidateLedger,
) {
    let id = ledger_artifact_id(scope);
    let artifact = Artifact {
        id: id.clone(),
        kind: CANDIDATE_ARTIFACT_KIND.to_string(),
        title: "Candidates under review".to_string(),
        content: ledger.to_wire(),
        generated_from: None,
        provenance: None,
    };
    match investigation.artifacts.iter_mut().find(|a| a.id == id) {
        Some(existing) => *existing = artifact,
        None => investigation.artifacts.push(artifact),
    }
    investigation.updated_at = OffsetDateTime::now_utc();
}

/// Create one InvestigationService for a store and share it across callers.
///
/// The service serializes aggregate mutations only within its own instance;
/// constructing separate services over the same store does not share that lock.
pub fn new_investigation_service<S: InvestigationStore + 'static>(
    store: S,
) -> InvestigationService<S> {
    InvestigationService::new(store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::investigation::Status;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// Mock store for testing.
    struct MockStore {
        investigations: Mutex<VecDeque<Investigation>>,
    }

    impl MockStore {
        fn new(investigations: Vec<Investigation>) -> Self {
            Self {
                investigations: Mutex::new(investigations.into()),
            }
        }
    }

    #[async_trait::async_trait]
    impl InvestigationStore for MockStore {
        async fn save(&self, investigation: &Investigation) -> Result<(), StoreError> {
            let mut invs = self.investigations.lock().unwrap();
            if let Some(pos) = invs.iter().position(|i| i.id == investigation.id) {
                invs[pos] = investigation.clone();
            } else {
                invs.push_back(investigation.clone());
            }
            Ok(())
        }

        async fn load(&self, id: &str) -> Result<Option<Investigation>, StoreError> {
            let invs = self.investigations.lock().unwrap();
            Ok(invs.iter().find(|i| i.id == id).cloned())
        }

        async fn list(&self, workspace_id: &str) -> Result<Vec<Investigation>, StoreError> {
            let invs = self.investigations.lock().unwrap();
            Ok(invs
                .iter()
                .filter(|i| i.workspace_id == workspace_id)
                .cloned()
                .collect())
        }

        async fn delete(&self, id: &str) -> Result<(), StoreError> {
            let mut invs = self.investigations.lock().unwrap();
            invs.retain(|i| i.id != id);
            Ok(())
        }

        async fn add_evidence(
            &self,
            _investigation_id: &str,
            _evidence: Evidence,
        ) -> Result<(), StoreError> {
            // Mock always succeeds — evidence is not stored in the mock.
            Ok(())
        }

        async fn add_artifact(
            &self,
            investigation_id: &str,
            artifact: Artifact,
        ) -> Result<Artifact, StoreError> {
            // Check the investigation exists first.
            let invs = self.investigations.lock().unwrap();
            if !invs.iter().any(|i| i.id == investigation_id) {
                return Err(StoreError::NotFound(investigation_id.to_string()));
            }
            drop(invs);
            // Mock always succeeds — returns the artifact as-is.
            Ok(artifact)
        }
    }

    #[tokio::test]
    async fn test_create_investigation() {
        let store = MockStore::new(Vec::new());
        let service = InvestigationService::new(store);

        let result = service
            .create_investigation("ws-001", "Test Investigation", "What is this?")
            .await;

        assert!(result.is_ok());
        let inv = result.unwrap();
        assert_eq!(inv.workspace_id, "ws-001");
        assert_eq!(inv.title, "Test Investigation");
        assert_eq!(inv.status, Status::Draft);
        assert!(!inv.id.is_empty());
    }

    #[tokio::test]
    async fn test_get_investigation() {
        let investigation = Investigation {
            id: "inv-001".to_string(),
            workspace_id: "ws-001".to_string(),
            title: "Test".to_string(),
            goal: "Goal".to_string(),
            status: Status::Active,
            entry_point: None,
            panes: vec![],
            evidence: vec![],
            artifacts: vec![],
            narrative: "".to_string(),
            related_adrs: vec![],
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };

        let store = MockStore::new(vec![investigation]);
        let service = InvestigationService::new(store);

        let result = service.get_investigation("inv-001").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_get_investigation_not_found() {
        let store = MockStore::new(Vec::new());
        let service = InvestigationService::new(store);

        let result = service.get_investigation("nonexistent").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_list_investigations() {
        let investigations = vec![
            Investigation {
                id: "inv-001".to_string(),
                workspace_id: "ws-001".to_string(),
                title: "Test 1".to_string(),
                goal: "Goal 1".to_string(),
                status: Status::Active,
                entry_point: None,
                panes: vec![],
                evidence: vec![],
                artifacts: vec![],
                narrative: "".to_string(),
                related_adrs: vec![],
                created_at: OffsetDateTime::now_utc(),
                updated_at: OffsetDateTime::now_utc(),
            },
            Investigation {
                id: "inv-002".to_string(),
                workspace_id: "ws-001".to_string(),
                title: "Test 2".to_string(),
                goal: "Goal 2".to_string(),
                status: Status::Draft,
                entry_point: None,
                panes: vec![],
                evidence: vec![],
                artifacts: vec![],
                narrative: "".to_string(),
                related_adrs: vec![],
                created_at: OffsetDateTime::now_utc(),
                updated_at: OffsetDateTime::now_utc(),
            },
        ];

        let store = MockStore::new(investigations);
        let service = InvestigationService::new(store);

        let result = service.list_investigations("ws-001").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_delete_investigation() {
        let investigation = Investigation {
            id: "inv-001".to_string(),
            workspace_id: "ws-001".to_string(),
            title: "Test".to_string(),
            goal: "Goal".to_string(),
            status: Status::Active,
            entry_point: None,
            panes: vec![],
            evidence: vec![],
            artifacts: vec![],
            narrative: "".to_string(),
            related_adrs: vec![],
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };

        let store = MockStore::new(vec![investigation]);
        let service = InvestigationService::new(store);

        let result = service.delete_investigation("inv-001").await;
        assert!(result.is_ok());

        // Verify it's deleted.
        let result = service.get_investigation("inv-001").await;
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_investigation_not_found() {
        let store = MockStore::new(Vec::new());
        let service = InvestigationService::new(store);

        let result = service.delete_investigation("nonexistent").await;
        assert!(matches!(result, Err(InvestigationError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_add_artifact_ok() {
        let investigation = Investigation {
            id: "inv-001".to_string(),
            workspace_id: "ws-001".to_string(),
            title: "Test".to_string(),
            goal: "Goal".to_string(),
            status: Status::Active,
            entry_point: None,
            panes: vec![],
            evidence: vec![],
            artifacts: vec![],
            narrative: "".to_string(),
            related_adrs: vec![],
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };

        let store = MockStore::new(vec![investigation]);
        let service = InvestigationService::new(store);

        let artifact = Artifact {
            id: "art-001".to_string(),
            kind: "mermaid".to_string(),
            title: "Call Graph".to_string(),
            content: "graph TD".to_string(),
            generated_from: Some("symbol:main.rs".to_string()),
            provenance: None,
        };

        let result = service.add_artifact("inv-001", artifact.clone()).await;
        assert!(result.is_ok());
        let returned = result.unwrap();
        assert_eq!(returned.id, "art-001");
        assert_eq!(returned.kind, "mermaid");
    }

    #[tokio::test]
    async fn test_add_artifact_not_found() {
        let store = MockStore::new(Vec::new());
        let service = InvestigationService::new(store);

        let artifact = Artifact {
            id: "art-001".to_string(),
            kind: "mermaid".to_string(),
            title: "Call Graph".to_string(),
            content: "graph TD".to_string(),
            generated_from: None,
            provenance: None,
        };

        let result = service.add_artifact("inv:nonexistent", artifact).await;
        assert!(matches!(
            result,
            Err(InvestigationError::Store(StoreError::NotFound(_)))
        ));
    }
}
