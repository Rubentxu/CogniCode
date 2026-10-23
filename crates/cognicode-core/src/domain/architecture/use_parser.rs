//! Minimal dependency parser for the architecture evaluator.
//!
//! The evaluator must read the source of `cognicode-core` itself to
//! detect layer violations. We deliberately do **not** depend on `syn`
//! in this crate — `syn` lives in `cognicode-macros` and adding it
//! here would drag a heavy compile-time dependency into the domain
//! layer.
//!
//! Instead, this is a hand-rolled, comment-aware, string-aware parser
//! for the two ways a source file can name a dependency:
//!
//! 1. A `use ...;` statement.
//! 2. A path written **inline** in an expression, e.g.
//!    `crate::infrastructure::graph::Thing::new()`.
//!
//! It is sufficient for the three rule families of e77:
//!
//! 1. `LayerDependencyRule` — matches by layer prefix (e.g. any
//!    dependency on `infrastructure` from inside `domain`).
//! 2. `ForbiddenDependencyRule` — matches by exact forbidden path
//!    (e.g. `sqlx::...`).
//! 3. `NamespaceBoundaryRule` — matches by namespace prefix (e.g. a
//!    dependency on `presentation` from inside `evidence_kernel`).
//!
//! All three reduce to: *given a list of named dependencies and a set
//! of forbidden prefixes, which ones match?* The parser only has to
//! produce that list reliably. It does not handle generics,
//! attributes, macro-generated code, or anything beyond the
//! `use foo::bar::{baz, qux as q};` shape.
//!
//! ## Why inline paths count
//!
//! A dependency that is written inline is exactly as binding as one
//! written in a `use` statement: the module still depends on the other
//! layer. A guard that only reads `use` statements therefore protects
//! the limit it declares only against half of the ways it can be
//! crossed — and the half it misses is the half that sneaks in. That
//! is why this parser reports both spellings.
//!
//! ## Why string literals are tracked
//!
//! A dependency named inside a string literal is not a dependency: it
//! is data — a fixture, a diagnostic message, a test oracle. Reading
//! those as code would report drifts that do not exist, so string
//! contents are blanked before the scan.

use std::fmt;

use serde::{Deserialize, Serialize};

// ============================================================================
// Types
// ============================================================================

/// A dependency named by a source file.
///
/// The dependency is recorded the way it was written, with the leading
/// `crate::` / `self::` / `super::` stripped. E.g. both
/// `use crate::infrastructure::db;` and
/// `crate::infrastructure::db::Pool::new()` are recorded as
/// `infrastructure::db`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyPath {
    /// The path as written (after stripping leading `crate::` /
    /// `self::` / `super::`). E.g. for `use crate::infrastructure::db;`
    /// the path is `infrastructure::db`.
    pub path: String,
    /// The file the dependency lives in, as a relative path (e.g.
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
pub enum DependencyPathError {
    /// The input was empty.
    EmptyInput,
}

impl fmt::Display for DependencyPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyInput => "empty source",
        })
    }
}

impl std::error::Error for DependencyPathError {}

/// The former name of [`DependencyPath`], kept so existing consumers
/// keep compiling. The parser reports inline paths too, so the old
/// name no longer describes what it carries.
#[deprecated(
    since = "0.101.31",
    note = "el parser ya no solo lee sentencias `use`: usa DependencyPath"
)]
pub type UseStatement = DependencyPath;

/// The former name of [`DependencyPathError`].
#[deprecated(since = "0.101.31", note = "renombrado a DependencyPathError")]
pub type UseStatementError = DependencyPathError;

// ============================================================================
// Parser
// ============================================================================

