//! Session-family tool handlers.
//!
//! Implements 9 MCP tools for brain-session management:
//! - `brain_open`    — open a new session
//! - `brain_attach`  — rejoin an existing session
//! - `brain_ask`     — ask a question within a session (focus-aware)
//! - `brain_focus`   — set the session's focus node
//! - `brain_status`   — get session status and metadata
//! - `brain_close`   — close (invalidate) a session
//!
//! Plus 3 multimodal tools (feature-gated):
//! - `brain_add_space`    — register a space in a session
//! - `brain_remove_space` — unregister a space from a session
//! - `brain_spaces`       — list registered spaces in a session

use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::{CallToolResult, Content};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp::envelope::{err_envelope, ok_envelope_with_provenance};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{McpContext, ProvenanceMetadata, TOOL_BRAIN_OPEN, TOOL_BRAIN_ATTACH, TOOL_BRAIN_ASK, TOOL_BRAIN_FOCUS, TOOL_BRAIN_STATUS, TOOL_BRAIN_CLOSE};
#[cfg(feature = "multimodal")]
use crate::mcp::{TOOL_BRAIN_ADD_SPACE, TOOL_BRAIN_REMOVE_SPACE, TOOL_BRAIN_SPACES};
use crate::session::DEFAULT_TTL_SECS;

// ============================================================================
// Arg structs — co-located with handler logic per the design decision.
// ============================================================================

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainOpenArgs {
    workspace_id: Option<String>,
    ttl: Option<u64>,
    #[cfg(feature = "multimodal")]
    #[serde(default)]
    spaces: Option<Vec<SpaceSpec>>,
}

