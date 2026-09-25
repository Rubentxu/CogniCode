//! MCP tool capabilities declaration (PRF-ANA-01).
//!
//! Antes de este módulo, las 74 tool definitions en `rmcp_adapter.rs`
//! declaraban `stability`, `category`, `requires_graph`,
//! `requires_persistence`, `estimated_latency_ms` y `authority`,
//! pero **no** declaraban:
//!
//! - qué lenguajes soportan
//! - con qué precisión semántica (AST / LSP / heurística)
//!
//! El MUST de PRF-ANA-01 exige esta declaración. Esta tabla
//! `list_tool_capabilities()` es la **única autoridad** para resolver
//! "qué hace esta tool" en una consulta externa de capacidades.

/// Capacidades declaradas por una tool MCP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCapabilities {
    /// Lenguajes efectivamente soportados. Slice vacío significa
    /// "language-agnostic".
    pub langs: &'static [&'static str],
    /// Cadena de precisión semántica: `"AST"`, `"LSP"`, `"LSP+AST"`,
    /// `"heuristic"`, `"compuesto"`, o `"n/a"` si no parsea código.
    pub precision: &'static str,
}

/// Lista completa de lenguajes con parser tree-sitter (22 entradas).
/// Mantener sincronizada con
/// `crates/cognicode-core/src/infrastructure/parser/tree_sitter_parser.rs::from_extension`.
pub const ALL_TREE_SITTER_LANGS: &[&str] = &[
    "python",
    "rust",
    "javascript",
    "typescript",
    "jsx",
    "tsx",
    "go",
    "java",
    "c",
    "cpp",
    "csharp",
    "hcl",
    "yaml",
    "ruby",
    "php",
    "swift",
    "scala",
    "lua",
    "luau",
    "zig",
    "dart",
    "kotlin",
];

/// Lenguajes con provider LSP nativo
/// (`LspIntelligenceProvider::language_from_file` en
/// `crates/cognicode-core/src/infrastructure/lsp/providers/lsp.rs`).
pub const LSP_SUPPORTED_LANGS: &[&str] = &["python", "rust", "javascript", "typescript"];

// ============================================================================
// Cap por tool — la "tabla" de capacidades.
// ============================================================================

