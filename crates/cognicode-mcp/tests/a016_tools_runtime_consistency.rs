//! A-016 / CP2 hard gate — `product/tools.json::authority` is
//! consistent with `CogniCodeHandler::MUTATING_TOOLS` runtime.
//!
//! The public product contract (A-006) declares each tool's
//! `authority` as either `"read"` or `"mutating"`. The runtime
//! enforces `--read-only` posture by filtering against a
//! hard-coded `MUTATING_TOOLS` list. If the two ever disagree,
//! one of them is wrong — and which one is "right" depends
//! entirely on context:
//!
//!   * `authority: "mutating"` in tools.json but the tool is
//!     NOT in `MUTATING_TOOLS`: under `--read-only` the runtime
//!     ACCEPTS the call, contradicting the public contract.
//!     Release-blocker.
//!
//!   * Tool in `MUTATING_TOOLS` but `authority: "read"`: under
//!     `--read-only` the runtime REJECTS the call, contradicting
//!     the runtime's own expectation. Less severe (false negative
//!     on the public contract), but a contract honesty failure.
//!
//! This test is the auto-pin: any future change that drifts one
//! side without the other fails in CI, with a clear diff.
//!
//! Lesson 95 (N+32) recorded a similar gap for `build_graph`
//! but the gap was a methodology error (the cache is in-memory,
//! not on disk). This pin does not fix that historical gap; it
//! makes any future gap of the SAME shape immediately visible.
//!
//! Strict TDD discipline: 2 tests, baseline + triangulate.

use serde_json::Value;
use std::path::PathBuf;

/// Resolve `product/tools.json` relative to the workspace root.
///
/// `CARGO_MANIFEST_DIR` is `<workspace>/crates/cognicode-mcp`; the
/// workspace root is two `parent()`s up.
fn product_tools_json_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR has a parent")
        .join("product")
        .join("tools.json")
}

fn load_tools_json() -> Value {
    let path = product_tools_json_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("product/tools.json must be readable: {e}"));
    serde_json::from_str::<Value>(&text)
        .unwrap_or_else(|e| panic!("product/tools.json must be valid JSON: {e}"))
}

/// Contract → runtime: every tool declared `mutating` in the
/// public contract MUST be in the runtime mutating set. Without
/// this, `--read-only` would accept a call the contract says is
/// mutating — the most damaging class of drift (false negative on
/// the public promise).
#[test]
fn a016_tools_runtime_consistency_every_contract_mutating_is_in_runtime() {
    let tools = load_tools_json();
    let tools_array = tools["tools"]
        .as_array()
        .expect("`tools` must be a JSON array");
    let mutating_set: std::collections::HashSet<String> =
        cognicode_core::interface::mcp::rmcp_adapter::mutating_tool_names()
            .into_iter()
            .collect();
    let contract_mutating: Vec<String> = tools_array
        .iter()
        .filter(|t| t.get("authority").and_then(|v| v.as_str()) == Some("mutating"))
        .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
        .collect();
    let missing: Vec<&String> = contract_mutating
        .iter()
        .filter(|n| !mutating_set.contains(*n))
        .collect();
    assert!(
        missing.is_empty(),
        "Tools declared `authority: mutating` in product/tools.json but NOT in \
         the runtime mutating set. Under `--read-only` the \
         runtime would ACCEPT these calls, contradicting the public contract. \
         Missing from runtime: {missing:?}. Either declare them with a \
         non-read authority in the tool or change tools.json."
    );
    assert!(
        !contract_mutating.is_empty(),
        "No tool declared `authority: mutating` in product/tools.json — the \
         public contract claims NO mutating tools. That is either a \
         documentation gap (a mutating tool is missing its declaration) or \
         the runtime mutating set is dead code. Inspect both."
    );
}

/// Runtime → contract: every entry in the runtime mutating set
/// MUST be declared `mutating` in the public contract. Without
/// this, a tool the runtime REFUSES under `--read-only` is
/// advertised to adopters as a read tool — a false negative on
/// the contract's reader-facing posture.
#[test]
fn a016_tools_runtime_consistency_every_runtime_mutating_is_in_contract() {
    let tools = load_tools_json();
    let tools_array = tools["tools"]
        .as_array()
        .expect("`tools` must be a JSON array");
    let contract_authority: std::collections::HashMap<String, String> = tools_array
        .iter()
        .filter_map(|t| {
            let name = t.get("name").and_then(|v| v.as_str())?;
            let auth = t.get("authority").and_then(|v| v.as_str())?;
            Some((name.to_string(), auth.to_string()))
        })
        .collect();
    let mut unexpected: Vec<String> = Vec::new();
    for runtime_tool in cognicode_core::interface::mcp::rmcp_adapter::mutating_tool_names() {
        match contract_authority.get(&runtime_tool) {
            Some(auth) if auth == "mutating" => {}
            Some(auth) => unexpected.push(format!(
                "`{runtime_tool}` is in the runtime mutating set but declared \
                 authority=read in tools.json (saw `{auth}`); the runtime \
                 refuses it under --read-only but the contract advertises it as read"
            )),
            None => unexpected.push(format!(
                "`{runtime_tool}` is in the runtime mutating set but is missing \
                 from product/tools.json entirely"
            )),
        }
    }
    assert!(unexpected.is_empty(), "{}", unexpected.join("\n  - "));
}

/// Optional triangulation: `authorities` field (presence and type)
/// on each tool entry must be a valid string. A non-string
/// authority would break both this pin and the runtime posture
/// checks in A-009 / A-010 / A-014.
#[test]
fn a016_tools_runtime_consistency_authority_is_present_and_string() {
    let tools = load_tools_json();
    let tools_array = tools["tools"]
        .as_array()
        .expect("`tools` must be a JSON array");
    let bad: Vec<String> = tools_array
        .iter()
        .filter_map(|t| {
            let name = t.get("name").and_then(|v| v.as_str())?;
            match t.get("authority") {
                None => Some(format!("{name}: missing `authority`")),
                Some(v) if !v.is_string() => {
                    Some(format!("{name}: `authority` must be a string, got {v:?}"))
                }
                _ => None,
            }
        })
        .collect();
    assert!(
        bad.is_empty(),
        "Tool entries with malformed `authority`:\n  - {}",
        bad.join("\n  - ")
    );
}