#[cfg(feature = "multimodal")]
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceSpec {
    space_name: Option<String>,
    space_kind: Option<String>,
    source_path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainAttachArgs {
    session_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainAskArgs {
    session_id: Option<String>,
    question: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainFocusArgs {
    session_id: Option<String>,
    focus_node: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainStatusArgs {
    session_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainCloseArgs {
    session_id: Option<String>,
}

#[cfg(feature = "multimodal")]
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainAddSpaceArgs {
    session_id: Option<String>,
    space_name: Option<String>,
    space_kind: Option<String>,
    source_path: Option<String>,
}

#[cfg(feature = "multimodal")]
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainRemoveSpaceArgs {
    session_id: Option<String>,
    space_id: Option<String>,
}

#[cfg(feature = "multimodal")]
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainSpacesArgs {
    session_id: Option<String>,
}

// ============================================================================
// ToolHandler implementations — one struct per tool
// ============================================================================

macro_rules! make_handler {
    ($name:ident, $tool_name:expr, $args:ty) => {
        struct $name;
        impl $name {
            fn schema() -> Value {
                serde_json::json!({
                    "type": "object",
                    "properties": <$args>::schema_fields(),
                })
            }
        }
    };
}

// --- brain_open ---

/// Handler for `brain_open` — opens a new session and returns its id + state.
struct BrainOpenHandler;

#[async_trait]
impl ToolHandler for BrainOpenHandler {
    fn name(&self) -> &'static str {
        TOOL_BRAIN_OPEN
    }

    fn arg_schema(&self) -> Value {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": {
                "workspace_id": {
                    "type": "string",
                    "description": "Workspace identifier for the new session (required)."
                },
                "ttl": {
                    "type": "integer",
                    "description": "Session time-to-live in seconds. 0 = no expiry. Range: 0..=86400 (24h). Defaults to 3600."
                }
            },
            "required": ["workspace_id"]
        });
        #[cfg(feature = "multimodal")]
        {
            schema["properties"]["spaces"] = serde_json::json!({
                "type": "array",
                "description": "Optional list of spaces to pre-register on session open.",
                "items": {
                    "type": "object",
                    "properties": {
                        "space_name": { "type": "string" },
                        "space_kind": { "type": "string", "enum": ["Repo", "Docs", "Issues"] },
                        "source_path": { "type": "string" }
                    }
                }
            });
        }
        schema
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: BrainOpenArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return self.err("missing_required_arg", &format!("{TOOL_BRAIN_OPEN}: invalid args: {e}")),
        };

        let workspace_id = match args.workspace_id {
            Some(w) if !w.is_empty() => w,
            _ => return self.err("invalid_workspace_id", "missing or empty required arg `workspace_id`"),
        };

        let ttl = args.ttl.unwrap_or(DEFAULT_TTL_SECS);
        if ttl > 86_400 {
            return self.err("invalid_ttl", "ttl must be in 0..=86400 (24h); 0 disables expiry");
        }

        let session_id = ctx.session_registry.open(
            workspace_id.clone(),
            ttl,
            ctx.search.as_ref().expect("search facade must be wired").clone(),
            ctx.view.as_ref().expect("view facade must be wired").clone(),
            ctx.workspace.as_ref().expect("workspace facade must be wired").clone(),
            ctx.graph.clone(),
        );

        // Multimodal: pre-register spaces if supplied.
        #[cfg(feature = "multimodal")]
        let mut space_errors: Vec<String> = Vec::new();
        #[cfg(feature = "multimodal")]
        if let Some(ref space_specs) = args.spaces {
            if !space_specs.is_empty() {
                use cognicode_core::domain::value_objects::{SpaceId, SpaceKind, Space};
                if let Ok(session) = ctx.session_registry.get(&session_id) {
                    for spec in space_specs {
                        if let (Some(ref name), Some(ref k)) = (&spec.space_name, &spec.space_kind) {
                            if !name.is_empty() && !k.is_empty() {
                                let kind = match k.to_lowercase().as_str() {
                                    "repo" => SpaceKind::Repo,
                                    "docs" => SpaceKind::Docs,
                                    "issues" => SpaceKind::Issues,
                                    other => {
                                        space_errors.push(format!("unknown space_kind '{other}' for space '{name}'"));
                                        continue;
                                    }
                                };
                                let sid = match SpaceId::try_new(name.clone()) {
                                    Ok(s) => s,
                                    Err(_) => {
                                        space_errors.push(format!("invalid space id '{name}'"));
                                        continue;
                                    }
                                };
                                let space = match Space::try_new(sid, name.clone(), kind) {
                                    Ok(s) => s,
                                    Err(e) => {
                                        space_errors.push(format!("could not build space '{name}': {e}"));
                                        continue;
                                    }
                                };
                                let space = match spec.source_path {
                                    Some(ref p) if !p.is_empty() => space.with_source_path(p.clone()),
                                    _ => space,
                                };
                                if let Err(e) = session.add_space(space) {
                                    space_errors.push(format!("add_space('{name}') failed: {e}"));
                                }
                            }
                        }
                    }
                }
                if !space_errors.is_empty() {
                    tracing::warn!(
                        "brain_open: {} space(s) failed to attach: {:?}",
                        space_errors.len(),
                        space_errors
                    );
                }
            }
        }

        let snap = ctx.session_registry
            .attach(&session_id)
            .expect("freshly opened session must be present")
            .snapshot();

        #[cfg(feature = "multimodal")]
        let space_errors_json = serde_json::json!(space_errors);
        #[cfg(not(feature = "multimodal"))]
        let space_errors_json = serde_json::json!([]);

        self.ok(&serde_json::json!({
            "session_id": session_id,
            "workspace_id": workspace_id,
            "ttl_secs": ttl,
            "state": snap,
            "space_errors": space_errors_json,
        }))
    }
}

impl BrainOpenHandler {
    fn err(&self, code: &str, msg: &str) -> CallToolResult {
        err_envelope(TOOL_BRAIN_OPEN, code, msg)
    }
    fn ok(&self, payload: &Value) -> CallToolResult {
        ok_envelope_with_provenance(TOOL_BRAIN_OPEN, payload, brain_provenance())
    }
}

// --- brain_attach ---

/// Handler for `brain_attach` — rejoin an existing session, refresh TTL.
struct BrainAttachHandler;

#[async_trait]
impl ToolHandler for BrainAttachHandler {
    fn name(&self) -> &'static str {
        TOOL_BRAIN_ATTACH
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "The session id to rejoin (required)."
                }
            },
            "required": ["session_id"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: BrainAttachArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return self.err("missing_required_arg", &format!("{TOOL_BRAIN_ATTACH}: invalid args: {e}")),
        };

        let session_id = match args.session_id {
            Some(s) if !s.is_empty() => s,
            _ => return self.err("missing_required_arg", "missing required arg `session_id`"),
        };

        ctx.session_registry.resolve_session_attached(TOOL_BRAIN_ATTACH, &session_id, |session| {
            let snap = session.snapshot();
            self.ok(&serde_json::json!({
                "session_id": session_id,
                "workspace_id": snap.workspace_id,
                "last_activity": snap.last_activity,
                "ttl_secs": snap.ttl,
                "focus_node": snap.focus_node,
            }))
        })
    }
}

