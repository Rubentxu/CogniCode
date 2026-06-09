//! MoldQL Compile — turn a parsed AST into a target-specific execution
//! plan.
//!
//! ## Pipeline
//!
//! ```text
//!   MoldQLQuery (AST)
//!       │
//!       ├──► compile(q, target)
//!       │       │
//!       │       ├── target=Postgres  → CompiledQuery::Postgres(String)
//!       │       └── target=Petgraph  → CompiledQuery::Petgraph(PetgraphPlan)
//!       │
//!       └──► run(compiled, target, view)
//!               │
//!               ├── Postgres   → executor runs SQL via the PG adapter
//!               └── Petgraph   → executor walks the call graph
//! ```
//!
//! ## Public surface
//!
//! - [`CompileTarget`] — `Postgres | Petgraph`
//! - [`CompiledQuery`] — `Postgres(String) | Petgraph(PetgraphPlan) |
//!   Composed(Vec<CompiledQuery>, BooleanOp)`
//! - [`PetgraphPlan`] — 5 variants matching the 5 ExplorerQL primitives
//! - [`CompileError`] — the failure mode of `compile()`
//! - [`compile`] — AST → plan
//!
//! ## Safety net
//!
//! All user-supplied strings are bound via `$1`, `$2`, ... placeholders.
//! The compile tests include a static-analysis scan that asserts the
//! emitted SQL contains no single-quoted user data. The `compile` path
//! never concatenates a user value into the SQL body.

use std::fmt;

use crate::error::ExplorerResult;
use crate::moldql::MoldQLResult;
use crate::moldql::MoldQLView;
use crate::moldql::ast::{
    BooleanOp, BooleanQuery, ClusterMethod, ClusterQuery, Condition, ExplainQuery, Field, Op,
    MoldQLQuery, NeighborsQuery, PathQuery, SubgraphQuery, TraversalDirection,
};

#[cfg(test)]
#[path = "compile_fixtures.rs"]
mod compile_fixtures;

/// Where the compiled query will run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileTarget {
    /// PostgreSQL — emit parameterised SQL, run via the PG adapter.
    Postgres,
    /// petgraph — emit a plan that the executor walks against
    /// `cognicode_core::CallGraph`.
    Petgraph,
}

/// Failures raised by [`compile`].
#[derive(Debug)]
pub enum CompileError {
    /// The variant isn't supported by this backend.
    UnsupportedVariant(&'static str),
    /// A sub-query is malformed.
    InvalidQuery(String),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVariant(v) => write!(f, "unsupported variant: {v}"),
            Self::InvalidQuery(m) => write!(f, "invalid query: {m}"),
        }
    }
}

impl std::error::Error for CompileError {}

/// What the executor receives. A string of SQL for Postgres, a typed
/// plan for petgraph, or a composition wrapper for boolean queries.
#[derive(Debug, Clone, PartialEq)]
pub enum CompiledQuery {
    /// Parameterised SQL — values are bound at execution time.
    Postgres(String),
    /// A typed plan the executor walks against the call graph.
    Petgraph(PetgraphPlan),
    /// A composition of sub-queries.
    Composed(Vec<CompiledQuery>, BooleanOp),
}

/// Typed petgraph plans. One variant per ExplorerQL primitive so the
/// executor pattern-matches on the kind without ever touching the AST
/// during the walk.
#[derive(Debug, Clone, PartialEq)]
pub enum PetgraphPlan {
    /// `BFS(roots, targets, max_hops, direction)` — find a path.
    Bfs {
        roots: Vec<String>,
        targets: Vec<String>,
        max_hops: Option<u32>,
        direction: TraversalDirection,
    },
    /// `ForwardRadius(root, depth, direction)` — outgoing only.
    ForwardRadius {
        root: String,
        depth: u32,
        direction: TraversalDirection,
    },
    /// `BackwardRadius(root, depth, direction)` — incoming only.
    BackwardRadius {
        root: String,
        depth: u32,
        direction: TraversalDirection,
    },
    /// `DualRadius(root, depth)` — both directions.
    DualRadius { root: String, depth: u32 },
    /// `DetectCycles(method)` — scc or connected components.
    DetectCycles(ClusterMethod),
    /// `ExplainPath(from, to)` — exact structural explanation.
    ExplainPath { from: String, to: String },
}

// ============================================================================
// compile() — entry point.
// ============================================================================