/// Parse `source` into the list of dependencies it names, attributing
/// each one to `file_path`.
///
/// The parser is line-oriented: it walks the source line by line,
/// skipping line comments (`// ...`) and tracking block comments
/// (`/* ... */`) so a dependency inside a comment is not parsed as
/// real code. String literals are blanked for the same reason: a
/// dependency named inside a string is data, not a dependency.
///
/// Both spellings are reported: a `use` statement, and a path written
/// inline in an expression.
pub fn parse_dependency_paths(
    source: &str,
    file_path: impl Into<String>,
) -> Result<Vec<DependencyPath>, DependencyPathError> {
    if source.is_empty() {
        return Err(DependencyPathError::EmptyInput);
    }
    let file_path = file_path.into();
    let mut out = Vec::new();
    let mut in_block_comment = false;
    let mut in_string = false;
    for (idx, raw_line) in source.lines().enumerate() {
        let line_no = (idx + 1) as u32;
        // Strings first: a `//` or `/*` inside a string literal is
        // data, not a comment. Stripping comments first would cut the
        // line at that `//`, leave the string unterminated, and
        // desynchronise the scanner for the rest of the file.
        let line = blank_string_literals(raw_line, &mut in_string);
        let line = strip_comments(&line, &mut in_block_comment);
        if line.trim().is_empty() {
            continue;
        }
        if let Some(path) = parse_use_line(line) {
            out.push(DependencyPath {
                path,
                file_path: file_path.clone(),
                line: line_no,
                module_path: None,
            });
            // A `use` statement names its dependency once. The inline
            // scan below would otherwise count the same dependency a
            // second time.
            continue;
        }
        for path in parse_inline_paths(line) {
            out.push(DependencyPath {
                path,
                file_path: file_path.clone(),
                line: line_no,
                module_path: None,
            });
        }
    }
    Ok(out)
}

/// The former name of [`parse_dependency_paths`].
///
/// It used to report only `use` statements; it now reports inline
/// paths too, which is why the name is deprecated.
#[allow(deprecated)]
#[deprecated(
    since = "0.101.31",
    note = "el parser ya no solo lee sentencias `use`: usa parse_dependency_paths"
)]
pub fn parse_use_lines(
    source: &str,
    file_path: impl Into<String>,
) -> Result<Vec<UseStatement>, UseStatementError> {
    parse_dependency_paths(source, file_path)
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

/// Blank the contents of string literals, keeping the quotes.
///
/// A dependency named inside a string is data — a fixture, a message,
/// an oracle — not a dependency. Blanking the contents keeps the
/// scanner from reporting drifts that do not exist, and keeps the
/// line's column positions intact.
///
/// The state is carried across lines because a string literal may
/// span several of them.
fn blank_string_literals(line: &str, in_string: &mut bool) -> String {
    let mut out = String::with_capacity(line.len());
    let mut escaped = false;
    for ch in line.chars() {
        if *in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => {
                    *in_string = false;
                    out.push(ch);
                }
                _ => {}
            }
            continue;
        }
        if ch == '"' {
            *in_string = true;
            out.push(ch);
            continue;
        }
        out.push(ch);
    }
    out
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

/// Parse a single line (already stripped of comments and string
/// contents). Returns the normalised path if the line is a `use`
/// statement, else `None`.
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

/// Parse the dependencies written **inline** in a line.
///
/// Only `crate::` paths are reported: `self::` and `super::` stay
/// inside the module that writes them, so they can never cross a
/// layer boundary. A `crate::` path can.
fn parse_inline_paths(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (idx, _) in line.match_indices("crate::") {
        // A `crate::` that is part of a longer identifier is not a
        // path (`mycrate::x`).
        if idx > 0
            && line[..idx]
                .chars()
                .next_back()
                .is_some_and(|c| c == '_' || c.is_alphanumeric())
        {
            continue;
        }
        let rest = &line[idx + "crate::".len()..];
        let path = take_path(rest);
        if path.is_empty() {
            continue;
        }
        out.push(path);
    }
    out
}

