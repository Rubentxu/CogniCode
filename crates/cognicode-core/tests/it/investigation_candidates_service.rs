use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use cognicode_core::application::investigation_service::InvestigationService;
use cognicode_core::domain::aggregates::call_graph::SymbolId;
use cognicode_core::domain::investigation::{Artifact, Evidence, Investigation, Status};
use cognicode_core::domain::investigation_candidates::{
    CandidateScope, CandidateState, EvidencePolarity,
};
use cognicode_core::domain::investigation_store::{InvestigationStore, StoreError};
use time::OffsetDateTime;

#[derive(Clone, Default)]
struct SharedStore(Arc<Mutex<HashMap<String, Investigation>>>);

#[async_trait]
impl InvestigationStore for SharedStore {
    async fn save(&self, investigation: &Investigation) -> Result<(), StoreError> {
        self.0
            .lock()
            .expect("store lock")
            .insert(investigation.id.clone(), investigation.clone());
        Ok(())
    }

    async fn load(&self, id: &str) -> Result<Option<Investigation>, StoreError> {
        Ok(self.0.lock().expect("store lock").get(id).cloned())
    }

    async fn list(&self, workspace_id: &str) -> Result<Vec<Investigation>, StoreError> {
        Ok(self
            .0
            .lock()
            .expect("store lock")
            .values()
            .filter(|investigation| investigation.workspace_id == workspace_id)
            .cloned()
            .collect())
    }

    async fn delete(&self, id: &str) -> Result<(), StoreError> {
        self.0.lock().expect("store lock").remove(id);
        Ok(())
    }

    async fn add_evidence(
        &self,
        investigation_id: &str,
        evidence: Evidence,
    ) -> Result<(), StoreError> {
        let mut investigations = self.0.lock().expect("store lock");
        let investigation = investigations
            .get_mut(investigation_id)
            .ok_or_else(|| StoreError::NotFound(investigation_id.to_string()))?;
        investigation.evidence.push(evidence);
        Ok(())
    }

    async fn add_artifact(
        &self,
        investigation_id: &str,
        artifact: Artifact,
    ) -> Result<Artifact, StoreError> {
        let mut investigations = self.0.lock().expect("store lock");
        let investigation = investigations
            .get_mut(investigation_id)
            .ok_or_else(|| StoreError::NotFound(investigation_id.to_string()))?;
        investigation.artifacts.push(artifact.clone());
        Ok(artifact)
    }
}

fn investigation() -> Investigation {
    let now = OffsetDateTime::now_utc();
    Investigation {
        id: "inv-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        title: "Investigate parser behavior".to_string(),
        goal: "Determine which parser handles this call".to_string(),
        status: Status::Active,
        entry_point: None,
        panes: Vec::new(),
        evidence: vec![Evidence {
            id: "evidence-1".to_string(),
            object_id: "src/parser.rs:parse:10".to_string(),
            view_id: None,
            note: "private source excerpt must not be copied into the candidate ledger".to_string(),
            pinned_at: now,
        }],
        artifacts: Vec::new(),
        narrative: String::new(),
        related_adrs: Vec::new(),
        created_at: now,
        updated_at: now,
    }
}

fn scope(session_id: &str) -> CandidateScope {
    CandidateScope::new("workspace-1", session_id).expect("valid scope")
}

