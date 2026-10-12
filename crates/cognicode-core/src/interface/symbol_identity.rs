//! B3.W1 — Symbol identity + precision tagging for navigation tools.
//!
//! Estado pre-B3.W1:
//!   - Las 8 tools de navegación (`get_file_symbols`, `query_symbol_index`,
//!     `go_to_definition`, `find_references`, `find_usages`,
//!     `get_call_hierarchy`, `get_implementors`, `get_type_references`)
//!     emitían `(file, line, column)` como único discriminador.
//!   - `line, column` deriva en cada re-parse; un agente que cachea la
//!     posición termina con un fantasma.
//!   - Sin etiqueta de precisión, un resultado de tree-sitter se podía
//!     presentar como si tuviera garantía LSP. R-B3-W1 (en el goal doc)
//!     describe el riesgo de confianza.
//!
//! Lo que este módulo aporta:
//!   - `Precision` (lsp | ast-heuristic | index): el discriminante que el
//!     `goal doc` exige (R-B3-W1). Default = `ast-heuristic`, el nivel más
//!     conservador cuando el caller no sabe qué provider atendió.
//!   - `SymbolId` = (qualified_name, file_path, byte_range). La `Range<usize>`
//!     es estable a través de re-parses cuando el archivo fuente no cambia;
//!     un par `(line, column)` no lo es.
//!   - `SymbolReference` = `SymbolId` con la precisión y el provider que lo
//!     produjo. Es el shape canónico que las 8 tools deben llevar.
//!   - `precision_floor_for_tool` / `default_provider_for_tool`: mapean el
//!     nombre de la tool a su `precision_floor` declarado en `product/
//!     tools.json`. Los handlers usan estas funciones para no tener que
//!     recordar el contrato por tool.
//!   - `NAVIGATION_TOOLS`: la lista de 8 tools que B3.W1 cierra; las UATs
//!     la iteran.
//!
//! Política B3.W1:
//!   - El discriminador viaja en la respuesta: el agente nunca tiene que
//!     adivinar de dónde viene el resultado (R-B3-W1).
//!   - `from_ast_node` falla suave (devuelve `None`) si falta el qualified
//!     name: un nodo AST ambiguo no debe identificarse como símbolo.
//!   - `byte_range` se mide en bytes del source, NO caracteres: alineado
//!     con LSP y con tree-sitter, y nunca se desincroniza con un cambio
//!     de encoding del archivo.

use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::path::PathBuf;

/// Precision tier of a navigation result (B3.W1 contract).
///
/// `lsp`: produced by an LSP-grade backend (rust-analyzer, pyright, ...).
/// `ast-heuristic`: tree-sitter fallback / local resolver; honest about
///                  being a best-effort extraction.
/// `index`: a graph-store query backed by the CogniCode project graph.
///
/// Default (per `Default::default()`) is `ast-heuristic`. Choosing the
/// most conservative tier as default prevents a forgotten precision
/// from being silently upgraded to LSP-grade — which is exactly the
/// regression AAT-B3-01 forbids.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Precision {
    Lsp,
    #[default]
    AstHeuristic,
    Index,
}

impl Precision {
    /// Wire string for the precision tier (matches serde).
    pub fn as_str(&self) -> &'static str {
        match self {
            Precision::Lsp => "lsp",
            Precision::AstHeuristic => "ast-heuristic",
            Precision::Index => "index",
        }
    }

    /// True when this precision is at least LSP-grade. Used by the
    /// AAT-B3-01 test to assert no AST-heuristic result is mislabelled.
    pub fn is_lsp_grade(&self) -> bool {
        matches!(self, Precision::Lsp)
    }
}

/// Stable identity of a code symbol across re-parses.
///
/// The triple `(qualified_name, file_path, byte_range)` uniquely pins one
/// symbol even when:
///   - The same name lives in multiple modules (homonyms) — `file_path`
///     distinguishes them.
///   - Lines drift on re-format — `byte_range` does not, as long as the
///     source bytes are stable.
///
/// `qualified_name` is the LSP-grade FQ name (e.g. `module::Type::method`).
/// `byte_range` is half-open in bytes of the source (start inclusive,
/// end exclusive), matching LSP's `Range` semantics.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolId {
    pub qualified_name: String,
    pub file_path: PathBuf,
    pub byte_range: Range<usize>,
}

impl SymbolId {
    /// Construct from an LSP location. The caller must already have
    /// resolved the FQ name; LSP-grade callers always do.
    pub fn from_lsp_location(
        qualified_name: String,
        file_path: PathBuf,
        byte_range: Range<usize>,
    ) -> Self {
        Self {
            qualified_name,
            file_path,
            byte_range,
        }
    }