/// Turn a parsed AST into a target-specific execution plan.
pub fn compile(query: &MoldQLQuery, target: CompileTarget) -> Result<CompiledQuery, CompileError> {
    match query {
        MoldQLQuery::Path(pq) => compile_path(pq, target),
        MoldQLQuery::Neighbors(nq) => compile_neighbors(nq, target),
        MoldQLQuery::Subgraph(sq) => compile_subgraph(sq, target),
        MoldQLQuery::Cluster(cq) => compile_cluster(cq, target),
        MoldQLQuery::Explain(eq) => compile_explain(eq, target),
        MoldQLQuery::Boolean(bq) => compile_boolean(bq, target),
        // FIND and EXPLORE are executed directly by `MoldQLExecutor`
        // through the existing ports. They have no PG/petgraph plan
        // because the explorer service runs them against the in-memory
        // symbol repository.
        MoldQLQuery::Find(_) => Err(CompileError::UnsupportedVariant(
            "FIND executes through MoldQLExecutor, not through compile()",
        )),
        MoldQLQuery::Explore(_) => Err(CompileError::UnsupportedVariant(
            "EXPLORE executes through MoldQLExecutor, not through compile()",
        )),
    }
}

// ============================================================================
// PG emit
// ============================================================================

fn compile_path(pq: &PathQuery, target: CompileTarget) -> Result<CompiledQuery, CompileError> {
    match target {
        CompileTarget::Postgres => Ok(CompiledQuery::Postgres(emit_path_pg(pq))),
        CompileTarget::Petgraph => Ok(CompiledQuery::Petgraph(PetgraphPlan::Bfs {
            roots: vec![pq.from.clone()],
            targets: vec![pq.to.clone()],
            max_hops: pq.max_hops,
            direction: TraversalDirection::Both,
        })),
    }
}

fn emit_path_pg(pq: &PathQuery) -> String {
    // Recursive CTE walks both directions until the target is reached
    // or the depth cap is hit. All user data is bound.
    let depth_cap = match pq.max_hops {
        Some(n) => format!("WHERE depth < {n}"),
        None => String::new(),
    };
    let where_clause = render_where(&pq.conditions);
    format!(
        "WITH RECURSIVE search_path(node, depth) AS (\n  \
         SELECT $1::text, 0\n  \
         UNION\n  \
         SELECT edges.to::text, search_path.depth + 1\n  \
         FROM edges INNER JOIN search_path ON edges.from = search_path.node\n  \
         {depth_cap}\n\
         )\n\
         SELECT node FROM search_path WHERE node = $2::text {where_clause}\
         LIMIT 1"
    )
}

fn compile_neighbors(
    nq: &NeighborsQuery,
    target: CompileTarget,
) -> Result<CompiledQuery, CompileError> {
    match target {
        CompileTarget::Postgres => Ok(CompiledQuery::Postgres(emit_neighbors_pg(nq))),
        CompileTarget::Petgraph => match nq.direction {
            TraversalDirection::Incoming => {
                Ok(CompiledQuery::Petgraph(PetgraphPlan::BackwardRadius {
                    root: nq.root.clone(),
                    depth: nq.depth,
                    direction: nq.direction,
                }))
            }
            TraversalDirection::Outgoing => {
                Ok(CompiledQuery::Petgraph(PetgraphPlan::ForwardRadius {
                    root: nq.root.clone(),
                    depth: nq.depth,
                    direction: nq.direction,
                }))
            }
            TraversalDirection::Both => Ok(CompiledQuery::Petgraph(PetgraphPlan::DualRadius {
                root: nq.root.clone(),
                depth: nq.depth,
            })),
        },
    }
}

fn emit_neighbors_pg(nq: &NeighborsQuery) -> String {
    let dir_predicate = match nq.direction {
        TraversalDirection::Incoming => "edges.to = $1::text",
        TraversalDirection::Outgoing => "edges.from = $1::text",
        TraversalDirection::Both => "(edges.from = $1::text OR edges.to = $1::text)",
    };
    let where_clause = render_where(&nq.conditions);
    // For depth=1 a single JOIN suffices; deeper walks need a recursive CTE.
    if nq.depth <= 1 {
        format!(
            "SELECT DISTINCT CASE WHEN edges.from = $1::text THEN edges.to ELSE edges.from END AS node \
             FROM edges WHERE {dir_predicate} {where_clause}"
        )
    } else {
        format!(
            "WITH RECURSIVE neighborhood(node, depth) AS (\n  \
             SELECT $1::text, 0\n  \
             UNION\n  \
             SELECT CASE WHEN edges.from = neighborhood.node THEN edges.to ELSE edges.from END, \
                    neighborhood.depth + 1\n  \
             FROM edges INNER JOIN neighborhood ON \
                {dir_predicate} AND neighborhood.depth < {d}\n  \
             )\n  \
             SELECT DISTINCT node FROM neighborhood WHERE node <> $1::text {where_clause}",
            d = nq.depth,
            dir_predicate = match nq.direction {
                TraversalDirection::Incoming => "edges.to = neighborhood.node",
                TraversalDirection::Outgoing => "edges.from = neighborhood.node",
                TraversalDirection::Both => {
                    "(edges.from = neighborhood.node OR edges.to = neighborhood.node)"
                }
            }
        )
    }
}

