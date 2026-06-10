//! ExplorerQL parser — extends the base MoldQL parser with 5 graph-native
//! primitives and boolean composition.
//!
//! ## Dispatch
//!
//! ```text
//!   parse_query (in parser.rs)
//!        │
//!        ▼
//!   parse_or_chain   (this file)
//!        │
//!        ├── parse_and_chain
//!        │       │
//!        │       └── parse_atom  ── dispatch on leading keyword:
//!        │             ├── FIND/EXPLORE  → existing parser
//!        │             ├── PATH/NEIGHBORS/SUBGRAPH/CLUSTER/EXPLAIN
//!        │             └── "("  → parse_paren → recurse
//!        │
//!        └── (OR is left-associative across AND chains)
//! ```
//!
//! ## Precedence
//!
//! `NOT > AND > OR`. Parentheses override precedence. Bare primitives
//! are NEVER wrapped in `Boolean` — only the explicit `(<a> AND <b>)`
//! or `NOT <q>` constructs produce a `MoldQLQuery::Boolean`.

use crate::moldql::ast::{
    BooleanOp, BooleanQuery, ClusterMethod, ClusterQuery, Condition, Direction, ExplainQuery,
    ExploreQuery, Field, FindQuery, MoldQLQuery, NeighborsQuery, Op, PathQuery, SubgraphQuery,
    TargetType, TraversalDirection, Value,
};
use crate::moldql::cursor::Cursor;
use crate::moldql::parser::{ParseError, parse_value_word};

/// Entry point for the new graph-native dialect. `parse_query` in
/// `parser.rs` calls this when a non-FIND/EXPLORE keyword is present
/// at the top level.
pub(crate) fn parse_query_or_chain(cursor: &mut Cursor<'_>) -> Result<MoldQLQuery, ParseError> {
    parse_or_chain(cursor)
}

/// Public atom entry point — exposed for `parser.rs` to call after
/// parsing a `FIND` or `EXPLORE` body, when the cursor is sitting on
/// the `AND` / `OR` / `NOT` keyword that follows.
pub(crate) fn parse_atom_public(cursor: &mut Cursor<'_>) -> Result<MoldQLQuery, ParseError> {
    parse_atom(cursor)
}

fn parse_or_chain(cursor: &mut Cursor<'_>) -> Result<MoldQLQuery, ParseError> {
    let mut left = parse_and_chain(cursor)?;
    cursor.skip_ws();
    while let Some(kw) = cursor.peek_keyword() {
        if !kw.eq_ignore_ascii_case("OR") {
            break;
        }
        cursor.consume_keyword("OR");
        let right = parse_and_chain(cursor)?;
        // Flatten left-associative chains: `A OR B OR C` →
        // `Boolean(Or, [A, B, C])` not nested binary trees.
        match &mut left {
            MoldQLQuery::Boolean(b) if b.op == BooleanOp::Or => {
                b.operands.push(right);
            }
            _ => {
                left = MoldQLQuery::Boolean(BooleanQuery {
                    op: BooleanOp::Or,
                    operands: vec![left, right],
                });
            }
        }
        cursor.skip_ws();
    }
    Ok(left)
}

fn parse_and_chain(cursor: &mut Cursor<'_>) -> Result<MoldQLQuery, ParseError> {
    let mut left = parse_atom(cursor)?;
    cursor.skip_ws();
    while let Some(kw) = cursor.peek_keyword() {
        if !kw.eq_ignore_ascii_case("AND") {
            break;
        }
        cursor.consume_keyword("AND");
        let right = parse_atom(cursor)?;
        match &mut left {
            MoldQLQuery::Boolean(b) if b.op == BooleanOp::And => {
                b.operands.push(right);
            }
            _ => {
                left = MoldQLQuery::Boolean(BooleanQuery {
                    op: BooleanOp::And,
                    operands: vec![left, right],
                });
            }
        }
        cursor.skip_ws();
    }
    Ok(left)
}