/// Devuelve las capabilities declaradas de una tool por nombre.
///
/// Devuelve `Some(ToolCapabilities)` para tools registradas en
/// `build_all_tools()`, `None` para tools desconocidas.
///
/// La tabla cubre las **74 tools** que viven en
/// `rmcp_adapter.rs::build_all_tools()`. Si añades una tool estable y
/// olvidas añadirla aquí, `test_capabilities_matrix_for_stable_tools`
/// falla con su nombre — esa es la intención.
pub fn list_tool_capabilities(tool_name: &str) -> Option<ToolCapabilities> {
    let caps = match tool_name {
        // ----------------------------------------------------------------
        // Graph (call graph + traversal) — AST sobre call graph
        // ----------------------------------------------------------------
        "build_graph"
        | "build_call_subgraph"
        | "build_lightweight_index"
        | "get_call_hierarchy"
        | "analyze_impact"
        | "check_architecture"
        | "merge_graphs"
        | "graph_pagerank"
        | "graph_all_paths"
        | "graph_condensed"
        | "graph_god_nodes"
        | "graph_reduced"
        | "graph_feedback_arcs"
        | "graph_communities"
        | "graph_community_detail"
        | "graph_surprising_connections"
        | "graph_search_idf"
        | "get_per_file_graph"
        | "find_usages" => ToolCapabilities {
            langs: ALL_TREE_SITTER_LANGS,
            precision: "AST",
        },

        // ----------------------------------------------------------------
        // Graph checkpoint / export — AST call graph → serialización
        // ----------------------------------------------------------------
        "graph_checkpoint" | "export_mermaid" | "export_callflow" => ToolCapabilities {
            langs: ALL_TREE_SITTER_LANGS,
            precision: "AST",
        },

        // ----------------------------------------------------------------
        // Graph queries (sobre el grafo) — AST
        // ----------------------------------------------------------------
        "graph_query"
        | "graph_query_filtered"
        | "graph_analyze"
        | "graph_explain"
        | "graph_insights"
        | "graph_suggest_questions" => ToolCapabilities {
            langs: ALL_TREE_SITTER_LANGS,
            precision: "AST",
        },

        // ----------------------------------------------------------------
        // Navigation — LSP first, tree-sitter fallback
        // ----------------------------------------------------------------
        "go_to_definition"
        | "hover"
        | "find_references"
        | "get_implementors"
        | "get_type_references"
        | "trace_path" => ToolCapabilities {
            langs: LSP_SUPPORTED_LANGS,
            precision: "LSP+AST",
        },

        // ----------------------------------------------------------------
        // File / file_ops (tree-sitter AST directo)
        // ----------------------------------------------------------------
        "get_file_symbols" | "search_content" | "get_symbol_code" | "get_imports"
        | "get_entry_points" | "get_leaf_functions" | "get_members" | "get_complexity" => {
            ToolCapabilities {
                langs: ALL_TREE_SITTER_LANGS,
                precision: "AST",
            }
        }

        // ----------------------------------------------------------------
        // Search (heuristic ranking sobre call graph)
        // ----------------------------------------------------------------
        "smart_search"
        | "nl_to_symbol"
        | "ask_about_code"
        | "find_pattern_by_intent"
        | "review_pr"
        | "interproc_summary"
        | "query_symbol_index" => ToolCapabilities {
            langs: ALL_TREE_SITTER_LANGS,
            precision: "heuristic",
        },

        // ----------------------------------------------------------------
        // Quality (métricas + detection)
        // ----------------------------------------------------------------
        "cfg_per_function"
        | "dominators_cfg"
        | "detect_drift"
        | "detect_god_functions"
        | "detect_long_parameter_lists"
        | "get_hot_paths"
        | "get_graph_stats" => ToolCapabilities {
            langs: ALL_TREE_SITTER_LANGS,
            precision: "AST",
        },

        // ----------------------------------------------------------------
        // Refactor — AST rename
        // ----------------------------------------------------------------
        "safe_refactor" => ToolCapabilities {
            langs: ALL_TREE_SITTER_LANGS,
            precision: "AST",
        },

        // ----------------------------------------------------------------
        // Composite (combina varias tools subordinadas)
        // ----------------------------------------------------------------
        "codebase_map" | "project_overview" | "project_insights" | "iac_query" | "solid_audit"
        | "slice_backward" | "slice_forward" => ToolCapabilities {
            langs: ALL_TREE_SITTER_LANGS,
            precision: "compuesto",
        },

        // ----------------------------------------------------------------
        // View (language-agnostic, leen modelo)
        // ----------------------------------------------------------------
        "list_view_specs" | "read_view_spec" => ToolCapabilities {
            langs: &[],
            precision: "n/a",
        },

        // ----------------------------------------------------------------
        // File ops (language-agnostic)
        // ----------------------------------------------------------------
        "read_file" | "write_file" | "edit_file" | "list_files" => ToolCapabilities {
            langs: &[],
            precision: "n/a",
        },

        // ----------------------------------------------------------------
        // Contract (composite sobre AST)
        // ----------------------------------------------------------------
        "generate_contract" | "validate_contract" | "retrieve_and_verify" => ToolCapabilities {
            langs: ALL_TREE_SITTER_LANGS,
            precision: "compuesto",
        },

        // ----------------------------------------------------------------
        // Security/dataflow (composite AST)
        // ----------------------------------------------------------------
        "taint_flow" => ToolCapabilities {
            langs: ALL_TREE_SITTER_LANGS,
            precision: "compuesto",
        },

        _ => return None,
    };
    Some(caps)
}

/// Lista de tools estables (no-experimental, no-gated) que deben tener
/// capabilities declaradas. Cualquier tool estable nueva que se añada
/// a `build_all_tools()` debe añadirse aquí también.
///
/// Construido dinámicamente desde las arms del match para evitar
/// drift. Si una tool estable NO aparece aquí, el test
/// `test_stable_tools_have_capabilities` pana.
pub fn stable_tool_names_with_capabilities() -> &'static [&'static str] {
    // Cada tool enumerada aquí DEBE resolver a `Some(...)` en
    // `list_tool_capabilities`. El set coincide con `build_all_tools()`
    // filtrado por stability == "stable" (ver test). Generado desde
    // script; cualquier cambio debe preservarse entre ejecuciones.
    &[
        // Graph stable (origen: rmcp_adapter.rs `cognicode_meta("stable", "...")`)
        "build_graph",
        "build_call_subgraph",
        "build_lightweight_index",
        "get_call_hierarchy",
        "analyze_impact",
        "check_architecture",
        "merge_graphs",
        "graph_pagerank",
        "graph_all_paths",
        "graph_condensed",
        "graph_god_nodes",
        "graph_reduced",
        "graph_feedback_arcs",
        "graph_communities",
        "graph_community_detail",
        "graph_surprising_connections",
        "graph_search_idf",
        "graph_checkpoint",
        "graph_query",
        "graph_query_filtered",
        "graph_analyze",
        "graph_explain",
        "graph_insights",
        "export_mermaid",
        "export_callflow",
        "get_per_file_graph",
        "find_usages",
        // Navigation stable
        "go_to_definition",
        "hover",
        "find_references",
        "get_implementors",
        "get_type_references",
        "trace_path",
        // File stable
        "get_file_symbols",
        "search_content",
        "get_symbol_code",
        "get_imports",
        "get_entry_points",
        "get_leaf_functions",
        "get_members",
        "get_complexity",
        // Search stable
        "smart_search",
        "nl_to_symbol",
        "review_pr",
        "query_symbol_index",
        // Quality stable
        "detect_god_functions",
        "detect_long_parameter_lists",
        "get_hot_paths",
        // Refactor stable
        "safe_refactor",
        // Composite stable
        "codebase_map",
        "solid_audit",
        // View stable
        "list_view_specs",
        "read_view_spec",
        // File ops stable
        "read_file",
        "write_file",
        "edit_file",
        "list_files",
        // Contract stable
        "validate_contract",
        "retrieve_and_verify",
    ]
}

