//! Hand-written recursive-descent parser for MoldQL.
//!
//! Zero new dependencies. The grammar is small (5 clauses, 1 precedence
//! level) so a parser-combinator crate would add more weight than value.
//!
//! ## Error model
//!
//! Errors carry a `(message, line, column)` triple. `Display` produces
//! `"<message> at line N, column M"`. The `Cursor` is byte-index-based;
//! line/column are recomputed on demand to keep the parser single-pass
//! and side-effect-free.

use std::fmt;

use crate::moldql::ast::{
    Condition, Direction, ExploreQuery, Field, FindQuery, MoldQLQuery, Op, TargetType, Value,
};

/// Diagnostic returned by [`parse`] when the input is malformed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub line: u32,
    pub column: u32,
}

impl ParseError {
    fn at(message: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            message: message.into(),
            line,
            column,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}, column {}",
            self.message, self.line, self.column
        )
    }
}

impl std::error::Error for ParseError {}

/// Parse a MoldQL query string into its AST.
///
/// # Errors
/// Returns [`ParseError`] for any malformed input. The error position
/// is 1-based for both line and column.
pub fn parse(input: &str) -> Result<MoldQLQuery, ParseError> {
    let mut cursor = Cursor::new(input);
    cursor.skip_ws();
    if cursor.is_eof() {
        return Err(ParseError::at(
            "empty query — expected FIND or EXPLORE",
            1,
            1,
        ));
    }
    let query = parse_query(&mut cursor)?;
    cursor.skip_ws();
    if !cursor.is_eof() {
        let (line, col) = cursor.position();
        return Err(ParseError::at(
            format!(
                "unexpected trailing input: `{}`",
                cursor.remaining()
            ),
            line,
            col,
        ));
    }
    Ok(query)
}

fn parse_query(cursor: &mut Cursor<'_>) -> Result<MoldQLQuery, ParseError> {
    let kw = cursor.peek_keyword().ok_or_else(|| {
        let (line, col) = cursor.position();
        ParseError::at("expected FIND or EXPLORE", line, col)
    })?;
    match kw.to_ascii_uppercase().as_str() {
        "FIND" => {
            cursor.consume_keyword("FIND");
            let find = parse_find_after_keyword(cursor)?;
            Ok(MoldQLQuery::Find(find))
        }
        "EXPLORE" => {
            cursor.consume_keyword("EXPLORE");
            let explore = parse_explore_after_keyword(cursor)?;
            Ok(MoldQLQuery::Explore(explore))
        }
        other => {
            let (line, col) = cursor.position();
            Err(ParseError::at(
                format!("expected FIND or EXPLORE, found `{other}`"),
                line,
                col,
            ))
        }
    }
}

// ----------------------------------------------------------------------------
// FIND
// ----------------------------------------------------------------------------

fn parse_find_after_keyword(cursor: &mut Cursor<'_>) -> Result<FindQuery, ParseError> {
    cursor.skip_ws();
    let target = parse_target(cursor)?;
    cursor.skip_ws();

    let mut scope: Option<String> = None;
    let mut conditions: Vec<Condition> = Vec::new();
    let mut apply_lens: Option<String> = None;

    // The order of optional clauses is: IN SCOPE, WHERE, APPLY.
    // The parser accepts them in any order — but the spec grammar lists
    // them in this order, so we follow it for predictability.
    loop {
        if cursor.is_eof() {
            break;
        }
        let next = cursor
            .peek_keyword()
            .ok_or_else(|| {
                let (line, col) = cursor.position();
                ParseError::at(
                    "unexpected token — expected IN, WHERE, APPLY, or end of query",
                    line,
                    col,
                )
            })?
            .to_ascii_uppercase();

        match next.as_str() {
            "IN" => {
                if scope.is_some() {
                    let (line, col) = cursor.position();
                    return Err(ParseError::at(
                        "duplicate IN SCOPE clause",
                        line,
                        col,
                    ));
                }
                scope = Some(parse_scope_clause(cursor)?);
            }
            "WHERE" => {
                cursor.consume_keyword("WHERE");
                conditions = parse_where_clauses(cursor)?;
            }
            "APPLY" => {
                if apply_lens.is_some() {
                    let (line, col) = cursor.position();
                    return Err(ParseError::at(
                        "duplicate APPLY clause",
                        line,
                        col,
                    ));
                }
                cursor.consume_keyword("APPLY");
                cursor.skip_ws();
                let lens = parse_identifier(cursor, "lens name")?;
                apply_lens = Some(lens);
            }
            _ => {
                let (line, col) = cursor.position();
                return Err(ParseError::at(
                    format!(
                        "expected IN, WHERE, APPLY, or end of query, found `{next}`"
                    ),
                    line,
                    col,
                ));
            }
        }
        cursor.skip_ws();
    }

    Ok(FindQuery {
        target,
        scope,
        conditions,
        apply_lens,
    })
}