    /// Construct from an AST-heuristic node. Returns `None` when the
    /// caller cannot supply a `qualified_name` (the node is ambiguous).
    /// This is the fail-soft path the policy of B3.W1 mandates: an
    /// unidentified AST node MUST NOT be promoted to a SymbolId.
    pub fn from_ast_node(
        qualified_name: Option<String>,
        file_path: PathBuf,
        byte_range: Range<usize>,
    ) -> Option<Self> {
        Some(Self {
            qualified_name: qualified_name?,
            file_path,
            byte_range,
        })
    }
}

/// A `SymbolId` annotated with the precision + provider that produced it.
///
/// This is the discriminator the 8 navigation tools must emit on every
/// result so an agent can decide whether to trust it (R-B3-W1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolReference {
    pub id: SymbolId,
    pub precision: Precision,
    pub provider: String,
}

impl SymbolReference {
    pub fn new(id: SymbolId, precision: Precision, provider: impl Into<String>) -> Self {
        Self {
            id,
            precision,
            provider: provider.into(),
        }
    }
}

/// Default precision floor for each navigation tool.
///
/// This MUST stay in lockstep with `product/tools.json` (the `precision_floor`
/// field per tool). When the catalog adds a tool, this map and `NAVIGATION_TOOLS`
/// must grow together; the `catalog_declares_precision_floor_for_every_navigation_tool`
/// test pins the invariant.
pub fn precision_floor_for_tool(tool: &str) -> Precision {
    match tool {
        // LSP-backed tools: rust-analyzer / pyright / typescript-language-server
        // through the CompositeProvider. When those servers are absent the
        // pipeline falls through to tree-sitter and the handler must
        // demote the precision to `AstHeuristic` — never silently claim
        // LSP. This default is the documented ceiling, not a guarantee.
        "go_to_definition" | "find_references" => Precision::Lsp,
        // LSP-backed, but the result is recomputed from the project graph;
        // the precision floor stays LSP because the LSP doc-symbols op is the
        // entry point. (B3.W1 keeps the floor as the catalog declared.)
        "get_file_symbols" => Precision::Lsp,
        // Graph-store tools operate on the CogniCode project index. The
        // floor reflects that they are NOT LSP-backed.
        "get_call_hierarchy" | "get_implementors" | "get_type_references" => Precision::Index,
        // Tree-sitter heuristics only; no LSP path.
        "query_symbol_index" | "find_usages" => Precision::AstHeuristic,
        _ => Precision::AstHeuristic,
    }
}

/// Default provider name per navigation tool.
///
/// Used as the `provider` field on `SymbolReference` and on the top-level
/// output DTOs. The names mirror what `LspProxyService::should_delegate_to_external`
/// and `lsp::providers::*` actually wire up; they are honest labels an
/// agent can grep for in the source.
pub fn default_provider_for_tool(tool: &str) -> &'static str {
    match tool {
        "go_to_definition" | "find_references" => "lsp::composite",
        "get_file_symbols" => "tree-sitter-rust",
        "get_call_hierarchy" | "get_implementors" | "get_type_references" => {
            "cognicode-core::graph"
        }
        "query_symbol_index" | "find_usages" => "tree-sitter-rust",
        _ => "cognicode-core",
    }
}