fn parse_atom(cursor: &mut Cursor<'_>) -> Result<MoldQLQuery, ParseError> {
    cursor.skip_ws();
    // Paren-wrapped: recurse with the inner chain, then expect `)`.
    if cursor.peek_char() == Some('(') {
        cursor.advance();
        cursor.skip_ws();
        let inner = parse_or_chain(cursor)?;
        cursor.skip_ws();
        if cursor.peek_char() != Some(')') {
            let (line, col) = cursor.position();
            return Err(ParseError::at(
                "expected `)` to close boolean group",
                line,
                col,
            ));
        }
        cursor.advance(); // consume `)`
        return Ok(inner);
    }

    // `NOT <atom>` — the only valid top-level use.
    if let Some(kw) = cursor.peek_keyword() {
        if kw.eq_ignore_ascii_case("NOT") {
            cursor.consume_keyword("NOT");
            cursor.skip_ws();
            let inner = parse_atom(cursor)?;
            return Ok(MoldQLQuery::Boolean(BooleanQuery {
                op: BooleanOp::Not,
                operands: vec![inner],
            }));
        }
    }

    // Otherwise it's a primitive.
    let kw = cursor.peek_keyword().ok_or_else(|| {
        let (line, col) = cursor.position();
        ParseError::at("expected primitive or `(`", line, col)
    })?;
    match kw.to_ascii_uppercase().as_str() {
        "PATH" => parse_path(cursor).map(MoldQLQuery::Path),
        "NEIGHBORS" => parse_neighbors(cursor).map(MoldQLQuery::Neighbors),
        "SUBGRAPH" => parse_subgraph(cursor).map(MoldQLQuery::Subgraph),
        "CLUSTER" => parse_cluster(cursor).map(MoldQLQuery::Cluster),
        "EXPLAIN" => parse_explain(cursor).map(MoldQLQuery::Explain),
        "FIND" => {
            cursor.consume_keyword("FIND");
            parse_find_after(cursor).map(MoldQLQuery::Find)
        }
        "EXPLORE" => {
            cursor.consume_keyword("EXPLORE");
            parse_explore_after(cursor).map(MoldQLQuery::Explore)
        }
        other => {
            let (line, col) = cursor.position();
            Err(ParseError::at(
                format!(
                    "unknown leading keyword `{other}` — expected FIND, EXPLORE, \
                     PATH, NEIGHBORS, SUBGRAPH, CLUSTER, or EXPLAIN"
                ),
                line,
                col,
            ))
        }
    }
}

// ============================================================================
// Re-entry points for FIND / EXPLORE (mirror the helpers in parser.rs but
// live here so the new module is self-contained).
// ============================================================================

fn parse_find_after(cursor: &mut Cursor<'_>) -> Result<FindQuery, ParseError> {
    cursor.skip_ws();
    let target = parse_target(cursor)?;
    let mut scope: Option<String> = None;
    let mut conditions: Vec<Condition> = Vec::new();
    let mut apply_lens: Option<String> = None;
    loop {
        cursor.skip_ws();
        if cursor.is_eof() {
            break;
        }
        let next = cursor
            .peek_keyword()
            .ok_or_else(|| {
                let (line, col) = cursor.position();
                ParseError::at("expected IN, WHERE, APPLY, or end of query", line, col)
            })?
            .to_ascii_uppercase();
        match next.as_str() {
            "IN" => {
                if scope.is_some() {
                    let (line, col) = cursor.position();
                    return Err(ParseError::at("duplicate IN SCOPE clause", line, col));
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
                    return Err(ParseError::at("duplicate APPLY clause", line, col));
                }
                cursor.consume_keyword("APPLY");
                cursor.skip_ws();
                let lens = parse_identifier(cursor, "lens name")?;
                apply_lens = Some(lens);
            }
            _ => {
                let (line, col) = cursor.position();
                return Err(ParseError::at(
                    format!("expected IN, WHERE, APPLY, or end of query, found `{next}`"),
                    line,
                    col,
                ));
            }
        }
    }
    Ok(FindQuery {
        target,
        scope,
        conditions,
        apply_lens,
    })
}

fn parse_explore_after(cursor: &mut Cursor<'_>) -> Result<ExploreQuery, ParseError> {
    cursor.skip_ws();
    let object_ref = parse_value_word(cursor)
        .map(|v| match v {
            Value::String(s) => s,
            Value::Number(n) => n.to_string(),
        })
        .ok_or_else(|| {
            let (line, col) = cursor.position();
            ParseError::at(
                "expected object_ref (e.g. symbol:src/main.rs:main:1)",
                line,
                col,
            )
        })?;
    cursor.skip_ws();
    parse_through(cursor)?;
    Ok(ExploreQuery {
        object_ref,
        direction: parse_direction_after(cursor)?,
        depth: parse_depth_after(cursor)?,
    })
}

// ============================================================================
// PATH
// ============================================================================

