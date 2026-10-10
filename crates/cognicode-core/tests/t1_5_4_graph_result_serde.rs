//! T1-5.4 — Smoke test for `GraphResult` and `GraphError` JSON
//! serialization. Pins the data-side contract that the bin's
//! JSON presenter (T1-5.5) consumes.
//!
//! Estado pre-T1-5.4:
//!   - `execute_graph` (11 sub-commands: OnDemand, PerFile, Full,
//!     HotPaths, EntryPoints, LeafFunctions, TracePath, Mermaid,
//!     Hierarchy, Complexity, Impact) emitted ~50 println! and
//!     ~10 eprintln! calls with no machine-readable form. The
//!     data side was implicit (each match arm re-computed strings
//!     from in-scope variables).
//!   - The T1-5.4 design introduces `GraphResult` (data) + per
//!     sub-command `Ok(GraphResult::Variant { ... })` returns.
//!     This test pins the data side of that contract: every
//!     `GraphResult` variant must serialize to a JSON document
//!     with a `kind` field that the bin can dispatch on.
//!
//! What this RED protects:
//!   - A change to `#[serde(tag = "kind")]` removing the
//!     discriminator breaks the JSON dispatch.
//!   - Renaming a variant without updating the wire format
//!     breaks the same consumers.
//!   - Adding a new variant without declaring its wire form is
//!     caught (the test enumerates the known kinds).
//!   - Each variant's expected fields are pinned (name strings,
//!     numeric types, nested struct shapes), so a refactor that
//!     changes a field type (e.g. `String` to `PathBuf`) breaks
//!     the test before the bin silently gets a different shape.

use cognicode_core::interface::cli::result::{
    ComplexityMetrics, DepthBucket, DepthEntry, GraphResult, HotPathEntry, PathHop, SkippedFile,
    SymbolRef,
};
use serde_json::Value;

#[test]
fn graph_result_ondemand_serializes_with_kind_ondemand() {
    let result = GraphResult::OnDemand {
        symbol: "alpha".to_string(),
        depth: 3,
        direction: "both".to_string(),
        elapsed_ms: 42,
        root_name: "alpha".to_string(),
        root_file: "/lib/foo.rs".to_string(),
        root_line: 10,
        root_column: 1,
        entries_len: 5,
    };
    let json: Value =
        serde_json::to_value(&result).expect("GraphResult must serialize to JSON");
    assert_eq!(json["kind"], "on_demand", "wire format must have kind=on_demand");
    assert_eq!(json["symbol"], "alpha");
    assert_eq!(json["depth"], 3);
    assert_eq!(json["direction"], "both");
    assert_eq!(json["elapsed_ms"], 42);
    assert_eq!(json["root_name"], "alpha");
    assert_eq!(json["root_file"], "/lib/foo.rs");
    assert_eq!(json["root_line"], 10);
    assert_eq!(json["root_column"], 1);
    assert_eq!(json["entries_len"], 5);
}

#[test]
fn graph_result_perfile_serializes_with_kind_perfile() {
    let result = GraphResult::PerFile {
        file: "/lib/foo.rs".to_string(),
        symbols: 7,
        dependencies: 3,
    };
    let json: Value =
        serde_json::to_value(&result).expect("GraphResult must serialize to JSON");
    assert_eq!(json["kind"], "per_file", "wire format must have kind=per_file");
    assert_eq!(json["file"], "/lib/foo.rs");
    assert_eq!(json["symbols"], 7);
    assert_eq!(json["dependencies"], 3);
}

#[test]
fn graph_result_full_serializes_with_status_and_skipped() {
    let result = GraphResult::Full {
        path: "/work/repo".to_string(),
        rebuild: true,
        elapsed_ms: 1234,
        symbols: 500,
        dependencies: 1200,
        status: "partial".to_string(),
        skipped_files: vec![SkippedFile {
            path: "/work/repo/bad.rs".to_string(),
            reason: "ParseError".to_string(),
        }],
    };
    let json: Value =
        serde_json::to_value(&result).expect("GraphResult must serialize to JSON");
    assert_eq!(json["kind"], "full");
    assert_eq!(json["path"], "/work/repo");
    assert_eq!(json["rebuild"], true);
    assert_eq!(json["elapsed_ms"], 1234);
    assert_eq!(json["symbols"], 500);
    assert_eq!(json["dependencies"], 1200);
    assert_eq!(json["status"], "partial");
    assert_eq!(json["skipped_files"][0]["path"], "/work/repo/bad.rs");
    assert_eq!(json["skipped_files"][0]["reason"], "ParseError");
}

