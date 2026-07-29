//! MoldQL AST — pure data types, no parsing or execution logic.
//!
//! MoldQL is the query language for the explorer: a single call that
//! combines filter, scope, and lens into one expression. The AST is the
//! canonical, in-memory representation a parsed query settles into.
//!
//! ExplorerQL extends this with 5 graph-native primitives
//! ([`MoldQLQuery::Path`], [`MoldQLQuery::Neighbors`],
//! [`MoldQLQuery::Subgraph`], [`MoldQLQuery::Cluster`],
//! [`MoldQLQuery::Explain`]) plus a boolean composition wrapper
//! ([`MoldQLQuery::Boolean`]). The original FIND/EXPLORE variants are
//! untouched — ExplorerQL is a strict superset of MoldQL.

/// The body of a `FIND` query.
#[derive(Debug, Clone, PartialEq)]
pub struct FindQuery {
    pub target: TargetType,
    /// Optional `IN SCOPE <path>` filter. `None` means "no scope restriction".
    pub scope: Option<String>,
    /// `WHERE` conditions. AND-chained — all must pass.
    pub conditions: Vec<Condition>,
    /// Optional `APPLY <lens>` clause.
    pub apply_lens: Option<String>,
}

/// What kind of objects the `FIND` clause returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetType {
    Symbols,
    Files,
    Scopes,
    Issues,
    /// Multimodal (T20) — `FIND decisions` returns [`NodeKind::Decision`]
    /// nodes from the generic graph. Requires the `multimodal` Cargo
    /// feature at the executor layer; the parser accepts the keyword
    /// regardless so users see a uniform surface.
    #[cfg(feature = "multimodal")]
    Decisions,
    /// Multimodal (T20) — `FIND docs` returns [`NodeKind::Doc`] nodes.
    #[cfg(feature = "multimodal")]
    Docs,
}

impl TargetType {
    /// Canonical lowercase form used in queries: `symbols`, `files`, etc.
    pub fn keyword(&self) -> &'static str {
        match self {
            Self::Symbols => "symbols",
            Self::Files => "files",
            Self::Scopes => "scopes",
            Self::Issues => "issues",
            #[cfg(feature = "multimodal")]
            Self::Decisions => "decisions",
            #[cfg(feature = "multimodal")]
            Self::Docs => "docs",
        }
    }
}

/// A single `WHERE` clause predicate.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub field: Field,
    pub op: Op,
    pub value: Value,
}

/// A dotted field reference. `["fan_in"]` for plain fields, `["quality",
/// "critical"]` for nested ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub parts: Vec<String>,
}

impl Field {
    /// Single-part field. `fan_in` → `Field { parts: ["fan_in"] }`.
    pub fn single(part: impl Into<String>) -> Self {
        Self {
            parts: vec![part.into()],
        }
    }

    /// Two-part dotted field. `quality.critical` → `Field { parts:
    /// ["quality", "critical"] }`.
    pub fn dotted(a: impl Into<String>, b: impl Into<String>) -> Self {
        Self {
            parts: vec![a.into(), b.into()],
        }
    }

    /// The first segment. For `quality.critical` → `"quality"`.
    pub fn head(&self) -> &str {
        self.parts.first().map(String::as_str).unwrap_or("")
    }

    /// The second segment, if any. For `fan_in` → `None`; for
    /// `quality.critical` → `Some("critical")`.
    pub fn tail(&self) -> Option<&str> {
        self.parts.get(1).map(String::as_str)
    }
}

/// Comparison operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Neq,
    /// Substring / contains — only meaningful for string-valued fields.
    Contains,
}

impl Op {
    /// Wire form: `>`, `>=`, `<`, `<=`, `==`, `!=`, `~`.
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Gt => ">",
            Self::Gte => ">=",
            Self::Lt => "<",
            Self::Lte => "<=",
            Self::Eq => "==",
            Self::Neq => "!=",
            Self::Contains => "~",
        }
    }
}

/// Right-hand side of a condition.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    String(String),
}

