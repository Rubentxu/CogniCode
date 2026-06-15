//! [`PersistenceService`] implementation.
//!
//! Provides exploration path persistence and ViewSpec CRUD.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;

#[cfg(feature = "postgres")]
use cognicode_core::infrastructure::persistence::PostgresRepository;

use crate::dto::{
    DecisionArtifactSummary, ExplorationPath, ExplorationSession,
    GenerateArtifactRequest, ObjectIdentityEntry,
    SaveExplorationRequest, SaveExplorationSessionRequest, ViewSpec,
};
use crate::error::{ExplorerError, ExplorerResult};
use crate::facades::PersistenceService;
use crate::domain::object_identity::ObjectIdentity;
use crate::registry::ViewSpecStore;

/// In-memory store for exploration paths, keyed by exploration id.
/// Phase 1C: process-lifetime only — paths do not survive a restart.
type ExplorationPathStore = Mutex<HashMap<String, ExplorationPath>>;

/// In-memory store for exploration sessions (ADR-016 Fase 3).
type ExplorationSessionStore = Mutex<HashMap<String, ExplorationSession>>;

/// Concrete implementation of [`PersistenceService`].
///
/// Holds:
/// - `view_spec_store` — optional ViewSpec persistence backend
/// - `postgres_repo` — optional PostgreSQL repository for named views
/// - `paths` — in-memory exploration path store
/// - `sessions` — in-memory exploration session store (ADR-016 Fase 3)
pub struct PersistenceServiceImpl {
    view_spec_store: Option<Arc<dyn ViewSpecStore>>,
    #[cfg(feature = "postgres")]
    postgres_repo: Option<Arc<PostgresRepository>>,
    paths: Arc<ExplorationPathStore>,
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
            paths: Arc::new(Mutex::new(HashMap::new())),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl PersistenceService for PersistenceServiceImpl {
    async fn save_exploration(
        &self,
        request: SaveExplorationRequest,
    ) -> ExplorerResult<ExplorationPath> {
        if request.columns.is_empty() {
            return Err(ExplorerError::ResolutionFailed(
                "exploration path requires at least one column".to_string(),
            ));
        }

        // Validate every column id is well-formed before we persist anything.
        let created_at = Utc::now().to_rfc3339();
        let mut seen: HashMap<String, ObjectIdentityEntry> = HashMap::new();
        for column in &request.columns {
            let identity = ObjectIdentity::parse_mvp_id(&column.object_id)?;
            let entry = ObjectIdentityEntry {
                id: identity.to_mvp_id(),
                object_type: identity.object_type(),
                natural_key: identity.natural_key(),
                first_seen: created_at.clone(),
            };
            seen.entry(entry.id.clone()).or_insert(entry);
        }
        let objects: Vec<ObjectIdentityEntry> = seen.into_values().collect();

        let path = ExplorationPath {
            id: format!("exploration:{}", Utc::now().timestamp_millis()),
            workspace_id: request.workspace_id,
            columns: request.columns,
            objects,
            lens: request.lens,
            created_at,
            navigation_mode: request.navigation_mode,
        };

        self.paths
            .lock()
            .map_err(|_| ExplorerError::Anyhow(anyhow::anyhow!("exploration path store poisoned")))?
            .insert(path.id.clone(), path.clone());

        Ok(path)
    }

    async fn generate_artifact(
        &self,
        exploration_id: &str,
        request: GenerateArtifactRequest,
    ) -> ExplorerResult<DecisionArtifactSummary> {
        let path = self
            .paths
            .lock()
            .map_err(|_| ExplorerError::Anyhow(anyhow::anyhow!("path store poisoned")))?
            .get(exploration_id)
            .cloned();

        match request.format {
            crate::dto::ArtifactFormat::JsonReplay => {
                let body = match path.as_ref() {
                    Some(p) => render_replay_json(p),
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
                let body = match path.as_ref() {
                    Some(p) => render_replay_markdown(p),
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
        let session = ExplorationSession {
            id: format!("session:{}", Utc::now().timestamp_millis()),
            workspace_id: request.workspace_id,
            events: request.events,
            navigation_mode: request.navigation_mode,
            created_at,
        };

        self.sessions
            .lock()
            .map_err(|_| ExplorerError::Anyhow(anyhow::anyhow!("session store poisoned")))?
            .insert(session.id.clone(), session.clone());

        Ok(session)
    }

    async fn load_exploration_session(
        &self,
        session_id: &str,
    ) -> ExplorerResult<Option<ExplorationSession>> {
        let guard = self
            .sessions
            .lock()
            .map_err(|_| ExplorerError::Anyhow(anyhow::anyhow!("session store poisoned")))?;
        Ok(guard.get(session_id).cloned())
    }
}

// ---------------------------------------------------------------------------
// Exploration path artifact rendering
// ---------------------------------------------------------------------------

fn render_replay_json(path: &ExplorationPath) -> String {
    let body = json!({
        "exploration_id": path.id,
        "version": 1,
        "objects": path.objects,
    });
    serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string())
}

fn render_replay_json_unknown(exploration_id: &str) -> String {
    let body = json!({
        "exploration_id": exploration_id,
        "version": 1,
        "objects": [],
        "warning": "exploration path not found in session store — no resolved objects available",
    });
    serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string())
}

fn render_replay_markdown(path: &ExplorationPath) -> String {
    let mut out = String::new();
    out.push_str("# Symbol exploration report\n\n");
    out.push_str(&format!("Exploration: `{}`\n\n", path.id));
    out.push_str(&format!("Created: `{}`\n\n", path.created_at));
    out.push_str(&format!("Objects ({}):\n\n", path.objects.len()));
    for obj in &path.objects {
        out.push_str(&format!(
            "- `{}` — type=`{}` natural_key=`{}` first_seen=`{}`\n",
            obj.id, obj.object_type, obj.natural_key, obj.first_seen
        ));
    }
    out
}

fn render_replay_markdown_unknown(exploration_id: &str) -> String {
    format!(
        "# Symbol exploration report\n\nExploration: `{exploration_id}`\n\n_No path data found in session store — the exploration may have been created in another process._\n"
    )
}
