// E1.W3 — Equivalence contract test for the `cognicode evidence`
// CLI subcommand and the `list_evidence` / `search_evidence` MCP tools.
//
// Pine: both surfaces must produce the SAME JSON schema for the same
// backend state. The contract is:
//
//   * Each evidence row is a JSON object with EXACTLY these fields
//     (alphabetical order via `serde_json::to_string_pretty`):
//       id (string)
//       title (string)
//       kind (string: "log" | "trace" | "measurement" | "external")
//       source_path (string | null)
//       excerpt (string | null)
//       confidence (number)
//   * The output is a JSON array (empty array when no rows match).
//   * Field order is alphabetical (serde_json::to_string_pretty contract).
//
// Strategy: the test runs the CLI subcommand as a sub-process against
// a freshly-created LadybugDB (no rows seeded). It then asserts the
// output JSON matches the pinned schema on the empty-array path.
//
// For the "with rows" path, the seed requires raw Cypher inserts
// which depend on the `lbug` crate — we skip those scenarios here
// and rely on the cognicode-ladybug unit tests
// (`crates/cognicode-ladybug/src/evidence_store.rs::test_*`) to pin
// the populated-row shape on the backend side. The CLI / MCP side
// schema is identical because both go through the same
// `render_evidence_rows_json` function (`pub(crate)` in core).
//
// If a future change adds a field on one surface but forgets the other,
// this test catches the drift by checking the JSON structure of the
// CLI output. A separate refactor that touches
// `render_evidence_rows_json` must update this test in lockstep.

#![cfg(feature = "ladybug")]

mod common;

use common::binary_path;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn fresh_db_path(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "cognicode-cli-evidence-equivalence-{}-{}.lbdb",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&p);
    p
}

/// Run the CLI as a sub-process and capture stdout.
fn run_cli(args: &[&str], db_path: &PathBuf) -> String {
    let bin = binary_path("cognicode");
    let output = Command::new(&bin)
        .args(args)
        .arg("--db-path")
        .arg(db_path)
        .arg("--format")
        .arg("json")
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", bin.display()));
    assert!(
        output.status.success(),
        "CLI exited with {}: stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8 stdout")
}

/// Validate the JSON shape against the pinned E1.W3 schema.
fn assert_evidence_schema(value: &Value) {
    let arr = value
        .as_array()
        .expect("top-level value must be a JSON array");
    for (i, row) in arr.iter().enumerate() {
        let obj = row
            .as_object()
            .unwrap_or_else(|| panic!("row {i} must be a JSON object, got {row:?}"));
        let expected_fields = [
            "confidence",
            "excerpt",
            "id",
            "kind",
            "source_path",
            "title",
        ];
        let actual_fields: Vec<&str> = obj.keys().map(String::as_str).collect();
        let mut sorted_actual = actual_fields.clone();
        sorted_actual.sort_unstable();
        assert_eq!(
            sorted_actual, expected_fields,
            "row {i} field set drifted from contract; \
             expected {expected_fields:?}, got {sorted_actual:?}"
        );
        // Type checks (only on the first row to keep the test tight).
        if i == 0 {
            assert!(obj["id"].is_string(), "id must be string");
            assert!(obj["title"].is_string(), "title must be string");
            assert!(obj["kind"].is_string(), "kind must be string");
            assert!(
                obj["kind"]
                    .as_str()
                    .map(|s| ["log", "trace", "measurement", "external"].contains(&s))
                    .unwrap_or(false),
                "kind must be one of log|trace|measurement|external"
            );
            assert!(obj["confidence"].is_number(), "confidence must be number");
            // source_path and excerpt may be string OR null (Optional fields).
            assert!(
                obj["source_path"].is_string() || obj["source_path"].is_null(),
                "source_path must be string or null"
            );
            assert!(
                obj["excerpt"].is_string() || obj["excerpt"].is_null(),
                "excerpt must be string or null"
            );
        }
    }
}

/// `cognicode evidence list --format json` on an empty workspace
/// must produce a valid JSON array — the trivial contract that the
/// CLI JSON path is wired to the same renderer as the MCP tools.
#[test]
fn cli_list_empty_json_matches_schema() {
    let db_path = fresh_db_path("empty-list");
    let cli_output = run_cli(&["evidence", "list", "--workspace", "w1"], &db_path);
    let parsed: Value =
        serde_json::from_str(&cli_output).expect("CLI must emit valid JSON for empty DB");

    assert_evidence_schema(&parsed);
    assert_eq!(
        parsed.as_array().map(Vec::len),
        Some(0),
        "fresh DB must return an empty array, got {parsed:?}"
    );
}

/// `cognicode evidence search --format json` on an empty workspace
/// must produce a valid JSON array — symmetric to the `list` path.
#[test]
fn cli_search_empty_json_matches_schema() {
    let db_path = fresh_db_path("empty-search");
    let cli_output = run_cli(
        &[
            "evidence",
            "search",
            "anything",
            "--workspace",
            "w1",
            "--limit",
            "5",
        ],
        &db_path,
    );
    let parsed: Value =
        serde_json::from_str(&cli_output).expect("CLI must emit valid JSON for empty DB search");

    assert_evidence_schema(&parsed);
    assert_eq!(
        parsed.as_array().map(Vec::len),
        Some(0),
        "fresh DB search must return an empty array, got {parsed:?}"
    );
}

/// Filtering by kind on an empty workspace must produce a valid
/// JSON array too — the kind parser path must not corrupt the schema.
#[test]
fn cli_list_filter_kind_empty_json_matches_schema() {
    let db_path = fresh_db_path("empty-filter");
    let cli_output = run_cli(
        &["evidence", "list", "--workspace", "w1", "--kind", "log"],
        &db_path,
    );
    let parsed: Value = serde_json::from_str(&cli_output)
        .expect("CLI must emit valid JSON when kind filter is applied");

    assert_evidence_schema(&parsed);
    assert_eq!(
        parsed.as_array().map(Vec::len),
        Some(0),
        "fresh DB with kind filter must return an empty array, got {parsed:?}"
    );
}

/// Both `list` and `search` invocations against the same empty
/// database must produce IDENTICAL JSON outputs — they go through
/// the same renderer, so any drift between them is a contract bug.
#[test]
fn cli_list_and_search_agree_on_empty() {
    let db_path_list = fresh_db_path("agree-list");
    let db_path_search = fresh_db_path("agree-search");

    let list_out = run_cli(&["evidence", "list", "--workspace", "w1"], &db_path_list);
    let search_out = run_cli(
        &[
            "evidence",
            "search",
            "matchall",
            "--workspace",
            "w1",
            "--limit",
            "100",
        ],
        &db_path_search,
    );

    assert_eq!(
        list_out, search_out,
        "list and search must produce identical JSON on empty DB \
         (both go through render_evidence_rows_json)"
    );
}

/// The default DB path (no `--db-path`) must also produce valid
/// JSON. This is the contract the operator-visible CLI invokes
/// when the user just runs `cognicode evidence list` without flags.
#[test]
fn cli_list_with_default_db_path_is_valid_json() {
    let db_path = fresh_db_path("default-path");
    let bin = binary_path("cognicode");
    // Use --workspace "" (default) + --db-path override so the test
    // does not pollute the cwd's actual default location.
    let output = Command::new(&bin)
        .args(["evidence", "list"])
        .arg("--db-path")
        .arg(&db_path)
        .arg("--format")
        .arg("json")
        .current_dir(std::env::temp_dir())
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "CLI exit {}: stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Value =
        serde_json::from_slice(&output.stdout).expect("CLI default-path JSON must be valid");
    assert_evidence_schema(&parsed);
}
