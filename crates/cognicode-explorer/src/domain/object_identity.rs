//! MVP object identity parsing.
//!
//! Recognised shapes (Phase 3):
//! - `symbol:{file}:{name}:{line}` — a single resolved symbol.
//! - `file:{path}` — a source file and the symbols it contains.
//! - `scope:{path}` — a directory prefix that groups files into a module
//!   candidate (Phase 2 derives the candidate, it is never persisted).
//! - `issue:{id}` — a single quality issue, by primary key.
//! - `rule:{rule_id}` — a quality rule; `rule_id` may contain `:` so
//!   the rest of the string after the first `:` is the literal id.
//!
//! Unknown prefixes are rejected so the surface stays explicit and new types
//! are added via a deliberate enum extension, not silent string matching.

use crate::error::{ExplorerError, ExplorerResult};
use cognicode_core::domain::aggregates::SymbolId;

/// One of the inspectable MVP object types: a single symbol, a file, a
/// scope (module candidate), a quality issue, or a quality rule. The
/// enum is the single source of truth for the MVP id grammar — see
/// [`ObjectIdentity::parse_mvp_id`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObjectIdentity {
    /// `symbol:{file}:{name}:{line}` — a single resolved symbol.
    Symbol {
        file: String,
        name: String,
        line: u32,
    },
    /// `file:{path}` — a source file and the symbols it contains.
    File { path: String },
    /// `scope:{path}` — a directory prefix; the candidate module the prefix
    /// represents is derived, never persisted.
    Scope { path: String },
    /// `issue:{id}` — a single quality issue (primary key from the
    /// `issues` table).
    QualityIssue { id: i64 },
    /// `rule:{rule_id}` — a quality rule. The `rule_id` may itself
    /// contain `:` (e.g. `rust:S100`) — the parser captures the
    /// remainder of the string after the first `:`.
    Rule { rule_id: String },
}

impl ObjectIdentity {
    /// Build a symbol identity from already-validated parts.
    pub fn new_symbol(file: impl Into<String>, name: impl Into<String>, line: u32) -> Self {
        Self::Symbol {
            file: file.into(),
            name: name.into(),
            line,
        }
    }

    /// Build a file identity from a workspace-relative path.
    pub fn new_file(path: impl Into<String>) -> Self {
        Self::File { path: path.into() }
    }

    /// Build a scope identity from a directory prefix.
    pub fn new_scope(path: impl Into<String>) -> Self {
        Self::Scope { path: path.into() }
    }

    /// Build a quality-issue identity from a primary key.
    pub fn new_quality_issue(id: i64) -> Self {
        Self::QualityIssue { id }
    }

    /// Build a quality-rule identity from a rule id.
    pub fn new_rule(rule_id: impl Into<String>) -> Self {
        Self::Rule {
            rule_id: rule_id.into(),
        }
    }

    /// Parse an MVP id. The accepted shapes are:
    /// - `symbol:{file}:{name}:{line}` (line > 0, file + name non-empty)
    /// - `file:{path}` (path non-empty)
    /// - `scope:{path}` (path non-empty)
    /// - `issue:{id}` (id > 0)
    /// - `rule:{rule_id}` (rule_id non-empty; colons allowed)
    ///
    /// Any other shape yields [`ExplorerError::ResolutionFailed`]. The path
    /// component of `file:` and `scope:` is everything after the first `:`
    /// re-joined, so paths that happen to contain colons are preserved
    /// verbatim (workspace-relative paths in this codebase do not contain
    /// colons, but the parser is robust to that).
    pub fn parse_mvp_id(raw: &str) -> ExplorerResult<Self> {
        let raw = raw.trim();
        let (prefix, rest) = raw
            .split_once(':')
            .ok_or_else(|| ExplorerError::ResolutionFailed(raw.to_string()))?;

        match prefix {
            "symbol" => Self::parse_symbol(rest, raw),
            "file" => Self::parse_file(rest, raw),
            "scope" => Self::parse_scope(rest, raw),
            "issue" => Self::parse_issue(rest, raw),
            "rule" => Self::parse_rule(rest, raw),
            _ => Err(ExplorerError::ResolutionFailed(raw.to_string())),
        }
    }