fn parse_path(cursor: &mut Cursor<'_>) -> Result<PathQuery, ParseError> {
    cursor.consume_keyword("PATH");
    cursor.skip_ws();
    if !cursor
        .peek_keyword()
        .map(|k| k.eq_ignore_ascii_case("FROM"))
        .unwrap_or(false)
    {
        let (line, col) = cursor.position();
        return Err(ParseError::at("PATH: expected `FROM`", line, col));
    }
    cursor.consume_keyword("FROM");
    cursor.skip_ws();
    let from = parse_object_ref(cursor, "PATH FROM <obj>")?;
    cursor.skip_ws();
    if !cursor
        .peek_keyword()
        .map(|k| k.eq_ignore_ascii_case("TO"))
        .unwrap_or(false)
    {
        let (line, col) = cursor.position();
        return Err(ParseError::at("PATH: expected `TO`", line, col));
    }
    cursor.consume_keyword("TO");
    cursor.skip_ws();
    let to = parse_object_ref(cursor, "PATH TO <obj>")?;
    let mut max_hops: Option<u32> = None;
    let mut conditions: Vec<Condition> = Vec::new();
    cursor.skip_ws();
    loop {
        if cursor.is_eof() {
            break;
        }
        let kw = cursor
            .peek_keyword()
            .map(|k| k.to_ascii_uppercase())
            .unwrap_or_default();
        match kw.as_str() {
            "MAX" => {
                cursor.consume_keyword("MAX");
                cursor.skip_ws();
                if !cursor
                    .peek_keyword()
                    .map(|k| k.eq_ignore_ascii_case("HOPS"))
                    .unwrap_or(false)
                {
                    let (line, col) = cursor.position();
                    return Err(ParseError::at(
                        "PATH: expected `HOPS` after `MAX`",
                        line,
                        col,
                    ));
                }
                cursor.consume_keyword("HOPS");
                cursor.skip_ws();
                max_hops = Some(parse_u32(cursor, "MAX HOPS")?);
            }
            "WHERE" => {
                cursor.consume_keyword("WHERE");
                conditions = parse_where_clauses(cursor)?;
            }
            _ => break,
        }
        cursor.skip_ws();
    }
    Ok(PathQuery {
        from,
        to,
        max_hops,
        conditions,
    })
}

// ============================================================================
// NEIGHBORS
// ============================================================================

fn parse_neighbors(cursor: &mut Cursor<'_>) -> Result<NeighborsQuery, ParseError> {
    cursor.consume_keyword("NEIGHBORS");
    cursor.skip_ws();
    let root = parse_object_ref(cursor, "NEIGHBORS <root>")?;
    cursor.skip_ws();
    if !cursor
        .peek_keyword()
        .map(|k| k.eq_ignore_ascii_case("DEPTH"))
        .unwrap_or(false)
    {
        let (line, col) = cursor.position();
        return Err(ParseError::at(
            "NEIGHBORS: expected `DEPTH` after root",
            line,
            col,
        ));
    }
    cursor.consume_keyword("DEPTH");
    cursor.skip_ws();
    let depth = parse_u32(cursor, "DEPTH")?;
    let depth = depth.min(5); // cap 5 per spec
    let mut direction = TraversalDirection::Both;
    let mut conditions: Vec<Condition> = Vec::new();
    cursor.skip_ws();
    loop {
        if cursor.is_eof() {
            break;
        }
        let kw = cursor
            .peek_keyword()
            .map(|k| k.to_ascii_uppercase())
            .unwrap_or_default();
        match kw.as_str() {
            "DIRECTION" => {
                cursor.consume_keyword("DIRECTION");
                cursor.skip_ws();
                direction = parse_direction_kw(cursor)?;
            }
            "WHERE" => {
                cursor.consume_keyword("WHERE");
                conditions = parse_where_clauses(cursor)?;
            }
            _ => break,
        }
        cursor.skip_ws();
    }
    Ok(NeighborsQuery {
        root,
        depth,
        direction,
        conditions,
    })
}

// ============================================================================
// SUBGRAPH
// ============================================================================

fn parse_subgraph(cursor: &mut Cursor<'_>) -> Result<SubgraphQuery, ParseError> {
    cursor.consume_keyword("SUBGRAPH");
    cursor.skip_ws();
    if !cursor
        .peek_keyword()
        .map(|k| k.eq_ignore_ascii_case("ROOT"))
        .unwrap_or(false)
    {
        let (line, col) = cursor.position();
        return Err(ParseError::at("SUBGRAPH: expected `ROOT`", line, col));
    }
    cursor.consume_keyword("ROOT");
    cursor.skip_ws();
    let root = parse_object_ref(cursor, "SUBGRAPH ROOT <obj>")?;
    let mut depth: u32 = 3; // default
    let mut direction = TraversalDirection::Both;
    let mut conditions: Vec<Condition> = Vec::new();
    cursor.skip_ws();
    loop {
        if cursor.is_eof() {
            break;
        }
        let kw = cursor
            .peek_keyword()
            .map(|k| k.to_ascii_uppercase())
            .unwrap_or_default();
        match kw.as_str() {
            "DEPTH" => {
                cursor.consume_keyword("DEPTH");
                cursor.skip_ws();
                depth = parse_u32(cursor, "DEPTH")?.min(5);
            }
            "DIRECTION" => {
                cursor.consume_keyword("DIRECTION");
                cursor.skip_ws();
                direction = parse_direction_kw(cursor)?;
            }
            "WHERE" => {
                cursor.consume_keyword("WHERE");
                conditions = parse_where_clauses(cursor)?;
            }
            _ => break,
        }
        cursor.skip_ws();
    }
    Ok(SubgraphQuery {
        root,
        depth,
        direction,
        conditions,
    })
}