/// Take the maximal `segment::segment::…` path at the start of `text`.
///
/// The scan works on characters, not bytes: a source file may hold
/// accented identifiers, and a path boundary has to land on a
/// character boundary.
fn take_path(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    loop {
        let mut segment = String::new();
        while i < chars.len() && (chars[i] == '_' || chars[i].is_alphanumeric()) {
            segment.push(chars[i]);
            i += 1;
        }
        if segment.is_empty() {
            break;
        }
        if i + 1 < chars.len() && chars[i] == ':' && chars[i + 1] == ':' {
            out.push_str(&segment);
            out.push_str("::");
            i += 2;
            continue;
        }
        out.push_str(&segment);
        break;
    }
    out.trim_end_matches(':').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_use() {
        let src = "use crate::domain::foo::Bar;\n";
        let out = parse_dependency_paths(src, "src/x.rs").unwrap();
        assert_eq!(
            out,
            vec![DependencyPath {
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
        let out = parse_dependency_paths(src, "src/x.rs").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "infrastructure");
    }

    #[test]
    fn ignores_line_comments() {
        let src = "// use foo::bar;\nuse baz::qux;\n";
        let out = parse_dependency_paths(src, "src/x.rs").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "baz::qux");
    }

    #[test]
    fn ignores_block_comments() {
        let src = "/* use foo::bar; */\nuse baz::qux;\n";
        let out = parse_dependency_paths(src, "src/x.rs").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "baz::qux");
    }

    #[test]
    fn strips_crate_prefix() {
        let src = "use crate::domain::foo;\nuse self::bar::Baz;\nuse super::Qux;\n";
        let out = parse_dependency_paths(src, "src/x.rs").unwrap();
        assert_eq!(out[0].path, "domain::foo");
        assert_eq!(out[1].path, "bar::Baz");
        assert_eq!(out[2].path, "Qux");
    }

    #[test]
    fn empty_input_errors() {
        assert!(parse_dependency_paths("", "src/x.rs").is_err());
    }

    #[test]
    fn no_dependencies_yields_empty_vec() {
        let out = parse_dependency_paths("fn main() {}\n", "src/x.rs").unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn reads_a_dependency_written_inline() {
        let src = "    let x = crate::infrastructure::graph::Thing::new();\n";
        let out = parse_dependency_paths(src, "src/x.rs").unwrap();
        assert_eq!(out.len(), 1, "the inline dependency must be reported");
        assert_eq!(out[0].path, "infrastructure::graph::Thing::new");
        assert_eq!(out[0].line, 1);
    }

    #[test]
    fn reads_several_inline_dependencies_in_one_line() {
        let src = "let a = crate::infrastructure::one::A::new(); let b = crate::infrastructure::two::B::new();\n";
        let out = parse_dependency_paths(src, "src/x.rs").unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].path, "infrastructure::one::A::new");
        assert_eq!(out[1].path, "infrastructure::two::B::new");
    }

    #[test]
    fn does_not_report_a_use_statement_twice() {
        let src = "use crate::infrastructure::db::Pool;\n";
        let out = parse_dependency_paths(src, "src/x.rs").unwrap();
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn ignores_dependencies_named_inside_a_string() {
        let src = "let fixture = \"use crate::infrastructure::db::Pool;\";\n";
        let out = parse_dependency_paths(src, "src/x.rs").unwrap();
        assert!(out.is_empty(), "a string is data, not a dependency");
    }

    #[test]
    fn ignores_dependencies_named_inside_a_comment() {
        let src = "// crate::infrastructure::db::Pool::new()\n";
        let out = parse_dependency_paths(src, "src/x.rs").unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn a_comment_marker_inside_a_string_does_not_desynchronise_the_scan() {
        // The string holds a `//`: it is data, not a comment. If the
        // scanner cut the line there, the string would stay open and
        // every later string would be read as code.
        let src = "let src = \"// use foo::bar;\";\nuse crate::infrastructure::db;\n";
        let out = parse_dependency_paths(src, "src/x.rs").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "infrastructure::db");
    }

    #[test]
    fn self_and_super_paths_stay_inside_their_module() {
        let src = "let a = self::helper::x();\nlet b = super::other::y();\n";
        let out = parse_dependency_paths(src, "src/x.rs").unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn a_longer_identifier_is_not_a_path() {
        let src = "let a = mycrate::infrastructure::x();\n";
        let out = parse_dependency_paths(src, "src/x.rs").unwrap();
        assert!(out.is_empty());
    }
}
