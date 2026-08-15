//! Parameter-alias resolution for the MCP dispatch boundary.
//!
//! UAT 2026-08-10 flagged DEFECT-1 (HIGH): a small set of MCP tools across
//! the surface had parameter names that did not match the canonical
//! naming convention the rest of the surface settled on. Rather than
//! break every external client on the v1.0.0 cut, this module holds a
//! single dispatch-time rename table that converts the canonical
//! (preferred) name to the legacy name the struct still expects, so
//! callers can use either name.
//!
//! Direction: canonical → legacy. `apply_aliases` only writes the
//! legacy key. If the legacy key is already present the canonical
//! key is dropped silently (caller wins on conflicts by NOT using
//! both).
//!
//! Removal is scheduled for v1.1 once the canonical names become the
//! only thing documented and clients have had time to migrate.

use serde_json::{Map, Value};

/// One alias: "this tool accepts `<canonical>` as a synonym for
/// `<legacy>`; the legacy key continues to work as well".
#[derive(Debug, Clone, Copy)]
pub struct ParamAlias {
    pub tool: &'static str,
    pub canonical: &'static str,
    pub legacy: &'static str,
}

/// Full alias table. Ordered by tool name for readable diffs. The
/// legacy column is the name the Input struct still uses today; the
/// canonical column is the name future tools use and the name v1.1
/// will require.
pub const ALIASES: &[ParamAlias] = &[
    // ── file path: canonical file_path, struct uses path ────────────
    ParamAlias {
        tool: "read_file",
        canonical: "file_path",
        legacy: "path",
    },
    ParamAlias {
        tool: "write_file",
        canonical: "file_path",
        legacy: "path",
    },
    ParamAlias {
        tool: "edit_file",
        canonical: "file_path",
        legacy: "path",
    },
    ParamAlias {
        tool: "search_content",
        canonical: "file_path",
        legacy: "path",
    },
    ParamAlias {
        tool: "list_files",
        canonical: "file_path",
        legacy: "path",
    },
    ParamAlias {
        tool: "get_symbol_code",
        canonical: "file_path",
        legacy: "file",
    },
    // ── call-graph endpoints: source/target → from_symbol/to_symbol ──
    ParamAlias {
        tool: "trace_path",
        canonical: "from_symbol",
        legacy: "source",
    },
    ParamAlias {
        tool: "trace_path",
        canonical: "to_symbol",
        legacy: "target",
    },
    // ── AI query endpoints: query, struct uses question ─────────────
    ParamAlias {
        tool: "ask_about_code",
        canonical: "query",
        legacy: "question",
    },
];

