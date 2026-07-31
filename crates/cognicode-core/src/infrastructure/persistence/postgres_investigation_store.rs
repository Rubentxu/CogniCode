//! PostgreSQL-backed implementation of the [`InvestigationStore`] trait.
//!
//! Uses the existing [`PostgresRepository`] methods for data access.
//! This module is feature-gated behind `postgres`.

#[cfg(feature = "postgres")]
use std::sync::Arc;

#[cfg(feature = "postgres")]
use async_trait::async_trait;
#[cfg(feature = "postgres")]
use sqlx::PgPool;

#[cfg(feature = "postgres")]
use crate::domain::investigation::Evidence;
#[cfg(feature = "postgres")]
use crate::domain::investigation::Investigation;
#[cfg(feature = "postgres")]
use crate::domain::investigation_store::{InvestigationStore, StoreError};
#[cfg(feature = "postgres")]
use crate::infrastructure::persistence::PostgresRepository;

/// PostgreSQL-backed implementation of [`InvestigationStore`].
#[cfg(feature = "postgres")]
pub struct PostgresInvestigationStore {
    pool: PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresInvestigationStore {
    /// Create a new store from a PostgreSQL connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new store from an existing [`PostgresRepository`].
    pub fn from_repo(repo: &Arc<PostgresRepository>) -> Self {
        Self::new(repo.with_pool(|p| p.clone()))
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl InvestigationStore for PostgresInvestigationStore {
    async fn save(&self, investigation: &Investigation) -> Result<(), StoreError> {
        let repo = PostgresRepository::from_pool(self.pool.clone());

        // Convert domain entity to row types.
        let panes_json = serde_json::to_value(&investigation.panes)
            .map_err(|e| StoreError::Encode(e.to_string()))?;

        let related_adrs_json = serde_json::to_value(&investigation.related_adrs)
            .map_err(|e| StoreError::Encode(e.to_string()))?;

        let row = crate::infrastructure::persistence::InvestigationRow {
            id: investigation.id.clone(),
            workspace_id: investigation.workspace_id.clone(),
            title: investigation.title.clone(),
            goal: investigation.goal.clone(),
            status: investigation.status.to_string(),
            entry_point: investigation.entry_point.clone(),
            panes: panes_json,
            narrative: investigation.narrative.clone(),
            related_adrs: related_adrs_json,
            created_at: investigation
                .created_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap()
                .to_string(),
            updated_at: investigation
                .updated_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap()
                .to_string(),
        };

        // Convert evidence to row types.
        let evidence_rows: Vec<_> = investigation
            .evidence
            .iter()
            .map(
                |e| crate::infrastructure::persistence::InvestigationEvidenceRow {
                    id: e.id.clone(),
                    investigation_id: investigation.id.clone(),
                    object_id: e.object_id.clone(),
                    view_id: e.view_id.clone(),
                    note: e.note.clone(),
                    pinned_at: e
                        .pinned_at
                        .format(&time::format_description::well_known::Rfc3339)
                        .unwrap()
                        .to_string(),
                },
            )
            .collect();

        // Convert artifacts to row types.
        let artifact_rows: Vec<_> = investigation
            .artifacts
            .iter()
            .map(
                |a| crate::infrastructure::persistence::InvestigationArtifactRow {
                    id: a.id.clone(),
                    investigation_id: investigation.id.clone(),
                    kind: a.kind.clone(),
                    title: a.title.clone(),
                    content: a.content.clone(),
                    generated_from: a.generated_from.clone(),
                    provenance: a
                        .provenance
                        .as_ref()
                        .map(|p| serde_json::to_value(p).ok())
                        .flatten(),
                },
            )
            .collect();

        repo.save_investigation_tx(&row, &evidence_rows, &artifact_rows)
            .await
            .map_err(|e| StoreError::Transaction(e.to_string()))
    }

    async fn load(&self, id: &str) -> Result<Option<Investigation>, StoreError> {
        let repo = PostgresRepository::from_pool(self.pool.clone());

        let row = repo
            .load_investigation(id)
            .await
            .map_err(|e| StoreError::Transaction(e.to_string()))?;

        match row {
            Some(row) => {
                let investigation = row_to_investigation(&row, &repo).await?;
                Ok(Some(investigation))
            }
            None => Ok(None),
        }
    }

    async fn list(&self, workspace_id: &str) -> Result<Vec<Investigation>, StoreError> {
        let repo = PostgresRepository::from_pool(self.pool.clone());

        let rows = repo
            .list_investigations(workspace_id)
            .await
            .map_err(|e| StoreError::Transaction(e.to_string()))?;

        let mut investigations = Vec::with_capacity(rows.len());
        for row in rows {
            let investigation = row_to_investigation(&row, &repo).await?;
            investigations.push(investigation);
        }

        Ok(investigations)
    }

    async fn delete(&self, id: &str) -> Result<(), StoreError> {
        let repo = PostgresRepository::from_pool(self.pool.clone());

        repo.delete_investigation(id)
            .await
            .map_err(|e| StoreError::Transaction(e.to_string()))
    }

    async fn add_evidence(
        &self,
        investigation_id: &str,
        evidence: Evidence,
    ) -> Result<(), StoreError> {
        let repo = PostgresRepository::from_pool(self.pool.clone());

        // Verify the investigation exists before adding evidence.
        repo.load_investigation(investigation_id)
            .await
            .map_err(|e| StoreError::Transaction(e.to_string()))?
            .ok_or_else(|| StoreError::NotFound(investigation_id.to_string()))?;

        let evidence_row = crate::infrastructure::persistence::InvestigationEvidenceRow {
            id: evidence.id,
            investigation_id: investigation_id.to_string(),
            object_id: evidence.object_id,
            view_id: evidence.view_id,
            note: evidence.note,
            pinned_at: evidence
                .pinned_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap()
                .to_string(),
        };

        repo.add_investigation_evidence(investigation_id, &evidence_row)
            .await
            .map_err(|e| StoreError::Transaction(e.to_string()))
    }

    async fn add_artifact(
        &self,
        investigation_id: &str,
        mut artifact: crate::domain::investigation::Artifact,
    ) -> Result<crate::domain::investigation::Artifact, StoreError> {
        let repo = PostgresRepository::from_pool(self.pool.clone());

        // Verify the investigation exists before adding artifact.
        repo.load_investigation(investigation_id)
            .await
            .map_err(|e| StoreError::Transaction(e.to_string()))?
            .ok_or_else(|| StoreError::NotFound(investigation_id.to_string()))?;

        // Server-side stamp provenance.created_at (spec: "server-stamped").
        if let Some(ref mut prov) = artifact.provenance {
            prov.created_at = time::OffsetDateTime::now_utc();
        }

        let provenance_json = artifact
            .provenance
            .as_ref()
            .map(|p| serde_json::to_value(p).ok())
            .flatten();

        let artifact_row = crate::infrastructure::persistence::InvestigationArtifactRow {
            id: artifact.id.clone(),
            investigation_id: investigation_id.to_string(),
            kind: artifact.kind.clone(),
            title: artifact.title.clone(),
            content: artifact.content.clone(),
            generated_from: artifact.generated_from.clone(),
            provenance: provenance_json,
        };

        repo.add_investigation_artifact(investigation_id, &artifact_row)
            .await
            .map_err(|e| StoreError::Transaction(e.to_string()))?;

        Ok(artifact)
    }
}

/// Convert an [`InvestigationRow`] to an [`Investigation`] domain entity.
#[cfg(feature = "postgres")]
async fn row_to_investigation(
    row: &crate::infrastructure::persistence::InvestigationRow,
    repo: &PostgresRepository,
) -> Result<Investigation, StoreError> {
    use time::OffsetDateTime;

    let panes: Vec<crate::domain::investigation::PaneSnapshot> =
        serde_json::from_value(row.panes.clone())
            .map_err(|e| StoreError::Decode(format!("panes: {e}")))?;

    let related_adrs: Vec<String> = serde_json::from_value(row.related_adrs.clone())
        .map_err(|e| StoreError::Decode(format!("related_adrs: {e}")))?;

    let status = crate::domain::investigation::Status::from_str(&row.status)
        .ok_or_else(|| StoreError::Decode(format!("invalid status: {}", row.status)))?;

    let created_at = OffsetDateTime::parse(
        &row.created_at,
        &time::format_description::well_known::Rfc3339,
    )
    .map_err(|e| StoreError::Decode(format!("created_at: {e}")))?;

    let updated_at = OffsetDateTime::parse(
        &row.updated_at,
        &time::format_description::well_known::Rfc3339,
    )
    .map_err(|e| StoreError::Decode(format!("updated_at: {e}")))?;

    // Load evidence and artifacts.
    let evidence_rows = repo
        .load_investigation_evidence(&row.id)
        .await
        .map_err(|e| StoreError::Transaction(e.to_string()))?;

    let evidence: Vec<_> = evidence_rows
        .into_iter()
        .map(|r| {
            let pinned_at =
                OffsetDateTime::parse(&r.pinned_at, &time::format_description::well_known::Rfc3339)
                    .map_err(|e| StoreError::Decode(format!("evidence pinned_at: {e}")))?;
            Ok(crate::domain::investigation::Evidence {
                id: r.id,
                object_id: r.object_id,
                view_id: r.view_id,
                note: r.note,
                pinned_at,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let artifact_rows = repo
        .load_investigation_artifacts(&row.id)
        .await
        .map_err(|e| StoreError::Transaction(e.to_string()))?;

    let artifacts: Vec<_> = artifact_rows
        .into_iter()
        .map(|r| {
            let provenance = r.provenance.and_then(|v| {
                serde_json::from_value::<crate::domain::investigation::DiagramProvenance>(v).ok()
            });
            crate::domain::investigation::Artifact {
                id: r.id,
                kind: r.kind,
                title: r.title,
                content: r.content,
                generated_from: r.generated_from,
                provenance,
            }
        })
        .collect();

    Ok(Investigation {
        id: row.id.clone(),
        workspace_id: row.workspace_id.clone(),
        title: row.title.clone(),
        goal: row.goal.clone(),
        status,
        entry_point: row.entry_point.clone(),
        panes,
        evidence,
        artifacts,
        narrative: row.narrative.clone(),
        related_adrs,
        created_at,
        updated_at,
    })
}
