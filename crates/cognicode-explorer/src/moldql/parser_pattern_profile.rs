//! Recursive-descent parser for the Pattern Profile grammar.
//!
//! Part of e28-3-moldql-pattern-profile-v1: PR1 Foundation.
//!
//! ## Grammar
//!
//! ```text
//! PatternQuery  ::= [SHORTEST] MATCH NodePattern (EdgePattern NodePattern)* [WHERE Clause+ RETURN Projection]
//! NodePattern   ::= '(' [BindingName ':'] Kind ')'
//! BindingName   ::= IDENTIFIER
//! Kind          ::= IDENTIFIER
//! EdgePattern   ::= '[' [BindingName ':'] Kind [*Quantifier] ArrowDirection ']'
//! Quantifier    ::= '?' | '+' | '*' DECIMAL '..' DECIMAL | '*' DECIMAL
//! ArrowDirection::= '->' | '<-' | '<->'
//! Projection    ::= PATH '(' BindingName (',' BindingName)* ')'
//!               | NODE '(' BindingName ')'
//!               | EDGE '(' BindingName ')'
//!               | BindingName (',' BindingName)* (',' Aggregation)+ [ORDER BY BindingName (ASC|DESC)] [LIMIT INTEGER]
//! Aggregation  ::= COUNT '(' [BindingName] ')' AS IDENTIFIER
//! Clause       ::= IDENTIFIER '.' IDENTIFIER Op Value
//! Op           ::= '=' | '!=' | '>' | '>=' | '<' | '<='
//! Value        ::= STRING | DECIMAL
//! ```
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(unused_imports)]

use crate::moldql::ParseError;
use crate::moldql::ast::{
    Aggregation, Binding, EdgeDirection, EdgePattern, Field, MoldQLQuery, OrderClause,
    OrderDirection, PathQuantifier, PatternOp, PatternPredicate, PatternProjection, PatternQuery,
    PatternValue, PredicateTarget, Value as AstValue,
};
use crate::moldql::cursor::Cursor;

/// Parse a pattern query from a string.
pub fn parse_pattern_query_from_str(input: &str) -> Result<MoldQLQuery, ParseError> {
    let mut cursor = Cursor::new(input);
    cursor.skip_ws();
    let query = parse_pattern_query(&mut cursor)?;
    cursor.skip_ws();
    if !cursor.is_eof() {
        let (line, col) = cursor.position();
        return Err(ParseError::at(
            format!("unexpected trailing input: `{}`", cursor.remaining()),
            line,
            col,
        ));
    }
    Ok(MoldQLQuery::Pattern(query))
}