fn parse_target(cursor: &mut Cursor<'_>) -> Result<TargetType, ParseError> {
    let raw = parse_identifier(cursor, "target type (symbols|files|scopes|issues)")?;
    match raw.to_ascii_lowercase().as_str() {
        "symbols" => Ok(TargetType::Symbols),
        "files" => Ok(TargetType::Files),
        "scopes" => Ok(TargetType::Scopes),
        "issues" => Ok(TargetType::Issues),
        other => {
            let (line, col) = cursor.position();
            Err(ParseError::at(
                format!("unknown target type `{other}` — expected symbols, files, scopes, or issues"),
                line,
                col,
            ))
        }
    }
}

fn parse_scope_clause(cursor: &mut Cursor<'_>) -> Result<String, ParseError> {
    cursor.consume_keyword("IN");
    cursor.skip_ws();
    cursor.consume_keyword("SCOPE");
    cursor.skip_ws();
    parse_value_word(cursor)
        .map(|v| match v {
            Value::String(s) => s,
            // A bare number for a scope path is meaningless — accept it as
            // a string for parser resilience, but the spec expects a path.
            Value::Number(n) => n.to_string(),
        })
        .ok_or_else(|| {
            let (line, col) = cursor.position();
            ParseError::at("expected scope path after `IN SCOPE`", line, col)
        })
}

fn parse_where_clauses(cursor: &mut Cursor<'_>) -> Result<Vec<Condition>, ParseError> {
    let mut conds = vec![parse_condition(cursor)?];
    cursor.skip_ws();
    while let Some(kw) = cursor.peek_keyword() {
        if !kw.eq_ignore_ascii_case("AND") {
            break;
        }
        cursor.consume_keyword("AND");
        conds.push(parse_condition(cursor)?);
        cursor.skip_ws();
    }
    Ok(conds)
}

fn parse_condition(cursor: &mut Cursor<'_>) -> Result<Condition, ParseError> {
    let field = parse_field(cursor)?;
    cursor.skip_ws();
    let op = parse_op(cursor)?;
    cursor.skip_ws();
    let value = parse_value(cursor).ok_or_else(|| {
        let (line, col) = cursor.position();
        ParseError::at("expected value after operator", line, col)
    })?;
    Ok(Condition { field, op, value })
}

fn parse_field(cursor: &mut Cursor<'_>) -> Result<Field, ParseError> {
    let head = parse_identifier(cursor, "field name")?;
    cursor.skip_ws();
    // Optional dotted second segment.
    if cursor.peek_char() == Some('.') {
        cursor.advance();
        cursor.skip_ws();
        let tail = parse_identifier(cursor, "field sub-name after `.`")?;
        Ok(Field::dotted(head, tail))
    } else {
        Ok(Field::single(head))
    }
}

fn parse_op(cursor: &mut Cursor<'_>) -> Result<Op, ParseError> {
    let c0 = cursor.peek_char().ok_or_else(|| {
        let (line, col) = cursor.position();
        ParseError::at("expected comparison operator", line, col)
    })?;
    let c1 = cursor.peek_char_at(1);
    match (c0, c1) {
        ('>', Some('=')) => {
            cursor.advance_by(2);
            Ok(Op::Gte)
        }
        ('<', Some('=')) => {
            cursor.advance_by(2);
            Ok(Op::Lte)
        }
        ('=', Some('=')) => {
            cursor.advance_by(2);
            Ok(Op::Eq)
        }
        ('=', _) => {
            // Single `=` is accepted as equality (SQL-style sugar) —
            // `kind = "Function"` is more idiomatic than `kind == "Function"`.
            cursor.advance();
            Ok(Op::Eq)
        }
        ('!', Some('=')) => {
            cursor.advance_by(2);
            Ok(Op::Neq)
        }
        ('>', _) => {
            cursor.advance();
            Ok(Op::Gt)
        }
        ('<', _) => {
            cursor.advance();
            Ok(Op::Lt)
        }
        ('~', _) => {
            cursor.advance();
            Ok(Op::Contains)
        }
        _ => {
            let (line, col) = cursor.position();
            Err(ParseError::at(
                "expected one of `>`, `>=`, `<`, `<=`, `=`, `==`, `!=`, `~`",
                line,
                col,
            ))
        }
    }
}