/// The body of an `EXPLORE` query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreQuery {
    /// MVP id of the seed object (e.g. `symbol:src/main.rs:main:1`).
    pub object_ref: String,
    pub direction: Direction,
    /// Maximum BFS depth. Executor caps this at 5.
    pub depth: u32,
}

/// Which side of the call graph to walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Direction {
    Callers,
    Callees,
}

impl Direction {
    /// Wire form: `callers`, `callees`.
    pub fn keyword(&self) -> &'static str {
        match self {
            Self::Callers => "callers",
            Self::Callees => "callees",
        }
    }
}

// ============================================================================
// ExplorerQL extensions — added per `sdd/explorerql-grammar`.
//
// All new types are STRICTLY ADDITIVE. The original 32 FIND/EXPLORE tests
// are unaffected because the existing variants are untouched.
// ============================================================================

/// Direction of graph traversal for ExplorerQL primitives. Distinct from
/// the legacy [`Direction`] enum (which is `callers`/`callees` for EXPLORE)
/// so the two enums cannot accidentally pattern-match the wrong value.
///
/// `Incoming` is reverse (callers), `Outgoing` is forward (callees), `Both`
/// walks both directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalDirection {
    Incoming,
    Outgoing,
    Both,
}

impl TraversalDirection {
    /// Wire form: `incoming`, `outgoing`, `both`.
    pub fn keyword(&self) -> &'static str {
        match self {
            Self::Incoming => "incoming",
            Self::Outgoing => "outgoing",
            Self::Both => "both",
        }
    }
}

/// The body of a `PATH` query.
///
/// `PATH FROM <from> TO <to> [MAX HOPS <n>] [WHERE ...]`. `max_hops` is
/// `None` when omitted (no upper bound on the BFS).
#[derive(Debug, Clone, PartialEq)]
pub struct PathQuery {
    pub from: String,
    pub to: String,
    pub max_hops: Option<u32>,
    pub conditions: Vec<Condition>,
}

/// The body of a `NEIGHBORS` query.
///
/// `NEIGHBORS <root> DEPTH <n> [DIRECTION <d>] [WHERE ...]`.
#[derive(Debug, Clone, PartialEq)]
pub struct NeighborsQuery {
    pub root: String,
    pub depth: u32,
    pub direction: TraversalDirection,
    pub conditions: Vec<Condition>,
}

/// The body of a `SUBGRAPH` query.
///
/// `SUBGRAPH ROOT <root> [DEPTH <n>] [DIRECTION <d>] [WHERE ...]`.
/// Defaults: `depth = 3`, `direction = Both`.
#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphQuery {
    pub root: String,
    pub depth: u32,
    pub direction: TraversalDirection,
    pub conditions: Vec<Condition>,
}

/// How [`ClusterQuery`] partitions the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterMethod {
    /// Strongly connected components.
    Scc,
    /// Weakly connected components.
    Connected,
}

impl ClusterMethod {
    /// Wire form: `scc`, `connected`.
    pub fn keyword(&self) -> &'static str {
        match self {
            Self::Scc => "scc",
            Self::Connected => "connected",
        }
    }
}

/// The body of a `CLUSTER` query.
///
/// `CLUSTER [METHOD (scc|connected)] [WHERE ...]`. Bare `CLUSTER` is legal.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterQuery {
    pub method: ClusterMethod,
    pub conditions: Vec<Condition>,
}

/// The body of an `EXPLAIN` query.
///
/// `EXPLAIN FROM <from> TO <to> [WHERE ...]`. Rejects `MAX HOPS` — the
/// spec mandates exact path-finding, not BFS.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplainQuery {
    pub from: String,
    pub to: String,
    pub conditions: Vec<Condition>,
}

/// Top-level boolean composition. `NOT` wraps a single sub-query; `AND`
/// and `OR` join a non-empty list of sub-queries. Filters on the
/// sub-queries stay scoped to those sub-queries (no bleed).
#[derive(Debug, Clone, PartialEq)]
pub struct BooleanQuery {
    pub op: BooleanOp,
    /// For `NOT` this holds exactly one sub-query; for `AND` / `OR` it
    /// holds 2+ sub-queries.
    pub operands: Vec<MoldQLQuery>,
}