impl BrainAttachHandler {
    fn err(&self, code: &str, msg: &str) -> CallToolResult {
        err_envelope(TOOL_BRAIN_ATTACH, code, msg)
    }
    fn ok(&self, payload: &Value) -> CallToolResult {
        ok_envelope_with_provenance(TOOL_BRAIN_ATTACH, payload, brain_provenance())
    }
}

// --- brain_ask ---

/// Handler for `brain_ask` — ask a question within a session (focus-aware).
struct BrainAskHandler;

#[async_trait]
impl ToolHandler for BrainAskHandler {
    fn name(&self) -> &'static str {
        TOOL_BRAIN_ASK
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "Session id (required)."
                },
                "question": {
                    "type": "string",
                    "description": "Natural-language question (required)."
                }
            },
            "required": ["session_id", "question"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: BrainAskArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return self.err("missing_required_arg", &format!("{TOOL_BRAIN_ASK}: invalid args: {e}")),
        };

        let session_id = match args.session_id {
            Some(s) if !s.is_empty() => s,
            _ => return self.err("missing_required_arg", "missing required arg `session_id`"),
        };

        let question = match args.question {
            Some(q) if !q.is_empty() => q,
            _ => return self.err("missing_required_arg", "missing required arg `question`"),
        };

        ctx.session_registry.resolve_session_async(TOOL_BRAIN_ASK, &session_id, |session| async move {
            let mut env = session.ask_with_session(&question).await;
            // Override provenance source to "brain-session" so consumers
            // can distinguish brain-mediated answers from raw ask answers.
            match env.provenance.as_mut() {
                Some(p) => p.source = Some("brain-session".to_string()),
                None => {
                    env.provenance = Some(crate::mcp::ProvenanceMetadata {
                        confidence: None,
                        source: Some("brain-session".to_string()),
                    });
                }
            }
            self.ok_brain_envelope(env)
        }).await
    }
}

impl BrainAskHandler {
    fn err(&self, code: &str, msg: &str) -> CallToolResult {
        err_envelope(TOOL_BRAIN_ASK, code, msg)
    }

    /// Handle ask — returns the full nested envelope as the payload.
    fn ok_brain_envelope(&self, env: crate::mcp::McpResultEnvelope<serde_json::Value>) -> CallToolResult {
        let json = serde_json::to_value(&env).unwrap_or(serde_json::Value::Null);
        ok_envelope_with_provenance(TOOL_BRAIN_ASK, &json, brain_provenance())
    }
}

// --- brain_focus ---

/// Handler for `brain_focus` — set the session's focus node.
struct BrainFocusHandler;

#[async_trait]
impl ToolHandler for BrainFocusHandler {
    fn name(&self) -> &'static str {
        TOOL_BRAIN_FOCUS
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "Session id (required)."
                },
                "focus_node": {
                    "type": "string",
                    "description": "MVP id of the focus node. Pass null or omit to clear. Empty string is an error."
                }
            },
            "required": ["session_id"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: BrainFocusArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return self.err("missing_required_arg", &format!("{TOOL_BRAIN_FOCUS}: invalid args: {e}")),
        };

        let session_id = match args.session_id {
            Some(s) if !s.is_empty() => s,
            _ => return self.err("missing_required_arg", "missing required arg `session_id`"),
        };

        // Empty string is explicit error; None/null means clear.
        let focus = match args.focus_node {
            Some(ref f) if f.is_empty() => {
                return self.err("invalid_focus_node", "focus_node must be a non-empty string or null");
            }
            Some(f) => Some(f),
            None => None,
        };

        ctx.session_registry.resolve_session(TOOL_BRAIN_FOCUS, &session_id, |session| {
            session.set_focus(focus.clone());
            self.ok(&serde_json::json!({
                "session_id": session_id,
                "focus_node": focus,
            }))
        })
    }
}

impl BrainFocusHandler {
    fn err(&self, code: &str, msg: &str) -> CallToolResult {
        err_envelope(TOOL_BRAIN_FOCUS, code, msg)
    }
    fn ok(&self, payload: &Value) -> CallToolResult {
        ok_envelope_with_provenance(TOOL_BRAIN_FOCUS, payload, brain_provenance())
    }
}