/// Parse a Pattern Query: `[SHORTEST] MATCH ...`
pub(crate) fn parse_pattern_query(cursor: &mut Cursor<'_>) -> Result<PatternQuery, ParseError> {
    cursor.skip_ws();

    // Optional SHORTEST keyword
    let shortest =
        if cursor.peek_keyword().map(|k| k.to_ascii_uppercase()) == Some("SHORTEST".into()) {
            cursor.consume_keyword("SHORTEST");
            cursor.skip_ws();
            true
        } else {
            false
        };

    // MATCH keyword
    let kw = cursor.peek_keyword().ok_or_else(|| {
        let (line, col) = cursor.position();
        ParseError::at("expected MATCH", line, col)
    })?;
    if !kw.eq_ignore_ascii_case("MATCH") {
        let (line, col) = cursor.position();
        return Err(ParseError::at(
            format!("expected MATCH, found `{kw}`"),
            line,
            col,
        ));
    }
    cursor.consume_keyword("MATCH");
    cursor.skip_ws();

    // At least one node pattern
    let first_node = parse_node_pattern(cursor)?;
    cursor.skip_ws();

    // Zero or more edge+node pairs
    let mut bindings = vec![first_node];
    let mut edges = Vec::new();

    while let Some(ch) = cursor.peek_char() {
        cursor.skip_ws();
        if ch == ')' || ch == 'W' || ch == 'w' || ch == 'R' || ch == 'r' {
            // End of pattern — no more edges
            break;
        }
        // Expect `-` (start of edge: `-[...]->`)
        if ch != '-' {
            let (line, col) = cursor.position();
            return Err(ParseError::at(
                format!("expected `-` to start edge, found `{ch}`"),
                line,
                col,
            ));
        }
        cursor.advance(); // consume '-'
        cursor.skip_ws();

        // Now expect `[` for edge pattern
        if cursor.peek_char() != Some('[') {
            let (line, col) = cursor.position();
            return Err(ParseError::at(
                "expected edge pattern `[...]` after `-`",
                line,
                col,
            ));
        }

        let mut edge = parse_edge_pattern(cursor)?;
        cursor.skip_ws();

        // After edge pattern, handle arrow direction: call parse_arrow to validate
        // Handles `->`, `<-`, `<->`, and errors on `>>` or other invalid arrows
        if cursor.peek_char() == Some('-') {
            match parse_arrow(cursor) {
                Ok(dir) => edge.direction = dir,
                Err(e) => return Err(e),
            }
        }
        cursor.skip_ws();

        let node = parse_node_pattern(cursor)?;
        cursor.skip_ws();

        edges.push(edge);
        bindings.push(node);
    }

    // Optional WHERE clause
    let predicates =
        if cursor.peek_keyword().map(|k| k.to_ascii_uppercase()) == Some("WHERE".into()) {
            cursor.consume_keyword("WHERE");
            cursor.skip_ws();
            parse_predicates(cursor)?
        } else {
            Vec::new()
        };

    // Optional RETURN clause
    let projection =
        if cursor.peek_keyword().map(|k| k.to_ascii_uppercase()) == Some("RETURN".into()) {
            cursor.consume_keyword("RETURN");
            cursor.skip_ws();
            parse_projection(cursor)?
        } else {
            // Default projection: PATH with all bindings
            PatternProjection::Path {
                bindings: bindings.iter().filter_map(|b| b.name.clone()).collect(),
            }
        };

    Ok(PatternQuery {
        shortest,
        bindings,
        edges,
        predicates,
        projection,
    })
}

/// Parse a node pattern: `(r:Route)` or `(:Route)`
fn parse_node_pattern(cursor: &mut Cursor<'_>) -> Result<Binding, ParseError> {
    cursor.skip_ws();

    // Expect '('
    if cursor.peek_char() != Some('(') {
        let (line, col) = cursor.position();
        return Err(ParseError::at(
            "expected `(` to open node pattern",
            line,
            col,
        ));
    }
    cursor.advance(); // consume '('
    cursor.skip_ws();

    // Optional name ':'
    let name = if cursor.peek_char() != Some(':') {
        // Check for bare identifier that might be a name before ':'
        let name = parse_identifier(cursor, "node name or kind")?;
        cursor.skip_ws();
        if cursor.peek_char() == Some(':') {
            cursor.advance(); // consume ':'
            cursor.skip_ws();
            let kind = parse_identifier(cursor, "node kind")?;
            cursor.skip_ws();
            // Expect ')'
            if cursor.peek_char() != Some(')') {
                let (line, col) = cursor.position();
                return Err(ParseError::at(
                    "expected `)` to close node pattern",
                    line,
                    col,
                ));
            }
            cursor.advance();
            Binding {
                name: Some(name),
                kind,
            }
        } else {
            // This is the kind only (anonymous binding)
            let kind = name;
            // Expect ')'
            if cursor.peek_char() != Some(')') {
                let (line, col) = cursor.position();
                return Err(ParseError::at(
                    "expected `)` to close node pattern",
                    line,
                    col,
                ));
            }
            cursor.advance();
            Binding { name: None, kind }
        }
    } else {
        // Anonymous node - starts with ':'
        cursor.advance(); // consume ':'
        cursor.skip_ws();
        let kind = parse_identifier(cursor, "node kind")?;
        cursor.skip_ws();
        if cursor.peek_char() != Some(')') {
            let (line, col) = cursor.position();
            return Err(ParseError::at(
                "expected `)` to close node pattern",
                line,
                col,
            ));
        }
        cursor.advance();
        Binding { name: None, kind }
    };

    Ok(name)
}