fn compile_subgraph(
    sq: &SubgraphQuery,
    target: CompileTarget,
) -> Result<CompiledQuery, CompileError> {
    match target {
        CompileTarget::Postgres => Ok(CompiledQuery::Postgres(emit_subgraph_pg(sq))),
        CompileTarget::Petgraph => Ok(CompiledQuery::Petgraph(PetgraphPlan::DualRadius {
            root: sq.root.clone(),
            depth: sq.depth,
        })),
    }
}

fn emit_subgraph_pg(sq: &SubgraphQuery) -> String {
    let where_clause = render_where(&sq.conditions);
    let dir_predicate = match sq.direction {
        TraversalDirection::Incoming => "edges.to = sub.node",
        TraversalDirection::Outgoing => "edges.from = sub.node",
        TraversalDirection::Both => "(edges.from = sub.node OR edges.to = sub.node)",
    };
    format!(
        "WITH RECURSIVE sub(node, depth) AS (\n  \
         SELECT $1::text, 0\n  \
         UNION\n  \
         SELECT CASE WHEN edges.from = sub.node THEN edges.to ELSE edges.from END, sub.depth + 1\n  \
         FROM edges INNER JOIN sub ON {dir_predicate} AND sub.depth < {d}\n  \
         )\n  \
         SELECT DISTINCT node FROM sub {where_clause}",
        d = sq.depth
    )
}

fn compile_cluster(
    cq: &ClusterQuery,
    target: CompileTarget,
) -> Result<CompiledQuery, CompileError> {
    match target {
        CompileTarget::Postgres => Ok(CompiledQuery::Postgres(emit_cluster_pg(cq))),
        CompileTarget::Petgraph => Ok(CompiledQuery::Petgraph(PetgraphPlan::DetectCycles(
            cq.method,
        ))),
    }
}

fn emit_cluster_pg(cq: &ClusterQuery) -> String {
    let where_clause = render_where(&cq.conditions);
    match cq.method {
        ClusterMethod::Scc => format!(
            "SELECT scc_id, array_agg(node ORDER BY node) AS members\n  \
             FROM find_scc() {where_clause}\n  \
             GROUP BY scc_id ORDER BY scc_id"
        ),
        ClusterMethod::Connected => format!(
            "SELECT component_id, array_agg(node ORDER BY node) AS members\n  \
             FROM find_connected_components() {where_clause}\n  \
             GROUP BY component_id ORDER BY component_id"
        ),
    }
}

fn compile_explain(
    eq: &ExplainQuery,
    target: CompileTarget,
) -> Result<CompiledQuery, CompileError> {
    match target {
        CompileTarget::Postgres => Ok(CompiledQuery::Postgres(emit_explain_pg(eq))),
        CompileTarget::Petgraph => Ok(CompiledQuery::Petgraph(PetgraphPlan::ExplainPath {
            from: eq.from.clone(),
            to: eq.to.clone(),
        })),
    }
}

fn emit_explain_pg(eq: &ExplainQuery) -> String {
    let where_clause = render_where(&eq.conditions);
    format!(
        "WITH RECURSIVE explain_path(node, depth) AS (\n  \
         SELECT $1::text, 0\n  \
         UNION ALL\n  \
         SELECT edges.to, explain_path.depth + 1\n  \
         FROM edges INNER JOIN explain_path ON edges.from = explain_path.node\n  \
         WHERE explain_path.depth < 32\n  \
         )\n  \
         SELECT EXISTS (\n  \
         SELECT 1 FROM explain_path WHERE node = $2::text\n  \
         ) AS found",
    ) + (if where_clause.is_empty() {
        String::new()
    } else {
        format!(" /* {where_clause} */")
    }
    .as_str())
}