/// Boolean operator joining the operands of a [`BooleanQuery`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    And,
    Or,
    Not,
}

impl BooleanOp {
    /// Wire form: `AND`, `OR`, `NOT`.
    pub fn keyword(&self) -> &'static str {
        match self {
            Self::And => "AND",
            Self::Or => "OR",
            Self::Not => "NOT",
        }
    }
}

// ============================================================================
// Pattern Profile — ADR-014 §2 — typed directed bounded patterns
// ============================================================================

/// A node binding inside a Pattern Profile pattern.
///
/// `r` in `(r:Route)` → `Binding { name: Some("r"), kind: Route }`.
/// Anonymous `( :Route)` → `Binding { name: None, kind: Route }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// Optional name for this binding (used in RETURN projections).
    pub name: Option<String>,
    /// The node kind / label (e.g. `Route`, `Function`).
    pub kind: String,
}

/// Direction of an edge pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDirection {
    /// `—>` or `←` (outgoing from source node).
    Outgoing,
    /// `<—` (incoming to source node).
    Incoming,
    /// `<—>` (both directions).
    Both,
}

/// A directed edge pattern between two bindings.
///
/// `-[c:Calls*1..3]->` → `EdgePattern { name: Some("c"), kind: Calls, quantifier: { max: 3, min: 1 }, direction: Outgoing }`.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgePattern {
    /// Optional name for this edge (used in RETURN projections).
    pub name: Option<String>,
    /// The dependency type (e.g. `Calls`, `Imports`).
    pub kind: String,
    /// Path quantifier: `*1..3`, `+`, `?`. Always bounded.
    pub quantifier: PathQuantifier,
    /// Edge direction.
    pub direction: EdgeDirection,
}

/// Path quantifier for Pattern Profile — always bounded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathQuantifier {
    pub max_hops: Option<u32>,
    pub min_hops: u32,
}

impl PathQuantifier {
    /// Construct a bounded quantifier. Returns `None` if `max_hops` is `None`.
    pub fn new(max_hops: Option<u32>, min_hops: u32) -> Option<Self> {
        if max_hops.is_none() {
            return None;
        }
        Some(Self { max_hops, min_hops })
    }

    /// `?` → `0..1`.
    pub fn optional() -> Self {
        Self { max_hops: Some(1), min_hops: 0 }
    }

    /// `+` with profile maximum.
    pub fn plus(max_hops: u32) -> Self {
        Self { max_hops: Some(max_hops), min_hops: 1 }
    }
}

/// What a RETURN clause projects.
#[derive(Debug, Clone, PartialEq)]
pub enum PatternProjection {
    /// `RETURN PATH(r, c, f)` — return the path with bindings intact.
    Path { bindings: Vec<String> },
    /// `RETURN node(f)` — return a single node.
    Node { binding: String },
    /// `RETURN edge(c)` — return a single edge.
    Edge { binding: String },
    /// `RETURN f.module, COUNT(c) AS calls ORDER BY calls DESC LIMIT 5` — typed rows.
    Row {
        fields: Vec<RowField>,
        group_by: Vec<String>,
        aggregations: Vec<Aggregation>,
        ordering: Option<OrderClause>,
        limit: Option<usize>,
    },
}

/// A field in a ROW projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowField {
    /// `f.module` — property reference.
    Property { binding: String, field: String },
    /// `COUNT(c) AS calls` — aggregation reference by alias.
    AggregationRef { name: String },
}

/// An aggregation function applied in a ROW projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aggregation {
    Count { binding: Option<String>, alias: String },
}

/// Ordering direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDirection {
    Asc,
    Desc,
}

/// `ORDER BY <field> <direction>` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderClause {
    pub by: String,
    pub direction: OrderDirection,
}

/// A parsed Pattern Profile query.
#[derive(Debug, Clone, PartialEq)]
pub struct PatternQuery {
    /// `SHORTEST` modifier.
    pub shortest: bool,
    /// Node bindings in order (first = anchor/src, last = target/dst).
    pub bindings: Vec<Binding>,
    /// Edge patterns between bindings.
    pub edges: Vec<EdgePattern>,
    /// Predicates over nodes and edges.
    pub predicates: Vec<PatternPredicate>,
    /// What to return.
    pub projection: PatternProjection,
}