/// Parse an edge pattern: `[c:Calls*1..3->]` or `[->]`
fn parse_edge_pattern(cursor: &mut Cursor<'_>) -> Result<EdgePattern, ParseError> {
    cursor.skip_ws();

    // Expect '['
    if cursor.peek_char() != Some('[') {
        let (line, col) = cursor.position();
        return Err(ParseError::at(
            "expected `[` to open edge pattern",
            line,
            col,
        ));
    }
    cursor.advance(); // consume '['
    cursor.skip_ws();

    // Optional name ':'
    let (name, kind) = if cursor.peek_char() != Some(':') {
        let name = parse_identifier_or_empty(cursor, "edge name or kind")?;
        cursor.skip_ws();
        if cursor.peek_char() == Some(':') {
            cursor.advance(); // consume ':'
            cursor.skip_ws();
            let kind = parse_identifier(cursor, "edge kind")?;
            cursor.skip_ws();
            (Some(name), kind)
        } else {
            // Anonymous edge - just a kind
            (None, name)
        }
    } else {
        cursor.advance(); // consume ':'
        cursor.skip_ws();
        let kind = parse_identifier(cursor, "edge kind")?;
        cursor.skip_ws();
        (None, kind)
    };

    // Optional quantifier
    let quantifier = if cursor.peek_char() == Some('*')
        || cursor.peek_char() == Some('+')
        || cursor.peek_char() == Some('?')
    {
        parse_quantifier(cursor)?
    } else {
        // Default: exactly 1 hop
        PathQuantifier {
            max_hops: Some(1),
            min_hops: 1,
        }
    };

    // Direction: always undirected inside brackets; outer loop sets actual direction
    // from the arrow that follows the closing `]`
    cursor.skip_ws();

    // Expect ']'
    if cursor.peek_char() != Some(']') {
        let (line, col) = cursor.position();
        return Err(ParseError::at(
            "expected `]` to close edge pattern",
            line,
            col,
        ));
    }
    cursor.advance();

    Ok(EdgePattern {
        name,
        kind,
        quantifier,
        direction: EdgeDirection::Both,
    })
}

/// Parse quantifier: ?, +, *1..3, *3
fn parse_quantifier(cursor: &mut Cursor<'_>) -> Result<PathQuantifier, ParseError> {
    let ch = cursor.peek_char().ok_or_else(|| {
        let (line, col) = cursor.position();
        ParseError::at("expected quantifier (*, +, ?)", line, col)
    })?;

    match ch {
        '?' => {
            cursor.advance();
            Ok(PathQuantifier {
                max_hops: Some(1),
                min_hops: 0,
            })
        }
        '+' => {
            cursor.advance();
            // + → 1..DEFAULT_MAX_HOPS (8)
            Ok(PathQuantifier {
                max_hops: Some(8),
                min_hops: 1,
            })
        }
        '*' => {
            cursor.advance();
            cursor.skip_ws();
            // Check for *m..n or *n
            let start = cursor.index;
            while let Some(c) = cursor.peek_char() {
                if c.is_ascii_digit() {
                    cursor.advance();
                } else {
                    break;
                }
            }
            let captured = &cursor.input[start..cursor.index];
            cursor.skip_ws();

            if captured.is_empty() {
                // Unbounded '*' → reject
                let (line, col) = cursor.position();
                return Err(ParseError::at(
                    "Pattern Profile rejects unbounded paths; use *m..n with finite n",
                    line,
                    col,
                ));
            }

            let n: u32 = captured.parse().map_err(|_| {
                let (line, col) = cursor.position();
                ParseError::at(
                    format!("invalid number `{captured}` in quantifier"),
                    line,
                    col,
                )
            })?;

            cursor.skip_ws();
            if cursor.peek_char() == Some('.') {
                cursor.advance(); // consume '.'
                if cursor.peek_char() != Some('.') {
                    let (line, col) = cursor.position();
                    return Err(ParseError::at("expected `..` after *m", line, col));
                }
                cursor.advance(); // consume '.'
                cursor.skip_ws();

                // Parse min hops
                let start2 = cursor.index;
                while let Some(c) = cursor.peek_char() {
                    if c.is_ascii_digit() {
                        cursor.advance();
                    } else {
                        break;
                    }
                }
                let min_str = &cursor.input[start2..cursor.index];
                let min_hops: u32 = min_str.parse().unwrap_or(0);
                cursor.skip_ws();

                Ok(PathQuantifier {
                    min_hops: n,
                    max_hops: Some(min_hops),
                })
            } else {
                // Just *n → min=0, max=n
                Ok(PathQuantifier {
                    max_hops: Some(n),
                    min_hops: 0,
                })
            }
        }
        _ => {
            let (line, col) = cursor.position();
            Err(ParseError::at(
                format!("unexpected quantifier `{ch}`"),
                line,
                col,
            ))
        }
    }
}