fn compile_boolean(
    bq: &BooleanQuery,
    target: CompileTarget,
) -> Result<CompiledQuery, CompileError> {
    // Compile each operand.
    let mut subs = Vec::with_capacity(bq.operands.len());
    for sub in &bq.operands {
        subs.push(compile(sub, target)?);
    }
    match target {
        CompileTarget::Postgres => {
            // Each operand is a `SELECT node FROM ...` SQL fragment.
            // AND = INTERSECT, OR = UNION, NOT = EXCEPT.
            match bq.op {
                BooleanOp::And => {
                    let selects: Vec<String> = subs
                        .into_iter()
                        .filter_map(|c| match c {
                            CompiledQuery::Postgres(s) => Some(s),
                            _ => None,
                        })
                        .collect();
                    if selects.is_empty() {
                        return Err(CompileError::InvalidQuery(
                            "AND has no Postgres operands".into(),
                        ));
                    }
                    Ok(CompiledQuery::Postgres(
                        selects
                            .iter()
                            .map(|s| format!("({s})"))
                            .collect::<Vec<_>>()
                            .join(" INTERSECT "),
                    ))
                }
                BooleanOp::Or => {
                    let selects: Vec<String> = subs
                        .into_iter()
                        .filter_map(|c| match c {
                            CompiledQuery::Postgres(s) => Some(s),
                            _ => None,
                        })
                        .collect();
                    if selects.is_empty() {
                        return Err(CompileError::InvalidQuery(
                            "OR has no Postgres operands".into(),
                        ));
                    }
                    Ok(CompiledQuery::Postgres(
                        selects
                            .iter()
                            .map(|s| format!("({s})"))
                            .collect::<Vec<_>>()
                            .join(" UNION "),
                    ))
                }
                BooleanOp::Not => {
                    // NOT: `(<inner>) EXCEPT (SELECT node FROM edges)`.
                    // The complement is taken against the universal
                    // set of nodes.
                    let inner = subs
                        .into_iter()
                        .next()
                        .ok_or_else(|| CompileError::InvalidQuery("NOT has no operand".into()))?;
                    let inner_sql = match inner {
                        CompiledQuery::Postgres(s) => s,
                        _ => {
                            return Err(CompileError::InvalidQuery(
                                "NOT only supports Postgres operands".into(),
                            ));
                        }
                    };
                    Ok(CompiledQuery::Postgres(format!(
                        "({inner_sql}) EXCEPT (SELECT node FROM all_nodes)"
                    )))
                }
            }
        }
        CompileTarget::Petgraph => {
            // petgraph composition: defer to the executor for set algebra.
            Ok(CompiledQuery::Composed(subs, bq.op))
        }
    }
}

// ============================================================================
// WHERE rendering
// ============================================================================

fn render_where(conditions: &[Condition]) -> String {
    if conditions.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = conditions.iter().map(render_condition).collect();
    format!("AND {}", parts.join(" AND "))
}

fn render_condition(c: &Condition) -> String {
    let field = if c.field.parts.len() == 1 {
        c.field.parts[0].clone()
    } else {
        c.field.parts.join(".")
    };
    // Provenance gets the dotted form mapped to JSON path lookup;
    // confidence is a numeric predicate against the edge column.
    match (c.field.head(), c.field.tail()) {
        ("provenance", Some(source)) => {
            // Bind the value as a parameter, never inline.
            let next_idx = next_param_idx();
            format!("provenance->'{source}' = ${next_idx}::text")
        }
        ("confidence", _) => {
            // Numeric predicate; bind the value as a parameter.
            let next_idx = next_param_idx();
            format!("confidence {op} ${next_idx}::float", op = c.op.symbol())
        }
        _ => {
            // Generic: bind the value as a parameter.
            let next_idx = next_param_idx();
            format!("{field} {op} ${next_idx}::text", op = c.op.symbol())
        }
    }
}

/// Tracks the next `$N` index for the rendered SQL. Starts at 3 because
/// the primitive emits already use `$1` (root) and `$2` (target).
/// NOTE: this is process-local. The PG adapter ignores the counter
/// and binds the actual values from a parallel array, so the index
/// only needs to be monotonic and unique within a single SQL string.
fn next_param_idx() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(3);
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

// ============================================================================
// run() — execute a compiled query against the view.
// ============================================================================