// --- brain_status ---

/// Handler for `brain_status` — get session status and metadata.
struct BrainStatusHandler;

#[async_trait]
impl ToolHandler for BrainStatusHandler {
    fn name(&self) -> &'static str {
        TOOL_BRAIN_STATUS
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "Session id (required)."
                }
            },
            "required": ["session_id"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: BrainStatusArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return self.err("missing_required_arg", &format!("{TOOL_BRAIN_STATUS}: invalid args: {e}")),
        };

        let session_id = match args.session_id {
            Some(s) if !s.is_empty() => s,
            _ => return self.err("missing_required_arg", "missing required arg `session_id`"),
        };

        ctx.session_registry.resolve_session(TOOL_BRAIN_STATUS, &session_id, |session| {
            let snap = session.snapshot();

            #[cfg(feature = "multimodal")]
            {
                let space_details: Vec<Value> = session
                    .spaces()
                    .into_iter()
                    .map(|s| {
                        serde_json::json!({
                            "id": s.id.as_str(),
                            "name": s.name,
                            "kind": s.kind.as_str(),
                            "source_path": s.source_path.map(|p| p.to_string_lossy().into_owned()),
                        })
                    })
                    .collect();
                let space_count = space_details.len();
                let mut payload = serde_json::to_value(&snap).unwrap_or(Value::Null);
                if let Some(ref mut obj) = payload.as_object_mut() {
                    obj.insert("space_count".to_string(), serde_json::json!(space_count));
                    obj.insert("space_details".to_string(), serde_json::json!(space_details));
                }
                self.ok(&payload)
            }

            #[cfg(not(feature = "multimodal"))]
            {
                let payload = serde_json::to_value(&snap).unwrap_or(Value::Null);
                self.ok(&payload)
            }
        })
    }
}

impl BrainStatusHandler {
    fn err(&self, code: &str, msg: &str) -> CallToolResult {
        err_envelope(TOOL_BRAIN_STATUS, code, msg)
    }
    fn ok(&self, payload: &Value) -> CallToolResult {
        ok_envelope_with_provenance(TOOL_BRAIN_STATUS, payload, brain_provenance())
    }
}

// --- brain_close ---

/// Handler for `brain_close` — close (invalidate) a session.
struct BrainCloseHandler;

#[async_trait]
impl ToolHandler for BrainCloseHandler {
    fn name(&self) -> &'static str {
        TOOL_BRAIN_CLOSE
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "Session id to close (required)."
                }
            },
            "required": ["session_id"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: BrainCloseArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return self.err("missing_required_arg", &format!("{TOOL_BRAIN_CLOSE}: invalid args: {e}")),
        };

        let session_id = match args.session_id {
            Some(s) if !s.is_empty() => s,
            _ => return self.err("missing_required_arg", "missing required arg `session_id`"),
        };

        // Idempotent: unknown/closed → closed: false, NOT an error.
        let closed = ctx.session_registry.close(&session_id);
        self.ok(&serde_json::json!({
            "session_id": session_id,
            "closed": closed,
        }))
    }
}

impl BrainCloseHandler {
    fn err(&self, code: &str, msg: &str) -> CallToolResult {
        err_envelope(TOOL_BRAIN_CLOSE, code, msg)
    }
    fn ok(&self, payload: &Value) -> CallToolResult {
        ok_envelope_with_provenance(TOOL_BRAIN_CLOSE, payload, brain_provenance())
    }
}

// --- Multimodal: brain_add_space ---

#[cfg(feature = "multimodal")]
struct BrainAddSpaceHandler;