/// Parse arrow direction: ->, <-, <->
fn parse_arrow(cursor: &mut Cursor<'_>) -> Result<EdgeDirection, ParseError> {
    cursor.skip_ws();
    let start = cursor.index;
    while let Some(c) = cursor.peek_char() {
        if matches!(c, '-' | '>' | '<') {
            cursor.advance();
        } else {
            break;
        }
    }
    let arrow = &cursor.input[start..cursor.index];
    match arrow {
        "->" => Ok(EdgeDirection::Outgoing),
        "<-" => Ok(EdgeDirection::Incoming),
        "<->" => Ok(EdgeDirection::Both),
        _ => {
            let (line, col) = cursor.position();
            Err(ParseError::at(
                format!("expected `->`, `<-`, or `<->`, found `{arrow}`"),
                line,
                col,
            ))
        }
    }
}

/// Parse RETURN projection
fn parse_projection(cursor: &mut Cursor<'_>) -> Result<PatternProjection, ParseError> {
    cursor.skip_ws();
    let kw = cursor.peek_keyword().ok_or_else(|| {
        let (line, col) = cursor.position();
        ParseError::at(
            "expected RETURN followed by PATH, NODE, EDGE, or field list",
            line,
            col,
        )
    })?;
    match kw.to_ascii_uppercase().as_str() {
        "PATH" => {
            cursor.consume_keyword("PATH");
            cursor.skip_ws();
            parse_parenthesized_list(cursor, |cursor| {
                parse_identifier(cursor, "binding name").map(Some)
            })
            .map(|bindings| PatternProjection::Path { bindings })
        }
        "NODE" => {
            cursor.consume_keyword("NODE");
            cursor.skip_ws();
            if cursor.peek_char() != Some('(') {
                let (line, col) = cursor.position();
                return Err(ParseError::at("expected `(` after NODE", line, col));
            }
            cursor.advance();
            cursor.skip_ws();
            let name = parse_identifier(cursor, "binding name")?;
            cursor.skip_ws();
            if cursor.peek_char() != Some(')') {
                let (line, col) = cursor.position();
                return Err(ParseError::at("expected `)` after binding name", line, col));
            }
            cursor.advance();
            Ok(PatternProjection::Node { binding: name })
        }
        "EDGE" => {
            cursor.consume_keyword("EDGE");
            cursor.skip_ws();
            if cursor.peek_char() != Some('(') {
                let (line, col) = cursor.position();
                return Err(ParseError::at("expected `(` after EDGE", line, col));
            }
            cursor.advance();
            cursor.skip_ws();
            let name = parse_identifier(cursor, "binding name")?;
            cursor.skip_ws();
            if cursor.peek_char() != Some(')') {
                let (line, col) = cursor.position();
                return Err(ParseError::at("expected `)` after binding name", line, col));
            }
            cursor.advance();
            Ok(PatternProjection::Edge { binding: name })
        }
        _ => {
            // Field list with optional aggregations
            parse_row_projection(cursor)
        }
    }
}