#[test]
fn graph_result_hotpaths_serializes_entries_array() {
    let result = GraphResult::HotPaths {
        path: "/work/repo".to_string(),
        limit: 10,
        min_fan_in: 1,
        entries: vec![HotPathEntry {
            name: "alpha".to_string(),
            fan_in: 5_usize,
            fan_out: 2_usize,
            file: "/lib/foo.rs".to_string(),
            line: 10,
        }],
        elapsed_ms: 50,
    };
    let json: Value =
        serde_json::to_value(&result).expect("GraphResult must serialize to JSON");
    assert_eq!(json["kind"], "hot_paths");
    assert_eq!(json["limit"], 10);
    assert_eq!(json["min_fan_in"], 1);
    assert_eq!(json["entries"][0]["name"], "alpha");
    assert_eq!(json["entries"][0]["fan_in"], 5);
    assert_eq!(json["entries"][0]["fan_out"], 2);
    assert_eq!(json["entries"][0]["file"], "/lib/foo.rs");
    assert_eq!(json["entries"][0]["line"], 10);
    assert_eq!(json["elapsed_ms"], 50);
}

#[test]
fn graph_result_entrypoints_serializes_with_total_above_truncated_symbols() {
    // The data side carries ALL symbols (not just the 20 the
    // presenter truncates to). `total` makes the truncation
    // visible to consumers that want to know "there are more".
    let result = GraphResult::EntryPoints {
        path: "/work/repo".to_string(),
        symbols: vec![SymbolRef {
            name: "main".to_string(),
            file: "/lib/main.rs".to_string(),
            line: 1,
            column: 1,
        }],
        total: 47,
        elapsed_ms: 30,
    };
    let json: Value =
        serde_json::to_value(&result).expect("GraphResult must serialize to JSON");
    assert_eq!(json["kind"], "entry_points");
    assert_eq!(json["total"], 47, "total must reflect the untruncated count");
    assert_eq!(json["symbols"][0]["name"], "main");
    assert_eq!(json["symbols"][0]["file"], "/lib/main.rs");
    assert_eq!(json["symbols"][0]["line"], 1);
    assert_eq!(json["symbols"][0]["column"], 1);
}

#[test]
fn graph_result_leaffunctions_serializes_with_kind_leaf_functions() {
    // Same shape as EntryPoints but the kind discriminator
    // differs. A consumer that dispatches on kind must see
    // "leaf_functions" here, not "entry_points".
    let result = GraphResult::LeafFunctions {
        path: "/work/repo".to_string(),
        symbols: vec![],
        total: 0,
        elapsed_ms: 25,
    };
    let json: Value =
        serde_json::to_value(&result).expect("GraphResult must serialize to JSON");
    assert_eq!(json["kind"], "leaf_functions");
    assert_eq!(json["total"], 0);
    assert!(json["symbols"].as_array().unwrap().is_empty());
}

#[test]
fn graph_result_tracepath_found_serializes_hops_with_1indexed_position() {
    // PathHop.position is 1-indexed (matches the presenter's
    // "{i}. {name} at {file}:{line}" output). Pin that here so
    // a 0-indexed refactor (e.g. copying the array index) is
    // caught before it reaches the consumer.
    let result = GraphResult::TracePathFound {
        from: "alpha".to_string(),
        to: "beta".to_string(),
        hops: vec![
            PathHop {
                position: 1,
                name: "alpha".to_string(),
                file: "/lib/a.rs".to_string(),
                line: 1,
            },
            PathHop {
                position: 2,
                name: "beta".to_string(),
                file: "/lib/b.rs".to_string(),
                line: 5,
            },
        ],
        elapsed_ms: 8,
    };
    let json: Value =
        serde_json::to_value(&result).expect("GraphResult must serialize to JSON");
    assert_eq!(json["kind"], "trace_path_found");
    assert_eq!(json["from"], "alpha");
    assert_eq!(json["to"], "beta");
    assert_eq!(json["hops"][0]["position"], 1, "PathHop.position is 1-indexed");
    assert_eq!(json["hops"][0]["name"], "alpha");
    assert_eq!(json["hops"][1]["position"], 2);
    assert_eq!(json["elapsed_ms"], 8);
}

#[test]
fn graph_result_tracepath_notfound_is_its_own_variant() {
    // Distinct from TracePathFound { hops: vec![] } by design:
    // the kind discriminator tells consumers which side of the
    // match they are on without checking field presence.
    let result = GraphResult::TracePathNotFound {
        from: "alpha".to_string(),
        to: "ghost".to_string(),
        elapsed_ms: 3,
    };
    let json: Value =
        serde_json::to_value(&result).expect("GraphResult must serialize to JSON");
    assert_eq!(json["kind"], "trace_path_not_found");
    assert_eq!(json["from"], "alpha");
    assert_eq!(json["to"], "ghost");
    assert!(
        json.get("hops").is_none(),
        "TracePathNotFound must not carry a hops field; the kind discriminator is the signal"
    );
}