#[cfg(feature = "multimodal")]
#[async_trait]
impl ToolHandler for BrainAddSpaceHandler {
    fn name(&self) -> &'static str {
        TOOL_BRAIN_ADD_SPACE
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": { "type": "string", "description": "Session id (required)." },
                "space_name": { "type": "string", "description": "Space name (required)." },
                "space_kind": { "type": "string", "enum": ["Repo", "Docs", "Issues"], "description": "Space kind (required)." },
                "source_path": { "type": "string", "description": "Optional source path or URL." }
            },
            "required": ["session_id", "space_name", "space_kind"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        use cognicode_core::domain::value_objects::{Space, SpaceId, SpaceKind};

        let args: BrainAddSpaceArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return self.err("missing_required_arg", &format!("{TOOL_BRAIN_ADD_SPACE}: invalid args: {e}")),
        };

        let session_id = match args.session_id {
            Some(s) if !s.is_empty() => s,
            _ => return self.err("missing_required_arg", "missing required arg `session_id`"),
        };
        let space_name = match args.space_name {
            Some(n) if !n.is_empty() => n,
            _ => return self.err("missing_required_arg", "missing required arg `space_name`"),
        };
        let kind_str = match args.space_kind {
            Some(ref k) if !k.is_empty() => k.clone(),
            _ => return self.err("missing_required_arg", "missing required arg `space_kind`"),
        };
        let space_kind = match kind_str.to_lowercase().as_str() {
            "repo" => SpaceKind::Repo,
            "docs" => SpaceKind::Docs,
            "issues" => SpaceKind::Issues,
            other => return self.err("invalid_space_kind",
                &format!("invalid space_kind `{other}`: expected one of Repo, Docs, Issues")),
        };
        let space_id = match SpaceId::try_new(space_name.clone()) {
            Ok(id) => id,
            Err(_) => return self.err("invalid_space_id", "space name could not be converted to a valid space id"),
        };
        let space = match Space::try_new(space_id, space_name.clone(), space_kind) {
            Ok(s) => s,
            Err(e) => return self.err("space_construction_error", &format!("failed to construct space: {e}")),
        };
        let space = match args.source_path {
            Some(ref p) if !p.is_empty() => space.with_source_path(p.clone()),
            _ => space,
        };

        ctx.session_registry.resolve_session(TOOL_BRAIN_ADD_SPACE, &session_id, |session| {
            if let Err(e) = session.add_space(space) {
                return self.err("duplicate_space_id", &format!("duplicate space id: {e}"));
            }

            self.ok(&serde_json::json!({
                "space_id": space_name,
                "space_name": space_name,
                "space_kind": space_kind.as_str(),
            }))
        })
    }
}

#[cfg(feature = "multimodal")]
impl BrainAddSpaceHandler {
    fn err(&self, code: &str, msg: &str) -> CallToolResult {
        err_envelope(TOOL_BRAIN_ADD_SPACE, code, msg)
    }
    fn ok(&self, payload: &Value) -> CallToolResult {
        ok_envelope_with_provenance(TOOL_BRAIN_ADD_SPACE, payload, brain_provenance())
    }
}

// --- Multimodal: brain_remove_space ---

#[cfg(feature = "multimodal")]
struct BrainRemoveSpaceHandler;

#[cfg(feature = "multimodal")]
#[async_trait]
impl ToolHandler for BrainRemoveSpaceHandler {
    fn name(&self) -> &'static str {
        TOOL_BRAIN_REMOVE_SPACE
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": { "type": "string", "description": "Session id (required)." },
                "space_id": { "type": "string", "description": "Space id to remove (required)." }
            },
            "required": ["session_id", "space_id"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        use cognicode_core::domain::value_objects::SpaceId;

        let args: BrainRemoveSpaceArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return self.err("missing_required_arg", &format!("{TOOL_BRAIN_REMOVE_SPACE}: invalid args: {e}")),
        };

        let session_id = match args.session_id {
            Some(s) if !s.is_empty() => s,
            _ => return self.err("missing_required_arg", "missing required arg `session_id`"),
        };
        let space_id_str = match args.space_id {
            Some(s) if !s.is_empty() => s,
            _ => return self.err("missing_required_arg", "missing required arg `space_id`"),
        };
        let space_id = match SpaceId::try_new(&space_id_str) {
            Ok(id) => id,
            Err(_) => return self.err("invalid_space_id", &format!("invalid space_id `{space_id_str}`")),
        };

        ctx.session_registry.resolve_session(TOOL_BRAIN_REMOVE_SPACE, &session_id, |session| {
            let removed = session.remove_space(&space_id);
            self.ok(&serde_json::json!({ "removed": removed }))
        })
    }
}

#[cfg(feature = "multimodal")]
impl BrainRemoveSpaceHandler {
    fn err(&self, code: &str, msg: &str) -> CallToolResult {
        err_envelope(TOOL_BRAIN_REMOVE_SPACE, code, msg)
    }
    fn ok(&self, payload: &Value) -> CallToolResult {
        ok_envelope_with_provenance(TOOL_BRAIN_REMOVE_SPACE, payload, brain_provenance())
    }
}