/// Parse row projection: f.module, COUNT(c) AS calls ORDER BY calls DESC LIMIT 5
fn parse_row_projection(cursor: &mut Cursor<'_>) -> Result<PatternProjection, ParseError> {
    cursor.skip_ws();
    let mut fields = Vec::new();
    let mut group_by = Vec::new();
    let mut aggregations = Vec::new();

    // Parse first field or aggregation
    loop {
        cursor.skip_ws();
        let ch = cursor.peek_char();
        if ch == Some(',') || ch == Some('O') || ch == Some('L') || ch.is_none() {
            break;
        }
        if ch == Some('C') {
            // Check for COUNT
            let _start = cursor.index;
            let kw = cursor.peek_keyword().unwrap_or_default();
            if kw.to_ascii_uppercase().starts_with("COUNT") {
                cursor.consume_keyword("COUNT");
                cursor.skip_ws();
                // Parse '('
                if cursor.peek_char() != Some('(') {
                    let (line, col) = cursor.position();
                    return Err(ParseError::at("expected `(` after COUNT", line, col));
                }
                cursor.advance();
                cursor.skip_ws();
                // Optional binding
                let binding = if cursor.peek_char() != Some(')') {
                    Some(parse_identifier(cursor, "edge or node name")?)
                } else {
                    None
                };
                cursor.skip_ws();
                if cursor.peek_char() != Some(')') {
                    let (line, col) = cursor.position();
                    return Err(ParseError::at(
                        "expected `)` after COUNT binding",
                        line,
                        col,
                    ));
                }
                cursor.advance();
                cursor.skip_ws();
                // AS alias
                cursor.consume_keyword("AS");
                cursor.skip_ws();
                let alias = parse_identifier(cursor, "aggregation alias")?;
                cursor.skip_ws();
                aggregations.push(Aggregation::Count { binding, alias });
            } else {
                break;
            }
        } else if ch == Some(',') {
            break;
        } else {
            // Field reference
            let name = parse_identifier(cursor, "field or binding name")?;
            cursor.skip_ws();
            if cursor.peek_char() == Some('.') {
                cursor.advance();
                cursor.skip_ws();
                let field = parse_identifier(cursor, "field name")?;
                fields.push(crate::moldql::ast::RowField::Property {
                    binding: name.clone(),
                    field: field.clone(),
                });
                group_by.push(format!("{}.{}", name, field));
            } else {
                fields.push(crate::moldql::ast::RowField::AggregationRef { name: name.clone() });
                group_by.push(name);
            }
        }

        if cursor.peek_char() == Some(',') {
            cursor.advance();
        } else {
            break;
        }
    }

    // Optional ORDER BY
    let ordering = if cursor.peek_keyword().map(|k| k.to_ascii_uppercase()) == Some("ORDER".into())
    {
        cursor.consume_keyword("ORDER");
        cursor.skip_ws();
        cursor.consume_keyword("BY");
        cursor.skip_ws();
        let by = parse_identifier(cursor, "order field")?;
        cursor.skip_ws();
        let direction =
            if cursor.peek_keyword().map(|k| k.to_ascii_uppercase()) == Some("DESC".into()) {
                cursor.consume_keyword("DESC");
                OrderDirection::Desc
            } else {
                if cursor.peek_keyword().map(|k| k.to_ascii_uppercase()) == Some("ASC".into()) {
                    cursor.consume_keyword("ASC");
                }
                OrderDirection::Asc
            };
        cursor.skip_ws();
        Some(OrderClause { by, direction })
    } else {
        None
    };

    // Optional LIMIT
    let limit = if cursor.peek_keyword().map(|k| k.to_ascii_uppercase()) == Some("LIMIT".into()) {
        cursor.consume_keyword("LIMIT");
        cursor.skip_ws();
        let n = parse_integer(cursor)?;
        Some(n as usize)
    } else {
        None
    };

    Ok(PatternProjection::Row {
        fields,
        group_by,
        aggregations,
        ordering,
        limit,
    })
}

/// Parse WHERE predicates
fn parse_predicates(cursor: &mut Cursor<'_>) -> Result<Vec<PatternPredicate>, ParseError> {
    let mut preds = Vec::new();
    loop {
        cursor.skip_ws();
        let pred = parse_predicate(cursor)?;
        preds.push(pred);
        cursor.skip_ws();
        // Check for AND to continue
        if cursor.peek_keyword().map(|k| k.to_ascii_uppercase()) == Some("AND".into()) {
            cursor.consume_keyword("AND");
            cursor.skip_ws();
        } else {
            break;
        }
    }
    Ok(preds)
}

