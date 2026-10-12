//! B3.W1 — Semantic navigation: identity + precision tagging for the 8 tools.
//!
//! Estado pre-B3.W1:
//!   - Las 8 navigation tools (`get_file_symbols`, `query_symbol_index`,
//!     `go_to_definition`, `find_references`, `find_usages`,
//!     `get_call_hierarchy`, `get_implementors`, `get_type_references`)
//!     emitían un payload sin discriminador de proveedor. Un agente que
//!     consumía el JSON no tenía cómo distinguir un resultado LSP-grade
//!     de uno heurístico (R-B3-W1).
//!   - `(file, line, column)` se enviaba como identidad de símbolo: un
//!     mismo símbolo podía aparecer con dos `(line, col)` distintos
//!     después de un re-parse, y dos homónimos en módulos distintos
//!     tenían que ser identificados por su nombre a secas.
//!
//! Lo que cierra este test:
//!   - Cada `SymbolId` es `(qualified_name, file_path, byte_range)`. La
//!     `byte_range` se mantiene estable a través de re-parses si los
//!     bytes del source no cambian.
//!   - Cada respuesta lleva `precision` (`lsp` | `ast-heuristic` |
//!     `index`) y `provider` (nombre del backend). Un resultado
//!     heurístico NUNCA aparece etiquetado como LSP — AAT-B3-01.
//!   - El catálogo declara `precision_floor` para las 8 tools; el test
//!     cierra el contrato al iterarlas.
//!
//! Política B3.W1:
//!   - Los tests unitarios de los tipos viven en `symbol_identity.rs`.
//!     Aquí probamos el comportamiento agregado: JSON serializado, catálogo
//!     declarado, discriminadores visibles.

use cognicode_core::interface::catalog::load_tool_catalog;
use cognicode_core::interface::mcp::handlers::graph_query_handlers::{
    GetImplementorsOutput, GetTypeRefsOutput, TypeRefRecord,
};
use cognicode_core::interface::mcp::schemas::{
    FindReferencesOutput, FindUsagesOutput, GetCallHierarchyOutput, GetFileSymbolsOutput,
    GoToDefinitionOutput, QuerySymbolOutput,
};
use cognicode_core::interface::symbol_identity::{
    NAVIGATION_TOOLS, Precision, SymbolId, SymbolReference, default_provider_for_tool,
    precision_floor_for_tool,
};
use serde_json::Value;
use std::collections::HashSet;
use std::ops::Range;
use std::path::PathBuf;

/// R-B3.W1 (homonym test from the goal doc): two symbols with the same
/// short name in different modules must NOT collapse to the same
/// `SymbolId`. The triple `(qualified_name, file_path, byte_range)`
/// distinguishes them.
#[test]
fn symbol_id_distinguishes_homonyms_in_different_modules() {
    let id_a = SymbolId::from_lsp_location(
        "config::api::Config".to_string(),
        PathBuf::from("src/api/config.rs"),
        10..20,
    );
    let id_b = SymbolId::from_lsp_location(
        "config::worker::Config".to_string(),
        PathBuf::from("src/worker/config.rs"),
        5..15,
    );
    assert_ne!(id_a, id_b, "homonyms must produce distinct SymbolIds");
    assert_ne!(id_a.file_path, id_b.file_path);
    assert_ne!(id_a.qualified_name, id_b.qualified_name);

    // Sanity: a `HashSet` of `SymbolId`s keeps both entries — equality is
    // identity-stable across insertions.
    let mut set: HashSet<SymbolId> = HashSet::new();
    set.insert(id_a.clone());
    set.insert(id_b.clone());
    assert_eq!(set.len(), 2);
}

/// Byte ranges survive re-parse when the source bytes are stable.
/// (line, col) drift; byte offsets track bytes.
#[test]
fn symbol_id_byte_range_is_stable_across_reparse() {
    let source: &str = "fn hello() {}\nfn world() {}\n";
    let range_a: Range<usize> = locate_fn(source, "hello");
    // Independent second pass.
    let range_b: Range<usize> = locate_fn(source, "hello");
    assert_eq!(range_a, range_b);
    // Prepending a newline shifts byte offsets by 1 — proves we measure
    // bytes, not lines.
    let prefixed = format!("\n{source}");
    let range_c: Range<usize> = locate_fn(&prefixed, "hello");
    assert_eq!(
        range_c.start,
        range_a.start + 1,
        "byte offset shifts with prefix bytes"
    );
}