    fn parse_symbol(rest: &str, raw: &str) -> ExplorerResult<Self> {
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() != 3 {
            return Err(ExplorerError::ResolutionFailed(raw.to_string()));
        }
        let file = parts[0];
        let name = parts[1];
        let line_str = parts[2];
        if file.is_empty() || name.is_empty() || line_str.is_empty() {
            return Err(ExplorerError::ResolutionFailed(raw.to_string()));
        }
        let line: u32 = line_str
            .parse()
            .map_err(|_| ExplorerError::ResolutionFailed(raw.to_string()))?;
        if line == 0 {
            return Err(ExplorerError::ResolutionFailed(raw.to_string()));
        }
        Ok(Self::Symbol {
            file: file.to_string(),
            name: name.to_string(),
            line,
        })
    }

    fn parse_file(rest: &str, raw: &str) -> ExplorerResult<Self> {
        if rest.is_empty() {
            return Err(ExplorerError::ResolutionFailed(raw.to_string()));
        }
        Ok(Self::File { path: rest.to_string() })
    }

    fn parse_scope(rest: &str, raw: &str) -> ExplorerResult<Self> {
        if rest.is_empty() {
            return Err(ExplorerError::ResolutionFailed(raw.to_string()));
        }
        Ok(Self::Scope { path: rest.to_string() })
    }

    fn parse_issue(rest: &str, raw: &str) -> ExplorerResult<Self> {
        if rest.is_empty() {
            return Err(ExplorerError::ResolutionFailed(raw.to_string()));
        }
        // Reject any extra colons — `issue:42:extra` is ambiguous and
        // should not silently truncate.
        if rest.contains(':') {
            return Err(ExplorerError::ResolutionFailed(raw.to_string()));
        }
        let id: i64 = rest
            .parse()
            .map_err(|_| ExplorerError::ResolutionFailed(raw.to_string()))?;
        if id <= 0 {
            return Err(ExplorerError::ResolutionFailed(raw.to_string()));
        }
        Ok(Self::QualityIssue { id })
    }

    fn parse_rule(rest: &str, raw: &str) -> ExplorerResult<Self> {
        if rest.is_empty() {
            return Err(ExplorerError::ResolutionFailed(raw.to_string()));
        }
        // `rule_id` may contain colons (e.g. `rust:S100`) so the rest
        // is captured verbatim — no further splitting is attempted.
        Ok(Self::Rule {
            rule_id: rest.to_string(),
        })
    }