/// Parse a single predicate: target.field Op Value
fn parse_predicate(cursor: &mut Cursor<'_>) -> Result<PatternPredicate, ParseError> {
    cursor.skip_ws();

    // Parse target: optional binding name followed by '.'
    let (target, field_name) = parse_predicate_target(cursor)?;

    cursor.skip_ws();
    let op = parse_pattern_op(cursor)?;
    cursor.skip_ws();
    let value = parse_pattern_value(cursor)?;

    if field_name == "provenance"
        && let PatternValue::String(ref s) = value
    {
        return Ok(PatternPredicate::Provenance {
            target: if let PredicateTarget::Node(n) = target {
                Some(n)
            } else {
                None
            },
            source: s.clone(),
        });
    }

    if field_name == "confidence"
        && let PatternValue::Number(n) = value
    {
        return Ok(PatternPredicate::Confidence {
            target,
            op,
            value: n,
        });
    }

    Ok(PatternPredicate::Property {
        target,
        field: field_name,
        op,
        value,
    })
}

/// Parse predicate target: [name.]field
fn parse_predicate_target(
    cursor: &mut Cursor<'_>,
) -> Result<(PredicateTarget, String), ParseError> {
    cursor.skip_ws();
    let start = cursor.index;
    while let Some(c) = cursor.peek_char() {
        if c.is_alphanumeric() || c == '_' {
            cursor.advance();
        } else {
            break;
        }
    }
    let first = &cursor.input[start..cursor.index];
    cursor.skip_ws();

    if cursor.peek_char() == Some('.') {
        cursor.advance();
        cursor.skip_ws();
        let field = parse_identifier(cursor, "field name")?;
        Ok((PredicateTarget::Node(first.into()), field))
    } else {
        // Just a field name with anonymous target
        Ok((PredicateTarget::Anonymous, first.into()))
    }
}

/// Parse comparison operator
fn parse_pattern_op(cursor: &mut Cursor<'_>) -> Result<PatternOp, ParseError> {
    let c0 = cursor.peek_char().ok_or_else(|| {
        let (line, col) = cursor.position();
        ParseError::at("expected comparison operator", line, col)
    })?;
    let c1 = cursor.peek_char_at(1);
    match (c0, c1) {
        ('>', Some('=')) => {
            cursor.advance_by(2);
            Ok(PatternOp::Gte)
        }
        ('<', Some('=')) => {
            cursor.advance_by(2);
            Ok(PatternOp::Lte)
        }
        ('=', Some('=')) => {
            cursor.advance_by(2);
            Ok(PatternOp::Eq)
        }
        ('=', _) => {
            cursor.advance();
            Ok(PatternOp::Eq)
        }
        ('!', Some('=')) => {
            cursor.advance_by(2);
            Ok(PatternOp::Neq)
        }
        ('>', _) => {
            cursor.advance();
            Ok(PatternOp::Gt)
        }
        ('<', _) => {
            cursor.advance();
            Ok(PatternOp::Lt)
        }
        _ => {
            let (line, col) = cursor.position();
            Err(ParseError::at(
                "expected one of `>`, `>=`, `<`, `<=`, `=`, `==`, `!=`",
                line,
                col,
            ))
        }
    }
}

/// Parse a pattern value: string or number
fn parse_pattern_value(cursor: &mut Cursor<'_>) -> Result<PatternValue, ParseError> {
    cursor.skip_ws();
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
            let (line, col) = cursor.position();
            return Err(ParseError::at("unterminated string", line, col));
        }
        let s = cursor.input[start..cursor.index].to_string();
        cursor.advance();
        Ok(PatternValue::String(s))
    } else {
        let start = cursor.index;
        while let Some(c) = cursor.peek_char() {
            if c.is_ascii_digit() || c == '.' {
                cursor.advance();
            } else {
                break;
            }
        }
        let num_str = &cursor.input[start..cursor.index];
        if num_str.is_empty() {
            let (line, col) = cursor.position();
            return Err(ParseError::at("expected number or string value", line, col));
        }
        let n: f64 = num_str.parse().map_err(|_| {
            let (line, col) = cursor.position();
            ParseError::at(format!("invalid number `{num_str}`"), line, col)
        })?;
        Ok(PatternValue::Number(n))
    }
}