// --- Multimodal: brain_spaces ---

#[cfg(feature = "multimodal")]
struct BrainSpacesHandler;

#[cfg(feature = "multimodal")]
#[async_trait]
impl ToolHandler for BrainSpacesHandler {
    fn name(&self) -> &'static str {
        TOOL_BRAIN_SPACES
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": { "type": "string", "description": "Session id (required)." }
            },
            "required": ["session_id"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: BrainSpacesArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return self.err("missing_required_arg", &format!("{TOOL_BRAIN_SPACES}: invalid args: {e}")),
        };

        let session_id = match args.session_id {
            Some(s) if !s.is_empty() => s,
            _ => return self.err("missing_required_arg", "missing required arg `session_id`"),
        };

        ctx.session_registry.resolve_session(TOOL_BRAIN_SPACES, &session_id, |session| {
            let spaces: Vec<Value> = session
                .spaces()
                .into_iter()
                .map(|s| {
                    serde_json::json!({
                        "id": s.id.as_str(),
                        "name": s.name,
                        "kind": s.kind.as_str(),
                        "source_path": s.source_path.map(|p| p.to_string_lossy().into_owned()),
                    })
                })
                .collect();

            self.ok(&serde_json::json!({ "spaces": spaces }))
        })
    }
}

#[cfg(feature = "multimodal")]
impl BrainSpacesHandler {
    fn err(&self, code: &str, msg: &str) -> CallToolResult {
        err_envelope(TOOL_BRAIN_SPACES, code, msg)
    }
    fn ok(&self, payload: &Value) -> CallToolResult {
        ok_envelope_with_provenance(TOOL_BRAIN_SPACES, payload, brain_provenance())
    }
}

// ============================================================================
// Schema helper for arg structs — enables `#[serde(default)]` + schema
// extraction without a proc macro.
// ============================================================================

trait SchemaFields {
    fn schema_fields() -> serde_json::Map<String, serde_json::Value>;
}

impl SchemaFields for BrainOpenArgs {
    fn schema_fields() -> serde_json::Map<String, serde_json::Value> {
        let mut m = serde_json::Map::new();
        m.insert("workspace_id".to_string(), serde_json::json!({
            "type": "string",
            "description": "Workspace identifier for the new session (required)."
        }));
        m.insert("ttl".to_string(), serde_json::json!({
            "type": "integer",
            "description": "Session time-to-live in seconds. 0 = no expiry. Range: 0..=86400."
        }));
        m
    }
}

impl SchemaFields for BrainAttachArgs {
    fn schema_fields() -> serde_json::Map<String, serde_json::Value> {
        let mut m = serde_json::Map::new();
        m.insert("session_id".to_string(), serde_json::json!({
            "type": "string",
            "description": "The session id to rejoin (required)."
        }));
        m
    }
}

impl SchemaFields for BrainAskArgs {
    fn schema_fields() -> serde_json::Map<String, serde_json::Value> {
        let mut m = serde_json::Map::new();
        m.insert("session_id".to_string(), serde_json::json!({
            "type": "string",
            "description": "Session id (required)."
        }));
        m.insert("question".to_string(), serde_json::json!({
            "type": "string",
            "description": "Natural-language question (required)."
        }));
        m
    }
}

impl SchemaFields for BrainFocusArgs {
    fn schema_fields() -> serde_json::Map<String, serde_json::Value> {
        let mut m = serde_json::Map::new();
        m.insert("session_id".to_string(), serde_json::json!({
            "type": "string",
            "description": "Session id (required)."
        }));
        m.insert("focus_node".to_string(), serde_json::json!({
            "type": "string",
            "description": "MVP id of the focus node. Omit or pass null to clear. Empty string is an error."
        }));
        m
    }
}

impl SchemaFields for BrainStatusArgs {
    fn schema_fields() -> serde_json::Map<String, serde_json::Value> {
        let mut m = serde_json::Map::new();
        m.insert("session_id".to_string(), serde_json::json!({
            "type": "string",
            "description": "Session id (required)."
        }));
        m
    }
}

impl SchemaFields for BrainCloseArgs {
    fn schema_fields() -> serde_json::Map<String, serde_json::Value> {
        let mut m = serde_json::Map::new();
        m.insert("session_id".to_string(), serde_json::json!({
            "type": "string",
            "description": "Session id to close (required)."
        }));
        m
    }
}