/// Apply the alias table to a JSON `arguments` object for the given tool.
///
/// Returns the list of (canonical, legacy) pairs that were actually
/// substituted, so the caller can emit a single `tracing::warn!` per
/// call rather than per alias.
///
/// Contract:
/// - If the legacy key is already present, the canonical key is
///   dropped silently — callers shouldn't send both.
/// - If only the canonical key is present, it is renamed to the
///   legacy key and the value is preserved.
/// - If neither is present, nothing changes.
pub fn apply_aliases(tool_name: &str, arguments: &mut Map<String, Value>) -> Vec<(String, String)> {
    let mut substitutions = Vec::new();
    for alias in ALIASES {
        if alias.tool != tool_name {
            continue;
        }
        if arguments.contains_key(alias.legacy) {
            // Legacy is already present — drop canonical silently so
            // callers that sent both don't get a duplicate key error.
            arguments.remove(alias.canonical);
            continue;
        }
        if let Some(value) = arguments.remove(alias.canonical) {
            arguments.insert(alias.legacy.to_string(), value);
            substitutions.push((alias.canonical.to_string(), alias.legacy.to_string()));
        }
    }
    substitutions
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map_from(pairs: &[(&str, Value)]) -> Map<String, Value> {
        let mut m = Map::new();
        for (k, v) in pairs {
            m.insert((*k).to_string(), v.clone());
        }
        m
    }

    #[test]
    fn canonical_name_is_renamed_to_legacy() {
        // Client sends the preferred name; we rewrite it for the struct.
        let mut args = map_from(&[("file_path", json!("src/main.rs"))]);
        let subs = apply_aliases("read_file", &mut args);
        assert_eq!(subs, vec![("file_path".to_string(), "path".to_string())]);
        assert_eq!(args.get("path"), Some(&json!("src/main.rs")));
        assert!(
            args.get("file_path").is_none(),
            "canonical key removed after rename"
        );
    }

    #[test]
    fn legacy_name_unchanged() {
        // Client still uses the legacy name — we do not touch it.
        let mut args = map_from(&[("path", json!("src/main.rs"))]);
        let subs = apply_aliases("read_file", &mut args);
        assert!(
            subs.is_empty(),
            "no substitution when legacy was used as-is"
        );
        assert_eq!(args.get("path"), Some(&json!("src/main.rs")));
    }

    #[test]
    fn legacy_wins_when_both_present() {
        // Caller shouldn't send both, but if they do we keep legacy.
        let mut args = map_from(&[
            ("path", json!("legacy.rs")),
            ("file_path", json!("canonical.rs")),
        ]);
        let subs = apply_aliases("read_file", &mut args);
        assert!(subs.is_empty(), "no substitution when legacy was provided");
        assert_eq!(args.get("path"), Some(&json!("legacy.rs")));
        assert!(
            args.get("file_path").is_none(),
            "canonical key dropped to keep request unambiguous"
        );
    }

    #[test]
    fn tool_specific_aliases_do_not_leak() {
        let mut args = map_from(&[("file_path", json!("anything"))]);
        let subs = apply_aliases("trace_path", &mut args);
        assert!(subs.is_empty(), "trace_path has no file_path alias");
        assert_eq!(args.get("file_path"), Some(&json!("anything")));
    }

    #[test]
    fn trace_path_handles_both_source_and_target() {
        let mut args = map_from(&[("from_symbol", json!("main")), ("to_symbol", json!("leaf"))]);
        let subs = apply_aliases("trace_path", &mut args);
        assert_eq!(
            subs,
            vec![
                ("from_symbol".to_string(), "source".to_string()),
                ("to_symbol".to_string(), "target".to_string()),
            ]
        );
        assert_eq!(args.get("source"), Some(&json!("main")));
        assert_eq!(args.get("target"), Some(&json!("leaf")));
    }

    #[test]
    fn ask_about_code_query_to_question() {
        let mut args = map_from(&[("query", json!("how does X work?"))]);
        let subs = apply_aliases("ask_about_code", &mut args);
        assert_eq!(subs, vec![("query".to_string(), "question".to_string())]);
        assert_eq!(args.get("question"), Some(&json!("how does X work?")));
    }

    #[test]
    fn unknown_tool_no_op() {
        let mut args = map_from(&[("file_path", json!("anything"))]);
        let subs = apply_aliases("not_a_real_tool", &mut args);
        assert!(subs.is_empty());
        assert_eq!(args.get("file_path"), Some(&json!("anything")));
    }

    /// Sanity check: every alias string pair is well-formed.
    #[test]
    fn every_alias_has_a_corresponding_canonical_and_legacy_field() {
        for alias in ALIASES {
            assert!(!alias.tool.is_empty(), "alias tool name must be non-empty");
            assert!(
                !alias.canonical.is_empty(),
                "alias canonical must be non-empty: tool={}",
                alias.tool
            );
            assert!(
                !alias.legacy.is_empty(),
                "alias legacy must be non-empty: tool={}",
                alias.tool
            );
            assert_ne!(
                alias.canonical, alias.legacy,
                "alias must rename to a different name: tool={}",
                alias.tool
            );
        }
    }

    /// Integration check: after `apply_aliases`, the resulting JSON
    /// object deserializes into the legacy `ReadFileInput` that the
    /// handler expects. This is the test that proves the alias table
    /// is wired to the actual struct, not just to a string.
    #[test]
    fn alias_renamed_map_deserializes_into_read_file_input() {
        use crate::interface::mcp::schemas::ReadFileInput;

        let mut args = Map::new();
        args.insert(
            "file_path".to_string(),
            Value::String("src/main.rs".to_string()),
        );
        let subs = apply_aliases("read_file", &mut args);
        assert_eq!(subs, vec![("file_path".to_string(), "path".to_string())]);

        let value = Value::Object(args);
        let input: ReadFileInput = serde_json::from_value(value)
            .expect("after alias rename, the Map must deserialize into ReadFileInput");
        assert_eq!(input.path, "src/main.rs");
    }
}