// ============================================================================
// CLUSTER
// ============================================================================

fn parse_cluster(cursor: &mut Cursor<'_>) -> Result<ClusterQuery, ParseError> {
    cursor.consume_keyword("CLUSTER");
    let mut method = ClusterMethod::Scc;
    let mut conditions: Vec<Condition> = Vec::new();
    cursor.skip_ws();
    loop {
        if cursor.is_eof() {
            break;
        }
        let kw = cursor
            .peek_keyword()
            .map(|k| k.to_ascii_uppercase())
            .unwrap_or_default();
        match kw.as_str() {
            "METHOD" => {
                cursor.consume_keyword("METHOD");
                cursor.skip_ws();
                let raw = parse_identifier(cursor, "method (scc|connected)")?;
                method = match raw.to_ascii_lowercase().as_str() {
                    "scc" => ClusterMethod::Scc,
                    "connected" => ClusterMethod::Connected,
                    other => {
                        let (line, col) = cursor.position();
                        return Err(ParseError::at(
                            format!(
                                "CLUSTER: unknown method `{other}` — expected scc or connected"
                            ),
                            line,
                            col,
                        ));
                    }
                };
            }
            "WHERE" => {
                cursor.consume_keyword("WHERE");
                conditions = parse_where_clauses(cursor)?;
            }
            _ => break,
        }
        cursor.skip_ws();
    }
    Ok(ClusterQuery { method, conditions })
}

// ============================================================================
// EXPLAIN
// ============================================================================

fn parse_explain(cursor: &mut Cursor<'_>) -> Result<ExplainQuery, ParseError> {
    cursor.consume_keyword("EXPLAIN");
    cursor.skip_ws();
    if !cursor
        .peek_keyword()
        .map(|k| k.eq_ignore_ascii_case("FROM"))
        .unwrap_or(false)
    {
        let (line, col) = cursor.position();
        return Err(ParseError::at("EXPLAIN: expected `FROM`", line, col));
    }
    cursor.consume_keyword("FROM");
    cursor.skip_ws();
    let from = parse_object_ref(cursor, "EXPLAIN FROM <obj>")?;
    cursor.skip_ws();
    if !cursor
        .peek_keyword()
        .map(|k| k.eq_ignore_ascii_case("TO"))
        .unwrap_or(false)
    {
        let (line, col) = cursor.position();
        return Err(ParseError::at("EXPLAIN: expected `TO`", line, col));
    }
    cursor.consume_keyword("TO");
    cursor.skip_ws();
    let to = parse_object_ref(cursor, "EXPLAIN TO <obj>")?;
    let mut conditions: Vec<Condition> = Vec::new();
    cursor.skip_ws();
    if let Some(kw) = cursor.peek_keyword() {
        if kw.eq_ignore_ascii_case("MAX") {
            let (line, col) = cursor.position();
            return Err(ParseError::at(
                "EXPLAIN rejects `MAX HOPS` (path is exact, not BFS)",
                line,
                col,
            ));
        }
        if kw.eq_ignore_ascii_case("WHERE") {
            cursor.consume_keyword("WHERE");
            conditions = parse_where_clauses(cursor)?;
        }
    }
    Ok(ExplainQuery {
        from,
        to,
        conditions,
    })
}

// ============================================================================
// Shared helpers (copied from parser.rs so the new module is self-contained).
// ============================================================================

fn parse_object_ref(cursor: &mut Cursor<'_>, what: &str) -> Result<String, ParseError> {
    parse_value_word(cursor)
        .map(|v| match v {
            Value::String(s) => s,
            Value::Number(n) => n.to_string(),
        })
        .ok_or_else(|| {
            let (line, col) = cursor.position();
            ParseError::at(format!("expected {what}"), line, col)
        })
}