fn parse_value(cursor: &mut Cursor<'_>) -> Option<Value> {
    // Quoted string first — content is everything up to the closing quote.
    if cursor.peek_char() == Some('"') {
        cursor.advance(); // opening quote
        let start = cursor.index;
        while let Some(c) = cursor.peek_char() {
            if c == '"' {
                break;
            }
            cursor.advance();
        }
        if cursor.peek_char() != Some('"') {
            return None; // unterminated — let caller handle
        }
        let s = cursor.input[start..cursor.index].to_string();
        cursor.advance(); // closing quote
        return Some(Value::String(s));
    }

    // Unquoted word — try number, fall back to plain string.
    parse_value_word(cursor)
}

/// Read a non-whitespace, non-quote, non-operator token.
///
/// Stops at whitespace, EOF, the operator characters, or `"`. Returns
/// `None` for an empty token.
fn parse_value_word(cursor: &mut Cursor<'_>) -> Option<Value> {
    let start = cursor.index;
    while let Some(c) = cursor.peek_char() {
        if c.is_whitespace() {
            break;
        }
        // Stop at operator start chars so `kind = "Function"` parses as
        // `kind`, then `=`, then `"Function"`.
        if matches!(c, '>' | '<' | '=' | '!' | '~' | '"') {
            break;
        }
        cursor.advance();
    }
    if cursor.index == start {
        return None;
    }
    let raw = &cursor.input[start..cursor.index];
    // Try to parse as a number first.
    if let Ok(n) = raw.parse::<f64>() {
        return Some(Value::Number(n));
    }
    Some(Value::String(raw.to_string()))
}

fn parse_identifier(cursor: &mut Cursor<'_>, what: &str) -> Result<String, ParseError> {
    let start = cursor.index;
    while let Some(c) = cursor.peek_char() {
        if c.is_whitespace() || matches!(c, '>' | '<' | '=' | '!' | '~' | '"' | '.') {
            break;
        }
        cursor.advance();
    }
    if cursor.index == start {
        let (line, col) = cursor.position();
        return Err(ParseError::at(format!("expected {what}"), line, col));
    }
    Ok(cursor.input[start..cursor.index].to_string())
}

// ----------------------------------------------------------------------------
// EXPLORE
// ----------------------------------------------------------------------------

fn parse_explore_after_keyword(
    cursor: &mut Cursor<'_>,
) -> Result<ExploreQuery, ParseError> {
    cursor.skip_ws();
    let object_ref = parse_value_word(cursor)
        .map(|v| match v {
            Value::String(s) => s,
            Value::Number(n) => n.to_string(),
        })
        .ok_or_else(|| {
            let (line, col) = cursor.position();
            ParseError::at("expected object_ref (e.g. symbol:src/main.rs:main:1)", line, col)
        })?;
    cursor.skip_ws();
    parse_through_clause(cursor)?;
    Ok(ExploreQuery {
        object_ref,
        direction: parse_direction_after_through(cursor)?,
        depth: parse_depth_after_direction(cursor)?,
    })
}

fn parse_through_clause(cursor: &mut Cursor<'_>) -> Result<(), ParseError> {
    let kw = cursor
        .peek_keyword()
        .ok_or_else(|| {
            let (line, col) = cursor.position();
            ParseError::at("expected `THROUGH` after object_ref", line, col)
        })?
        .to_ascii_uppercase();
    if kw != "THROUGH" {
        let (line, col) = cursor.position();
        return Err(ParseError::at(
            format!("expected `THROUGH`, found `{kw}`"),
            line,
            col,
        ));
    }
    cursor.consume_keyword("THROUGH");
    Ok(())
}

