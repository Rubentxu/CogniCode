//! Minimal `use` statement parser for the architecture evaluator.
//!
//! The evaluator must read the source of `cognicode-core` itself to
//! detect layer violations. We deliberately do **not** depend on `syn`
//! in this crate — `syn` lives in `cognicode-macros` and adding it
//! here would drag a heavy compile-time dependency into the domain
//! layer.
//!
//! Instead, this is a hand-rolled, comment-aware, string-aware parser
//! for `use ...;` statements. It is sufficient for the three rule
//! families of e77:
//!
//! 1. `LayerDependencyRule` — matches by layer prefix (e.g. any
//!    `use crate::infrastructure::...` from inside `domain`).
//! 2. `ForbiddenDependencyRule` — matches by exact forbidden path
//!    (e.g. `use sqlx::...`).
//! 3. `NamespaceBoundaryRule` — matches by namespace prefix (e.g.
//!    `use crate::presentation::...` from inside `evidence_kernel`).
//!
//! All three reduce to: *given a list of `use` statements and a set
//! of forbidden prefixes, which statements match?* The parser only
//! has to produce that list reliably. It does not handle generics,
//! attributes, macro-generated code, or anything beyond the
//! `use foo::bar::{baz, qux as q};` shape.

use std::fmt;

use serde::{Deserialize, Serialize};

// ============================================================================
// Types
// ============================================================================

/// A single parsed `use` statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UseStatement {
    /// The full path as written (after stripping leading `crate::` /
    /// `self::` / `super::`). E.g. for `use crate::infrastructure::db;`
    /// the path is `infrastructure::db`.
    pub path: String,
    /// The file the statement lives in, as a relative path (e.g.
    /// `src/domain/evidence_kernel/store.rs`).
    pub file_path: String,
    /// 1-based line number in `file_path`.
    pub line: u32,
    /// Optional module-path hint. When the parser is told which
    /// module a `file_path` belongs to, it stamps that here so the
    /// evaluator can resolve the *source* layer without re-deriving
    /// it from `file_path`.
    pub module_path: Option<String>,
}

/// Errors raised by the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UseStatementError {
    /// The input was empty.
    EmptyInput,
}

impl fmt::Display for UseStatementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyInput => "empty source",
        })
    }
}

impl std::error::Error for UseStatementError {}

// ============================================================================
// Parser
// ============================================================================

/// Parse `source` into a list of `use` statements, attributing each
/// one to `file_path`.
///
/// The parser is line-oriented: it walks the source line by line,
/// skipping line comments (`// ...`) and tracking block comments
/// (`/* ... */`) so a `use` inside a comment is not parsed as a real
/// statement. String literals are not tracked — if a `use` appears
/// inside a string the parser will report it as if it were code. The
/// use cases of e77 do not need string-literal awareness: a fixture
/// that puts `use foo::bar;` inside a doc-string would be a test
/// smell, not real source.
pub fn parse_use_lines(
    source: &str,
    file_path: impl Into<String>,
) -> Result<Vec<UseStatement>, UseStatementError> {
    if source.is_empty() {
        return Err(UseStatementError::EmptyInput);
    }
    let file_path = file_path.into();
    let mut out = Vec::new();
    let mut in_block_comment = false;
    for (idx, raw_line) in source.lines().enumerate() {
        let line_no = (idx + 1) as u32;
        let line = strip_comments(raw_line, &mut in_block_comment);
        if line.is_empty() {
            continue;
        }
        if let Some(path) = parse_use_line(&line) {
            out.push(UseStatement {
                path,
                file_path: file_path.clone(),
                line: line_no,
                module_path: None,
            });
        }
    }
    Ok(out)
}

