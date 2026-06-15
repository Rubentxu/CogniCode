//! Named-views tool handlers.
//!
//! Implements 4 MCP tools for persisted view management:
//! - `view_save`    — persist a named view projection
//! - `view_load`    — load and re-invoke a saved named view
//! - `view_list`    — list all named views for a scope
//! - `view_delete`  — delete a named view by id

use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::{CallToolResult, Content};
use serde::Deserialize;
use serde_json::Value;

use crate::error::ExplorerError;
use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{
    McpContext, TOOL_VIEW_DELETE, TOOL_VIEW_LIST, TOOL_VIEW_LOAD, TOOL_VIEW_SAVE,
};

/// Generate a v4-ish UUID string using a clock + atomic counter.
/// Format: `"xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx"`.
fn uuid_v4_string() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mix = now.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(n);
    let bytes: [u8; 16] = bytemuck_like_bytes(mix);
    let mut hex = String::with_capacity(36);
    for (i, b) in bytes.iter().enumerate() {
        if i == 4 || i == 6 || i == 8 || i == 10 {
            hex.push('-');
        }
        hex.push_str(&format!("{b:02x}"));
    }
    let mut chars: Vec<char> = hex.chars().collect();
    chars[14] = '4';
    let v = chars[19];
    chars[19] = match v {
        '8' | '9' | 'a' | 'b' => v,
        _ => '8',
    };
    chars.into_iter().collect()
}

/// Deterministic 16-byte expansion of a 64-bit seed.
fn bytemuck_like_bytes(seed: u64) -> [u8; 16] {
    let lo = seed;
    let hi = seed.wrapping_mul(0xFF51_AFD7_ED55_28CC);
    let mut out = [0u8; 16];
    out[0..8].copy_from_slice(&lo.to_le_bytes());
    out[8..16].copy_from_slice(&hi.to_le_bytes());
    out
}

// ============================================================================
// Arg structs
// ============================================================================

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ViewSaveArgs {
    workspace_id: Option<String>,
    owner: Option<String>,
    name: Option<String>,
    description: Option<String>,
    level: Option<String>,
    lens: Option<String>,
    focus_node: Option<String>,
    max_depth: Option<i32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ViewLoadArgs {
    id: Option<String>,
    workspace_id: Option<String>,
    owner: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ViewListArgs {
    workspace_id: Option<String>,
    owner: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ViewDeleteArgs {
    id: Option<String>,
    workspace_id: Option<String>,
    owner: Option<String>,
}

// ============================================================================
// ToolHandler implementations
// ============================================================================

// --- view_save ---

struct ViewSaveHandler;

#[async_trait]
impl ToolHandler for ViewSaveHandler {
    fn name(&self) -> &'static str {
        TOOL_VIEW_SAVE
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "workspace_id": {
                    "type": "string",
                    "description": "Workspace id this view is scoped to (required)."
                },
                "owner": {
                    "type": "string",
                    "description": "Principal (user id) that owns this view (required)."
                },
                "name": {
                    "type": "string",
                    "description": "Display name for the view (required)."
                },
                "description": {
                    "type": "string",
                    "description": "Optional free-form description."
                },
                "level": {
                    "type": "string",
                    "description": "Projection level identifier (required)."
                },
                "lens": {
                    "type": "string",
                    "description": "Lens identifier (required)."
                },
                "focus_node": {
                    "type": "string",
                    "description": "Object id of the focus (required)."
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum depth for the projection (>= 0) (required)."
                }
            },
            "required": ["workspace_id", "owner", "name", "level", "lens", "focus_node", "max_depth"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: ViewSaveArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_VIEW_SAVE, "invalid_args",
                &format!("{TOOL_VIEW_SAVE}: invalid args: {e}")),
        };

        let workspace_id = match args.workspace_id {
            Some(s) if !s.is_empty() => s,
            _ => return err_envelope(TOOL_VIEW_SAVE, "invalid_input",
                "view_save: missing required arg `workspace_id`"),
        };
        let owner = match args.owner {
            Some(s) if !s.is_empty() => s,
            _ => return err_envelope(TOOL_VIEW_SAVE, "invalid_input",
                "view_save: missing required arg `owner`"),
        };
        let name = match args.name {
            Some(s) if !s.is_empty() => s,
            _ => return err_envelope(TOOL_VIEW_SAVE, "invalid_input",
                "view_save: missing required arg `name`"),
        };
        let level = match args.level {
            Some(s) if !s.is_empty() => s,
            _ => return err_envelope(TOOL_VIEW_SAVE, "invalid_input",
                "view_save: missing required arg `level`"),
        };
        let lens = match args.lens {
            Some(s) if !s.is_empty() => s,
            _ => return err_envelope(TOOL_VIEW_SAVE, "invalid_input",
                "view_save: missing required arg `lens`"),
        };
        let focus_node = match args.focus_node {
            Some(s) if !s.is_empty() => s,
            _ => return err_envelope(TOOL_VIEW_SAVE, "invalid_input",
                "view_save: missing required arg `focus_node`"),
        };
        let max_depth = match args.max_depth {
            Some(d) => d,
            None => return err_envelope(TOOL_VIEW_SAVE, "invalid_input",
                "view_save: missing required arg `max_depth`"),
        };

        // Input validation fires BEFORE the persistence availability check,
        // matching the original ExplorerService behavior.
        if max_depth < 0 {
            return err_envelope(TOOL_VIEW_SAVE, "invalid_input",
                "view_save: max_depth must be >= 0");
        }

        let persistence = match &ctx.persistence {
            Some(p) => p,
            None => {
                return err_envelope(
                    TOOL_VIEW_SAVE,
                    "view_specs_require_postgres_feature",
                    "view_specs_require_postgres_feature",
                );
            }
        };

        // Build a ViewSpec from the handler args.
        // The lens parameter maps to view_kind; we use VerticalSlice as default.
        let lens_str = lens.as_str();
        let view_kind = match lens_str {
            "callgraph" | "call-graph" => crate::dto::ViewKind::CallGraph,
            "overview" => crate::dto::ViewKind::VerticalSlice,
            "source" => crate::dto::ViewKind::SourceView,
            "evidence" => crate::dto::ViewKind::EvidenceView,
            "quality" => crate::dto::ViewKind::QualityHotspots,
            "data_flow" => crate::dto::ViewKind::DataFlow,
            "dependency_graph" | "dependencies" => crate::dto::ViewKind::DependencyGraph,
            _ => crate::dto::ViewKind::VerticalSlice,
        };

        let spec = crate::dto::ViewSpec {
            id: uuid_v4_string(),
            title: name.clone(),
            applies_to: crate::dto::InspectableObjectType::Symbol,
            view_kind,
            data_source: crate::dto::DataSource::Moldql {
                query: format!("focus {}", focus_node),
            },
            transform: None,
            renderer_kind: crate::dto::RendererKind::Graph,
            props: serde_json::json!({
                "description": args.description,
                "level": level,
                "focus_node": focus_node,
                "max_depth": max_depth,
            }),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            owner: owner.clone(),
        };

        match persistence.save_view_spec(&spec, &workspace_id, &owner).await {
            Ok(()) => ok_envelope(TOOL_VIEW_SAVE, &serde_json::json!({ "id": spec.id, "title": spec.title })),
            Err(ExplorerError::FeatureDisabled(_)) => err_envelope(
                TOOL_VIEW_SAVE,
                "view_specs_require_postgres_feature",
                "view_specs_require_postgres_feature",
            ),
            Err(ExplorerError::InvalidInput(msg)) => {
                err_envelope(TOOL_VIEW_SAVE, "invalid_input", &msg)
            }
            Err(ExplorerError::Anyhow(_)) => err_envelope(
                TOOL_VIEW_SAVE,
                "storage_error",
                "storage_error",
            ),
            Err(other) => err_envelope(TOOL_VIEW_SAVE, "storage_error", &other.to_string()),
        }
    }
}