#[tokio::test]
async fn candidate_ledgers_reference_evidence_and_are_isolated_by_session() {
    let store = SharedStore::default();
    store
        .save(&investigation())
        .await
        .expect("seed investigation");
    let service = InvestigationService::new(store.clone());
    let first = scope("session-1");
    let second = scope("session-2");

    let first_candidate = service
        .propose_candidate(
            &first,
            "inv-1",
            "revision-1",
            &SymbolId::new("src/a.rs:parse:10"),
            "candidate reason",
        )
        .await
        .expect("first candidate");
    let homonym = service
        .propose_candidate(
            &first,
            "inv-1",
            "revision-1",
            &SymbolId::new("src/b.rs:parse:10"),
            "same symbol name, different source identity",
        )
        .await
        .expect("homonym with distinct identity");
    service
        .link_candidate_evidence(
            &first,
            "inv-1",
            &first_candidate.id,
            "evidence-1",
            EvidencePolarity::Supports,
        )
        .await
        .expect("link existing evidence by ID");

    let second_candidate = service
        .propose_candidate(
            &second,
            "inv-1",
            "revision-1",
            &SymbolId::new("src/c.rs:parse:10"),
            "another session's candidate",
        )
        .await
        .expect("second session candidate");

    let first_ledger = service
        .candidates(&first, "inv-1")
        .await
        .expect("first session may read its ledger")
        .expect("first ledger exists");
    let second_ledger = service
        .candidates(&second, "inv-1")
        .await
        .expect("second session may read its ledger")
        .expect("second ledger exists");
    assert_eq!(first_ledger.candidates().len(), 2);
    assert_eq!(first_ledger.candidates()[1].id, homonym.id);
    assert_eq!(second_ledger.candidates().len(), 1);
    assert_eq!(second_ledger.candidates()[0].id, second_candidate.id);

    let wrong_workspace = CandidateScope::new("workspace-2", "session-1").unwrap();
    assert!(service.candidates(&wrong_workspace, "inv-1").await.is_err());

    let stored = store.load("inv-1").await.expect("load").expect("exists");
    let candidate_artifact = stored
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == "investigation_candidates")
        .expect("candidate artifact stored in the existing aggregate");
    assert!(candidate_artifact.content.contains("evidence-1"));
    assert!(
        !candidate_artifact
            .content
            .contains("private source excerpt")
    );
    assert!(
        !candidate_artifact
            .content
            .contains("src/parser.rs:parse:10")
    );

    service
        .clear_candidates(&first, "inv-1")
        .await
        .expect("clear only the first session's ledger");
    assert!(
        service
            .candidates(&first, "inv-1")
            .await
            .expect("first ledger is cleared")
            .is_none()
    );
    assert_eq!(
        service
            .candidates(&second, "inv-1")
            .await
            .expect("second session still has access")
            .expect("second ledger remains")
            .candidates()
            .len(),
        1
    );
    service
        .delete_investigation("inv-1")
        .await
        .expect("deleting the aggregate also deletes all candidate ledgers");
    assert!(store.load("inv-1").await.expect("load").is_none());
}

#[tokio::test]
async fn updating_a_stale_investigation_does_not_erase_candidate_ledgers() {
    let store = SharedStore::default();
    store
        .save(&investigation())
        .await
        .expect("seed investigation");
    let service = InvestigationService::new(store);
    let stale = service
        .get_investigation("inv-1")
        .await
        .expect("load")
        .expect("exists");
    service
        .propose_candidate(
            &scope("session-1"),
            "inv-1",
            "revision-1",
            &SymbolId::new("src/a.rs:parse:10"),
            "candidate reason",
        )
        .await
        .expect("proposal");
    let mut stale = stale;
    stale.title = "Updated investigation".to_string();

    service
        .update_investigation(stale)
        .await
        .expect("update non-candidate fields from the previously loaded view");

    let ledger = service
        .candidates(&scope("session-1"), "inv-1")
        .await
        .expect("candidate annotations remain scoped to the investigation")
        .expect("update must preserve the candidate ledger");
    assert_eq!(ledger.candidates().len(), 1);
    assert_eq!(
        service
            .get_investigation("inv-1")
            .await
            .expect("load")
            .expect("exists")
            .title,
        "Updated investigation"
    );
}

#[tokio::test]
async fn a_candidate_artifact_id_collision_never_overwrites_an_existing_artifact() {
    let mut seeded = investigation();
    seeded.artifacts.push(Artifact {
        id: "candidates:session-1".to_string(),
        kind: "markdown".to_string(),
        title: "User-authored artifact".to_string(),
        content: "preserve this content".to_string(),
        generated_from: None,
        provenance: None,
    });
    let store = SharedStore::default();
    store.save(&seeded).await.expect("seed investigation");
    let service = InvestigationService::new(store.clone());

    assert!(
        service
            .propose_candidate(
                &scope("session-1"),
                "inv-1",
                "revision-1",
                &SymbolId::new("src/a.rs:parse:10"),
                "candidate reason",
            )
            .await
            .is_err()
    );
    assert!(
        service
            .clear_candidates(&scope("session-1"), "inv-1")
            .await
            .is_err()
    );
    assert!(
        service
            .add_artifact(
                "inv-1",
                Artifact {
                    id: "candidates:session-2".to_string(),
                    kind: "markdown".to_string(),
                    title: "Reserved namespace collision".to_string(),
                    content: "must not shadow a ledger".to_string(),
                    generated_from: None,
                    provenance: None,
                },
            )
            .await
            .is_err()
    );

    let stored = store.load("inv-1").await.expect("load").expect("exists");
    assert_eq!(stored.artifacts.len(), 1);
    assert_eq!(stored.artifacts[0].kind, "markdown");
    assert_eq!(stored.artifacts[0].content, "preserve this content");
}