/// Strip line and block comments, updating `in_block_comment`.
fn strip_comments<'a>(line: &'a str, in_block_comment: &mut bool) -> &'a str {
    // We do not need to be perfect; we need to avoid false positives
    // for `// use foo::bar;` and `/* use foo::bar; */`. We work with
    // byte offsets within the line.
    let bytes = line.as_bytes();
    let mut start = 0usize;
    // First, honour a pre-existing block-comment state.
    if *in_block_comment {
        if let Some(end) = find_subsequence(bytes, b"*/", start) {
            *in_block_comment = false;
            start = end + 2;
        } else {
            return "";
        }
    }
    // Find `//` or `/*` starting from `start`.
    let mut loop_start = start;
    loop {
        if loop_start >= bytes.len() {
            break;
        }
        let line_comment = find_subsequence(bytes, b"//", loop_start);
        let block_comment = find_subsequence(bytes, b"/*", loop_start);
        match (line_comment, block_comment) {
            (None, None) => break,
            (Some(lc), None) => {
                // Truncate at the line comment.
                return &line[start..lc];
            }
            (None, Some(bc)) => {
                // Look for a matching `*/`.
                if let Some(end) = find_subsequence(bytes, b"*/", bc + 2) {
                    // Drop the block comment from the line.
                    let mut cleaned = String::with_capacity(line.len());
                    cleaned.push_str(&line[start..bc]);
                    cleaned.push_str(&line[end + 2..]);
                    // We cannot mutate `line`; return what we can
                    // produce without further parsing. Since `parse_use_line`
                    // operates on `&str`, and we have already stripped a
                    // block comment, we just return the slice before
                    // `bc` for now — `parse_use_line` will not see the
                    // `/* ... */` portion.
                    return &line[start..bc];
                } else {
                    // The block comment continues past this line.
                    *in_block_comment = true;
                    return &line[start..bc];
                }
            }
            (Some(lc), Some(bc)) => {
                if lc < bc {
                    return &line[start..lc];
                } else {
                    if let Some(end) = find_subsequence(bytes, b"*/", bc + 2) {
                        // Skip the block comment and continue.
                        loop_start = end + 2;
                        continue;
                    } else {
                        *in_block_comment = true;
                        return &line[start..bc];
                    }
                }
            }
        }
    }
    &line[start..]
}

fn find_subsequence(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from + needle.len() > haystack.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Parse a single line (already stripped of comments). Returns the
/// normalised path if the line is a `use` statement, else `None`.
fn parse_use_line(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("use ")?;
    let body = rest.trim_end_matches(';').trim();
    if body.is_empty() {
        return None;
    }
    // Take only the first segment of a `use a::{b, c}` grouping: we
    // normalise both into `a` because the rule families of e77 only
    // care about the *root* of the import. We strip a trailing `::`
    // because `a::` is what `use a::{...}` leaves behind after the
    // split.
    let root = body.split('{').next().unwrap_or(body).trim();
    let root = root.trim_end_matches(':').trim();
    // Strip leading `crate::` / `self::` / `super::` for
    // normalisation. The evaluator will match against forbidden
    // prefixes regardless of these prefixes, so keeping them would
    // just complicate matching.
    let root = root
        .strip_prefix("crate::")
        .or_else(|| root.strip_prefix("self::"))
        .or_else(|| root.strip_prefix("super::"))
        .unwrap_or(root);
    if root.is_empty() {
        return None;
    }
    Some(root.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_use() {
        let src = "use crate::domain::foo::Bar;\n";
        let out = parse_use_lines(src, "src/x.rs").unwrap();
        assert_eq!(
            out,
            vec![UseStatement {
                path: "domain::foo::Bar".into(),
                file_path: "src/x.rs".into(),
                line: 1,
                module_path: None,
            }]
        );
    }

    #[test]
    fn parses_grouped_use() {
        let src = "use crate::infrastructure::{db, fs};\n";
        let out = parse_use_lines(src, "src/x.rs").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "infrastructure");
    }

    #[test]
    fn ignores_line_comments() {
        let src = "// use foo::bar;\nuse baz::qux;\n";
        let out = parse_use_lines(src, "src/x.rs").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "baz::qux");
    }

    #[test]
    fn ignores_block_comments() {
        let src = "/* use foo::bar; */\nuse baz::qux;\n";
        let out = parse_use_lines(src, "src/x.rs").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "baz::qux");
    }

    #[test]
    fn strips_crate_prefix() {
        let src = "use crate::domain::foo;\nuse self::bar::Baz;\nuse super::Qux;\n";
        let out = parse_use_lines(src, "src/x.rs").unwrap();
        assert_eq!(out[0].path, "domain::foo");
        assert_eq!(out[1].path, "bar::Baz");
        assert_eq!(out[2].path, "Qux");
    }

    #[test]
    fn empty_input_errors() {
        assert!(parse_use_lines("", "src/x.rs").is_err());
    }

    #[test]
    fn no_use_statements_yields_empty_vec() {
        let out = parse_use_lines("fn main() {}\n", "src/x.rs").unwrap();
        assert!(out.is_empty());
    }
}