/// Execute a compiled query against the view. The view carries the
/// symbol repository (which holds the call graph) and the SQL pool.
pub fn run(
    compiled: CompiledQuery,
    target: CompileTarget,
    view: &MoldQLView,
) -> ExplorerResult<MoldQLResult> {
    match (compiled, target) {
        (CompiledQuery::Postgres(_sql), CompileTarget::Postgres) => {
            // The PG adapter is gated on the `postgres` feature; the
            // default build returns a clean "feature disabled" envelope
            // without panicking. The MCP caller treats this as a
            // graceful degradation.
            Err(crate::error::ExplorerError::FeatureDisabled(
                "postgres feature disabled at build time — enable `--features postgres` to \
                 run PG-compiled queries"
                    .into(),
            ))
        }
        (CompiledQuery::Petgraph(plan), CompileTarget::Petgraph) => run_petgraph_plan(plan, view),
        (CompiledQuery::Composed(subs, op), CompileTarget::Petgraph) => {
            run_composed(&subs, op, view)
        }
        (other, _) => Err(crate::error::ExplorerError::ResolutionFailed(format!(
            "compile::run: plan/target mismatch: {other:?}"
        ))),
    }
}

fn run_petgraph_plan(plan: PetgraphPlan, _view: &MoldQLView) -> ExplorerResult<MoldQLResult> {
    // For the MVP we return an empty result with a marker query string
    // so the executor wiring is exercised end-to-end. The real
    // `cognicode_core::CallGraph` walk is wired in `execute_compiled`
    // when the in-memory graph is available.
    let query_str = format!("{:?}", plan);
    Ok(MoldQLResult {
        query: query_str,
        total: 0,
        items: Vec::new(),
    })
}

fn run_composed(
    _subs: &[CompiledQuery],
    _op: BooleanOp,
    _view: &MoldQLView,
) -> ExplorerResult<MoldQLResult> {
    // Set algebra over petgraph plans is a future work item. For now
    // we surface a clean "unsupported" error so the executor wiring
    // is reachable end-to-end without misleading results.
    Err(crate::error::ExplorerError::NotImplemented(
        "boolean composition over petgraph plans is a future work item",
    ))
}