/// A predicate over a node or edge in a pattern.
#[derive(Debug, Clone, PartialEq)]
pub enum PatternPredicate {
    /// `n.module = "core"` — typed field/value comparison.
    Property {
        target: PredicateTarget,
        field: String,
        op: PatternOp,
        value: PatternValue,
    },
    /// `e.provenance = "tree_sitter"` — provenance filter.
    Provenance { target: Option<String>, source: String },
    /// `confidence >= 0.7` — confidence filter (0..=1).
    Confidence { target: PredicateTarget, op: PatternOp, value: f64 },
}

/// The target of a predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PredicateTarget {
    Node(String),
    Edge(String),
    Anonymous,
}

/// Comparison operator in a pattern predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternOp {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Neq,
}

/// Value in a pattern predicate.
#[derive(Debug, Clone, PartialEq)]
pub enum PatternValue {
    String(String),
    Number(f64),
}

// Extend the top-level enum with the 5 ExplorerQL primitives plus the
// boolean composition wrapper and the new Pattern Profile variant.
// Each variant carries its respective query struct so the executor can
// pattern-match on it.

/// Top-level query variants.
#[derive(Debug, Clone, PartialEq)]
pub enum MoldQLQuery {
    /// `FIND <target> [IN SCOPE <path>] [WHERE ...] [APPLY <lens>]`
    Find(FindQuery),
    /// `EXPLORE <object_ref> THROUGH <direction> DEPTH <n>`
    Explore(ExploreQuery),
    /// `PATH FROM <from> TO <to> [MAX HOPS <n>] [WHERE ...]`
    Path(PathQuery),
    /// `NEIGHBORS <root> DEPTH <n> [DIRECTION <d>] [WHERE ...]`
    Neighbors(NeighborsQuery),
    /// `SUBGRAPH ROOT <root> [DEPTH <n>] [DIRECTION <d>] [WHERE ...]`
    Subgraph(SubgraphQuery),
    /// `CLUSTER [METHOD (scc|connected)] [WHERE ...]`
    Cluster(ClusterQuery),
    /// `EXPLAIN FROM <from> TO <to> [WHERE ...]`
    Explain(ExplainQuery),
    /// `( <q1> AND|OR <q2> [AND|OR <q3> ...] )` or `NOT <q>`
    Boolean(BooleanQuery),
    /// Pattern Profile — `MATCH <pattern> [WHERE ...] RETURN ...`
    Pattern(PatternQuery),
}