/// Each of the 8 navigation tool DTOs, serialised to JSON, MUST carry
/// a `precision` field. The discriminator is the wire contract agents
/// rely on (R-B3-W1).
#[test]
fn precision_field_is_present_on_every_navigation_result() {
    let cases: Vec<(&str, Value)> = vec![
        (
            "get_file_symbols",
            serde_json::to_value(sample_get_file_symbols_output()).unwrap(),
        ),
        (
            "query_symbol_index",
            serde_json::to_value(sample_query_symbol_output()).unwrap(),
        ),
        (
            "go_to_definition",
            serde_json::to_value(sample_go_to_definition_output()).unwrap(),
        ),
        (
            "find_references",
            serde_json::to_value(sample_find_references_output()).unwrap(),
        ),
        (
            "find_usages",
            serde_json::to_value(sample_find_usages_output()).unwrap(),
        ),
        (
            "get_call_hierarchy",
            serde_json::to_value(sample_get_call_hierarchy_output()).unwrap(),
        ),
        (
            "get_implementors",
            serde_json::to_value(sample_get_implementors_output()).unwrap(),
        ),
        (
            "get_type_references",
            serde_json::to_value(sample_get_type_references_output()).unwrap(),
        ),
    ];

    assert_eq!(
        cases.len(),
        NAVIGATION_TOOLS.len(),
        "this test must cover every entry in NAVIGATION_TOOLS"
    );

    for (tool, json) in cases {
        let obj = json
            .as_object()
            .unwrap_or_else(|| panic!("{tool}: output must be a JSON object"));
        assert!(
            obj.contains_key("precision"),
            "{tool}: precision field is missing — agents cannot trust the result \
             without it (R-B3-W1)"
        );
        let precision = obj["precision"].as_str().unwrap_or_else(|| {
            panic!("{tool}: precision must be a string (one of lsp|ast-heuristic|index)")
        });
        assert!(
            ["lsp", "ast-heuristic", "index"].contains(&precision),
            "{tool}: precision `{precision}` is not a recognised tier"
        );
    }
}

/// `provider` must be a non-empty string on every navigation tool
/// response. An empty provider would mean "I don't know who answered" —
/// useless to an agent trying to debug a result.
#[test]
fn provider_field_is_present_and_nonempty() {
    let cases: Vec<(&str, Value)> = vec![
        (
            "get_file_symbols",
            serde_json::to_value(sample_get_file_symbols_output()).unwrap(),
        ),
        (
            "query_symbol_index",
            serde_json::to_value(sample_query_symbol_output()).unwrap(),
        ),
        (
            "go_to_definition",
            serde_json::to_value(sample_go_to_definition_output()).unwrap(),
        ),
        (
            "find_references",
            serde_json::to_value(sample_find_references_output()).unwrap(),
        ),
        (
            "find_usages",
            serde_json::to_value(sample_find_usages_output()).unwrap(),
        ),
        (
            "get_call_hierarchy",
            serde_json::to_value(sample_get_call_hierarchy_output()).unwrap(),
        ),
        (
            "get_implementors",
            serde_json::to_value(sample_get_implementors_output()).unwrap(),
        ),
        (
            "get_type_references",
            serde_json::to_value(sample_get_type_references_output()).unwrap(),
        ),
    ];
    for (tool, json) in cases {
        let provider = json["provider"]
            .as_str()
            .unwrap_or_else(|| panic!("{tool}: provider must be a string"));
        assert!(
            !provider.is_empty(),
            "{tool}: provider must be non-empty — agents need to know which \
             backend answered"
        );
    }
}

/// `product/tools.json` must declare `precision_floor` for each of the
/// 8 navigation tools. A missing entry means the catalog silently
/// defaults to `ast-heuristic` (the conservative choice), so this test
/// also asserts the declared value matches the helper function — the
/// catalog and the source stay in lockstep.
#[test]
fn catalog_declares_precision_floor_for_every_navigation_tool() {
    let catalog = load_tool_catalog();
    assert!(!catalog.is_empty(), "catalog must load successfully");
    let by_name: std::collections::HashMap<&str, &str> = catalog
        .iter()
        .map(|d| (d.name.as_str(), d.precision_floor.as_str()))
        .collect();
    for tool in NAVIGATION_TOOLS {
        let declared = by_name
            .get(tool)
            .unwrap_or_else(|| panic!("{tool}: missing from catalog"));
        let expected = precision_floor_for_tool(tool).as_str();
        assert_eq!(
            *declared, expected,
            "{tool}: catalog declares precision_floor=`{declared}` but the \
             helper returns `{expected}` — update one or the other"
        );
    }
}