    /// The lowercase tag used in MVP ids and on the wire:
    /// `"symbol"`, `"file"`, `"scope"`, `"issue"`, or `"rule"`.
    pub fn object_type_str(&self) -> &'static str {
        match self {
            Self::Symbol { .. } => "symbol",
            Self::File { .. } => "file",
            Self::Scope { .. } => "scope",
            Self::QualityIssue { .. } => "issue",
            Self::Rule { .. } => "rule",
        }
    }

    /// The string written into [`crate::dto::ObjectIdentityEntry::object_type`]
    /// for persistence. Phase 2 keeps it equal to [`Self::object_type_str`]
    /// but the method exists so future changes (e.g. promoting "scope" to
    /// "module") can move independently of the wire tag.
    pub fn object_type(&self) -> String {
        self.object_type_str().to_string()
    }

    /// The path component of a file or scope identity. Returns `None` for
    /// symbol, issue, and rule identities.
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::File { path } | Self::Scope { path } => Some(path.as_str()),
            Self::Symbol { .. } | Self::QualityIssue { .. } | Self::Rule { .. } => None,
        }
    }

    /// `true` for the file variant.
    pub fn is_file(&self) -> bool {
        matches!(self, Self::File { .. })
    }

    /// `true` for the scope variant.
    pub fn is_scope(&self) -> bool {
        matches!(self, Self::Scope { .. })
    }

    /// `true` for the symbol variant.
    pub fn is_symbol(&self) -> bool {
        matches!(self, Self::Symbol { .. })
    }

    /// `true` for the quality-issue variant.
    pub fn is_quality_issue(&self) -> bool {
        matches!(self, Self::QualityIssue { .. })
    }

    /// `true` for the rule variant.
    pub fn is_rule(&self) -> bool {
        matches!(self, Self::Rule { .. })
    }

    /// Render the canonical MVP id (`symbol:...` / `file:...` /
    /// `scope:...` / `issue:...` / `rule:...`).
    pub fn to_mvp_id(&self) -> String {
        match self {
            Self::Symbol { file, name, line } => format!("symbol:{file}:{name}:{line}"),
            Self::File { path } => format!("file:{path}"),
            Self::Scope { path } => format!("scope:{path}"),
            Self::QualityIssue { id } => format!("issue:{id}"),
            Self::Rule { rule_id } => format!("rule:{rule_id}"),
        }
    }

    /// Render the canonical `SymbolId` used by the call graph. Only
    /// meaningful for the symbol variant; `None` for every other variant.
    pub fn to_symbol_id(&self) -> Option<SymbolId> {
        match self {
            Self::Symbol { file, name, line } => {
                Some(SymbolId::new(format!("{file}:{name}:{line}")))
            }
            _ => None,
        }
    }

    /// The natural key for this identity — the form stored in
    /// [`crate::dto::ObjectIdentityEntry::natural_key`].
    ///
    /// - Symbol → `file:name:line` (matches `SymbolId::as_str`).
    /// - File / Scope → the path.
    /// - Issue → the string form of the primary key.
    /// - Rule → the rule id verbatim.
    pub fn natural_key(&self) -> String {
        match self {
            Self::Symbol { file, name, line } => format!("{file}:{name}:{line}"),
            Self::File { path } | Self::Scope { path } => path.clone(),
            Self::QualityIssue { id } => id.to_string(),
            Self::Rule { rule_id } => rule_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_symbol_mvp_id() {
        let id = ObjectIdentity::parse_mvp_id("symbol:src/foo.rs:bar:42").unwrap();
        assert_eq!(
            id,
            ObjectIdentity::Symbol {
                file: "src/foo.rs".into(),
                name: "bar".into(),
                line: 42
            }
        );
        assert_eq!(id.object_type_str(), "symbol");
        assert_eq!(id.to_symbol_id().unwrap().as_str(), "src/foo.rs:bar:42");
        assert_eq!(id.to_mvp_id(), "symbol:src/foo.rs:bar:42");
        assert_eq!(id.natural_key(), "src/foo.rs:bar:42");
    }

    #[test]
    fn parses_valid_file_mvp_id() {
        let id = ObjectIdentity::parse_mvp_id("file:src/main.rs").unwrap();
        assert_eq!(id, ObjectIdentity::File { path: "src/main.rs".into() });
        assert_eq!(id.object_type_str(), "file");
        assert!(id.is_file());
        assert_eq!(id.path(), Some("src/main.rs"));
        assert_eq!(id.to_mvp_id(), "file:src/main.rs");
        assert_eq!(id.natural_key(), "src/main.rs");
        assert!(id.to_symbol_id().is_none());
    }

    #[test]
    fn parses_valid_scope_mvp_id() {
        let id = ObjectIdentity::parse_mvp_id("scope:src/foo").unwrap();
        assert_eq!(id, ObjectIdentity::Scope { path: "src/foo".into() });
        assert_eq!(id.object_type_str(), "scope");
        assert!(id.is_scope());
        assert_eq!(id.path(), Some("src/foo"));
        assert_eq!(id.to_mvp_id(), "scope:src/foo");
        assert_eq!(id.natural_key(), "src/foo");
        assert!(id.to_symbol_id().is_none());
    }

    #[test]
    fn parses_valid_issue_mvp_id() {
        let id = ObjectIdentity::parse_mvp_id("issue:42").unwrap();
        assert_eq!(id, ObjectIdentity::QualityIssue { id: 42 });
        assert_eq!(id.object_type_str(), "issue");
        assert!(id.is_quality_issue());
        assert!(id.path().is_none());
        assert_eq!(id.to_mvp_id(), "issue:42");
        assert_eq!(id.natural_key(), "42");
        assert!(id.to_symbol_id().is_none());
    }

    #[test]
    fn parses_valid_rule_mvp_id_with_colons() {
        let id = ObjectIdentity::parse_mvp_id("rule:rust:S100").unwrap();
        assert_eq!(
            id,
            ObjectIdentity::Rule {
                rule_id: "rust:S100".into()
            }
        );
        assert_eq!(id.object_type_str(), "rule");
        assert!(id.is_rule());
        assert_eq!(id.to_mvp_id(), "rule:rust:S100");
        assert_eq!(id.natural_key(), "rust:S100");
    }

    #[test]
    fn parses_rule_mvp_id_without_colon() {
        let id = ObjectIdentity::parse_mvp_id("rule:no-colon-rule").unwrap();
        assert_eq!(
            id,
            ObjectIdentity::Rule {
                rule_id: "no-colon-rule".into()
            }
        );
    }

    #[test]
    fn rejects_missing_prefix() {
        let err = ObjectIdentity::parse_mvp_id("src/foo.rs:bar:42").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_unknown_prefix() {
        let err = ObjectIdentity::parse_mvp_id("module:src/foo").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_symbol_with_too_few_segments() {
        let err = ObjectIdentity::parse_mvp_id("symbol:src/foo.rs:bar").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_symbol_with_too_many_segments() {
        let err = ObjectIdentity::parse_mvp_id("symbol:src/foo.rs:bar:42:extra").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_symbol_with_empty_file_segment() {
        let err = ObjectIdentity::parse_mvp_id("symbol::bar:42").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_symbol_with_empty_name_segment() {
        let err = ObjectIdentity::parse_mvp_id("symbol:src/foo.rs::42").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_symbol_with_empty_line_segment() {
        let err = ObjectIdentity::parse_mvp_id("symbol:src/foo.rs:bar:").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_symbol_with_non_numeric_line() {
        let err = ObjectIdentity::parse_mvp_id("symbol:src/foo.rs:bar:xx").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_symbol_with_zero_line() {
        let err = ObjectIdentity::parse_mvp_id("symbol:src/foo.rs:bar:0").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_file_with_empty_path() {
        let err = ObjectIdentity::parse_mvp_id("file:").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_file_with_no_colon() {
        let err = ObjectIdentity::parse_mvp_id("file").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_scope_with_empty_path() {
        let err = ObjectIdentity::parse_mvp_id("scope:").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_scope_with_no_colon() {
        let err = ObjectIdentity::parse_mvp_id("scope").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_issue_with_non_numeric_id() {
        let err = ObjectIdentity::parse_mvp_id("issue:abc").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_issue_with_zero_id() {
        let err = ObjectIdentity::parse_mvp_id("issue:0").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_issue_with_negative_id() {
        let err = ObjectIdentity::parse_mvp_id("issue:-1").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_issue_with_empty_id() {
        let err = ObjectIdentity::parse_mvp_id("issue:").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_issue_with_extra_segments() {
        // `issue:42:extra` is ambiguous — the rule id parser accepts
        // colons in the rest, but issue ids are strictly integers.
        let err = ObjectIdentity::parse_mvp_id("issue:42:extra").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn rejects_rule_with_empty_id() {
        let err = ObjectIdentity::parse_mvp_id("rule:").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn new_constructors_emit_canonical_forms() {
        let sym = ObjectIdentity::new_symbol("src/a.rs", "alpha", 7);
        assert_eq!(sym.to_mvp_id(), "symbol:src/a.rs:alpha:7");
        let file = ObjectIdentity::new_file("src/a.rs");
        assert_eq!(file.to_mvp_id(), "file:src/a.rs");
        let scope = ObjectIdentity::new_scope("src");
        assert_eq!(scope.to_mvp_id(), "scope:src");
        let issue = ObjectIdentity::new_quality_issue(99);
        assert_eq!(issue.to_mvp_id(), "issue:99");
        let rule = ObjectIdentity::new_rule("rust:S100");
        assert_eq!(rule.to_mvp_id(), "rule:rust:S100");
    }
}