fn parse_target(cursor: &mut Cursor<'_>) -> Result<TargetType, ParseError> {
    // Build the list of valid keywords at compile time so the
    // error message can list them all (T20) — `decisions` /
    // `docs` only appear when the `multimodal` feature is on.
    let valid_keywords: &[&str] = &[
        TargetType::Symbols.keyword(),
        TargetType::Files.keyword(),
        TargetType::Scopes.keyword(),
        TargetType::Issues.keyword(),
        #[cfg(feature = "multimodal")]
        TargetType::Decisions.keyword(),
        #[cfg(feature = "multimodal")]
        TargetType::Docs.keyword(),
    ];
    let what = format!("target type ({})", valid_keywords.join("|"));
    let raw = parse_identifier(cursor, &what)?;
    match raw.to_ascii_lowercase().as_str() {
        "symbols" => Ok(TargetType::Symbols),
        "files" => Ok(TargetType::Files),
        "scopes" => Ok(TargetType::Scopes),
        "issues" => Ok(TargetType::Issues),
        #[cfg(feature = "multimodal")]
        "decisions" => Ok(TargetType::Decisions),
        #[cfg(feature = "multimodal")]
        "docs" => Ok(TargetType::Docs),
        other => {
            let (line, col) = cursor.position();
            Err(ParseError::at(
                format!(
                    "unknown target type `{other}` — expected {}",
                    valid_keywords.join(", ")
                ),
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
    if cursor.peek_char() == Some('"') {
        cursor.advance();
        let start = cursor.index;
        while let Some(c) = cursor.peek_char() {
            if c == '"' {
                break;
            }
            cursor.advance();
        }
        if cursor.peek_char() != Some('"') {
            return None;
        }
        let s = cursor.input[start..cursor.index].to_string();
        cursor.advance();
        return Some(Value::String(s));
    }
    parse_value_word(cursor)
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

fn parse_u32(cursor: &mut Cursor<'_>, what: &str) -> Result<u32, ParseError> {
    let raw = parse_value_word(cursor).ok_or_else(|| {
        let (line, col) = cursor.position();
        ParseError::at(format!("expected {what}"), line, col)
    })?;
    match raw {
        Value::Number(n) => {
            if n < 0.0 || n.fract() != 0.0 {
                let (line, col) = cursor.position();
                Err(ParseError::at(
                    format!("{what} must be a non-negative integer"),
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
                format!("{what} must be a number, found `{s}`"),
                line,
                col,
            ))
        }
    }
}

fn parse_through(cursor: &mut Cursor<'_>) -> Result<(), ParseError> {
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

fn parse_direction_after(cursor: &mut Cursor<'_>) -> Result<Direction, ParseError> {
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

fn parse_depth_after(cursor: &mut Cursor<'_>) -> Result<u32, ParseError> {
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
    parse_u32(cursor, "DEPTH")
}

fn parse_direction_kw(cursor: &mut Cursor<'_>) -> Result<TraversalDirection, ParseError> {
    let raw = parse_identifier(cursor, "direction (incoming|outgoing|both)")?;
    match raw.to_ascii_lowercase().as_str() {
        "incoming" => Ok(TraversalDirection::Incoming),
        "outgoing" => Ok(TraversalDirection::Outgoing),
        "both" => Ok(TraversalDirection::Both),
        other => {
            let (line, col) = cursor.position();
            Err(ParseError::at(
                format!("expected `incoming`, `outgoing`, or `both`, found `{other}`"),
                line,
                col,
            ))
        }
    }
}

// ============================================================================
// Tests — TDD-first. Each test references a query that the parser
// must accept and produces a specific AST shape.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moldql::parser::parse;

    fn p(input: &str) -> MoldQLQuery {
        parse(input).unwrap_or_else(|e| panic!("expected Ok for `{input}`, got {e}"))
    }

    // -- Task 1.6 — RED: boolean composition scaffold ----------------------

    #[test]
    fn parse_boolean_and_or() {
        // "FIND x AND EXPLORE y" — atom is FIND, second atom is EXPLORE.
        // The base parser doesn't accept `AND` between FIND and EXPLORE,
        // so this becomes a top-level boolean.
        let q = p("FIND symbols AND EXPLORE symbol:a:b:1 THROUGH callers DEPTH 1");
        match q {
            MoldQLQuery::Boolean(b) => {
                assert_eq!(b.op, BooleanOp::And);
                assert_eq!(b.operands.len(), 2);
                assert!(matches!(b.operands[0], MoldQLQuery::Find(_)));
                assert!(matches!(b.operands[1], MoldQLQuery::Explore(_)));
            }
            other => panic!("expected Boolean, got {other:?}"),
        }
    }

    #[test]
    fn parse_or_chain() {
        let q = p("PATH FROM a TO b OR PATH FROM c TO d");
        match q {
            MoldQLQuery::Boolean(b) => {
                assert_eq!(b.op, BooleanOp::Or);
                assert_eq!(b.operands.len(), 2);
            }
            other => panic!("expected Boolean(Or), got {other:?}"),
        }
    }

    #[test]
    fn parse_not() {
        let q = p("NOT PATH FROM a TO b");
        match q {
            MoldQLQuery::Boolean(b) => {
                assert_eq!(b.op, BooleanOp::Not);
                assert_eq!(b.operands.len(), 1);
                assert!(matches!(b.operands[0], MoldQLQuery::Path(_)));
            }
            other => panic!("expected Boolean(Not), got {other:?}"),
        }
    }

    #[test]
    fn parse_paren_group() {
        let q = p("(PATH FROM a TO b) AND (PATH FROM c TO d)");
        match q {
            MoldQLQuery::Boolean(b) => {
                assert_eq!(b.op, BooleanOp::And);
                assert_eq!(b.operands.len(), 2);
            }
            other => panic!("expected Boolean(And), got {other:?}"),
        }
    }

    #[test]
    fn parse_paren_around_single_primitive_unwraps() {
        // A bare paren around a single primitive should NOT wrap it in
        // a Boolean variant — the parens are just grouping, the inner
        // primitive is the only operand and there's no operator.
        let q = p("(PATH FROM a TO b)");
        assert!(matches!(q, MoldQLQuery::Path(_)));
    }

    #[test]
    fn parse_and_or_precedence() {
        // `A AND B OR C AND D` should parse as `(A AND B) OR (C AND D)`,
        // per the spec precedence (AND > OR).
        let q = p("PATH FROM a TO b AND PATH FROM c TO d OR PATH FROM e TO f AND PATH FROM g TO h");
        match q {
            MoldQLQuery::Boolean(b) => {
                assert_eq!(b.op, BooleanOp::Or);
                assert_eq!(b.operands.len(), 2);
                // Each operand is an And-chain.
                match &b.operands[0] {
                    MoldQLQuery::Boolean(b0) => {
                        assert_eq!(b0.op, BooleanOp::And);
                        assert_eq!(b0.operands.len(), 2);
                    }
                    other => panic!("expected inner And, got {other:?}"),
                }
                match &b.operands[1] {
                    MoldQLQuery::Boolean(b1) => {
                        assert_eq!(b1.op, BooleanOp::And);
                        assert_eq!(b1.operands.len(), 2);
                    }
                    other => panic!("expected inner And, got {other:?}"),
                }
            }
            other => panic!("expected Boolean(Or), got {other:?}"),
        }
    }

    #[test]
    fn unknown_keyword_lists_seven() {
        // Spec §1: unknown keyword → ParseError listing all 7 leading
        // keywords (FIND, EXPLORE, PATH, NEIGHBORS, SUBGRAPH, CLUSTER, EXPLAIN).
        let err = parse("FOO x").unwrap_err();
        let m = err.message;
        for k in &[
            "FIND",
            "EXPLORE",
            "PATH",
            "NEIGHBORS",
            "SUBGRAPH",
            "CLUSTER",
            "EXPLAIN",
        ] {
            assert!(m.contains(k), "error message should mention {k}, got: {m}");
        }
    }

    // -- Task 2.x — clause parsers ---------------------------------------

    // -- PATH --
    #[test]
    fn parse_path_basic() {
        let q = p("PATH FROM a TO b");
        let MoldQLQuery::Path(pq) = q else {
            panic!("expected Path");
        };
        assert_eq!(pq.from, "a");
        assert_eq!(pq.to, "b");
        assert!(pq.max_hops.is_none());
        assert!(pq.conditions.is_empty());
    }

    #[test]
    fn parse_path_max_hops() {
        let q = p("PATH FROM a TO b MAX HOPS 3");
        let MoldQLQuery::Path(pq) = q else {
            panic!("expected Path")
        };
        assert_eq!(pq.max_hops, Some(3));
    }

    #[test]
    fn parse_path_rejects_non_int_max_hops() {
        let err = parse("PATH FROM a TO b MAX HOPS 1.5").unwrap_err();
        assert!(err.message.contains("integer"));
    }

    #[test]
    fn parse_path_with_where() {
        let q = p("PATH FROM a TO b WHERE provenance.lsp = \"x\"");
        let MoldQLQuery::Path(pq) = q else {
            panic!("expected Path")
        };
        assert_eq!(pq.conditions.len(), 1);
    }

    // -- NEIGHBORS --
    #[test]
    fn parse_neighbors_basic() {
        let q = p("NEIGHBORS a DEPTH 2");
        let MoldQLQuery::Neighbors(nq) = q else {
            panic!("expected Neighbors")
        };
        assert_eq!(nq.root, "a");
        assert_eq!(nq.depth, 2);
        assert_eq!(nq.direction, TraversalDirection::Both);
    }

    #[test]
    fn parse_neighbors_incoming() {
        let q = p("NEIGHBORS a DEPTH 1 DIRECTION incoming");
        let MoldQLQuery::Neighbors(nq) = q else {
            panic!("expected Neighbors")
        };
        assert_eq!(nq.direction, TraversalDirection::Incoming);
    }

    #[test]
    fn parse_neighbors_outgoing() {
        let q = p("NEIGHBORS a DEPTH 1 DIRECTION outgoing");
        let MoldQLQuery::Neighbors(nq) = q else {
            panic!("expected Neighbors")
        };
        assert_eq!(nq.direction, TraversalDirection::Outgoing);
    }

    #[test]
    fn parse_neighbors_both() {
        let q = p("NEIGHBORS a DEPTH 1 DIRECTION both");
        let MoldQLQuery::Neighbors(nq) = q else {
            panic!("expected Neighbors")
        };
        assert_eq!(nq.direction, TraversalDirection::Both);
    }

    #[test]
    fn parse_neighbors_depth_capped_at_5() {
        let q = p("NEIGHBORS a DEPTH 99");
        let MoldQLQuery::Neighbors(nq) = q else {
            panic!("expected Neighbors")
        };
        assert_eq!(nq.depth, 5);
    }

    // -- SUBGRAPH --
    #[test]
    fn parse_subgraph_basic() {
        let q = p("SUBGRAPH ROOT a");
        let MoldQLQuery::Subgraph(sq) = q else {
            panic!("expected Subgraph")
        };
        assert_eq!(sq.root, "a");
        assert_eq!(sq.depth, 3);
        assert_eq!(sq.direction, TraversalDirection::Both);
    }

    #[test]
    fn parse_subgraph_with_where() {
        let q = p("SUBGRAPH ROOT a WHERE confidence >= 0.5");
        let MoldQLQuery::Subgraph(sq) = q else {
            panic!("expected Subgraph")
        };
        assert_eq!(sq.depth, 3);
        assert_eq!(sq.conditions.len(), 1);
    }

    #[test]
    fn parse_subgraph_custom_depth() {
        let q = p("SUBGRAPH ROOT a DEPTH 2");
        let MoldQLQuery::Subgraph(sq) = q else {
            panic!("expected Subgraph")
        };
        assert_eq!(sq.depth, 2);
    }

    // -- CLUSTER --
    #[test]
    fn parse_cluster_scc() {
        let q = p("CLUSTER METHOD scc");
        let MoldQLQuery::Cluster(cq) = q else {
            panic!("expected Cluster")
        };
        assert_eq!(cq.method, ClusterMethod::Scc);
    }

    #[test]
    fn parse_cluster_connected() {
        let q = p("CLUSTER METHOD connected");
        let MoldQLQuery::Cluster(cq) = q else {
            panic!("expected Cluster")
        };
        assert_eq!(cq.method, ClusterMethod::Connected);
    }

    #[test]
    fn parse_cluster_bare_default_scc() {
        let q = p("CLUSTER");
        let MoldQLQuery::Cluster(cq) = q else {
            panic!("expected Cluster")
        };
        assert_eq!(cq.method, ClusterMethod::Scc);
    }

    #[test]
    fn parse_cluster_with_where() {
        let q = p("CLUSTER WHERE cluster_id = 42");
        let MoldQLQuery::Cluster(cq) = q else {
            panic!("expected Cluster")
        };
        assert_eq!(cq.conditions.len(), 1);
    }

    // -- EXPLAIN --
    #[test]
    fn parse_explain_basic() {
        let q = p("EXPLAIN FROM a TO b");
        let MoldQLQuery::Explain(eq) = q else {
            panic!("expected Explain")
        };
        assert_eq!(eq.from, "a");
        assert_eq!(eq.to, "b");
    }

    #[test]
    fn parse_explain_rejects_max_hops() {
        let err = parse("EXPLAIN FROM a TO b MAX HOPS 3").unwrap_err();
        assert!(err.message.contains("MAX HOPS"));
    }

    #[test]
    fn parse_explain_with_where() {
        let q = p("EXPLAIN FROM a TO b WHERE confidence <= 0.8");
        let MoldQLQuery::Explain(eq) = q else {
            panic!("expected Explain")
        };
        assert_eq!(eq.conditions.len(), 1);
    }

    // -- WHERE filters (provenance + confidence) -------------------------

    #[test]
    fn where_filter_provenance_lsp() {
        let q = p("PATH FROM a TO b WHERE provenance.lsp = \"rust\"");
        let MoldQLQuery::Path(pq) = q else {
            panic!("expected Path")
        };
        let c = &pq.conditions[0];
        assert_eq!(
            c.field.parts,
            vec!["provenance".to_string(), "lsp".to_string()]
        );
    }

    #[test]
    fn where_filter_provenance_tree_sitter() {
        let q = p("PATH FROM a TO b WHERE provenance.tree_sitter = \"py\"");
        let MoldQLQuery::Path(pq) = q else {
            panic!("expected Path")
        };
        let c = &pq.conditions[0];
        assert_eq!(c.field.tail(), Some("tree_sitter"));
    }

    #[test]
    fn where_filter_confidence_lower_bound() {
        let q = p("PATH FROM a TO b WHERE confidence >= 0.7");
        let MoldQLQuery::Path(pq) = q else {
            panic!("expected Path")
        };
        let c = &pq.conditions[0];
        assert_eq!(c.field.parts, vec!["confidence".to_string()]);
        assert_eq!(c.op, Op::Gte);
        assert_eq!(c.value, Value::Number(0.7));
    }

    #[test]
    fn where_filter_confidence_upper_bound() {
        let q = p("PATH FROM a TO b WHERE confidence <= 0.3");
        let MoldQLQuery::Path(pq) = q else {
            panic!("expected Path")
        };
        assert_eq!(pq.conditions[0].op, Op::Lte);
    }

    #[test]
    fn where_filter_combined_provenance_and_confidence() {
        let q = p("PATH FROM a TO b WHERE provenance.lsp = \"rust\" AND confidence >= 0.5");
        let MoldQLQuery::Path(pq) = q else {
            panic!("expected Path")
        };
        assert_eq!(pq.conditions.len(), 2);
        assert_eq!(pq.conditions[0].field.head(), "provenance");
        assert_eq!(pq.conditions[1].field.parts, vec!["confidence".to_string()]);
    }

    // -- Boolean precedence snapshots ------------------------------------

    #[test]
    fn boolean_precedence_parens() {
        let q = p("PATH FROM a TO b AND (PATH FROM c TO d OR PATH FROM e TO f)");
        match q {
            MoldQLQuery::Boolean(b) => {
                assert_eq!(b.op, BooleanOp::And);
                assert_eq!(b.operands.len(), 2);
                // Right operand is the OR chain.
                match &b.operands[1] {
                    MoldQLQuery::Boolean(b1) => {
                        assert_eq!(b1.op, BooleanOp::Or);
                        assert_eq!(b1.operands.len(), 2);
                    }
                    other => panic!("expected inner Or, got {other:?}"),
                }
            }
            other => panic!("expected Boolean(And), got {other:?}"),
        }
    }

    // -- T20: multimodal FIND targets (parser layer) ---------------------
    //
    // RED gate: `FIND decisions` and `FIND docs` must parse into a
    // `MoldQLQuery::Find` with the new `TargetType` variants. The
    // compile + execute layers are covered in T21; the parser only
    // has to accept the keyword and produce a well-formed AST.
    //
    // These tests are gated behind the `multimodal` Cargo feature
    // because the variants themselves are cfg-gated on
    // `TargetType` in `ast.rs`.

    #[cfg(feature = "multimodal")]
    #[test]
    fn parse_target_decisions() {
        let q = p("FIND decisions");
        let MoldQLQuery::Find(fq) = q else {
            panic!("expected Find");
        };
        assert_eq!(fq.target, TargetType::Decisions);
        assert_eq!(fq.target.keyword(), "decisions");
    }

    #[cfg(feature = "multimodal")]
    #[test]
    fn parse_target_docs() {
        let q = p("FIND docs");
        let MoldQLQuery::Find(fq) = q else {
            panic!("expected Find");
        };
        assert_eq!(fq.target, TargetType::Docs);
        assert_eq!(fq.target.keyword(), "docs");
    }

    #[cfg(feature = "multimodal")]
    #[test]
    fn parse_target_decisions_with_where() {
        let q = p("FIND decisions WHERE status == accepted");
        let MoldQLQuery::Find(fq) = q else {
            panic!("expected Find");
        };
        assert_eq!(fq.target, TargetType::Decisions);
        assert_eq!(fq.conditions.len(), 1);
    }

    #[cfg(feature = "multimodal")]
    #[test]
    fn parse_target_decisions_uppercase() {
        let q = p("FIND DECISIONS");
        let MoldQLQuery::Find(fq) = q else {
            panic!("expected Find");
        };
        assert_eq!(fq.target, TargetType::Decisions);
    }

    #[cfg(feature = "multimodal")]
    #[test]
    fn parse_target_unknown_lists_all_six() {
        // The error message must list all 6 valid targets (4
        // code + 2 multimodal). The list keeps users from
        // guessing — they see the full surface on a typo.
        let err = parse("FIND widgets").unwrap_err();
        let m = err.message;
        for k in &["symbols", "files", "scopes", "issues", "decisions", "docs"] {
            assert!(m.contains(k), "error message should mention {k}, got: {m}");
        }
    }

    // -- T20 regression: the 4 legacy targets keep parsing identically.

    #[test]
    fn parse_target_legacy_targets_regression() {
        for (q, expected) in [
            ("FIND symbols", TargetType::Symbols),
            ("FIND files", TargetType::Files),
            ("FIND scopes", TargetType::Scopes),
            ("FIND issues", TargetType::Issues),
        ] {
            let parsed = p(q);
            let MoldQLQuery::Find(fq) = parsed else {
                panic!("expected Find for `{q}`, got something else");
            };
            assert_eq!(fq.target, expected, "wrong target for `{q}`");
        }
    }
}