// ============================================================================
// Envelope helpers — use the shared mcp::envelope module.
// err_with_code and ok_brain are removed; the impl blocks delegate to
// err_envelope and ok_envelope_with_provenance via brain_provenance().
// ============================================================================

/// Provenance metadata for all brain-session tools.
fn brain_provenance() -> ProvenanceMetadata {
    ProvenanceMetadata {
        source: Some("brain-session".to_string()),
        confidence: None,
    }
}

// ============================================================================
// Registry builder — populates the registry with all session handlers.
// Called by the module that wires the full server.
// ============================================================================

/// Register all 9 session-family handlers into the registry.
pub fn register_session_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(BrainOpenHandler);
    registry.register(BrainAttachHandler);
    registry.register(BrainAskHandler);
    registry.register(BrainFocusHandler);
    registry.register(BrainStatusHandler);
    registry.register(BrainCloseHandler);
    #[cfg(feature = "multimodal")]
    {
        registry.register(BrainAddSpaceHandler);
        registry.register(BrainRemoveSpaceHandler);
        registry.register(BrainSpacesHandler);
    }
}

// ============================================================================
// Regression tests — lock wire-level error/success semantics
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{CallToolResult, Content};
    use serde_json::json;

    // ------------------------------------------------------------------------
    // err_envelope wire semantics
    // ------------------------------------------------------------------------

    /// Regression: err_envelope must return CallToolResult::error, NOT success.
    /// The sessions.rs:988 bug was that err_with_code returned success.
    #[test]
    fn err_envelope_returns_error_variant() {
        let result = err_envelope("brain_open", "session_not_found", "no session");
        assert!(
            matches!(result, CallToolResult::Error(_)),
            "err_envelope must return CallToolResult::error, not success"
        );
    }

    #[test]
    fn err_envelope_json_payload_has_error_code() {
        let result = err_envelope("brain_attach", "session_not_found", "not found");
        let CallToolResult::Error(items) = result else {
            panic!("expected CallToolResult::Error");
        };
        let Content::Text(text) = &items[0] else {
            panic!("expected Content::Text");
        };
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["payload"]["error_code"], "session_not_found");
        assert_eq!(parsed["payload"]["error"], "not found");
        assert_eq!(parsed["tool_name"], "brain_attach");
    }

    #[test]
    fn err_envelope_all_session_error_codes() {
        // These are the 7 error codes used across session handlers.
        let codes = [
            ("brain_open", "invalid_workspace_id"),
            ("brain_attach", "session_not_found"),
            ("brain_attach", "session_expired"),
            ("brain_ask", "session_not_found"),
            ("brain_focus", "invalid_focus_node"),
            ("brain_open", "invalid_ttl"),
            ("brain_add_space", "invalid_space_id"),
        ];
        for (tool, code) in codes {
            let result = err_envelope(tool, code, "test message");
            assert!(
                matches!(result, CallToolResult::Error(_)),
                "err_envelope({tool}, {code}, ...) must return Error variant"
            );
        }
    }

    // ------------------------------------------------------------------------
    // ok_envelope_with_provenance wire semantics
    // ------------------------------------------------------------------------

    #[test]
    fn ok_envelope_with_provenance_returns_success_variant() {
        let result = ok_envelope_with_provenance(
            "brain_open",
            &serde_json::json!({}),
            brain_provenance(),
        );
        assert!(
            matches!(result, CallToolResult::Success(_)),
            "ok_envelope_with_provenance must return CallToolResult::success"
        );
    }

    #[test]
    fn ok_envelope_with_provenance_has_brain_session_provenance() {
        let result = ok_envelope_with_provenance(
            "brain_ask",
            &serde_json::json!({"answer": "42"}),
            brain_provenance(),
        );
        let CallToolResult::Success(items) = result else {
            panic!("expected CallToolResult::Success");
        };
        let Content::Text(text) = &items[0] else {
            panic!("expected Content::Text");
        };
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["provenance"]["source"], "brain-session");
    }

    // ------------------------------------------------------------------------
    // brain_provenance
    // ------------------------------------------------------------------------

    #[test]
    fn brain_provenance_has_brain_session_source() {
        let prov = brain_provenance();
        assert_eq!(prov.source.as_deref(), Some("brain-session"));
        assert!(prov.confidence.is_none());
    }

    #[test]
    fn brain_provenance_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ProvenanceMetadata>();
    }
}