fn parse_direction_after_through(cursor: &mut Cursor<'_>) -> Result<Direction, ParseError> {
    cursor.skip_ws();
    let raw = parse_identifier(cursor, "direction (callers|callees)")?;
    match raw.to_ascii_lowercase().as_str() {
        "callers" => Ok(Direction::Callers),
        "callees" => Ok(Direction::Callees),
        other => {
            let (line, col) = cursor.position();
            Err(ParseError::at(
                format!("expected `callers` or `callees`, found `{other}`"),
                line,
                col,
            ))
        }
    }
}

fn parse_depth_after_direction(cursor: &mut Cursor<'_>) -> Result<u32, ParseError> {
    cursor.skip_ws();
    let kw = cursor
        .peek_keyword()
        .ok_or_else(|| {
            let (line, col) = cursor.position();
            ParseError::at("expected `DEPTH <n>`", line, col)
        })?
        .to_ascii_uppercase();
    if kw != "DEPTH" {
        let (line, col) = cursor.position();
        return Err(ParseError::at(
            format!("expected `DEPTH`, found `{kw}`"),
            line,
            col,
        ));
    }
    cursor.consume_keyword("DEPTH");
    cursor.skip_ws();
    let raw = parse_value_word(cursor)
        .ok_or_else(|| {
            let (line, col) = cursor.position();
            ParseError::at("expected integer after `DEPTH`", line, col)
        })
        .and_then(|v| match v {
            Value::Number(n) => {
                if n < 0.0 || n.fract() != 0.0 {
                    let (line, col) = cursor.position();
                    Err(ParseError::at(
                        "DEPTH must be a non-negative integer",
                        line,
                        col,
                    ))
                } else {
                    Ok(n as u32)
                }
            }
            Value::String(s) => {
                let (line, col) = cursor.position();
                Err(ParseError::at(
                    format!("DEPTH must be a number, found `{s}`"),
                    line,
                    col,
                ))
            }
        })?;
    Ok(raw)
}

// ============================================================================
// Cursor
// ============================================================================

