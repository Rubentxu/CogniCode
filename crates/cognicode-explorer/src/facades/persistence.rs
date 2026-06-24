//! [`PersistenceService`] implementation.
//!
//! Provides exploration session persistence and ViewSpec CRUD (ADR-045 Phase 1).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;

#[cfg(feature = "postgres")]
use cognicode_core::infrastructure::persistence::PostgresRepository;

use crate::dto::{
    DecisionArtifactSummary, ExplorationSession, GenerateArtifactRequest,
    SaveExplorationSessionRequest, ViewSpec,
};
use crate::error::{ExplorerError, ExplorerResult};
use crate::facades::PersistenceService;
use crate::registry::ViewSpecStore;

/// In-memory store for exploration sessions (ADR-016 Fase 3).
type ExplorationSessionStore = Mutex<HashMap<String, ExplorationSession>>;

/// Concrete implementation of [`PersistenceService`].
///
/// Holds:
/// - `view_spec_store` — optional ViewSpec persistence backend
/// - `postgres_repo` — optional PostgreSQL repository for named views
/// - `sessions` — in-memory exploration session store (ADR-016 Fase 3)
pub struct PersistenceServiceImpl {
    view_spec_store: Option<Arc<dyn ViewSpecStore>>,
    #[cfg(feature = "postgres")]
    postgres_repo: Option<Arc<PostgresRepository>>,
    sessions: Arc<ExplorationSessionStore>,
}

impl PersistenceServiceImpl {
    /// Construct a new `PersistenceServiceImpl`.
    pub fn new(
        view_spec_store: Option<Arc<dyn ViewSpecStore>>,
        #[cfg(feature = "postgres")] postgres_repo: Option<Arc<PostgresRepository>>,
    ) -> Self {
        Self {
            view_spec_store,
            #[cfg(feature = "postgres")]
            postgres_repo,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl PersistenceService for PersistenceServiceImpl {
    async fn generate_artifact(
        &self,
        exploration_id: &str,
        request: GenerateArtifactRequest,
    ) -> ExplorerResult<DecisionArtifactSummary> {
        let session = self
            .sessions
            .lock()
            .map_err(|_| ExplorerError::Anyhow(anyhow::anyhow!("session store poisoned")))?
            .get(exploration_id)
            .cloned();

        match request.format {
            crate::dto::ArtifactFormat::JsonReplay => {
                let body = match session.as_ref() {
                    Some(s) => render_replay_json(s),
                    None => render_replay_json_unknown(exploration_id),
                };
                Ok(DecisionArtifactSummary {
                    id: format!("artifact:{exploration_id}:json"),
                    format: request.format,
                    title: "Exploration JSON replay".into(),
                    content: body,
                })
            }
            crate::dto::ArtifactFormat::Markdown | crate::dto::ArtifactFormat::Html => {
                let body = match session.as_ref() {
                    Some(s) => render_replay_markdown(s),
                    None => render_replay_markdown_unknown(exploration_id),
                };
                Ok(DecisionArtifactSummary {
                    id: format!("artifact:{exploration_id}:md"),
                    format: request.format,
                    title: "Symbol exploration report".into(),
                    content: body,
                })
            }
        }
    }

    async fn save_view_spec(
        &self,
        spec: &ViewSpec,
        workspace_id: &str,
        owner: &str,
    ) -> ExplorerResult<()> {
        let store = self.view_spec_store.as_ref().ok_or_else(|| {
            ExplorerError::FeatureDisabled("view_spec_store requires postgres feature".into())
        })?;
        store
            .save(spec, workspace_id, owner)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("save_view_spec: {e}")))
    }

    async fn load_view_spec(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> ExplorerResult<Option<ViewSpec>> {
        let store = self.view_spec_store.as_ref().ok_or_else(|| {
            ExplorerError::FeatureDisabled("view_spec_store requires postgres feature".into())
        })?;
        store
            .load(id, workspace_id, owner)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("load_view_spec: {e}")))
    }