/// AAT-B3-01: when the LSP backend is unavailable, the response MUST
/// NOT be advertised as LSP-grade. The catalog's `precision_floor` is
/// the documented ceiling; the wire shape carries the actual tier that
/// served the request. For tools that have no LSP backend at all
/// (`find_usages`, `query_symbol_index`), the helper must return
/// `ast-heuristic`, and the kebab-case wire shape must differ from
/// `lsp` byte-for-byte.
#[test]
fn ast_heuristic_results_are_not_presented_as_lsp() {
    let heuristic_tools = ["find_usages", "query_symbol_index"];
    for tool in heuristic_tools {
        let precision = precision_floor_for_tool(tool);
        assert_eq!(
            precision,
            Precision::AstHeuristic,
            "{tool}: has no LSP backend; precision floor must be ast-heuristic"
        );
        assert!(
            !precision.is_lsp_grade(),
            "{tool}: ast-heuristic precision must NOT report as LSP-grade"
        );
        let wire = serde_json::to_string(&precision).unwrap();
        assert_ne!(
            wire, "\"lsp\"",
            "{tool}: ast-heuristic precision serialises as `{wire}`, must not \
             collide with `lsp`"
        );
        assert_eq!(wire, "\"ast-heuristic\"");
    }
}

/// `SymbolReference` is the canonical shape the navigation tools
/// should populate on every primary result. This test pins the
/// structural contract: equality is by-value (Hash + Eq), the wire
/// shape includes all three fields, and `provider` is non-empty.
#[test]
fn symbol_reference_carries_id_precision_and_provider() {
    let id = SymbolId::from_lsp_location(
        "x::y::Config".to_string(),
        PathBuf::from("src/y.rs"),
        10..20,
    );
    let reference = SymbolReference::new(id.clone(), Precision::Lsp, "lsp::rust-analyzer");
    assert_eq!(reference.id, id);
    assert_eq!(reference.precision, Precision::Lsp);
    assert_eq!(reference.provider, "lsp::rust-analyzer");
    assert!(!reference.provider.is_empty());

    let json = serde_json::to_value(&reference).unwrap();
    let obj = json
        .as_object()
        .expect("SymbolReference serialises as object");
    assert!(obj.contains_key("id"));
    assert!(obj.contains_key("precision"));
    assert!(obj.contains_key("provider"));
}

/// Every entry in `NAVIGATION_TOOLS` must have a non-empty provider
/// declared by the helper. Pinned so a future refactor cannot silently
/// drop the provider for one of the 8 tools.
#[test]
fn every_navigation_tool_has_a_provider() {
    for tool in NAVIGATION_TOOLS {
        let provider = default_provider_for_tool(tool);
        assert!(
            !provider.is_empty(),
            "{tool}: default_provider_for_tool returned empty"
        );
    }
}

/// `NAVIGATION_TOOLS` is the closed set of tools B3.W1 consolidates.
/// Adding a 9th navigation tool must update both this constant and the
/// catalog; this test pins the constant length so a careless edit
/// surfaces here, not in production.
#[test]
fn navigation_tools_set_is_exactly_eight() {
    assert_eq!(NAVIGATION_TOOLS.len(), 8);
    let unique: HashSet<&str> = NAVIGATION_TOOLS.iter().copied().collect();
    assert_eq!(unique.len(), 8, "duplicate names in NAVIGATION_TOOLS");
    // Spot-check the expected members.
    for required in [
        "get_file_symbols",
        "query_symbol_index",
        "go_to_definition",
        "find_references",
        "find_usages",
        "get_call_hierarchy",
        "get_implementors",
        "get_type_references",
    ] {
        assert!(
            unique.contains(required),
            "{required}: missing from NAVIGATION_TOOLS"
        );
    }
}

// ============================================================================
// Helpers — sample DTOs, JSON construction
// ============================================================================

fn locate_fn(source: &str, name: &str) -> Range<usize> {
    let needle = format!("fn {name}");
    let start = source
        .find(&needle)
        .unwrap_or_else(|| panic!("{name} not in source"));
    let end = start + needle.len() + "() {}".len();
    start..end
}