/// Parse an identifier
fn parse_identifier(cursor: &mut Cursor<'_>, what: &str) -> Result<String, ParseError> {
    let start = cursor.index;
    while let Some(c) = cursor.peek_char() {
        if c.is_alphanumeric() || c == '_' {
            cursor.advance();
        } else {
            break;
        }
    }
    if cursor.index == start {
        let (line, col) = cursor.position();
        return Err(ParseError::at(format!("expected {what}"), line, col));
    }
    Ok(cursor.input[start..cursor.index].to_string())
}

/// Parse an identifier but allow empty (for anonymous edges)
fn parse_identifier_or_empty(cursor: &mut Cursor<'_>, _what: &str) -> Result<String, ParseError> {
    let start = cursor.index;
    while let Some(c) = cursor.peek_char() {
        if c.is_alphanumeric() || c == '_' {
            cursor.advance();
        } else {
            break;
        }
    }
    Ok(cursor.input[start..cursor.index].to_string())
}

/// Parse a comma-separated list inside parentheses
fn parse_parenthesized_list<T>(
    cursor: &mut Cursor<'_>,
    item_parser: impl Fn(&mut Cursor<'_>) -> Result<Option<T>, ParseError>,
) -> Result<Vec<T>, ParseError> {
    cursor.skip_ws();
    if cursor.peek_char() != Some('(') {
        let (line, col) = cursor.position();
        return Err(ParseError::at("expected `(`", line, col));
    }
    cursor.advance();
    cursor.skip_ws();

    let mut items = Vec::new();
    if cursor.peek_char() != Some(')') {
        loop {
            cursor.skip_ws();
            if cursor.peek_char() == Some(')') {
                break;
            }
            if let Some(item) = item_parser(cursor)? {
                items.push(item);
            }
            cursor.skip_ws();
            if cursor.peek_char() == Some(',') {
                cursor.advance();
            } else {
                break;
            }
        }
    }

    cursor.skip_ws();
    if cursor.peek_char() != Some(')') {
        let (line, col) = cursor.position();
        return Err(ParseError::at("expected `)` to close list", line, col));
    }
    cursor.advance();
    Ok(items)
}

/// Parse an integer value
fn parse_integer(cursor: &mut Cursor<'_>) -> Result<u32, ParseError> {
    let start = cursor.index;
    while let Some(c) = cursor.peek_char() {
        if c.is_ascii_digit() {
            cursor.advance();
        } else {
            break;
        }
    }
    let s = &cursor.input[start..cursor.index];
    if s.is_empty() {
        let (line, col) = cursor.position();
        return Err(ParseError::at("expected integer", line, col));
    }
    s.parse::<u32>().map_err(|_| {
        let (line, col) = cursor.position();
        ParseError::at(format!("invalid integer `{s}`"), line, col)
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moldql::ast::{EdgeDirection, PatternProjection};

    fn parse_ok(input: &str) -> MoldQLQuery {
        parse_pattern_query_from_str(input)
            .unwrap_or_else(|e| panic!("expected Ok for `{input}`, got {e}"))
    }

    fn parse_err(input: &str) -> ParseError {
        parse_pattern_query_from_str(input)
            .err()
            .unwrap_or_else(|| panic!("expected Err for `{input}`, got Ok"))
    }

    // === Happy paths ===

    #[test]
    fn basic_match() {
        let q = parse_ok("MATCH (r:Route) RETURN PATH(r)");
        let MoldQLQuery::Pattern(pq) = q else {
            panic!("expected Pattern")
        };
        assert!(!pq.shortest);
        assert_eq!(pq.bindings.len(), 1);
        assert_eq!(pq.bindings[0].name.as_deref(), Some("r"));
        assert_eq!(pq.bindings[0].kind, "Route");
    }

    #[test]
    fn match_with_edge() {
        let q = parse_ok("MATCH (r:Route)-[:Calls*1..3]->(f:Function) RETURN PATH(r,f)");
        let MoldQLQuery::Pattern(pq) = q else {
            panic!("expected Pattern")
        };
        assert_eq!(pq.bindings.len(), 2);
        assert_eq!(pq.edges.len(), 1);
        assert_eq!(pq.edges[0].kind, "Calls");
        assert_eq!(pq.edges[0].direction, EdgeDirection::Outgoing);
        assert_eq!(pq.edges[0].quantifier.max_hops, Some(3));
    }

    #[test]
    fn shortest_match() {
        let q = parse_ok("SHORTEST MATCH (a:Route)-[:Calls*1..6]->(b:Function) RETURN PATH(a,b)");
        let MoldQLQuery::Pattern(pq) = q else {
            panic!("expected Pattern")
        };
        assert!(pq.shortest);
    }

    #[test]
    fn optional_quantifier() {
        let q = parse_ok("MATCH (f:Function)-[:Calls?]->(x:Function) RETURN PATH(f,x)");
        let MoldQLQuery::Pattern(pq) = q else {
            panic!("expected Pattern")
        };
        assert_eq!(pq.edges[0].quantifier.max_hops, Some(1));
        assert_eq!(pq.edges[0].quantifier.min_hops, 0);
    }

    #[test]
    fn anonymous_nodes() {
        let q = parse_ok("MATCH (:Route)-[:Calls]->(:Function) RETURN PATH()");
        let MoldQLQuery::Pattern(pq) = q else {
            panic!("expected Pattern")
        };
        assert!(pq.bindings[0].name.is_none());
        assert!(pq.bindings[1].name.is_none());
    }

    #[test]
    fn node_projection() {
        let q = parse_ok("MATCH (f:Function) RETURN NODE(f)");
        let MoldQLQuery::Pattern(pq) = q else {
            panic!("expected Pattern")
        };
        assert!(matches!(pq.projection, PatternProjection::Node { .. }));
    }

    #[test]
    fn edge_projection() {
        let q = parse_ok("MATCH (a)-[c:Calls]->(b) RETURN EDGE(c)");
        let MoldQLQuery::Pattern(pq) = q else {
            panic!("expected Pattern")
        };
        assert!(matches!(pq.projection, PatternProjection::Edge { .. }));
    }

    #[test]
    fn row_projection_with_ordering() {
        let q = parse_ok(
            "MATCH (f:Function)-[c:Calls]->(g) RETURN COUNT(c) AS calls ORDER BY calls DESC LIMIT 5",
        );
        let MoldQLQuery::Pattern(pq) = q else {
            panic!("expected Pattern")
        };
        assert!(matches!(pq.projection, PatternProjection::Row { .. }));
        if let PatternProjection::Row {
            ordering, limit, ..
        } = &pq.projection
        {
            assert!(ordering.is_some());
            assert_eq!(*limit, Some(5));
        }
    }

    // === Error cases ===

    #[test]
    fn unbounded_path_rejected() {
        let e = parse_err("MATCH (a:Function)-[:Calls*]->(b:Function) RETURN b");
        assert!(e.message.contains("unbounded"), "got: {}", e.message);
    }

    #[test]
    fn missing_closing_paren() {
        let e = parse_err("MATCH (r:Route RETURN PATH(r)");
        assert!(e.message.contains(")"), "got: {}", e.message);
    }

    #[test]
    fn missing_closing_bracket() {
        let e = parse_err("MATCH (r:Route)-[c:Calls RETURN PATH(r,c)");
        assert!(e.message.contains("]"), "got: {}", e.message);
    }

    #[test]
    fn empty_query_errors() {
        let e = parse_err("");
        assert!(e.message.contains("MATCH"), "got: {}", e.message);
    }

    #[test]
    fn unknown_arrow_errors() {
        let e = parse_err("MATCH (a)-[b]->>(b) RETURN PATH(a,b)");
        assert!(e.message.contains("->>"), "got: {}", e.message);
    }
}