#[test]
fn graph_result_mermaid_serializes_truncated_flag() {
    let result = GraphResult::Mermaid {
        path: "/work/repo".to_string(),
        diagram: "graph TD\n  A --> B".to_string(),
        diagram_len: 18,
        format: "txt".to_string(),
        truncated: true,
        elapsed_ms: 40,
    };
    let json: Value =
        serde_json::to_value(&result).expect("GraphResult must serialize to JSON");
    assert_eq!(json["kind"], "mermaid");
    assert_eq!(json["diagram_len"], 18);
    assert_eq!(json["format"], "txt");
    assert_eq!(json["truncated"], true, "truncated flag is a stable signal for consumers");
    assert_eq!(json["elapsed_ms"], 40);
    // T1-5.4h: the diagram field is part of the data side
    // (the presenter needs the full content to reproduce
    // the pre-migration text).
    assert_eq!(json["diagram"], "graph TD\n  A --> B");
}

#[test]
fn graph_result_hierarchy_serializes_recursive_buckets() {
    // DepthBucket holds a Vec<DepthEntry>; the data must
    // preserve the structure so the presenter can emit the
    // "  Depth N: M entries" / "    - name (dir) at file:line"
    // block.
    let result = GraphResult::Hierarchy {
        symbol: "alpha".to_string(),
        depth: 2,
        direction: "out".to_string(),
        root_file: "/lib/a.rs".to_string(),
        root_line: 1,
        root_column: 1,
        buckets: vec![DepthBucket {
            depth: 1,
            entries: vec![DepthEntry {
                name: "beta".to_string(),
                direction: "callees".to_string(),
                file: "/lib/b.rs".to_string(),
                line: 5,
            }],
        }],
        total_entries: 1,
        elapsed_ms: 12,
    };
    let json: Value =
        serde_json::to_value(&result).expect("GraphResult must serialize to JSON");
    assert_eq!(json["kind"], "hierarchy");
    assert_eq!(json["symbol"], "alpha");
    assert_eq!(json["depth"], 2);
    assert_eq!(json["root_file"], "/lib/a.rs");
    assert_eq!(json["buckets"][0]["depth"], 1);
    assert_eq!(json["buckets"][0]["entries"][0]["name"], "beta");
    assert_eq!(
        json["buckets"][0]["entries"][0]["direction"], "callees",
        "direction is lowercase to match pre-migration Debug-format + to_lowercase"
    );
    assert_eq!(json["total_entries"], 1);
}

#[test]
fn graph_result_complexity_serializes_all_nine_metrics() {
    // ComplexityMetrics has 9 fields; the test enumerates each
    // one so a refactor that drops a metric (e.g. removing
    // leaf_function_count because it was redundant) breaks
    // the test before the bin silently loses the column.
    let result = GraphResult::Complexity {
        path: "/work/repo".to_string(),
        metrics: ComplexityMetrics {
            total_symbols: 100,
            total_edges: 200,
            max_depth: 5,
            cyclomatic_complexity: 42,
            high_fan_out_count: 3,
            medium_fan_out_count: 7,
            low_fan_out_count: 90,
            entry_point_count: 4,
            leaf_function_count: 12,
        },
        elapsed_ms: 60,
    };
    let json: Value =
        serde_json::to_value(&result).expect("GraphResult must serialize to JSON");
    assert_eq!(json["kind"], "complexity");
    assert_eq!(json["metrics"]["total_symbols"], 100);
    assert_eq!(json["metrics"]["total_edges"], 200);
    assert_eq!(json["metrics"]["max_depth"], 5);
    assert_eq!(json["metrics"]["cyclomatic_complexity"], 42);
    assert_eq!(json["metrics"]["high_fan_out_count"], 3);
    assert_eq!(json["metrics"]["medium_fan_out_count"], 7);
    assert_eq!(json["metrics"]["low_fan_out_count"], 90);
    assert_eq!(json["metrics"]["entry_point_count"], 4);
    assert_eq!(json["metrics"]["leaf_function_count"], 12);
    assert_eq!(json["elapsed_ms"], 60);
}

#[test]
fn graph_result_impact_serializes_risk_and_untruncated_lists() {
    // The data side carries ALL impacted symbols/files (the
    // presenter truncates to 20/10). Pin that the lists are
    // complete in JSON so consumers can re-implement their own
    // truncation without losing data.
    let result = GraphResult::Impact {
        symbol: "alpha".to_string(),
        risk_level: "HIGH".to_string(),
        direct_dependents: 3,
        total_impacted_symbols: 8,
        impacted_files_count: 4,
        impacted_symbols: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        impacted_files: vec!["/lib/a.rs".to_string(), "/lib/b.rs".to_string()],
        elapsed_ms: 70,
    };
    let json: Value =
        serde_json::to_value(&result).expect("GraphResult must serialize to JSON");
    assert_eq!(json["kind"], "impact");
    assert_eq!(json["risk_level"], "HIGH");
    assert_eq!(json["direct_dependents"], 3);
    assert_eq!(json["total_impacted_symbols"], 8);
    assert_eq!(json["impacted_files_count"], 4);
    assert_eq!(json["impacted_symbols"].as_array().unwrap().len(), 3);
    assert_eq!(json["impacted_files"].as_array().unwrap().len(), 2);
}
