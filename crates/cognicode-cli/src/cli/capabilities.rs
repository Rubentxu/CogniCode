//! The capabilities document: what the running binary can do.
//!
//! The document is built from the product's own assets — the tool
//! catalogue, the profile surface and the runtime posture — so the
//! answer a client reads is the answer the running build can give.
//!
//! It lives in its own module because the document and the resolver
//! that finds the assets it is built from are one concern, and the
//! command executor is another: the executor asks for the answer and
//! writes it.

use std::path::{Path, PathBuf};

use serde_json::Value;

// ============================================================================
// A-014 / CP2.4 — `cognicode capabilities` subcommand.
//
// The `build_capabilities_doc` function is intentionally free-standing
// (not a method on `CommandExecutor`) so unit tests can call it
// directly without spawning the binary. The only side effect is the
// optional `git rev-parse HEAD` call for `source_commit`, which has a
// graceful fallback to the empty string when git is unavailable.
// ============================================================================

/// Build the JSON document for `cognicode capabilities` (A-014).
///
/// Sources (in precedence order):
///   1. `product/tools.json` for the public tool catalogue.
///   2. `product/profiles.json` for the public profile surface.
///   3. `cognicode_core::product::PROFILE_POSTURES` (canonical runtime
///      posture; overrides the published `mutating` flag if the two
///      disagree — the runtime is authoritative).
///   4. `CogniCodeHandler::MUTATING_TOOLS` (canonical runtime
///      mutating set; the source of truth for `--read-only` filtering).
///   5. `git rev-parse HEAD` for `source_commit` (best-effort).
///
/// No network, no mutation. Reading from `product/*.json` falls back
/// to runtime-only data when the artefacts are missing (rare; only
/// happens if the binary was built against a tree where the files
/// have been moved). A warning is emitted on stderr in that case,
/// unless `json_emission` is set: JSON output is pipeable unfiltered,
/// so a successful run must never write diagnostics to stderr.
pub(crate) fn build_capabilities_doc(json_emission: bool) -> Value {
    use cognicode_core::product::PROFILE_POSTURES;

    let workspace_root = resolve_product_assets_root();
    let tools_path = workspace_root.join("product").join("tools.json");
    let profiles_path = workspace_root.join("product").join("profiles.json");

    let mut doc = serde_json::json!({
        "schema_version": "cognicode.capabilities/v1",
        "cli_version": env!("CARGO_PKG_VERSION"),
        "source_commit": current_source_commit(&workspace_root),
    });

    let mut profiles_arr: Vec<Value> = Vec::new();
    if let Ok(text) = std::fs::read_to_string(&profiles_path)
        && let Ok(parsed) = serde_json::from_str::<Value>(&text)
        && let Some(arr) = parsed.get("profiles").and_then(|v| v.as_array())
    {
        let posture_by_id: std::collections::HashMap<&str, bool> = PROFILE_POSTURES
            .iter()
            .map(|(id, posture)| (*id, !posture.is_read_only()))
            .collect();
        for p in arr {
            let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let mut entry = p.clone();
            if let Some(&mutating) = posture_by_id.get(id) {
                // Runtime posture is authoritative.
                entry["mutating"] = Value::Bool(mutating);
            }
            profiles_arr.push(entry);
        }
    }
    if profiles_arr.is_empty() {
        profiles_arr = PROFILE_POSTURES
            .iter()
            .map(|(id, posture)| {
                serde_json::json!({
                    "id": id,
                    "mutating": !posture.is_read_only(),
                })
            })
            .collect();
    }
    doc["profiles"] = Value::Array(profiles_arr);

    // Tools: the typed catalog is the single source of truth. B2.W2
    // replaced the manual `serde_json::Value` clone-loop with a round trip
    // through `ToolDescriptor`, so the JSON output stays shape-compatible
    // with `product/tools.json` and the parity test compares the CLI
    // document to the MCP catalog field-for-field.
    //
    // `load_tool_catalog_active()` is feature-filtered so the CLI document
    // matches the MCP `tools/list` exactly: both are the *live* surface.
    // The catalogue file stays the source of truth on disk; the `feature`
    // field says which entries are live in this build (73 in the default
    // build, 74 with `evidence-cli-ladybug`).
    let catalog = cognicode_core::interface::catalog::load_tool_catalog_active();
    let tools_arr: Vec<Value> = catalog
        .iter()
        .map(|d| serde_json::to_value(d).expect("ToolDescriptor is serialisable"))
        .collect();
    if tools_arr.is_empty() && !json_emission {
        eprintln!(
            "warning: {} not found; emitting empty tools array",
            tools_path.display()
        );
    }
    doc["tools"] = Value::Array(tools_arr);

    // Derived from the tools' own declarations, so this report cannot
    // disagree with what `--read-only` actually refuses.
    let mutating_tools: Vec<String> = catalog
        .iter()
        .filter(|descriptor| descriptor.authority != "read")
        .map(|descriptor| descriptor.name.clone())
        .collect();
    doc["runtime"] = serde_json::json!({
        "mutating_tools": mutating_tools,
        "mutating_tools_count": mutating_tools.len(),
    });

    doc
}

