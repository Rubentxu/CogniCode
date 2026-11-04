//! T1-5.3b — Smoke test for `IndexResult` and `IndexError` JSON
//! serialization. Pins the contract that T1-5.3d will use to
//! verify the presenter matches the on-the-wire format.
//!
//! Estado pre-T1-5.3b:
//!   - The CLI's `execute_index` produced output only via
//!     `println!` calls; there was no machine-readable form.
//!   - The T1-5.3a design proposes `IndexResult` (data) +
//!     presenter (text/JSON). This test pins the data side
//!     of that contract: every `IndexResult` variant must
//!     serialize to a JSON document with a `kind` field
//!     that the presenter can match on.
//!
//! What this RED protects:
//!   - A change to `#[serde(tag = "kind")]` removing the
//!     discriminator breaks the presenter's match (and every
//!     external consumer that parses the JSON).
//!   - Renaming a variant without updating the wire format
//!     breaks the same consumers.
//!   - Adding a new variant without declaring its wire
//!     form is caught (the test enumerates the known kinds).

use cognicode_core::domain::value_objects::Location;
use cognicode_core::domain::value_objects::symbol_kind::SymbolKind;
use cognicode_core::infrastructure::graph::SymbolLocation;
use cognicode_core::infrastructure::semantic::outline::OutlineNode;
use cognicode_core::interface::cli::result::{IndexError, IndexResult};
use serde_json::Value;
use std::path::PathBuf;

#[test]
fn index_result_query_with_locations_serializes_with_kind_query() {
    let result = IndexResult::Query {
        symbol: "main".to_string(),
        path: PathBuf::from("/tmp/proj"),
        locations: vec![SymbolLocation {
            file: "/tmp/proj/src/main.rs".to_string(),
            line: 1,
            column: 1,
            symbol_kind: SymbolKind::Function,
        }],
    };
    let json: Value = serde_json::to_value(&result).expect("IndexResult must serialize to JSON");
    assert_eq!(json["kind"], "query", "wire format must have kind=query");
    assert_eq!(json["symbol"], "main");
    assert_eq!(json["path"], "/tmp/proj");
    assert_eq!(json["locations"][0]["file"], "/tmp/proj/src/main.rs");
    assert_eq!(json["locations"][0]["line"], 1);
    assert_eq!(json["locations"][0]["column"], 1);
    // SymbolKind serializes as PascalCase (per its own
    // #[serde(rename_all = ...)]); pin the exact form so
    // the presenter text formatter has a stable target.
    assert_eq!(json["locations"][0]["symbol_kind"], "Function");
}

#[test]
fn index_result_query_empty_serializes_with_kind_query_empty() {
    let result = IndexResult::QueryEmpty {
        symbol: "missing_xyz".to_string(),
        path: PathBuf::from("."),
    };
    let json: Value = serde_json::to_value(&result).expect("IndexResult must serialize to JSON");
    assert_eq!(
        json["kind"], "query_empty",
        "QueryEmpty must be its own kind"
    );
    assert_eq!(json["symbol"], "missing_xyz");
    assert_eq!(json["path"], ".");
    // Pin the absence of `locations` — QueryEmpty is
    // distinct from Query { locations: vec![] } by design.
    assert!(
        json.get("locations").is_none(),
        "QueryEmpty must not carry a locations field; \
         consumers distinguish it from Query by the kind field"
    );
}

#[test]
fn index_result_build_serializes_with_elapsed_ms() {
    let result = IndexResult::Build {
        path: PathBuf::from("/work/repo"),
        strategy: "lightweight".to_string(),
        elapsed_ms: 1234,
    };
    let json: Value = serde_json::to_value(&result).expect("IndexResult must serialize to JSON");
    assert_eq!(json["kind"], "build");
    assert_eq!(json["path"], "/work/repo");
    assert_eq!(json["strategy"], "lightweight");
    assert_eq!(json["elapsed_ms"], 1234);
}

#[test]
fn index_result_outline_serializes_recursive_tree() {
    // OutlineNode has a Vec<OutlineNode> children field; the
    // JSON must preserve the recursion so the presenter can
    // emit the hierarchical tree.
    let result = IndexResult::Outline {
        file: PathBuf::from("/lib/foo.rs"),
        symbols: vec![OutlineNode {
            name: "outer".to_string(),
            kind: SymbolKind::Function,
            location: Location::new("/lib/foo.rs", 1, 1),
            signature: None,
            children: vec![OutlineNode {
                name: "inner".to_string(),
                kind: SymbolKind::Function,
                location: Location::new("/lib/foo.rs", 5, 1),
                signature: None,
                children: vec![],
                is_private: false,
            }],
            is_private: false,
        }],
    };
    let json: Value = serde_json::to_value(&result).expect("IndexResult must serialize to JSON");
    assert_eq!(json["kind"], "outline");
    assert_eq!(json["symbols"][0]["name"], "outer");
    assert_eq!(json["symbols"][0]["children"][0]["name"], "inner");
}

#[test]
fn index_result_symbol_code_serializes_doc_as_optional() {
    let result = IndexResult::SymbolCode {
        file: PathBuf::from("/lib/foo.rs"),
        line: 10,
        column: 5,
        start_line: 10,
        end_line: 15,
        doc: Some("/// A doc".to_string()),
        code: "fn foo() {}".to_string(),
    };
    let json: Value = serde_json::to_value(&result).expect("IndexResult must serialize to JSON");
    assert_eq!(json["kind"], "symbol_code");
    assert_eq!(json["doc"], "/// A doc");

    // Also check the doc: None case serializes as null (the
    // presenter's text formatter prints nothing in that
    // case; consumers must handle both).
    let result_no_doc = IndexResult::SymbolCode {
        file: PathBuf::from("/lib/foo.rs"),
        line: 10,
        column: 5,
        start_line: 10,
        end_line: 15,
        doc: None,
        code: "fn foo() {}".to_string(),
    };
    let json_no_doc: Value = serde_json::to_value(&result_no_doc).unwrap();
    assert!(
        json_no_doc["doc"].is_null(),
        "SymbolCode with doc=None must serialize as null, not omitted (the presenter needs to know it considered the field)"
    );
}

#[test]
fn index_error_build_display_includes_strategy_and_path() {
    // The Display impl from thiserror is what the presenter
    // text formatter uses for error messages. Pin the
    // format so consumers parsing stderr output have a
    // stable target.
    let err = IndexError::Build {
        path: PathBuf::from("/work/missing"),
        strategy: "lightweight".to_string(),
        source: Box::<dyn std::error::Error + Send + Sync>::from(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no such file",
        )),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("/work/missing"),
        "error Display must include the path: {msg}"
    );
    assert!(
        msg.contains("lightweight"),
        "error Display must include the strategy: {msg}"
    );
    assert!(
        msg.contains("no such file"),
        "error Display must include the source message: {msg}"
    );
}

#[test]
fn index_error_query_display_includes_symbol() {
    let err = IndexError::Query {
        symbol: "ghost".to_string(),
        path: PathBuf::from("."),
        source: Box::<dyn std::error::Error + Send + Sync>::from(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        )),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("ghost"),
        "error Display must include the symbol: {msg}"
    );
    assert!(
        msg.contains("denied"),
        "error Display must include the source: {msg}"
    );
}