// --- view_load ---

struct ViewLoadHandler;

#[async_trait]
impl ToolHandler for ViewLoadHandler {
    fn name(&self) -> &'static str {
        TOOL_VIEW_LOAD
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Named view id returned by `view_save` (required)."
                },
                "workspace_id": {
                    "type": "string",
                    "description": "Workspace id scope guard (required)."
                },
                "owner": {
                    "type": "string",
                    "description": "Owner scope guard (required)."
                }
            },
            "required": ["id", "workspace_id", "owner"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: ViewLoadArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_VIEW_LOAD, "invalid_args",
                &format!("{TOOL_VIEW_LOAD}: invalid args: {e}")),
        };

        let id = match args.id {
            Some(s) if !s.is_empty() => s,
            _ => return err_envelope(TOOL_VIEW_LOAD, "invalid_input",
                "view_load: missing required arg `id`"),
        };
        let workspace_id = match args.workspace_id {
            Some(s) if !s.is_empty() => s,
            _ => return err_envelope(TOOL_VIEW_LOAD, "invalid_input",
                "view_load: missing required arg `workspace_id`"),
        };
        let owner = match args.owner {
            Some(s) if !s.is_empty() => s,
            _ => return err_envelope(TOOL_VIEW_LOAD, "invalid_input",
                "view_load: missing required arg `owner`"),
        };

        let persistence = match &ctx.persistence {
            Some(p) => p,
            None => {
                return err_envelope(
                    TOOL_VIEW_LOAD,
                    "view_specs_require_postgres_feature",
                    "view_specs_require_postgres_feature",
                );
            }
        };

        match persistence.load_view_spec(&id, &workspace_id, &owner).await {
            Ok(Some(spec)) => ok_envelope(TOOL_VIEW_LOAD, &spec),
            Ok(None) => err_envelope(TOOL_VIEW_LOAD, "not_found", "not_found"),
            Err(ExplorerError::FeatureDisabled(_)) => err_envelope(
                TOOL_VIEW_LOAD,
                "view_specs_require_postgres_feature",
                "view_specs_require_postgres_feature",
            ),
            Err(ExplorerError::NotFound(_)) => {
                err_envelope(TOOL_VIEW_LOAD, "not_found", "not_found")
            }
            Err(other) => err_envelope(TOOL_VIEW_LOAD, "storage_error", &other.to_string()),
        }
    }
}

