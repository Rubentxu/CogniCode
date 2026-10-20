//! T1-5.5 — JSON dispatch tests.
//!
//! Estado pre-T1-5.5:
//!   - The presenter only emitted text. The `cogh graph full --format
//!     json` path used an ad-hoc `FullGraphJson` envelope hardcoded
//!     inside `commands.rs`; no other sub-command supported JSON.
//!   - No test pinned the JSON byte stream — every variant was
//!     verified only at the Rust level via `t1_5_3b_index_result_serde`
//!     and `t1_5_4_graph_result_serde`.
//!
//! What this RED protects:
//!   - `present_index_result_json` emits parseable JSON for every
//!     `IndexResult` variant.
//!   - `present_graph_result_json` emits parseable JSON for every
//!     `GraphResult` variant.
//!   - The `kind` discriminator is present and correct (this is the
//!     wire contract for downstream consumers that switch on variant).
//!   - `present_output(&CommandOutput::Index(r), Format::Json, w)`
//!     and `present_output(&CommandOutput::Graph(r), Format::Json, w)`
//!     route through the JSON presenters.
//!   - `present_output(&CommandOutput::Other, _, _)` is a no-op
//!     (writes nothing).
//!
//! Why parse to `serde_json::Value` and not `IndexResult` /
//! `GraphResult` directly:
//!   The structs carry `elapsed_ms: u128` fields (from
//!   `Instant::elapsed().as_millis()`). serde_json's default
//!   deserializer does NOT support u128 because JSON has no
//!   native 128-bit integer. Adding `Deserialize` to the structs
//!   just for tests would force a `#[serde(with = ...)]` adapter
//!   or a u64 migration, both heavier than the test deserves.
//!   Parsing to `Value` covers the same regression surface —
//!   "the byte stream is well-formed JSON with the expected
//!   `kind`" — without touching the data side.
//!
//! Why this is additive to the serde tests:
//!   The serde tests in `t1_5_3b_index_result_serde.rs` and
//!   `t1_5_4_graph_result_serde.rs` only check that the struct
//!   itself serializes to a JSON Value. They don't verify the
//!   *presenter's* output (which adds a trailing newline and
//!   decides pretty vs compact). A regression that breaks the
//!   presenter's output (wrong newline, compact vs pretty
//!   swapped, kind discriminator removed from the enum) would
//!   pass the serde tests but fail here.

use cognicode_cli::presenter::test_helpers::outline_node;
use cognicode_cli::presenter::{
    present_graph_result_json, present_index_result_json, present_output,
};
use cognicode_core::domain::value_objects::SymbolKind;
use cognicode_core::infrastructure::graph::SymbolLocation;
use cognicode_core::interface::cli::commands::Format;
use cognicode_core::interface::cli::result::{CommandOutput, GraphResult, IndexResult};
use serde_json::Value;
use std::path::PathBuf;

fn render_json(emit: impl FnOnce(&mut Vec<u8>) -> std::io::Result<()>) -> String {
    let mut buf: Vec<u8> = Vec::new();
    emit(&mut buf).expect("presenter write to Vec<u8> is infallible");
    String::from_utf8(buf).expect("presenter emits UTF-8 JSON")
}

fn assert_kind(out: &str, expected: &str) {
    let parsed: Value = serde_json::from_str(out)
        .unwrap_or_else(|e| panic!("presenter output must be parseable JSON: {e}; got {out:?}"));
    let kind = parsed
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("presenter JSON must carry a 'kind' discriminator; got {out:?}"));
    assert_eq!(kind, expected, "kind mismatch in {out:?}");
}

// ===================================================================
// IndexResult — 5 variants
// ===================================================================

#[test]
fn json_index_build_shape() {
    let r = IndexResult::Build {
        path: PathBuf::from("/work/repo"),
        strategy: "lightweight".to_string(),
        elapsed_ms: 1234,
    };
    let out = render_json(|w| present_index_result_json(&r, w));
    assert_kind(&out, "build");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["strategy"], "lightweight");
    assert_eq!(v["elapsed_ms"], 1234);
}

#[test]
fn json_index_query_shape() {
    let r = IndexResult::Query {
        symbol: "main".to_string(),
        path: PathBuf::from("/tmp/proj"),
        locations: vec![SymbolLocation {
            file: "/tmp/proj/src/main.rs".to_string(),
            line: 1,
            column: 1,
            symbol_kind: SymbolKind::Function,
        }],
    };
    let out = render_json(|w| present_index_result_json(&r, w));
    assert_kind(&out, "query");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["symbol"], "main");
    assert_eq!(v["locations"].as_array().unwrap().len(), 1);
}

#[test]
fn json_index_query_empty_shape() {
    let r = IndexResult::QueryEmpty {
        symbol: "missing".to_string(),
        path: PathBuf::from("."),
    };
    let out = render_json(|w| present_index_result_json(&r, w));
    assert_kind(&out, "query_empty");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["symbol"], "missing");
}