    async fn list_view_specs(
        &self,
        workspace_id: &str,
        owner: &str,
    ) -> ExplorerResult<Vec<ViewSpec>> {
        let store = self.view_spec_store.as_ref().ok_or_else(|| {
            ExplorerError::FeatureDisabled("view_spec_store requires postgres feature".into())
        })?;
        store
            .list(workspace_id, owner)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("list_view_specs: {e}")))
    }

    async fn delete_view_spec(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> ExplorerResult<bool> {
        let store = self.view_spec_store.as_ref().ok_or_else(|| {
            ExplorerError::FeatureDisabled("view_spec_store requires postgres feature".into())
        })?;
        store
            .delete(id, workspace_id, owner)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("delete_view_spec: {e}")))
    }

    /// List all saved `ExplorationSession` records for a workspace.
    ///
    /// ## KNOWN-DEBT (ADR-045 Phase 2 — resolved)
    ///
    /// - Debt 1 ✅: Orphaned `GET /api/explorations/:id` route removed.
    /// - Debt 2 ✅: Dual model unified onto `ExplorationSession` (ADR-040 Wave 3 aligned).
    /// - Debt 3 ✅: Postgres persistence added — exploration sessions survive server restarts (v0.12.6).
    async fn list_explorations(
        &self,
        workspace_id: &str,
    ) -> ExplorerResult<Vec<ExplorationSession>> {
        #[cfg(feature = "postgres")]
        if let Some(ref repo) = self.postgres_repo {
            let rows = repo
                .list_exploration_sessions(workspace_id)
                .await
                .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("list_explorations: {e}")))?;
            let sessions: Vec<ExplorationSession> = rows
                .into_iter()
                .map(|row| ExplorationSession {
                    id: row.id,
                    workspace_id: row.workspace_id,
                    events: serde_json::from_str(&row.events.to_string()).unwrap_or_default(),
                    navigation_mode: row.navigation_mode,
                    panes: serde_json::from_str(&row.panes.to_string()).unwrap_or_default(),
                    created_at: row.created_at,
                })
                .collect();
            return Ok(sessions);
        }

        let sessions = self.sessions.lock().map_err(|_| {
            ExplorerError::Anyhow(anyhow::anyhow!("exploration session store poisoned"))
        })?;
        Ok(sessions
            .values()
            .filter(|s| s.workspace_id == workspace_id)
            .cloned()
            .collect())
    }

    // --- Exploration Session (ADR-016 Fase 3) ---

    async fn save_exploration_session(
        &self,
        request: SaveExplorationSessionRequest,
    ) -> ExplorerResult<ExplorationSession> {
        if request.events.is_empty() {
            return Err(ExplorerError::ResolutionFailed(
                "exploration session requires at least one event".to_string(),
            ));
        }

        let created_at = Utc::now().to_rfc3339();
        let id = format!("session:{}", Utc::now().timestamp_millis());
        let session = ExplorationSession {
            id: id.clone(),
            workspace_id: request.workspace_id.clone(),
            events: request.events.clone(),
            navigation_mode: request.navigation_mode.clone(),
            panes: request.panes.clone(),
            created_at: created_at.clone(),
        };

        #[cfg(feature = "postgres")]
        if let Some(ref repo) = self.postgres_repo {
            let events_json = serde_json::to_string(&request.events)
                .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("serialize events: {e}")))?;
            let panes_json = serde_json::to_string(&request.panes)
                .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("serialize panes: {e}")))?;
            repo.save_exploration_session(
                &id,
                &request.workspace_id,
                &events_json,
                &request.navigation_mode,
                &panes_json,
            )
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("save_exploration_session: {e}")))?;
            return Ok(session);
        }

        self.sessions
            .lock()
            .map_err(|_| ExplorerError::Anyhow(anyhow::anyhow!("session store poisoned")))?
            .insert(id, session.clone());

        Ok(session)
    }

    async fn load_exploration_session(
        &self,
        session_id: &str,
    ) -> ExplorerResult<Option<ExplorationSession>> {
        #[cfg(feature = "postgres")]
        if let Some(ref repo) = self.postgres_repo {
            // Try to find the workspace_id from in-memory store first
            // to scope the PG query correctly
            let workspace_id = {
                let guard = self.sessions.lock().map_err(|_| {
                    ExplorerError::Anyhow(anyhow::anyhow!("session store poisoned"))
                })?;
                guard.get(session_id).map(|s| s.workspace_id.clone())
            };
            if let Some(ws_id) = workspace_id {
                if let Ok(Some(row)) = repo.load_exploration_session(session_id, &ws_id).await {
                    return Ok(Some(ExplorationSession {
                        id: row.id,
                        workspace_id: row.workspace_id,
                        events: serde_json::from_str(&row.events.to_string()).unwrap_or_default(),
                        navigation_mode: row.navigation_mode,
                        panes: serde_json::from_str(&row.panes.to_string()).unwrap_or_default(),
                        created_at: row.created_at,
                    }));
                }
            }
        }

        let guard = self
            .sessions
            .lock()
            .map_err(|_| ExplorerError::Anyhow(anyhow::anyhow!("session store poisoned")))?;
        Ok(guard.get(session_id).cloned())
    }
}

// ---------------------------------------------------------------------------
// Exploration session artifact rendering (ADR-045 Phase 1)
// ---------------------------------------------------------------------------

fn render_replay_json(session: &ExplorationSession) -> String {
    let body = json!({
        "exploration_id": session.id,
        "version": 1,
        "events": session.events,
        "panes": session.panes,
    });
    serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string())
}

fn render_replay_json_unknown(exploration_id: &str) -> String {
    let body = json!({
        "exploration_id": exploration_id,
        "version": 1,
        "events": [],
        "panes": [],
        "warning": "exploration session not found in session store — no data available",
    });
    serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string())
}

fn render_replay_markdown(session: &ExplorationSession) -> String {
    let mut out = String::new();
    out.push_str("# Symbol exploration report\n\n");
    out.push_str(&format!("Exploration: `{}`\n\n", session.id));
    out.push_str(&format!("Created: `{}`\n\n", session.created_at));
    out.push_str(&format!("Events ({}):\n\n", session.events.len()));
    for event in &session.events {
        out.push_str(&format!(
            "- `{}` — view=`{}` ts=`{}`\n",
            event.object_id,
            event.view_id.as_deref().unwrap_or("none"),
            event.ts
        ));
    }
    out
}

fn render_replay_markdown_unknown(exploration_id: &str) -> String {
    format!(
        "# Symbol exploration report\n\nExploration: `{exploration_id}`\n\n_No session data found in store — the exploration may have been created in another process._\n"
    )
}