/// Resolve the root that carries `product/*.json` for the **running** binary.
///
/// Thin wrapper over [`cognicode_core::interface::catalog::product_assets_root`],
/// which owns the anchor and the reasoning behind it. The CLI and the MCP
/// catalogue must resolve product assets the same way; the second copy of this
/// logic is what let an installed server advertise an empty tool catalogue
/// while the CLI worked perfectly against the same binary.
///
/// **Why not `env!("CARGO_MANIFEST_DIR")`.** That macro is resolved at compile
/// time and bakes the *build machine's* directory into the binary. Proven
/// failure: a binary compiled inside worktree `/tmp/a014vanish`, run after
/// that worktree was deleted, printed
/// `warning: /tmp/a014vanish/product/tools.json not found; emitting empty
/// tools array` and reported `tools: 0`. Since `release-validate.yml` builds
/// on a GitHub runner, every installed binary carried a path its user cannot
/// have, and the command whose entire purpose is machine-readable discovery
/// advertised zero capabilities.
///
/// The returned value is a path even when nothing was found: the caller
/// already warns on stderr and falls back to built-in defaults, and a wrong
/// absolute path there would only make the warning point at somewhere
/// meaningless.
pub(crate) fn resolve_product_assets_root() -> PathBuf {
    cognicode_core::interface::catalog::product_assets_root()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

/// Resolve `source_commit` for the capabilities document.
///
/// `git rev-parse HEAD` via subprocess; returns the empty string if
/// git is missing or the working tree is not a git checkout.
fn current_source_commit(workspace_root: &Path) -> String {
    std::process::Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(workspace_root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}
/// The text answer of `cognicode capabilities`, rendered and not printed.
///
/// The core returns the text and the presenter writes it: a core that
/// prints its own answer can interleave it with the document a
/// consumer is parsing.
pub(crate) fn format_capabilities_text(doc: &Value) -> String {
    let tools = doc["tools"].as_array().map(|a| a.len()).unwrap_or(0);
    let profiles = doc["profiles"].as_array().map(|a| a.len()).unwrap_or(0);
    let mutating = doc["runtime"]["mutating_tools"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    let mut text = String::new();
    text.push_str("CogniCode Capabilities\n");
    text.push_str(&format!(
        "  cli_version:    {}\n",
        doc["cli_version"].as_str().unwrap_or("")
    ));
    text.push_str(&format!(
        "  source_commit:  {}\n",
        doc["source_commit"].as_str().unwrap_or("")
    ));
    text.push_str(&format!("  tools:          {tools}\n"));
    text.push_str(&format!("  profiles:       {profiles}\n"));
    text.push_str(&format!("  mutating_tools: {mutating} (runtime)\n"));
    text.push('\n');
    text.push_str("Pass `--format json` for the machine-readable contract.\n");
    text
}