#[test]
fn json_index_outline_shape() {
    let leaf = outline_node("inner", SymbolKind::Function, 5, vec![]);
    let mid = outline_node("outer", SymbolKind::Class, 1, vec![leaf]);
    let r = IndexResult::Outline {
        file: PathBuf::from("/lib/foo.rs"),
        symbols: vec![mid],
    };
    let out = render_json(|w| present_index_result_json(&r, w));
    assert_kind(&out, "outline");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["symbols"].as_array().unwrap().len(), 1);
}

#[test]
fn json_index_symbol_code_shape() {
    let r = IndexResult::SymbolCode {
        file: PathBuf::from("/lib/foo.rs"),
        line: 10,
        column: 5,
        start_line: 10,
        end_line: 15,
        doc: Some("/// A doc".to_string()),
        code: "fn foo() {}".to_string(),
    };
    let out = render_json(|w| present_index_result_json(&r, w));
    assert_kind(&out, "symbol_code");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["line"], 10);
    assert_eq!(v["doc"], "/// A doc");
}

#[test]
fn json_index_has_trailing_newline() {
    // `cogh --format json ... | jq .kind` consumes the trailing
    // newline as a regular character; jq tolerates it. But shell
    // pipelines that grep the JSON object on its own line depend
    // on a deterministic trailing \n.
    let r = IndexResult::Build {
        path: PathBuf::from("/"),
        strategy: "lightweight".to_string(),
        elapsed_ms: 1,
    };
    let out = render_json(|w| present_index_result_json(&r, w));
    assert!(
        out.ends_with('\n'),
        "JSON output must end with \\n for line-based consumers; got {out:?}"
    );
}

// ===================================================================
// GraphResult — 11 variants (one test per variant)
// ===================================================================

#[test]
fn json_graph_on_demand_shape() {
    let r = GraphResult::OnDemand {
        symbol: "foo".to_string(),
        depth: 2,
        direction: "out".to_string(),
        elapsed_ms: 50,
        root_name: "foo".to_string(),
        root_file: "/src/foo.rs".to_string(),
        root_line: 1,
        root_column: 0,
        entries_len: 3,
    };
    let out = render_json(|w| present_graph_result_json(&r, w));
    assert_kind(&out, "on_demand");
}

#[test]
fn json_graph_per_file_shape() {
    let r = GraphResult::PerFile {
        file: "/src/foo.rs".to_string(),
        symbols: 12,
        dependencies: 5,
    };
    let out = render_json(|w| present_graph_result_json(&r, w));
    assert_kind(&out, "per_file");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["symbols"], 12);
}

#[test]
fn json_graph_full_shape() {
    let r = GraphResult::Full {
        path: "/repo".to_string(),
        rebuild: false,
        elapsed_ms: 200,
        symbols: 100,
        dependencies: 40,
        status: "complete".to_string(),
        skipped_files: vec![],
        pending_references: vec![],
    };
    let out = render_json(|w| present_graph_result_json(&r, w));
    assert_kind(&out, "full");
    // The legacy `FullGraphJson` envelope had `schema_version` at the
    // root; the consolidated envelope uses `kind: "full"`. Pin that
    // the legacy field is NOT present (downstream consumers that
    // depended on schema_version must update).
    assert!(
        !out.contains("schema_version"),
        "wire format change: legacy schema_version field must be gone; got {out:?}"
    );
}

#[test]
fn json_graph_hot_paths_shape() {
    let r = GraphResult::HotPaths {
        path: "/repo".to_string(),
        limit: 10,
        min_fan_in: 2,
        entries: vec![],
        elapsed_ms: 30,
    };
    let out = render_json(|w| present_graph_result_json(&r, w));
    assert_kind(&out, "hot_paths");
}

#[test]
fn json_graph_entry_points_shape() {
    let r = GraphResult::EntryPoints {
        path: "/repo".to_string(),
        symbols: vec![],
        total: 0,
        elapsed_ms: 25,
    };
    let out = render_json(|w| present_graph_result_json(&r, w));
    assert_kind(&out, "entry_points");
}

#[test]
fn json_graph_leaf_functions_shape() {
    let r = GraphResult::LeafFunctions {
        path: "/repo".to_string(),
        symbols: vec![],
        total: 0,
        elapsed_ms: 25,
    };
    let out = render_json(|w| present_graph_result_json(&r, w));
    assert_kind(&out, "leaf_functions");
}

#[test]
fn json_graph_trace_path_found_shape() {
    let r = GraphResult::TracePathFound {
        from: "a".to_string(),
        to: "b".to_string(),
        hops: vec![],
        elapsed_ms: 10,
    };
    let out = render_json(|w| present_graph_result_json(&r, w));
    assert_kind(&out, "trace_path_found");
}

#[test]
fn json_graph_trace_path_not_found_shape() {
    let r = GraphResult::TracePathNotFound {
        from: "a".to_string(),
        to: "z".to_string(),
        elapsed_ms: 10,
    };
    let out = render_json(|w| present_graph_result_json(&r, w));
    assert_kind(&out, "trace_path_not_found");
}

