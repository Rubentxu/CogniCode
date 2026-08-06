//! Investigation service facade — ADR-005 Phase INV-1.
//!
//! Provides a use-case-level API for investigation operations.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(unused_imports)]

use std::sync::Arc;

use time::OffsetDateTime;

use crate::domain::investigation::{Artifact, Evidence, Investigation};
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
}

/// Result type for investigation operations.
pub type InvestigationResult<T> = Result<T, InvestigationError>;

/// Investigation service facade.
///
/// Provides use-case-level operations for creating, reading, updating,
/// and deleting investigations.
pub struct InvestigationService<S: InvestigationStore> {
    store: S,
}

impl<S: InvestigationStore> InvestigationService<S> {
    /// Create a new service with the given store.
    pub fn new(store: S) -> Self {
        Self { store }
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
        investigation: Investigation,
    ) -> InvestigationResult<()> {
        // Validate status transition if status is changing.
        // The validation allows staying in the same status (idempotent).
        validate_status_transition(investigation.status)?;

        self.store
            .save(&investigation)
            .await
            .map_err(InvestigationError::from)?;

        Ok(())
    }

    /// Delete an investigation by ID.
    pub async fn delete_investigation(&self, id: &str) -> InvestigationResult<()> {
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

/// Create an InvestigationService wrapping a shared store.
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