/// Single-pass byte cursor. Tracks `index` into `input` and recomputes
/// 1-based `line` / `column` on demand.
struct Cursor<'a> {
    input: &'a str,
    index: usize,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, index: 0 }
    }

    fn is_eof(&self) -> bool {
        self.index >= self.input.len()
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.index..].chars().next()
    }

    fn peek_char_at(&self, offset: usize) -> Option<char> {
        self.input[self.index..].chars().nth(offset)
    }

    fn advance(&mut self) {
        if let Some(c) = self.peek_char() {
            self.index += c.len_utf8();
        }
    }

    fn advance_by(&mut self, chars: usize) {
        for _ in 0..chars {
            self.advance();
        }
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Read a keyword (whitespace-delimited, stops at operator chars
    /// and dots) and rewind — only the peek is non-destructive. The
    /// caller uses [`Self::consume_keyword`] to actually advance.
    fn peek_keyword(&self) -> Option<String> {
        let mut temp = Cursor {
            input: self.input,
            index: self.index,
        };
        temp.skip_ws();
        let start = temp.index;
        while let Some(c) = temp.peek_char() {
            if c.is_whitespace() || matches!(c, '>' | '<' | '=' | '!' | '~' | '"' | '.') {
                break;
            }
            temp.advance();
        }
        if temp.index == start {
            return None;
        }
        Some(temp.input[start..temp.index].to_string())
    }

    /// Advance past `keyword` (case-insensitive) and the trailing
    /// whitespace. Panics if the keyword does not match the current
    /// token — callers must check with `peek_keyword` first.
    fn consume_keyword(&mut self, keyword: &str) {
        self.skip_ws();
        let start = self.index;
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() || matches!(c, '>' | '<' | '=' | '!' | '~' | '"' | '.') {
                break;
            }
            self.advance();
        }
        let raw = &self.input[start..self.index];
        debug_assert!(
            raw.eq_ignore_ascii_case(keyword),
            "consume_keyword({keyword}) on `{raw}`"
        );
        self.skip_ws();
    }

    /// (line, column) for the current index. 1-based.
    fn position(&self) -> (u32, u32) {
        let mut line: u32 = 1;
        let mut col: u32 = 1;
        for (i, c) in self.input.char_indices() {
            if i >= self.index {
                break;
            }
            if c == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    /// Remaining input for diagnostic messages. Clipped to 32 chars.
    fn remaining(&self) -> String {
        let s: String = self.input[self.index..]
            .chars()
            .take(32)
            .collect();
        s
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moldql::ast::{Direction, Op, TargetType, Value};

    fn parse_ok(input: &str) -> MoldQLQuery {
        parse(input).unwrap_or_else(|e| panic!("expected Ok for `{input}`, got {e}"))
    }

    fn parse_err(input: &str) -> ParseError {
        parse(input)
            .err()
            .unwrap_or_else(|| panic!("expected Err for `{input}`, got Ok"))
    }

    // ---- FIND happy paths --------------------------------------------------

    #[test]
    fn find_symbols_no_clauses() {
        let q = parse_ok("FIND symbols");
        assert!(matches!(
            q,
            MoldQLQuery::Find(FindQuery {
                target: TargetType::Symbols,
                scope: None,
                conditions,
                apply_lens: None,
            }) if conditions.is_empty()
        ));
    }

    #[test]
    fn find_with_where_numeric() {
        let q = parse_ok("FIND symbols WHERE fan_in > 5");
        let MoldQLQuery::Find(f) = q else { panic!("expected Find") };
        assert_eq!(f.target, TargetType::Symbols);
        assert_eq!(f.conditions.len(), 1);
        let c = &f.conditions[0];
        assert_eq!(c.field.parts, vec!["fan_in".to_string()]);
        assert_eq!(c.op, Op::Gt);
        assert_eq!(c.value, Value::Number(5.0));
    }

    #[test]
    fn find_with_in_scope() {
        let q = parse_ok("FIND files IN SCOPE src");
        let MoldQLQuery::Find(f) = q else { panic!("expected Find") };
        assert_eq!(f.target, TargetType::Files);
        assert_eq!(f.scope.as_deref(), Some("src"));
        assert!(f.conditions.is_empty());
    }

    #[test]
    fn find_with_scope_where_apply() {
        let q = parse_ok(
            "FIND symbols IN SCOPE src WHERE kind = \"Function\" AND fan_out < 3 APPLY hotspots",
        );
        let MoldQLQuery::Find(f) = q else { panic!("expected Find") };
        assert_eq!(f.target, TargetType::Symbols);
        assert_eq!(f.scope.as_deref(), Some("src"));
        assert_eq!(f.conditions.len(), 2);
        assert_eq!(f.apply_lens.as_deref(), Some("hotspots"));
        assert_eq!(f.conditions[0].field.parts, vec!["kind".to_string()]);
        assert_eq!(f.conditions[0].op, Op::Eq);
        assert_eq!(
            f.conditions[0].value,
            Value::String("Function".to_string())
        );
        assert_eq!(f.conditions[1].field.parts, vec!["fan_out".to_string()]);
        assert_eq!(f.conditions[1].op, Op::Lt);
        assert_eq!(f.conditions[1].value, Value::Number(3.0));
    }

    #[test]
    fn find_quality_dotted_field() {
        let q = parse_ok("FIND files WHERE quality.critical > 0");
        let MoldQLQuery::Find(f) = q else { panic!("expected Find") };
        let c = &f.conditions[0];
        assert_eq!(c.field.parts, vec!["quality".to_string(), "critical".to_string()]);
        assert_eq!(c.op, Op::Gt);
        assert_eq!(c.value, Value::Number(0.0));
    }

    #[test]
    fn find_all_operators() {
        for (input, expected) in [
            ("FIND symbols WHERE fan_in > 5", Op::Gt),
            ("FIND symbols WHERE fan_in >= 5", Op::Gte),
            ("FIND symbols WHERE fan_in < 5", Op::Lt),
            ("FIND symbols WHERE fan_in <= 5", Op::Lte),
            ("FIND symbols WHERE fan_in == 5", Op::Eq),
            ("FIND symbols WHERE fan_in != 5", Op::Neq),
            ("FIND symbols WHERE name ~ \"main\"", Op::Contains),
        ] {
            let q = parse_ok(input);
            let MoldQLQuery::Find(f) = q else { panic!("expected Find for `{input}`") };
            assert_eq!(f.conditions[0].op, expected, "operator for `{input}`");
        }
    }

    #[test]
    fn find_issues_target() {
        let q = parse_ok("FIND issues WHERE severity = \"Critical\"");
        let MoldQLQuery::Find(f) = q else { panic!("expected Find") };
        assert_eq!(f.target, TargetType::Issues);
        assert_eq!(
            f.conditions[0].value,
            Value::String("Critical".to_string())
        );
    }

    #[test]
    fn find_unquoted_string_value() {
        let q = parse_ok("FIND symbols WHERE kind = Function");
        let MoldQLQuery::Find(f) = q else { panic!("expected Find") };
        assert_eq!(f.conditions[0].value, Value::String("Function".to_string()));
    }

    // ---- EXPLORE happy paths ----------------------------------------------

    #[test]
    fn explore_callers() {
        let q = parse_ok("EXPLORE symbol:src/main.rs:main:1 THROUGH callers DEPTH 3");
        let MoldQLQuery::Explore(e) = q else { panic!("expected Explore") };
        assert_eq!(e.object_ref, "symbol:src/main.rs:main:1");
        assert_eq!(e.direction, Direction::Callers);
        assert_eq!(e.depth, 3);
    }

    #[test]
    fn explore_callees() {
        let q = parse_ok("EXPLORE symbol:src/main.rs:main:1 THROUGH callees DEPTH 1");
        let MoldQLQuery::Explore(e) = q else { panic!("expected Explore") };
        assert_eq!(e.direction, Direction::Callees);
        assert_eq!(e.depth, 1);
    }

    #[test]
    fn explore_depth_zero() {
        let q = parse_ok("EXPLORE symbol:src/main.rs:main:1 THROUGH callers DEPTH 0");
        let MoldQLQuery::Explore(e) = q else { panic!("expected Explore") };
        assert_eq!(e.depth, 0);
    }

    // ---- Case insensitivity ----------------------------------------------

    #[test]
    fn case_insensitive_keywords() {
        let q = parse_ok("find SYMBOLS where fan_in > 5");
        let MoldQLQuery::Find(f) = q else { panic!("expected Find") };
        assert_eq!(f.target, TargetType::Symbols);
        assert_eq!(f.conditions[0].op, Op::Gt);
    }

    // ---- Error cases -------------------------------------------------------

    #[test]
    fn empty_query_errors() {
        let e = parse_err("");
        assert!(e.message.contains("empty"));
    }

    #[test]
    fn unknown_leading_keyword_errors() {
        let e = parse_err("FOO symbols");
        assert!(e.message.contains("FIND or EXPLORE"));
    }

    #[test]
    fn unknown_target_type_errors() {
        let e = parse_err("FIND widgets");
        assert!(e.message.contains("unknown target type"));
    }

    #[test]
    fn missing_value_errors() {
        let e = parse_err("FIND symbols WHERE fan_in >");
        assert!(e.message.contains("value"));
    }

    #[test]
    fn missing_operator_errors() {
        let e = parse_err("FIND symbols WHERE fan_in");
        assert!(e.message.contains("operator"));
    }

    #[test]
    fn trailing_garbage_errors() {
        // The query is well-formed up to "fan_in > 5", but `BOGUS` is
        // not a recognised clause keyword. The parser surfaces a clear
        // "expected IN, WHERE, APPLY, or end of query" message naming
        // the offending token.
        let e = parse_err("FIND symbols WHERE fan_in > 5 BOGUS");
        assert!(
            e.message.contains("BOGUS"),
            "error should name the unexpected token, got: {}",
            e.message
        );
    }

    #[test]
    fn invalid_direction_errors() {
        let e = parse_err("EXPLORE symbol:src/main.rs:main:1 THROUGH sideways DEPTH 1");
        assert!(e.message.contains("callers"));
    }

    #[test]
    fn missing_depth_errors() {
        let e = parse_err("EXPLORE symbol:src/main.rs:main:1 THROUGH callers");
        assert!(e.message.contains("DEPTH"));
    }

    #[test]
    fn invalid_depth_value_errors() {
        let e = parse_err("EXPLORE symbol:src/main.rs:main:1 THROUGH callers DEPTH 1.5");
        assert!(e.message.contains("integer"));
    }

    #[test]
    fn missing_in_scope_path_errors() {
        let e = parse_err("FIND symbols IN SCOPE");
        assert!(e.message.contains("scope path"));
    }

    #[test]
    fn error_carries_position() {
        let e = parse_err("FOO");
        assert_eq!(e.line, 1);
        assert_eq!(e.column, 1);
    }
}
