//! Conversion helpers between [`ViewSpec`] (explorer-side DTO)
//! and [`ViewSpecPayload`] (port-side wire format).
//!
//! The port trait in `cognicode_core::domain::ports::view_spec_store`
//! operates on the wire-format `ViewSpecPayload`, while the explorer
//! application code keeps working with the rich `ViewSpec` DTO. This
//! module owns the `serde_json` round-trip between the two so the
//! application code does not see the boundary.

use crate::dto::{DataSource, InspectableObjectType, RendererKind, Transform, ViewKind, ViewSpec};
use crate::error::{ExplorerError, ExplorerResult};
use cognicode_core::domain::ports::{ViewSpecPayload, ViewSpecStoreError};

/// Convert an explorer-side [`ViewSpec`] into the port-side
/// [`ViewSpecPayload`].
///
/// Serializes the rich enum fields (`DataSource`, `Transform`,
/// `ViewKind`) to JSON, and maps `InspectableObjectType` to its
/// snake_case wire form (the `view_specs.applies_to` column value).
pub fn view_spec_to_payload(spec: &ViewSpec) -> ExplorerResult<ViewSpecPayload> {
    let applies_to = inspectable_to_string(&spec.applies_to);
    let view_kind =
        serde_json::to_value(&spec.view_kind).map_err(|e| store_err(format!("view_kind: {e}")))?;
    let data_source = serde_json::to_value(&spec.data_source)
        .map_err(|e| store_err(format!("data_source: {e}")))?;
    let transform = match &spec.transform {
        Some(t) => Some(serde_json::to_value(t).map_err(|e| store_err(format!("transform: {e}")))?),
        None => None,
    };
    let renderer_kind = renderer_kind_to_wire(&spec.renderer_kind);

    Ok(ViewSpecPayload {
        id: spec.id.clone(),
        title: spec.title.clone(),
        applies_to,
        view_kind,
        data_source,
        transform,
        renderer_kind,
        props: spec.props.clone(),
        created_at: spec.created_at.clone(),
        updated_at: spec.updated_at.clone(),
        owner: spec.owner.clone(),
        seed_object_id: spec.seed_object_id.clone(),
        seed_view_id: spec.seed_view_id.clone(),
        applies_when: spec.applies_when.clone(),
    })
}

/// Convert a port-side [`ViewSpecPayload`] into the explorer-side
/// [`ViewSpec`].
pub fn payload_to_view_spec(payload: ViewSpecPayload) -> ExplorerResult<ViewSpec> {
    let applies_to = inspectable_from_string(&payload.applies_to)
        .ok_or_else(|| store_err(format!("unknown applies_to: {}", payload.applies_to)))?;
    let view_kind: ViewKind = serde_json::from_value(payload.view_kind)
        .map_err(|e| store_err(format!("view_kind: {e}")))?;
    let data_source: DataSource = serde_json::from_value(payload.data_source)
        .map_err(|e| store_err(format!("data_source: {e}")))?;
    let transform: Option<Transform> = match payload.transform {
        Some(v) => {
            Some(serde_json::from_value(v).map_err(|e| store_err(format!("transform: {e}")))?)
        }
        None => None,
    };
    let renderer_kind = renderer_kind_from_wire(&payload.renderer_kind);

    Ok(ViewSpec {
        id: payload.id,
        title: payload.title,
        applies_to,
        view_kind,
        data_source,
        transform,
        renderer_kind,
        props: payload.props,
        created_at: payload.created_at,
        updated_at: payload.updated_at,
        owner: payload.owner,
        seed_object_id: payload.seed_object_id,
        seed_view_id: payload.seed_view_id,
        applies_when: payload.applies_when,
    })
}

/// Map a [`ViewSpecStoreError`] to an [`ExplorerError`] for `?` chains.
impl From<ViewSpecStoreError> for ExplorerError {
    fn from(err: ViewSpecStoreError) -> Self {
        match err {
            ViewSpecStoreError::Store(msg) => {
                ExplorerError::Anyhow(anyhow::anyhow!("view_spec store: {msg}"))
            }
            ViewSpecStoreError::Conflict(msg) => ExplorerError::Conflict(msg),
            ViewSpecStoreError::NotFound(msg) => ExplorerError::NotFound(msg),
        }
    }
}

fn store_err(msg: String) -> ExplorerError {
    ExplorerError::Anyhow(anyhow::anyhow!(msg))
}

// ---- wire ↔ rich conversions (used by payload <-> spec bridges) ----

fn inspectable_to_string(kind: &InspectableObjectType) -> String {
    let s = serde_json::to_string(kind).unwrap_or_else(|_| "file".to_string());
    s.trim_matches('"').to_string()
}

/// Public helper that lets non-port code (e.g. the [`ViewRegistry`] in
/// `registry.rs`) map an `InspectableObjectType` enum variant to the
/// snake_case wire form used by the `view_specs.applies_to` column
/// and [`ViewSpecStore::list_for_workspace`].
pub fn inspectable_to_wire(kind: InspectableObjectType) -> String {
    inspectable_to_string(&kind)
}

/// Public helper that maps a snake_case wire form (the `applies_to`
/// column value) back to the explorer-side `InspectableObjectType`
/// enum variant. Returns `None` for unknown values — callers should
/// pick a safe default (e.g. `InspectableObjectType::File`).
pub fn wire_to_inspectable(s: &str) -> Option<InspectableObjectType> {
    inspectable_from_string(s)
}

fn inspectable_from_string(s: &str) -> Option<InspectableObjectType> {
    Some(match s {
        "workspace" => InspectableObjectType::Workspace,
        "scope" => InspectableObjectType::Scope,
        "symbol" => InspectableObjectType::Symbol,
        "file" => InspectableObjectType::File,
        "module" => InspectableObjectType::Module,
        "evidence" => InspectableObjectType::Evidence,
        "decision_artifact" => InspectableObjectType::DecisionArtifact,
        "quality_issue" => InspectableObjectType::QualityIssue,
        "rule" => InspectableObjectType::Rule,
        "saved_exploration" => InspectableObjectType::SavedExploration,
        "investigation" => InspectableObjectType::Investigation,
        "doc" => InspectableObjectType::Doc,
        "adr" => InspectableObjectType::Adr,
        _ => return None,
    })
}

fn renderer_kind_to_wire(kind: &RendererKind) -> String {
    serde_json::to_string(kind)
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_else(|_| "json".to_string())
}

fn renderer_kind_from_wire(s: &str) -> RendererKind {
    match s {
        "graph" => RendererKind::Graph,
        "table" => RendererKind::Table,
        "tree" => RendererKind::Tree,
        "code" => RendererKind::Code,
        "markdown" => RendererKind::Markdown,
        "vega_lite" => RendererKind::VegaLite,
        "json" => RendererKind::Json,
        "composite" => RendererKind::Composite,
        other => RendererKind::Custom(other.to_string()),
    }
}
