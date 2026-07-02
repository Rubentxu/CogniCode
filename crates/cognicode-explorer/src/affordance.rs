//! Affordance matrix for typed overview affordances.
//!
//! Provides static affordance data for known `InspectableObjectType` variants.
//! Graceful degradation: unknown types return an empty vector (not an error).

use serde::{Deserialize, Serialize};

// ============================================================================
// Core types
// ============================================================================

/// An affordance is a typed action users can take on an object.
///
/// `scaffold_id` is `None` when the affordance uses a custom MoldQL query
/// rather than a pre-defined scaffold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Affordance {
    /// Wire-compatible object type this affordance applies to.
    pub object_type: &'static str,
    /// Short human-readable label.
    pub label: &'static str,
    /// One-sentence description of what the affordance shows.
    pub description: &'static str,
    /// The semantic ViewKind this affordance selects.
    pub view_kind: &'static str,
    /// Optional pre-defined scaffold identifier.
    pub scaffold_id: Option<&'static str>,
    /// Display priority; lower values sort first.
    pub priority: u8,
}

/// A sorted list of affordances.
pub type AffordanceMatrix = &'static [Affordance];

// ============================================================================
// Known object types (must match `InspectableObjectType` in dto.rs)
// ============================================================================

/// Object types covered by the static matrix.
pub const KNOWN_OBJECT_TYPES: &[&str] = &[
    "workspace",
    "scope",
    "symbol",
    "file",
    "module",
    "evidence",
    "decision_artifact",
    "quality_issue",
    "rule",
    "investigation",
];

// ============================================================================
// Static affordance matrix
// ============================================================================

/// Static affordance data for all known `InspectableObjectType` variants.
///
/// Each object's affordances are sorted by ascending `priority`.
pub static AFFORDANCE_MATRIX: &[Affordance] = &[
    // ------------------------------------------------------------------------
    // workspace
    // ------------------------------------------------------------------------
    Affordance {
        object_type: "workspace",
        label: "Overview",
        description: "Summary of the workspace: entry points, god nodes, hot paths.",
        view_kind: "architecture_rationale",
        scaffold_id: Some("workspace_overview"),
        priority: 1,
    },
    Affordance {
        object_type: "workspace",
        label: "Dependency Graph",
        description: "Full crate/package dependency graph.",
        view_kind: "dependency_graph",
        scaffold_id: Some("workspace_dep_graph"),
        priority: 2,
    },
    Affordance {
        object_type: "workspace",
        label: "Quality Hotspots",
        description: "Files and symbols with the most quality issues.",
        view_kind: "quality_hotspots",
        scaffold_id: Some("workspace_quality_hotspots"),
        priority: 3,
    },
    // ------------------------------------------------------------------------
    // scope
    // ------------------------------------------------------------------------
    Affordance {
        object_type: "scope",
        label: "Module Tree",
        description: "Navigable tree of modules, traits, and structs.",
        view_kind: "source_view",
        scaffold_id: Some("scope_module_tree"),
        priority: 1,
    },
    Affordance {
        object_type: "scope",
        label: "Call Hierarchy",
        description: "Top-level callers and callees for this scope.",
        view_kind: "call_graph",
        scaffold_id: Some("scope_call_hierarchy"),
        priority: 2,
    },
    Affordance {
        object_type: "scope",
        label: "Quality Gate",
        description: "Issue counts and rating for this scope.",
        view_kind: "quality_hotspots",
        scaffold_id: Some("scope_quality_gate"),
        priority: 3,
    },
    // ------------------------------------------------------------------------
    // symbol
    // ------------------------------------------------------------------------
    Affordance {
        object_type: "symbol",
        label: "Vertical Slice",
        description: "Full call chain plus data flow from this symbol.",
        view_kind: "vertical_slice",
        scaffold_id: Some("symbol_vertical_slice"),
        priority: 1,
    },
    Affordance {
        object_type: "symbol",
        label: "Call Graph",
        description: "Callers and callees around this symbol.",
        view_kind: "call_graph",
        scaffold_id: Some("symbol_call_graph"),
        priority: 2,
    },
    Affordance {
        object_type: "symbol",
        label: "Source",
        description: "Source file slice around this symbol.",
        view_kind: "source_view",
        scaffold_id: Some("symbol_source"),
        priority: 3,
    },
    // ------------------------------------------------------------------------
    // file
    // ------------------------------------------------------------------------
    Affordance {
        object_type: "file",
        label: "Source",
        description: "Full file contents with syntax highlighting.",
        view_kind: "source_view",
        scaffold_id: Some("file_source"),
        priority: 1,
    },
    Affordance {
        object_type: "file",
        label: "Symbols",
        description: "All symbols defined in this file.",
        view_kind: "source_view",
        scaffold_id: Some("file_symbols"),
        priority: 2,
    },
    Affordance {
        object_type: "file",
        label: "Quality Gate",
        description: "Issue summary for this file.",
        view_kind: "quality_hotspots",
        scaffold_id: Some("file_quality_gate"),
        priority: 3,
    },
    // ------------------------------------------------------------------------
    // module
    // ------------------------------------------------------------------------
    Affordance {
        object_type: "module",
        label: "Module Tree",
        description: "Items exposed by this module.",
        view_kind: "source_view",
        scaffold_id: Some("module_tree"),
        priority: 1,
    },
    Affordance {
        object_type: "module",
        label: "API Surface",
        description: "Public API surface of this module.",
        view_kind: "api_surface",
        scaffold_id: Some("module_api_surface"),
        priority: 2,
    },
    // ------------------------------------------------------------------------
    // evidence
    // ------------------------------------------------------------------------
    Affordance {
        object_type: "evidence",
        label: "Evidence View",
        description: "Raw evidence block with provenance and confidence.",
        view_kind: "evidence_view",
        scaffold_id: Some("evidence_view"),
        priority: 1,
    },
    // ------------------------------------------------------------------------
    // decision_artifact
    // ------------------------------------------------------------------------
    Affordance {
        object_type: "decision_artifact",
        label: "Decision Graph",
        description: "Linked decisions, code, tests, and evidence.",
        view_kind: "decision_graph",
        scaffold_id: Some("decision_graph"),
        priority: 1,
    },
    // ------------------------------------------------------------------------
    // quality_issue
    // ------------------------------------------------------------------------
    Affordance {
        object_type: "quality_issue",
        label: "Issue Detail",
        description: "Full issue location, message, and rule reference.",
        view_kind: "source_view",
        scaffold_id: Some("quality_issue_detail"),
        priority: 1,
    },
    // ------------------------------------------------------------------------
    // rule
    // ------------------------------------------------------------------------
    Affordance {
        object_type: "rule",
        label: "Rule Overview",
        description: "Rule description and all related issues.",
        view_kind: "quality_hotspots",
        scaffold_id: Some("rule_overview"),
        priority: 1,
    },
    // ------------------------------------------------------------------------
    // investigation
    // ------------------------------------------------------------------------
    Affordance {
        object_type: "investigation",
        label: "Composed Narrative",
        description: "Living narrative of this investigation.",
        view_kind: "composed_narrative",
        scaffold_id: Some("investigation_narrative"),
        priority: 1,
    },
    Affordance {
        object_type: "investigation",
        label: "Evidence Pack",
        description: "All pinned evidence for this investigation.",
        view_kind: "evidence_pack",
        scaffold_id: Some("investigation_evidence_pack"),
        priority: 2,
    },
];