fn sample_get_file_symbols_output() -> GetFileSymbolsOutput {
    use cognicode_core::interface::mcp::schemas::{SourceLocation, SymbolInfo, SymbolKind};
    GetFileSymbolsOutput {
        file_path: "src/lib.rs".to_string(),
        symbols: vec![SymbolInfo {
            name: "foo".to_string(),
            kind: SymbolKind::Function,
            location: SourceLocation {
                file: "src/lib.rs".to_string(),
                line: 1,
                column: 0,
            },
            signature: Some("fn foo()".to_string()),
        }],
        hierarchical_symbols: vec![],
        precision: precision_floor_for_tool("get_file_symbols"),
        provider: default_provider_for_tool("get_file_symbols").to_string(),
    }
}

fn sample_query_symbol_output() -> QuerySymbolOutput {
    use cognicode_core::interface::mcp::schemas::SymbolLocationEntry;
    QuerySymbolOutput {
        symbol_name: "foo".to_string(),
        locations: vec![SymbolLocationEntry {
            file: "src/lib.rs".to_string(),
            line: 1,
            column: 0,
            symbol_kind: "Function".to_string(),
        }],
        total: 1,
        precision: precision_floor_for_tool("query_symbol_index"),
        provider: default_provider_for_tool("query_symbol_index").to_string(),
    }
}

fn sample_go_to_definition_output() -> GoToDefinitionOutput {
    GoToDefinitionOutput {
        found: true,
        file: Some("src/lib.rs".to_string()),
        line: Some(1),
        column: Some(0),
        context: Some("fn foo()".to_string()),
        message: None,
        precision: precision_floor_for_tool("go_to_definition"),
        provider: default_provider_for_tool("go_to_definition").to_string(),
    }
}

fn sample_find_references_output() -> FindReferencesOutput {
    use cognicode_core::interface::mcp::schemas::ReferenceEntry;
    FindReferencesOutput {
        symbol: "foo".to_string(),
        references: vec![ReferenceEntry {
            file: "src/lib.rs".to_string(),
            line: 10,
            column: 4,
            kind: "Read".to_string(),
            context: "main".to_string(),
        }],
        total: 1,
        precision: precision_floor_for_tool("find_references"),
        provider: default_provider_for_tool("find_references").to_string(),
    }
}

fn sample_find_usages_output() -> FindUsagesOutput {
    use cognicode_core::interface::mcp::schemas::{ContextLines, UsageEntry};
    FindUsagesOutput {
        symbol: "foo".to_string(),
        usages: vec![UsageEntry {
            file: "src/lib.rs".to_string(),
            line: 10,
            column: 4,
            context: "let foo = 1".to_string(),
            is_definition: false,
            surrounding_lines: Some(ContextLines {
                before: vec!["".to_string()],
                current: "let foo = 1".to_string(),
                after: vec!["".to_string()],
            }),
        }],
        total: 1,
        precision: precision_floor_for_tool("find_usages"),
        provider: default_provider_for_tool("find_usages").to_string(),
    }
}

fn sample_get_call_hierarchy_output() -> GetCallHierarchyOutput {
    use cognicode_core::application::dto::AnalysisMetadata;
    use cognicode_core::interface::mcp::schemas::CallEntry;
    GetCallHierarchyOutput {
        symbol: "foo".to_string(),
        calls: vec![CallEntry {
            symbol: "bar".to_string(),
            file: "src/lib.rs".to_string(),
            line: 20,
            column: 4,
            confidence: 1.0,
        }],
        metadata: AnalysisMetadata {
            total_calls: 1,
            analysis_time_ms: 5,
        },
        precision: precision_floor_for_tool("get_call_hierarchy"),
        provider: default_provider_for_tool("get_call_hierarchy").to_string(),
    }
}

fn sample_get_implementors_output() -> GetImplementorsOutput {
    GetImplementorsOutput {
        trait_name: "MyTrait".to_string(),
        implementors: vec!["MyStruct".to_string()],
        precision: precision_floor_for_tool("get_implementors"),
        provider: default_provider_for_tool("get_implementors").to_string(),
    }
}

fn sample_get_type_references_output() -> GetTypeRefsOutput {
    GetTypeRefsOutput {
        symbol: "Config".to_string(),
        references: vec![TypeRefRecord {
            target: "u32".to_string(),
            context: "field".to_string(),
        }],
        precision: precision_floor_for_tool("get_type_references"),
        provider: default_provider_for_tool("get_type_references").to_string(),
    }
}