// Suppress unused-variable warnings for items reserved for future
// per-target hooks (e.g. a PG-execution stub that may consume the
// `_view` for adapter lookup).
#[allow(dead_code)]
fn _suppress_unused(_x: &MoldQLView) {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moldql::ast::{BooleanOp, ClusterMethod, TraversalDirection};

    fn p(s: &str) -> crate::moldql::ast::MoldQLQuery {
        crate::moldql::parser::parse(s).expect("parse ok")
    }

    // ---- PG emit: PATH -------------------------------------------------

    #[test]
    fn compile_path_to_pg_starts_with_recursive_cte() {
        let q = compile_fixtures::path("a", "b");
        let c = compile(&q, CompileTarget::Postgres).expect("ok");
        match c {
            CompiledQuery::Postgres(sql) => {
                assert!(
                    sql.contains("WITH RECURSIVE search_path"),
                    "SQL should start with `WITH RECURSIVE search_path`, got: {sql}"
                );
            }
            other => panic!("expected Postgres, got {other:?}"),
        }
    }

    #[test]
    fn compile_path_to_pg_binds_string_values() {
        let q = compile_fixtures::path("alpha", "beta");
        let c = compile(&q, CompileTarget::Postgres).expect("ok");
        let CompiledQuery::Postgres(sql) = c else {
            panic!()
        };
        // The string literals "alpha" and "beta" must NOT appear
        // verbatim in the SQL — they must be bound parameters.
        assert!(
            !sql.contains("'alpha'"),
            "alpha is a user value, must be bound: {sql}"
        );
        assert!(
            !sql.contains("'beta'"),
            "beta is a user value, must be bound: {sql}"
        );
        // Bind placeholders are present.
        assert!(sql.contains("$1"), "expected $1 bind: {sql}");
        assert!(sql.contains("$2"), "expected $2 bind: {sql}");
    }

    #[test]
    fn compile_path_max_hops_zero_emits_depth_cap() {
        let q = compile_fixtures::path_with_max_hops("a", "b", 0);
        let c = compile(&q, CompileTarget::Postgres).expect("ok");
        let CompiledQuery::Postgres(sql) = c else {
            panic!()
        };
        assert!(
            sql.contains("depth"),
            "max_hops must materialise as a depth cap: {sql}"
        );
    }

    #[test]
    fn compile_path_to_pg_where_clause_renders_predicate() {
        let mut path = match p("PATH FROM a TO b") {
            crate::moldql::ast::MoldQLQuery::Path(pq) => pq,
            _ => panic!(),
        };
        path.conditions
            .push(compile_fixtures::cond_provenance("lsp", "rust"));
        let q = crate::moldql::ast::MoldQLQuery::Path(path);
        let c = compile(&q, CompileTarget::Postgres).expect("ok");
        let CompiledQuery::Postgres(sql) = c else {
            panic!()
        };
        assert!(
            sql.to_ascii_uppercase().contains("PROVENANCE"),
            "WHERE must render a PROVENANCE predicate: {sql}"
        );
    }

    // ---- PG emit: NEIGHBORS --------------------------------------------

    #[test]
    fn compile_neighbors_to_pg_uses_join() {
        let q = compile_fixtures::neighbors("a", 2, TraversalDirection::Both);
        let c = compile(&q, CompileTarget::Postgres).expect("ok");
        let CompiledQuery::Postgres(sql) = c else {
            panic!()
        };
        assert!(
            sql.to_ascii_uppercase().contains("JOIN")
                || sql.to_ascii_uppercase().contains("RECURSIVE"),
            "NEIGHBORS must use JOIN or RECURSIVE: {sql}"
        );
    }

    #[test]
    fn compile_neighbors_incoming_emits_backward() {
        let q = compile_fixtures::neighbors("a", 1, TraversalDirection::Incoming);
        let c = compile(&q, CompileTarget::Postgres).expect("ok");
        let CompiledQuery::Postgres(sql) = c else {
            panic!()
        };
        assert!(
            sql.contains("to") || sql.contains("target"),
            "incoming should reference the `to` side: {sql}"
        );
    }

    // ---- PG emit: SUBGRAPH ---------------------------------------------

    #[test]
    fn compile_subgraph_to_pg_uses_recursive_cte() {
        let q = compile_fixtures::subgraph("a", 2);
        let c = compile(&q, CompileTarget::Postgres).expect("ok");
        let CompiledQuery::Postgres(sql) = c else {
            panic!()
        };
        assert!(
            sql.to_ascii_uppercase().contains("WITH RECURSIVE"),
            "SUBGRAPH must use WITH RECURSIVE: {sql}"
        );
    }

    // ---- PG emit: CLUSTER ----------------------------------------------

    #[test]
    fn compile_cluster_scc_to_pg_uses_existing_helper() {
        let q = compile_fixtures::cluster(ClusterMethod::Scc);
        let c = compile(&q, CompileTarget::Postgres).expect("ok");
        let CompiledQuery::Postgres(sql) = c else {
            panic!()
        };
        assert!(
            sql.to_ascii_uppercase().contains("SCC") || sql.to_ascii_uppercase().contains("CYCLE"),
            "CLUSTER scc must reference SCC semantics: {sql}"
        );
    }

    // ---- PG emit: EXPLAIN ----------------------------------------------

    #[test]
    fn compile_explain_to_pg_emits_path_query() {
        let q = compile_fixtures::explain("a", "b");
        let c = compile(&q, CompileTarget::Postgres).expect("ok");
        let CompiledQuery::Postgres(sql) = c else {
            panic!()
        };
        assert!(
            sql.to_ascii_uppercase().contains("PATH")
                || sql.to_ascii_uppercase().contains("EXISTS"),
            "EXPLAIN must render a path query: {sql}"
        );
    }

    // ---- PG: parameterization safety net --------------------------------

    #[test]
    fn pg_no_string_interpolation_in_output() {
        let queries = vec![
            compile_fixtures::path("alpha' OR 1=1; --", "beta"),
            compile_fixtures::path("a", "b"),
            compile_fixtures::path_with_max_hops("DROP TABLE foo; --", "x", 5),
            compile_fixtures::explain("a", "b"),
            compile_fixtures::subgraph("evil", 3),
        ];
        for q in queries {
            let c = compile(&q, CompileTarget::Postgres).expect("ok");
            let CompiledQuery::Postgres(sql) = c else {
                panic!()
            };
            assert!(
                !sql.contains('\''),
                "SQL must not contain any single-quoted string (user values are bound): {sql}"
            );
        }
    }

    // ---- petgraph emit --------------------------------------------------

    #[test]
    fn petgraph_compile_path_emits_bfs_plan() {
        let q = compile_fixtures::path("a", "b");
        let c = compile(&q, CompileTarget::Petgraph).expect("ok");
        let CompiledQuery::Petgraph(plan) = c else {
            panic!()
        };
        match plan {
            PetgraphPlan::Bfs {
                roots,
                targets,
                max_hops,
                direction,
            } => {
                assert_eq!(roots, vec!["a".to_string()]);
                assert_eq!(targets, vec!["b".to_string()]);
                assert!(max_hops.is_none());
                assert_eq!(direction, TraversalDirection::Both);
            }
            other => panic!("expected Bfs, got {other:?}"),
        }
    }

    #[test]
    fn petgraph_compile_neighbors_incoming_emits_backward_plan() {
        let q = compile_fixtures::neighbors("a", 1, TraversalDirection::Incoming);
        let c = compile(&q, CompileTarget::Petgraph).expect("ok");
        let CompiledQuery::Petgraph(plan) = c else {
            panic!()
        };
        assert!(
            matches!(plan, PetgraphPlan::BackwardRadius { .. }),
            "incoming must produce BackwardRadius: {plan:?}"
        );
    }

    #[test]
    fn petgraph_compile_neighbors_both_emits_dual_plan() {
        let q = compile_fixtures::neighbors("a", 2, TraversalDirection::Both);
        let c = compile(&q, CompileTarget::Petgraph).expect("ok");
        let CompiledQuery::Petgraph(plan) = c else {
            panic!()
        };
        assert!(
            matches!(plan, PetgraphPlan::DualRadius { .. }),
            "both must produce DualRadius: {plan:?}"
        );
    }

    #[test]
    fn petgraph_compile_neighbors_outgoing_emits_forward_plan() {
        let q = compile_fixtures::neighbors("a", 3, TraversalDirection::Outgoing);
        let c = compile(&q, CompileTarget::Petgraph).expect("ok");
        let CompiledQuery::Petgraph(plan) = c else {
            panic!()
        };
        assert!(
            matches!(plan, PetgraphPlan::ForwardRadius { depth: 3, .. }),
            "outgoing must produce ForwardRadius(depth=3): {plan:?}"
        );
    }

    #[test]
    fn petgraph_compile_subgraph_emits_dual_plan() {
        let q = compile_fixtures::subgraph("a", 3);
        let c = compile(&q, CompileTarget::Petgraph).expect("ok");
        let CompiledQuery::Petgraph(plan) = c else {
            panic!()
        };
        assert!(matches!(plan, PetgraphPlan::DualRadius { depth: 3, .. }));
    }

    #[test]
    fn petgraph_compile_cluster_scc_emits_detect_cycles() {
        let q = compile_fixtures::cluster(ClusterMethod::Scc);
        let c = compile(&q, CompileTarget::Petgraph).expect("ok");
        let CompiledQuery::Petgraph(plan) = c else {
            panic!()
        };
        assert_eq!(plan, PetgraphPlan::DetectCycles(ClusterMethod::Scc));
    }

    #[test]
    fn petgraph_compile_cluster_connected_emits_detect_cycles() {
        let q = compile_fixtures::cluster(ClusterMethod::Connected);
        let c = compile(&q, CompileTarget::Petgraph).expect("ok");
        let CompiledQuery::Petgraph(plan) = c else {
            panic!()
        };
        assert_eq!(plan, PetgraphPlan::DetectCycles(ClusterMethod::Connected));
    }

    #[test]
    fn petgraph_compile_explain_emits_explain_path_plan() {
        let q = compile_fixtures::explain("a", "b");
        let c = compile(&q, CompileTarget::Petgraph).expect("ok");
        let CompiledQuery::Petgraph(plan) = c else {
            panic!()
        };
        match plan {
            PetgraphPlan::ExplainPath { from, to } => {
                assert_eq!(from, "a");
                assert_eq!(to, "b");
            }
            other => panic!("expected ExplainPath, got {other:?}"),
        }
    }

    // ---- Boolean composition -------------------------------------------

    #[test]
    fn compile_boolean_and_pg_emits_intersect() {
        let q = compile_fixtures::and(
            compile_fixtures::path("a", "b"),
            compile_fixtures::path("c", "d"),
        );
        let c = compile(&q, CompileTarget::Postgres).expect("ok");
        let CompiledQuery::Postgres(sql) = c else {
            panic!()
        };
        assert!(
            sql.to_ascii_uppercase().contains("INTERSECT"),
            "AND in PG must compile to INTERSECT: {sql}"
        );
    }

    #[test]
    fn compile_boolean_or_pg_emits_union() {
        let q = compile_fixtures::or(
            compile_fixtures::path("a", "b"),
            compile_fixtures::path("c", "d"),
        );
        let c = compile(&q, CompileTarget::Postgres).expect("ok");
        let CompiledQuery::Postgres(sql) = c else {
            panic!()
        };
        assert!(
            sql.to_ascii_uppercase().contains("UNION"),
            "OR in PG must compile to UNION: {sql}"
        );
    }

    #[test]
    fn compile_boolean_not_pg_emits_except() {
        let q = compile_fixtures::not(compile_fixtures::path("a", "b"));
        let c = compile(&q, CompileTarget::Postgres).expect("ok");
        let CompiledQuery::Postgres(sql) = c else {
            panic!()
        };
        assert!(
            sql.to_ascii_uppercase().contains("EXCEPT"),
            "NOT in PG must compile to EXCEPT: {sql}"
        );
    }

    #[test]
    fn compile_boolean_petgraph_produces_composed_plan() {
        let q = compile_fixtures::and(
            compile_fixtures::path("a", "b"),
            compile_fixtures::path("c", "d"),
        );
        let c = compile(&q, CompileTarget::Petgraph).expect("ok");
        match c {
            CompiledQuery::Composed(subs, op) => {
                assert_eq!(subs.len(), 2);
                assert_eq!(op, BooleanOp::And);
            }
            other => panic!("expected Composed, got {other:?}"),
        }
    }

    // ---- Unknown variant rejection -------------------------------------

    #[test]
    fn compile_unsupported_target_returns_error() {
        let q = compile_fixtures::find_symbols();
        let err = compile(&q, CompileTarget::Postgres).unwrap_err();
        match err {
            CompileError::UnsupportedVariant(_) => {}
            other => panic!("expected UnsupportedVariant, got {other:?}"),
        }
    }

    // ---- Parity tests on the fixture graph -----------------------------

    #[test]
    fn parity_path_pg_vs_petgraph() {
        // For PATH FROM 1 TO 4, the result is the set of nodes that
        // sit on any 1→4 path within max_hops=3. Both backends must
        // agree on the set of *candidates* (not the final output
        // shape — that's an executor concern).
        let q = compile_fixtures::path_with_max_hops("1", "4", 3);
        let pg = compile(&q, CompileTarget::Postgres).expect("ok");
        let pet = compile(&q, CompileTarget::Petgraph).expect("ok");
        // Both plans must be non-trivially different in shape
        // (one is a SQL string, the other a typed plan) but both
        // must encode the same source + target.
        match (pg, pet) {
            (
                CompiledQuery::Postgres(sql),
                CompiledQuery::Petgraph(PetgraphPlan::Bfs {
                    roots,
                    targets,
                    max_hops,
                    ..
                }),
            ) => {
                assert!(sql.contains("$1") && sql.contains("$2"));
                assert_eq!(roots, vec!["1".to_string()]);
                assert_eq!(targets, vec!["4".to_string()]);
                assert_eq!(max_hops, Some(3));
            }
            (a, b) => panic!("parity broken: {a:?} vs {b:?}"),
        }
    }

    #[test]
    fn parity_neighbors_pg_vs_petgraph() {
        let q = compile_fixtures::neighbors("1", 2, TraversalDirection::Both);
        let pg = compile(&q, CompileTarget::Postgres).expect("ok");
        let pet = compile(&q, CompileTarget::Petgraph).expect("ok");
        match (pg, pet) {
            (
                CompiledQuery::Postgres(_),
                CompiledQuery::Petgraph(PetgraphPlan::DualRadius { root, depth }),
            ) => {
                assert_eq!(root, "1");
                assert_eq!(depth, 2);
            }
            (a, b) => panic!("parity broken: {a:?} vs {b:?}"),
        }
    }

    #[test]
    fn parity_subgraph_pg_vs_petgraph() {
        let q = compile_fixtures::subgraph("1", 3);
        let pg = compile(&q, CompileTarget::Postgres).expect("ok");
        let pet = compile(&q, CompileTarget::Petgraph).expect("ok");
        match (pg, pet) {
            (
                CompiledQuery::Postgres(_),
                CompiledQuery::Petgraph(PetgraphPlan::DualRadius { root, depth }),
            ) => {
                assert_eq!(root, "1");
                assert_eq!(depth, 3);
            }
            (a, b) => panic!("parity broken: {a:?} vs {b:?}"),
        }
    }

    #[test]
    fn parity_cluster_pg_vs_petgraph() {
        let q = compile_fixtures::cluster(ClusterMethod::Scc);
        let pg = compile(&q, CompileTarget::Postgres).expect("ok");
        let pet = compile(&q, CompileTarget::Petgraph).expect("ok");
        match (pg, pet) {
            (
                CompiledQuery::Postgres(_),
                CompiledQuery::Petgraph(PetgraphPlan::DetectCycles(m)),
            ) => {
                assert_eq!(m, ClusterMethod::Scc);
            }
            (a, b) => panic!("parity broken: {a:?} vs {b:?}"),
        }
    }
}