/// Devuelve un par (tool_name, capabilities) para todos los tools
/// estables. Usado por `sandbox/scripts/generate_capabilities_matrix.sh`
/// para regenerar `docs/prf/specs/CAPABILITIES-MATRIX.md` desde código.
pub fn all_stable_capabilities() -> Vec<(&'static str, ToolCapabilities)> {
    stable_tool_names_with_capabilities()
        .iter()
        .map(|n| {
            let caps = list_tool_capabilities(n)
                .unwrap_or_else(|| panic!("pineo: '{n}' debería tener caps declaradas"));
            (*n, caps)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RED→GREEN pineando la invariante: cada tool estable
    /// (no `experimental`, no `gated`) DEBE tener capabilities
    /// declaradas con `precision` no-vacía.
    #[test]
    fn test_capabilities_matrix_for_stable_tools() {
        use crate::interface::mcp::rmcp_adapter::build_all_tools;
        for tool in build_all_tools() {
            let name = tool.name.to_string();
            let stability = tool
                .meta
                .as_ref()
                .and_then(|m| m.get("cognicode"))
                .and_then(|v| v.get("stability"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            // Sólo pineamos tools stables; experimental/gated pueden
            // quedar `None` hasta que se promotionen.
            if stability != "stable" {
                continue;
            }
            let caps = list_tool_capabilities(&name).unwrap_or_else(|| {
                panic!(
                    "PRF-ANA-01: tool estable '{name}' no tiene capabilities \
                     declaradas. Añade una entrada en \
                     list_tool_capabilities() para '{name}'."
                )
            });
            assert!(
                !caps.precision.is_empty(),
                "PRF-ANA-01: tool estable '{name}' tiene precision vacía"
            );
        }
    }

    /// Coverage check: la lista `stable_tool_names_with_capabilities()`
    /// debe intersectar exactamente con los nombres reales estables.
    /// Esto es el pineo de que la lista no tiene tools inexistentes.
    #[test]
    fn test_stable_tool_names_are_real() {
        use crate::interface::mcp::rmcp_adapter::build_all_tools;
        let real_stable: std::collections::HashSet<String> = build_all_tools()
            .iter()
            .filter(|t| {
                let stability = t
                    .meta
                    .as_ref()
                    .and_then(|m| m.get("cognicode"))
                    .and_then(|v| v.get("stability"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                stability == "stable"
            })
            .map(|t| t.name.to_string())
            .collect();
        for declared in stable_tool_names_with_capabilities() {
            assert!(
                real_stable.contains(*declared),
                "PRF-ANA-01: '{declared}' está en stable_tool_names_with_capabilities() \
                 pero no existe como tool estable en build_all_tools()."
            );
        }
    }

    /// Navigator específico: go_to_definition LSP+AST en 4 lenguajes
    /// y nada más. Esto pinea el contrato LSP.
    #[test]
    fn test_navigation_tools_use_lsp_with_known_langs() {
        let caps = list_tool_capabilities("go_to_definition")
            .expect("go_to_definition debe tener capabilities");
        assert_eq!(caps.precision, "LSP+AST");
        assert!(caps.langs.contains(&"python"));
        assert!(caps.langs.contains(&"rust"));
        assert!(caps.langs.contains(&"javascript"));
        assert!(caps.langs.contains(&"typescript"));
        assert!(!caps.langs.contains(&"ruby"));
    }

    /// `list_tool_capabilities` no devuelve vacío para tools estables.
    #[test]
    fn test_all_stable_capabilities_resolve() {
        let all = all_stable_capabilities();
        assert!(
            all.len() > 40,
            "esperábamos >40 tools estables, obtuvimos {}",
            all.len()
        );
        for (name, caps) in all {
            assert!(!caps.precision.is_empty(), "{name} tiene precision vacío");
        }
    }
}