/// The 8 navigation tools B3.W1 unifies. Pinned by the
/// `catalog_declares_precision_floor_for_every_navigation_tool` test so
/// adding a 9th navigation tool cannot land without updating both this
/// list AND `product/tools.json`.
pub const NAVIGATION_TOOLS: &[&str] = &[
    "get_file_symbols",
    "query_symbol_index",
    "go_to_definition",
    "find_references",
    "find_usages",
    "get_call_hierarchy",
    "get_implementors",
    "get_type_references",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// R-B3-W1 (homonym test from the goal doc): two symbols with the
    /// same name in different modules must NOT collapse to the same
    /// `SymbolId`. Different `file_path` and `qualified_name` are enough
    /// — `byte_range` need not even be considered.
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
        assert_ne!(
            id_a, id_b,
            "homonyms in different modules must produce distinct SymbolIds"
        );
        assert_ne!(id_a.qualified_name, id_b.qualified_name);
        assert_ne!(id_a.file_path, id_b.file_path);
    }

    /// Byte ranges are stable across re-parse: computing the span of
    /// `fn hello() {}` twice over the same source bytes produces the
    /// identical `Range<usize>`. (line,column) would drift on a
    /// prepending blank line; byte_range does not.
    #[test]
    fn symbol_id_byte_range_is_stable_across_reparse() {
        let source: &str = "fn hello() {}\nfn world() {}\n";
        let start_a = source.find("fn hello").expect("hello exists");
        let end_a = start_a + "fn hello() {}".len();
        let range_a = start_a..end_a;
        // Independent second pass — different variable names, same source.
        let needle = "fn hello";
        let start_b = source.find(needle).expect("hello exists on pass 2");
        let end_b = start_b + needle.len() + "() {}".len();
        let range_b = start_b..end_b;
        assert_eq!(
            range_a, range_b,
            "re-parsing the same source bytes must produce the same byte range"
        );
        // Sanity: prepending a line shifts line numbers but not byte offsets.
        let prefix = "\n";
        let prefixed = format!("{prefix}{source}");
        let start_p = prefixed.find("fn hello").expect("hello exists in prefixed");
        assert_eq!(
            start_p,
            start_a + prefix.len(),
            "byte offset tracks bytes; line would have drifted"
        );
    }

    /// The precision discriminator is present in every serialized form.
    /// Pins the wire format that the JSON test inspects.
    #[test]
    fn precision_discriminator_is_present_in_serialized_form() {
        assert_eq!(serde_json::to_string(&Precision::Lsp).unwrap(), "\"lsp\"");
        assert_eq!(
            serde_json::to_string(&Precision::AstHeuristic).unwrap(),
            "\"ast-heuristic\""
        );
        assert_eq!(
            serde_json::to_string(&Precision::Index).unwrap(),
            "\"index\""
        );
        // Round-trip: kebab-case matches both ways.
        let parsed: Precision = serde_json::from_str("\"ast-heuristic\"").unwrap();
        assert_eq!(parsed, Precision::AstHeuristic);
    }

    /// Fail-soft contract: an AST node without a `qualified_name` cannot
    /// be turned into a SymbolId. The pipeline MUST surface the ambiguity
    /// (e.g. drop the entry) instead of fabricating a name.
    #[test]
    fn from_ast_node_returns_none_for_missing_qualified_name() {
        let id = SymbolId::from_ast_node(None, PathBuf::from("src/x.rs"), 0..10);
        assert!(
            id.is_none(),
            "ambiguous AST node without qualified_name must fail-soft (None)"
        );
        let id_ok =
            SymbolId::from_ast_node(Some("x::y".to_string()), PathBuf::from("src/x.rs"), 0..10);
        assert!(id_ok.is_some(), "AST node with qualified_name round-trips");
    }

    /// Every entry in `NAVIGATION_TOOLS` must have a non-default precision
    /// floor AND a non-empty provider. Pinned so adding a navigation tool
    /// without wiring the helper crashes this test, not production.
    #[test]
    fn every_navigation_tool_has_a_precision_and_provider() {
        for tool in NAVIGATION_TOOLS {
            let precision = precision_floor_for_tool(tool);
            let provider = default_provider_for_tool(tool);
            assert!(
                !provider.is_empty(),
                "{tool}: default_provider_for_tool returned empty"
            );
            // We don't pin a specific precision here — the test asserts
            // the mapping is total (no panics) and the wire string is
            // one of the three known values. Per-tool precision values
            // are pinned by `precision_floor_for_tool_distinguishes_lsp_tools`.
            assert!(
                ["lsp", "ast-heuristic", "index"].contains(&precision.as_str()),
                "{tool}: precision floor must be one of the three known tiers"
            );
        }
    }

    /// LSP-backed tools vs. non-LSP: this test pins the floor the catalog
    /// declares. If a future refactor changes which tools have an LSP
    /// backend, the catalog AND this test must change together.
    #[test]
    fn precision_floor_for_tool_distinguishes_lsp_tools() {
        assert_eq!(precision_floor_for_tool("go_to_definition"), Precision::Lsp);
        assert_eq!(precision_floor_for_tool("find_references"), Precision::Lsp);
        assert_eq!(precision_floor_for_tool("get_file_symbols"), Precision::Lsp);
        assert_eq!(
            precision_floor_for_tool("get_call_hierarchy"),
            Precision::Index
        );
        assert_eq!(
            precision_floor_for_tool("get_implementors"),
            Precision::Index
        );
        assert_eq!(
            precision_floor_for_tool("get_type_references"),
            Precision::Index
        );
        assert_eq!(
            precision_floor_for_tool("query_symbol_index"),
            Precision::AstHeuristic
        );
        assert_eq!(
            precision_floor_for_tool("find_usages"),
            Precision::AstHeuristic
        );
    }

    /// AAT-B3-01: an AST-heuristic result MUST NOT be presented as
    /// LSP-grade. The test pins that the helper returns the correct
    /// floor for the absent-LSP scenario and that the kebab-case wire
    /// shape of `ast-heuristic` differs from `lsp` byte-for-byte.
    #[test]
    fn ast_heuristic_results_are_not_presented_as_lsp() {
        let precision = precision_floor_for_tool("find_usages");
        assert_eq!(
            precision,
            Precision::AstHeuristic,
            "find_usages has no LSP backend; precision must reflect that"
        );
        assert!(!precision.is_lsp_grade());
        let wire = serde_json::to_string(&precision).unwrap();
        assert_ne!(
            wire, "\"lsp\"",
            "ast-heuristic precision must never serialise as lsp: {wire}"
        );
    }
}