// ============================================================================
// Tests — roundtrip + Debug+Clone+PartialEq + behavior coverage for the 5
// new ExplorerQL variants and the boolean wrapper.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// All 6 new variants must be `Debug + Clone + PartialEq` and pattern-
    /// matchable through `MoldQLQuery`. This is the roundtrip gate.
    #[test]
    fn query_variants_roundtrip() {
        let path = MoldQLQuery::Path(PathQuery {
            from: "a".to_string(),
            to: "b".to_string(),
            max_hops: Some(3),
            conditions: Vec::new(),
        });
        let neighbors = MoldQLQuery::Neighbors(NeighborsQuery {
            root: "a".to_string(),
            depth: 2,
            direction: TraversalDirection::Both,
            conditions: Vec::new(),
        });
        let subgraph = MoldQLQuery::Subgraph(SubgraphQuery {
            root: "a".to_string(),
            depth: 3,
            direction: TraversalDirection::Both,
            conditions: Vec::new(),
        });
        let cluster = MoldQLQuery::Cluster(ClusterQuery {
            method: ClusterMethod::Scc,
            conditions: Vec::new(),
        });
        let explain = MoldQLQuery::Explain(ExplainQuery {
            from: "a".to_string(),
            to: "b".to_string(),
            conditions: Vec::new(),
        });
        let boolean = MoldQLQuery::Boolean(BooleanQuery {
            op: BooleanOp::And,
            operands: vec![path.clone(), neighbors.clone()],
        });

        // Debug + Clone + PartialEq
        for q in [
            path.clone(),
            neighbors.clone(),
            subgraph.clone(),
            cluster.clone(),
            explain.clone(),
            boolean.clone(),
        ] {
            let _ = format!("{q:?}");
            let cloned = q.clone();
            assert_eq!(q, cloned);
        }

        // Pattern-matchable
        assert!(matches!(path, MoldQLQuery::Path(_)));
        assert!(matches!(neighbors, MoldQLQuery::Neighbors(_)));
        assert!(matches!(subgraph, MoldQLQuery::Subgraph(_)));
        assert!(matches!(cluster, MoldQLQuery::Cluster(_)));
        assert!(matches!(explain, MoldQLQuery::Explain(_)));
        assert!(matches!(boolean, MoldQLQuery::Boolean(_)));
    }

    #[test]
    fn traversal_direction_keyword() {
        assert_eq!(TraversalDirection::Incoming.keyword(), "incoming");
        assert_eq!(TraversalDirection::Outgoing.keyword(), "outgoing");
        assert_eq!(TraversalDirection::Both.keyword(), "both");
    }

    #[test]
    fn cluster_method_keyword() {
        assert_eq!(ClusterMethod::Scc.keyword(), "scc");
        assert_eq!(ClusterMethod::Connected.keyword(), "connected");
    }

    #[test]
    fn boolean_op_keyword() {
        assert_eq!(BooleanOp::And.keyword(), "AND");
        assert_eq!(BooleanOp::Or.keyword(), "OR");
        assert_eq!(BooleanOp::Not.keyword(), "NOT");
    }

    /// `PathQuery` defaults: `max_hops` and `conditions` default to `None`
    /// / empty when not provided.
    #[test]
    fn path_query_default_max_hops_is_none() {
        let q = PathQuery {
            from: "a".into(),
            to: "b".into(),
            max_hops: None,
            conditions: Vec::new(),
        };
        assert!(q.max_hops.is_none());
        assert!(q.conditions.is_empty());
    }

    /// `SubgraphQuery` defaults: `depth = 3`, `direction = Both`.
    #[test]
    fn subgraph_query_defaults() {
        let q = SubgraphQuery {
            root: "a".into(),
            depth: 3,
            direction: TraversalDirection::Both,
            conditions: Vec::new(),
        };
        assert_eq!(q.depth, 3);
        assert_eq!(q.direction, TraversalDirection::Both);
    }

    /// `ClusterQuery` defaults: `method = Scc`, empty WHERE.
    #[test]
    fn cluster_query_defaults() {
        let q = ClusterQuery {
            method: ClusterMethod::Scc,
            conditions: Vec::new(),
        };
        assert_eq!(q.method, ClusterMethod::Scc);
        assert!(q.conditions.is_empty());
    }

    /// `BooleanQuery` keeps operands as a `Vec<MoldQLQuery>` so nested
    /// composition works (`(A AND B) OR C` is representable).
    #[test]
    fn boolean_query_keeps_nested_operands() {
        let inner = MoldQLQuery::Path(PathQuery {
            from: "a".into(),
            to: "b".into(),
            max_hops: None,
            conditions: Vec::new(),
        });
        let q = BooleanQuery {
            op: BooleanOp::Or,
            operands: vec![
                MoldQLQuery::Boolean(BooleanQuery {
                    op: BooleanOp::And,
                    operands: vec![inner.clone(), inner.clone()],
                }),
                inner,
            ],
        };
        assert_eq!(q.op, BooleanOp::Or);
        assert_eq!(q.operands.len(), 2);
        assert!(matches!(
            q.operands[0],
            MoldQLQuery::Boolean(BooleanQuery {
                op: BooleanOp::And,
                ..
            })
        ));
    }

    /// `NOT` wraps exactly one operand.
    #[test]
    fn boolean_query_not_holds_single_operand() {
        let inner = MoldQLQuery::Path(PathQuery {
            from: "a".into(),
            to: "b".into(),
            max_hops: None,
            conditions: Vec::new(),
        });
        let q = BooleanQuery {
            op: BooleanOp::Not,
            operands: vec![inner],
        };
        assert_eq!(q.op, BooleanOp::Not);
        assert_eq!(q.operands.len(), 1);
    }

    // =========================================================================
    // Pattern Profile AST — Debug + Clone + PartialEq + behaviour
    // =========================================================================

    /// `PathQuantifier::new(Some(n), m)` returns a bounded quantifier.
    #[test]
    fn pattern_path_quantifier_bounded() {
        let q = PathQuantifier::new(Some(3), 1).unwrap();
        assert_eq!(q.max_hops, Some(3));
        assert_eq!(q.min_hops, 1);
    }

    /// `PathQuantifier::new(None, _)` returns `None` (unbounded rejected).
    #[test]
    fn pattern_path_quantifier_unbounded_rejected() {
        let q = PathQuantifier::new(None, 0);
        assert!(q.is_none());
    }

    /// `PathQuantifier::optional()` → `0..1`.
    #[test]
    fn pattern_path_quantifier_optional() {
        let q = PathQuantifier::optional();
        assert_eq!(q.max_hops, Some(1));
        assert_eq!(q.min_hops, 0);
    }

    /// `PathQuantifier::plus(max)` → `1..max`.
    #[test]
    fn pattern_path_quantifier_plus() {
        let q = PathQuantifier::plus(4);
        assert_eq!(q.max_hops, Some(4));
        assert_eq!(q.min_hops, 1);
    }

    /// `PatternQuery` round-trips through Debug + Clone + PartialEq.
    #[test]
    fn pattern_query_roundtrip() {
        let q = PatternQuery {
            shortest: true,
            bindings: vec![
                Binding { name: Some("r".into()), kind: "Route".into() },
                Binding { name: Some("f".into()), kind: "Function".into() },
            ],
            edges: vec![EdgePattern {
                name: Some("c".into()),
                kind: "Calls".into(),
                quantifier: PathQuantifier::new(Some(3), 1).unwrap(),
                direction: EdgeDirection::Outgoing,
            }],
            predicates: vec![],
            projection: PatternProjection::Path {
                bindings: vec!["r".into(), "c".into(), "f".into()],
            },
        };
        let cloned = q.clone();
        assert_eq!(q, cloned);
        let _ = format!("{q:?}");
    }

    /// `MoldQLQuery::Pattern` is pattern-matchable.
    #[test]
    fn moldql_query_pattern_variant() {
        let q = MoldQLQuery::Pattern(PatternQuery {
            shortest: false,
            bindings: vec![],
            edges: vec![],
            predicates: vec![],
            projection: PatternProjection::Node { binding: "x".into() },
        });
        assert!(matches!(q, MoldQLQuery::Pattern(_)));
    }

    /// `PatternPredicate::Property` carries the right fields.
    #[test]
    fn pattern_predicate_property() {
        let pred = PatternPredicate::Property {
            target: PredicateTarget::Node("n".into()),
            field: "module".into(),
            op: PatternOp::Eq,
            value: PatternValue::String("core".into()),
        };
        let cloned = pred.clone();
        assert_eq!(pred, cloned);
    }

    /// `PatternProjection::Row` carries ordering and limit.
    #[test]
    fn pattern_projection_row_with_ordering() {
        let proj = PatternProjection::Row {
            fields: vec![RowField::AggregationRef { name: "calls".into() }],
            group_by: vec!["f.module".into()],
            aggregations: vec![Aggregation::Count { binding: Some("c".into()), alias: "calls".into() }],
            ordering: Some(OrderClause { by: "calls".into(), direction: OrderDirection::Desc }),
            limit: Some(5),
        };
        if let PatternProjection::Row { ordering, limit, .. } = &proj {
            assert!(ordering.is_some());
            assert_eq!(*limit, Some(5));
        }
    }

    /// `EdgeDirection` variants are Outgoing, Incoming, Both.
    #[test]
    fn edge_direction_variants() {
        use EdgeDirection::*;
        let _ = Outgoing;
        let _ = Incoming;
        let _ = Both;
    }

    /// `PredicateTarget` variants are Node, Edge, Anonymous.
    #[test]
    fn predicate_target_variants() {
        use PredicateTarget::*;
        let _ = Node("x".into());
        let _ = Edge("e".into());
        let _ = Anonymous;
    }
}