// ============================================================================
// Query helpers
// ============================================================================

/// Returns all affordances for the given `object_type`, sorted by priority.
pub fn get_affordances(object_type: &str) -> Vec<&'static Affordance> {
    let mut result: Vec<_> = AFFORDANCE_MATRIX
        .iter()
        .filter(|a| a.object_type == object_type)
        .collect();
    result.sort_by_key(|a| a.priority);
    result
}

/// Returns the highest-priority affordance for the given `object_type`.
pub fn get_default_affordance(object_type: &str) -> Option<&'static Affordance> {
    get_affordances(object_type).first().copied()
}

/// Returns the set of object types present in the matrix.
pub fn covered_object_types() -> Vec<&'static str> {
    KNOWN_OBJECT_TYPES.to_vec()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_covers_all_known_types() {
        for ot in KNOWN_OBJECT_TYPES {
            let affordances = get_affordances(ot);
            assert!(
                !affordances.is_empty(),
                "object_type `{ot}` should have at least one affordance"
            );
        }
    }

    #[test]
    fn unknown_type_returns_empty() {
        let result = get_affordances("nonexistent_type_xyz");
        assert!(result.is_empty());
    }

    #[test]
    fn unknown_type_default_is_none() {
        let result = get_default_affordance("nonexistent_type_xyz");
        assert!(result.is_none());
    }

    #[test]
    fn affordances_sorted_by_priority() {
        for ot in KNOWN_OBJECT_TYPES {
            let affordances = get_affordances(ot);
            for window in affordances.windows(2) {
                assert!(
                    window[0].priority <= window[1].priority,
                    "`{}` affordances not sorted: {} vs {}",
                    ot,
                    window[0].priority,
                    window[1].priority
                );
            }
        }
    }

    #[test]
    fn default_affordance_is_highest_priority() {
        for ot in KNOWN_OBJECT_TYPES {
            let affordances = get_affordances(ot);
            if let Some(default) = get_default_affordance(ot) {
                assert_eq!(
                    default, affordances[0],
                    "default_affordance should be first after sorting"
                );
            }
        }
    }

    #[test]
    fn workspace_has_expected_affordances() {
        let affs = get_affordances("workspace");
        assert!(affs.len() >= 3);
        assert_eq!(affs[0].view_kind, "architecture_rationale");
        assert_eq!(affs[1].view_kind, "dependency_graph");
    }

    #[test]
    fn symbol_has_vertical_slice() {
        let affs = get_affordances("symbol");
        let labels: Vec<_> = affs.iter().map(|a| a.label).collect();
        assert!(
            labels.contains(&"Vertical Slice"),
            "symbol should have Vertical Slice affordance, got {labels:?}"
        );
    }
}