#[tokio::test]
async fn removed_evidence_cannot_be_revalidated_or_used_to_confirm_a_candidate() {
    let store = SharedStore::default();
    store
        .save(&investigation())
        .await
        .expect("seed investigation");
    let service = InvestigationService::new(store.clone());
    let session = scope("session-1");
    let candidate = service
        .propose_candidate(
            &session,
            "inv-1",
            "revision-1",
            &SymbolId::new("src/parser.rs:parse:10"),
            "parser hypothesis",
        )
        .await
        .expect("proposal");
    service
        .link_candidate_evidence(
            &session,
            "inv-1",
            &candidate.id,
            "evidence-1",
            EvidencePolarity::Supports,
        )
        .await
        .expect("link pinned evidence");
    service
        .apply_candidate_revision(&session, "inv-1", "revision-1", "revision-2", &[])
        .await
        .expect("mark references stale at the new revision");
    store
        .0
        .lock()
        .expect("store lock")
        .get_mut("inv-1")
        .expect("investigation")
        .evidence
        .clear();

    assert!(
        service
            .revalidate_candidate(&session, "inv-1", &candidate.id)
            .await
            .is_err()
    );
    assert!(
        service
            .confirm_candidate(&session, "inv-1", &candidate.id)
            .await
            .is_err()
    );
    let ledger = service
        .candidates(&session, "inv-1")
        .await
        .expect("candidate ledger remains readable")
        .expect("ledger exists");
    assert!(
        ledger
            .candidate(&candidate.id)
            .expect("candidate remains")
            .is_stale("revision-2")
    );
}

#[tokio::test]
async fn concurrent_candidates_survive_and_unproven_revision_reopens_without_confirmation() {
    let store = SharedStore::default();
    store
        .save(&investigation())
        .await
        .expect("seed investigation");
    let service = InvestigationService::new(store);
    let session = scope("session-1");
    let left_subject = SymbolId::new("src/a.rs:parse:10");
    let right_subject = SymbolId::new("src/b.rs:parse:10");

    let (left, right) = tokio::join!(
        service.propose_candidate(
            &session,
            "inv-1",
            "revision-1",
            &left_subject,
            "first hypothesis",
        ),
        service.propose_candidate(
            &session,
            "inv-1",
            "revision-1",
            &right_subject,
            "second hypothesis",
        ),
    );
    let left = left.expect("first concurrent proposal");
    let right = right.expect("second concurrent proposal");
    service
        .link_candidate_evidence(
            &session,
            "inv-1",
            &left.id,
            "evidence-1",
            EvidencePolarity::Supports,
        )
        .await
        .expect("supporting evidence");
    service
        .confirm_candidate(&session, "inv-1", &left.id)
        .await
        .expect("current evidence permits explicit confirmation");

    let ledger = service
        .apply_candidate_revision(&session, "inv-1", "revision-1", "revision-2", &[])
        .await
        .expect("apply revision");
    let reopened = ledger.candidate(&left.id).expect("left remains");
    assert_eq!(reopened.state, CandidateState::Candidate);
    assert!(reopened.is_stale("revision-2"));
    assert!(reopened.reopened_by_revision);
    assert_eq!(
        ledger.candidate(&right.id).expect("right remains").state,
        CandidateState::Candidate
    );
    assert!(
        service
            .confirm_candidate(&session, "inv-1", &left.id)
            .await
            .is_err()
    );

    service
        .revalidate_candidate(&session, "inv-1", &left.id)
        .await
        .expect("caller explicitly revalidated the subject and evidence references");
    let confirmed = service
        .confirm_candidate(&session, "inv-1", &left.id)
        .await
        .expect("fresh supporting evidence allows explicit confirmation");
    assert_eq!(confirmed.state, CandidateState::Confirmed);
}
