//! Wire shape of a view in list responses, shared by the MCP handlers and the
//! built-in descriptor table. Serde only: it names no interface type.

use serde::{Deserialize, Serialize};

/// Lightweight view descriptor used in list responses.
/// Mirrors cognicode_explorer::dto::ViewDescriptorDto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDescriptor {
    pub id: String,
    pub title: String,
    /// Wire-format view kind tag (e.g. "decision_support_pack", "call_graph").
    #[serde(default)]
    pub view_kind: String,
    #[serde(default)]
    pub is_builtin: bool,
    /// Source tag: "runtime" for persisted specs, None for built-ins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}