// --- view_list ---

struct ViewListHandler;

#[async_trait]
impl ToolHandler for ViewListHandler {
    fn name(&self) -> &'static str {
        TOOL_VIEW_LIST
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "workspace_id": {
                    "type": "string",
                    "description": "Workspace id (required)."
                },
                "owner": {
                    "type": "string",
                    "description": "Owner (required)."
                }
            },
            "required": ["workspace_id", "owner"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: ViewListArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_VIEW_LIST, "invalid_args",
                &format!("{TOOL_VIEW_LIST}: invalid args: {e}")),
        };

        let workspace_id = match args.workspace_id {
            Some(s) if !s.is_empty() => s,
            _ => return err_envelope(TOOL_VIEW_LIST, "invalid_input",
                "view_list: missing required arg `workspace_id`"),
        };
        let owner = match args.owner {
            Some(s) if !s.is_empty() => s,
            _ => return err_envelope(TOOL_VIEW_LIST, "invalid_input",
                "view_list: missing required arg `owner`"),
        };

        let persistence = match &ctx.persistence {
            Some(p) => p,
            None => {
                return err_envelope(
                    TOOL_VIEW_LIST,
                    "view_specs_require_postgres_feature",
                    "view_specs_require_postgres_feature",
                );
            }
        };

        match persistence.list_view_specs(&workspace_id, &owner).await {
            Ok(specs) => ok_envelope(TOOL_VIEW_LIST, &specs),
            Err(ExplorerError::FeatureDisabled(_)) => err_envelope(
                TOOL_VIEW_LIST,
                "view_specs_require_postgres_feature",
                "view_specs_require_postgres_feature",
            ),
            Err(ExplorerError::InvalidInput(msg)) => {
                err_envelope(TOOL_VIEW_LIST, "invalid_input", &msg)
            }
            Err(other) => err_envelope(TOOL_VIEW_LIST, "storage_error", &other.to_string()),
        }
    }
}

// --- view_delete ---

struct ViewDeleteHandler;

#[async_trait]
impl ToolHandler for ViewDeleteHandler {
    fn name(&self) -> &'static str {
        TOOL_VIEW_DELETE
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Named view id returned by `view_save` (required)."
                },
                "workspace_id": {
                    "type": "string",
                    "description": "Workspace id scope guard (required)."
                },
                "owner": {
                    "type": "string",
                    "description": "Owner scope guard (required)."
                }
            },
            "required": ["id", "workspace_id", "owner"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: ViewDeleteArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_VIEW_DELETE, "invalid_args",
                &format!("{TOOL_VIEW_DELETE}: invalid args: {e}")),
        };

        let id = match args.id {
            Some(s) if !s.is_empty() => s,
            _ => return err_envelope(TOOL_VIEW_DELETE, "invalid_input",
                "view_delete: missing required arg `id`"),
        };
        let workspace_id = match args.workspace_id {
            Some(s) if !s.is_empty() => s,
            _ => return err_envelope(TOOL_VIEW_DELETE, "invalid_input",
                "view_delete: missing required arg `workspace_id`"),
        };
        let owner = match args.owner {
            Some(s) if !s.is_empty() => s,
            _ => return err_envelope(TOOL_VIEW_DELETE, "invalid_input",
                "view_delete: missing required arg `owner`"),
        };

        let persistence = match &ctx.persistence {
            Some(p) => p,
            None => {
                return err_envelope(
                    TOOL_VIEW_DELETE,
                    "view_specs_require_postgres_feature",
                    "view_specs_require_postgres_feature",
                );
            }
        };

        match persistence.delete_view_spec(&id, &workspace_id, &owner).await {
            Ok(removed) => ok_envelope(TOOL_VIEW_DELETE, &serde_json::json!({ "deleted": removed })),
            Err(ExplorerError::FeatureDisabled(_)) => err_envelope(
                TOOL_VIEW_DELETE,
                "view_specs_require_postgres_feature",
                "view_specs_require_postgres_feature",
            ),
            Err(ExplorerError::NotFound(_)) => {
                err_envelope(TOOL_VIEW_DELETE, "not_found", "not_found")
            }
            Err(ExplorerError::InvalidInput(msg)) => {
                err_envelope(TOOL_VIEW_DELETE, "invalid_input", &msg)
            }
            Err(other) => err_envelope(TOOL_VIEW_DELETE, "storage_error", &other.to_string()),
        }
    }
}

// ============================================================================
// Registry builder
// ============================================================================

/// Register all 4 named-views handlers into the registry.
pub fn register_named_views_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(ViewSaveHandler);
    registry.register(ViewLoadHandler);
    registry.register(ViewListHandler);
    registry.register(ViewDeleteHandler);
}
