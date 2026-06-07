use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::ExplorerError;
use crate::dto::{GenerateArtifactRequest, OpenWorkspaceRequest, SaveExplorationRequest};
use crate::service::ExplorerService;

#[derive(Clone)]
pub struct ApiState {
    service: Arc<ExplorerService>,
}

impl ApiState {
    pub fn new(service: ExplorerService) -> Self {
        Self {
            service: Arc::new(service),
        }
    }
}

pub fn router(service: ExplorerService) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/workspaces/open", post(open_workspace))
        .route("/api/workspaces/:workspace_id/index", post(index_workspace))
        .route("/api/workspaces/:workspace_id/spotter", get(spotter))
        .route("/api/objects/:object_id", get(inspect_object))
        .route("/api/objects/:object_id/views", get(available_views))
        .route(
            "/api/objects/:object_id/views/:view_id",
            get(contextual_view),
        )
        .route("/api/objects/:object_id/lenses", get(available_lenses))
        .route(
            "/api/objects/:object_id/lenses/:lens_id",
            get(apply_lens),
        )
        .route("/api/explorations", post(save_exploration))
        .route(
            "/api/explorations/:exploration_id/artifacts",
            post(generate_artifact),
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(ApiState::new(service))
}

pub async fn serve(service: ExplorerService, addr: SocketAddr) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(service)).await?;
    Ok(())
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "service": "cognicode-explorer" }))
}

async fn open_workspace(
    State(state): State<ApiState>,
    Json(request): Json<OpenWorkspaceRequest>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.open_workspace(request)?).into_response())
}

async fn index_workspace(Path(_workspace_id): Path<String>) -> Result<Response, ApiError> {
    Err(ApiError(ExplorerError::NotImplemented(
        "workspace indexing will delegate to CogniCode graph/index builders",
    )))
}

#[derive(Debug, Deserialize)]
struct SpotterQuery {
    q: String,
    kind: Option<String>,
}

async fn spotter(
    State(state): State<ApiState>,
    Path(_workspace_id): Path<String>,
    Query(query): Query<SpotterQuery>,
) -> Result<Response, ApiError> {
    Ok(Json(
        state
            .service
            .spotter_search(&query.q, query.kind.as_deref())?,
    )
    .into_response())
}

async fn inspect_object(
    State(state): State<ApiState>,
    Path(object_id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.inspect_object(&object_id)?).into_response())
}

async fn available_views(
    State(state): State<ApiState>,
    Path(object_id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.available_views(&object_id)?).into_response())
}

async fn contextual_view(
    State(state): State<ApiState>,
    Path((object_id, view_id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.contextual_view(&object_id, &view_id)?).into_response())
}

async fn available_lenses(
    State(state): State<ApiState>,
    Path(object_id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.available_lenses(&object_id)?).into_response())
}

async fn apply_lens(
    State(state): State<ApiState>,
    Path((object_id, lens_id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.apply_lens(&object_id, &lens_id)?).into_response())
}

async fn save_exploration(
    State(state): State<ApiState>,
    Json(request): Json<SaveExplorationRequest>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.save_exploration(request)?).into_response())
}

async fn generate_artifact(
    State(state): State<ApiState>,
    Path(exploration_id): Path<String>,
    Json(request): Json<GenerateArtifactRequest>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.generate_artifact(&exploration_id, request)?).into_response())
}

struct ApiError(ExplorerError);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0 {
            ExplorerError::WorkspaceNotFound(_)
            | ExplorerError::ObjectNotFound(_)
            | ExplorerError::SourceUnavailable { .. } => StatusCode::NOT_FOUND,
            ExplorerError::ViewNotAvailable { .. } => StatusCode::NOT_FOUND,
            ExplorerError::ResolutionFailed(_) => StatusCode::BAD_REQUEST,
            ExplorerError::GraphNotReady => StatusCode::SERVICE_UNAVAILABLE,
            ExplorerError::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            ExplorerError::Anyhow(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = serde_json::json!({
            "error": self.0.to_string(),
        });

        (status, Json(body)).into_response()
    }
}

impl From<ExplorerError> for ApiError {
    fn from(error: ExplorerError) -> Self {
        Self(error)
    }
}
