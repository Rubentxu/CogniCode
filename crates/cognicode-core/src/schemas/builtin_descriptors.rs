//! Built-in view descriptors — single source of truth for the 8 compiled-in views.
//!
//! Used by both the MCP handlers (consolidated_handlers.rs) and the explorer
//! registry (registry.rs) to avoid duplicating the descriptor list.

use crate::interface::mcp::schemas::ViewDescriptor;

/// Raw descriptor data that can be used to construct ViewDescriptors at runtime.
pub struct BuiltinDescriptorRaw {
    pub id: &'static str,
    pub title: &'static str,
    /// Wire-format view_kind tag (e.g. "call_graph", "vertical_slice").
    pub view_kind: &'static str,
}

/// The 8 built-in view descriptors (raw data).
/// Keep in sync with REAL_EXECUTOR_DESCRIPTORS in explorer registry.rs.
pub const BUILTIN_DESCRIPTORS_RAW: &[BuiltinDescriptorRaw] = &[
    BuiltinDescriptorRaw {
        id: "overview",
        title: "Overview",
        view_kind: "vertical_slice",
    },
    BuiltinDescriptorRaw {
        id: "call-graph",
        title: "Call Graph",
        view_kind: "call_graph",
    },
    BuiltinDescriptorRaw {
        id: "source",
        title: "Source",
        view_kind: "source_view",
    },
    BuiltinDescriptorRaw {
        id: "quality",
        title: "Quality",
        view_kind: "quality_hotspots",
    },
    BuiltinDescriptorRaw {
        id: "evidence",
        title: "Evidence",
        view_kind: "evidence_view",
    },
    BuiltinDescriptorRaw {
        id: "symbols",
        title: "Symbols",
        view_kind: "usage_examples",
    },
    BuiltinDescriptorRaw {
        id: "dependencies",
        title: "Dependencies",
        view_kind: "dependency_graph",
    },
    BuiltinDescriptorRaw {
        id: "hotspots",
        title: "Hotspots",
        view_kind: "quality_hotspots",
    },
];

impl BuiltinDescriptorRaw {
    /// Convert raw descriptor to a full ViewDescriptor.
    pub fn to_view_descriptor(&self) -> ViewDescriptor {
        ViewDescriptor {
            id: self.id.to_string(),
            title: self.title.to_string(),
            view_kind: self.view_kind.to_string(),
            is_builtin: true,
            source: None,
        }
    }
}

/// Returns the 8 built-in view descriptors as a Vec.
pub fn builtin_descriptors() -> Vec<ViewDescriptor> {
    BUILTIN_DESCRIPTORS_RAW
        .iter()
        .map(|raw| raw.to_view_descriptor())
        .collect()
}