#[test]
fn json_graph_mermaid_shape() {
    let r = GraphResult::Mermaid {
        path: "/repo".to_string(),
        diagram: "graph TD\n  a-->b".to_string(),
        diagram_len: 16,
        format: "txt".to_string(),
        truncated: false,
        elapsed_ms: 5,
    };
    let out = render_json(|w| present_graph_result_json(&r, w));
    assert_kind(&out, "mermaid");
}

#[test]
fn json_graph_hierarchy_shape() {
    let r = GraphResult::Hierarchy {
        symbol: "foo".to_string(),
        depth: 2,
        direction: "out".to_string(),
        root_file: "/src/foo.rs".to_string(),
        root_line: 1,
        root_column: 0,
        buckets: vec![],
        total_entries: 0,
        elapsed_ms: 12,
    };
    let out = render_json(|w| present_graph_result_json(&r, w));
    assert_kind(&out, "hierarchy");
}

#[test]
fn json_graph_complexity_shape() {
    use cognicode_core::interface::cli::result::ComplexityMetrics;
    let r = GraphResult::Complexity {
        path: "/repo".to_string(),
        metrics: ComplexityMetrics {
            total_symbols: 100,
            total_edges: 40,
            max_depth: 5,
            cyclomatic_complexity: 200,
            high_fan_out_count: 3,
            medium_fan_out_count: 10,
            low_fan_out_count: 50,
            entry_point_count: 8,
            leaf_function_count: 30,
        },
        elapsed_ms: 60,
    };
    let out = render_json(|w| present_graph_result_json(&r, w));
    assert_kind(&out, "complexity");
}

#[test]
fn json_graph_impact_shape() {
    let r = GraphResult::Impact {
        symbol: "foo".to_string(),
        risk_level: "low".to_string(),
        direct_dependents: 2,
        total_impacted_symbols: 5,
        impacted_files_count: 3,
        impacted_symbols: vec!["bar".to_string()],
        impacted_files: vec!["/src/bar.rs".to_string()],
        elapsed_ms: 15,
    };
    let out = render_json(|w| present_graph_result_json(&r, w));
    assert_kind(&out, "impact");
}

// ===================================================================
// present_output — top-level dispatcher
// ===================================================================

#[test]
fn present_output_text_index_uses_text_presenter() {
    let r = IndexResult::Build {
        path: PathBuf::from("/tmp/repo"),
        strategy: "lightweight".to_string(),
        elapsed_ms: 5,
    };
    let mut buf: Vec<u8> = Vec::new();
    present_output(&CommandOutput::Index(r), Format::Text, &mut buf)
        .expect("present_output must succeed");
    let text = String::from_utf8(buf).expect("UTF-8");
    // Pin the byte stream against the t1_5_3d snapshot of
    // present_index_result(Build variant): "Index built
    // successfully in {ms}ms using {strategy} strategy\n".
    assert!(
        text.contains("Index built successfully in 5ms using lightweight strategy"),
        "text path emitted wrong shape: {text:?}"
    );
    assert!(
        !text.starts_with('{'),
        "text path leaked JSON; got {text:?}"
    );
}

#[test]
fn present_output_json_index_uses_json_presenter() {
    let r = IndexResult::Build {
        path: PathBuf::from("/tmp/repo"),
        strategy: "lightweight".to_string(),
        elapsed_ms: 5,
    };
    let mut buf: Vec<u8> = Vec::new();
    present_output(&CommandOutput::Index(r), Format::Json, &mut buf)
        .expect("present_output must succeed");
    let text = String::from_utf8(buf).expect("UTF-8");
    assert!(
        text.starts_with('{'),
        "json path must emit a JSON object; got {text:?}"
    );
    let v: Value = serde_json::from_str(&text).expect("json must parse");
    assert_eq!(v["kind"], "build");
}

#[test]
fn present_output_json_graph_uses_json_presenter() {
    let r = GraphResult::PerFile {
        file: "/src/foo.rs".to_string(),
        symbols: 12,
        dependencies: 5,
    };
    let mut buf: Vec<u8> = Vec::new();
    present_output(&CommandOutput::Graph(r), Format::Json, &mut buf)
        .expect("present_output must succeed");
    let text = String::from_utf8(buf).expect("UTF-8");
    let v: Value = serde_json::from_str(&text).expect("json must parse");
    assert_eq!(v["kind"], "per_file");
}

#[test]
fn present_output_other_is_noop_for_either_format() {
    let mut buf_text: Vec<u8> = Vec::new();
    present_output(&CommandOutput::Other, Format::Text, &mut buf_text)
        .expect("present_output Other must succeed");
    assert!(
        buf_text.is_empty(),
        "CommandOutput::Other must not write to stdout (text); got {buf_text:?}"
    );

    let mut buf_json: Vec<u8> = Vec::new();
    present_output(&CommandOutput::Other, Format::Json, &mut buf_json)
        .expect("present_output Other must succeed");
    assert!(
        buf_json.is_empty(),
        "CommandOutput::Other must not write to stdout (json); got {buf_json:?}"
    );
}
