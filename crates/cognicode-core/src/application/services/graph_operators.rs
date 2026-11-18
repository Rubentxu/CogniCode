//! Typed read operators over the call graph.
//!
//! A question about the graph — who implements this trait, who calls this
//! function from that module, the five most trusted consumers — is a small
//! plan of operators, not a bespoke walk per surface:
//!
//! ```text
//! Lookup -> Expand -> Filter -> TopK
//! ```
//!
//! - **Lookup** turns the target into one symbol, by the domain's rule for
//!   names (a name several symbols carry is ambiguous, never picked) or by
//!   the id a caller already holds.
//! - **Expand** follows one or more [`Relation`]s from it, breadth-first, up
//!   to a depth. A symbol is reached once, so a cycle ends.
//! - **Filter** keeps the rows that match the [`TypedQuery`] filter. It
//!   restricts what is *listed*, never what is *followed*: a caller outside
//!   the module can still be the way to a caller inside it.
//! - **TopK** ranks the rows (nearest, then most trusted, then by id) and
//!   keeps the first `k`.
//!
//! This is a read plan and nothing more. It decides *how* the graph is read
//! — through its reverse index or by scanning every edge — and says so in
//! the [`Explain`]; it does not decide what a surface renders, what a
//! snapshot means or who is authorised. The relations and filters are a
//! closed set of types, so a query is never text that gets executed.
//!
//! ## The budget
//!
//! A query may declare a ceiling of fact visits, the unit the domain
//! budgets already count: one visit per edge examined, plus one for the
//! lookup. The ceiling is checked before each visit and never exceeded. A
//! query that reaches it stops and says [`Completeness::Partial`]; its rows
//! are the ones found before the stop, which are rows of the full answer.
//!
//! ## The estimate
//!
//! The first hop is estimated exactly from the index. Later hops are scaled
//! by the graph's mean out-degree, which is a guess and is published next
//! to the real spend. When a [`CostEstimator`] holds measured spends for the
//! same shape of query, the mean of those replaces the structural guess and
//! the explain says so.

use std::collections::{HashSet, VecDeque};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::application::change_tracking::planner::WorkId;
use crate::application::change_tracking::scheduling::CostEstimator;
use crate::application::services::graph_queries::{GraphQueryError, resolve};
use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::budgets::{BudgetAuthorizer, BudgetDeclaration, BudgetKind, BudgetState};
use crate::domain::naming::NamespacedName;
use crate::domain::value_objects::{DependencyType, Provenance};

#[cfg(test)]
mod tests;

/// Which end of an edge a relation starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The edges that arrive at the symbol.
    Incoming,
    /// The edges that leave the symbol.
    Outgoing,
}

/// A relation between symbols that a query follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    /// The symbols with an edge, of any type, to the target.
    Callers,
    /// The symbols the target has an edge, of any type, to.
    Callees,
    /// The symbols that inherit from, or implement, the target.
    Implementors,
    /// The symbols the target inherits from, or implements.
    Contracts,
    /// The symbols that depend on the target through any edge but
    /// containment.
    Dependents,
}

impl Relation {
    /// The name the relation has on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Relation::Callers => "callers",
            Relation::Callees => "callees",
            Relation::Implementors => "implementors",
            Relation::Contracts => "contracts",
            Relation::Dependents => "dependents",
        }
    }

    /// The relation a wire name stands for, or the refusal.
    pub fn parse(name: &str) -> Result<Self, InvalidQuery> {
        [
            Relation::Callers,
            Relation::Callees,
            Relation::Implementors,
            Relation::Contracts,
            Relation::Dependents,
        ]
        .into_iter()
        .find(|relation| relation.as_str() == name)
        .ok_or_else(|| InvalidQuery::UnknownRelation(name.to_string()))
    }

    fn side(self) -> Side {
        match self {
            Relation::Callers | Relation::Implementors | Relation::Dependents => Side::Incoming,
            Relation::Callees | Relation::Contracts => Side::Outgoing,
        }
    }

    fn admits(self, kind: &DependencyType) -> bool {
        match self {
            Relation::Callers | Relation::Callees => true,
            Relation::Implementors | Relation::Contracts => *kind == DependencyType::Inherits,
            Relation::Dependents => *kind != DependencyType::Contains,
        }
    }
}

/// The symbol a query starts from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A name, resolved by the domain's rule: a homonym is refused.
    Name(String),
    /// A symbol the caller already identified.
    Id(SymbolId),
}

/// A bounded read of the graph: where to start, what to follow, what to
/// keep and how much it may cost.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedQuery {
    target: Target,
    relations: Vec<Relation>,
    depth: u32,
    module: Option<String>,
    exclude_tests: bool,
    top_k: Option<usize>,
    budget: Option<u64>,
}

impl TypedQuery {
    /// A query for the symbols `relations` reach from `target`, one hop
    /// away, with no filter, no limit and no budget.
    pub fn new(target: Target, relations: Vec<Relation>) -> Self {
        Self {
            target,
            relations,
            depth: 1,
            module: None,
            exclude_tests: false,
            top_k: None,
            budget: None,
        }
    }

    /// Follows the relations up to `depth` hops.
    pub fn with_depth(mut self, depth: u32) -> Self {
        self.depth = depth;
        self
    }

    /// Lists only the symbols declared in a module: a directory, matched by
    /// path suffix component by component like a file is.
    pub fn in_module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    /// Leaves the symbols that declare a test out of the rows.
    pub fn without_tests(mut self) -> Self {
        self.exclude_tests = true;
        self
    }

    /// Keeps the `k` best rows.
    pub fn with_top_k(mut self, k: usize) -> Self {
        self.top_k = Some(k);
        self
    }

    /// Spends at most `visits` fact visits.
    pub fn with_budget(mut self, visits: u64) -> Self {
        self.budget = Some(visits);
        self
    }

    fn validate(&self) -> Result<(), InvalidQuery> {
        let named_nothing = matches!(&self.target, Target::Name(name) if name.trim().is_empty());
        if named_nothing {
            return Err(InvalidQuery::EmptyTarget);
        }
        if self.relations.is_empty() {
            return Err(InvalidQuery::NoRelation);
        }
        if self.depth == 0 {
            return Err(InvalidQuery::ZeroDepth);
        }
        if self.top_k == Some(0) {
            return Err(InvalidQuery::ZeroTopK);
        }
        if self.budget == Some(0) {
            return Err(InvalidQuery::ZeroBudget);
        }
        if self.module.as_ref().is_some_and(|m| m.trim().is_empty()) {
            return Err(InvalidQuery::EmptyModule);
        }
        Ok(())
    }

    /// The shape of the query, as the name its measured costs are kept
    /// under: queries that follow the same relations cost alike.
    fn work(&self) -> Option<WorkId> {
        let relations: Vec<&str> = self.relations.iter().map(|r| r.as_str()).collect();
        NamespacedName::new(format!("graph_query.{}", relations.join("-")))
            .ok()
            .map(WorkId::new)
    }
}

/// Why a query was refused before it ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidQuery {
    /// The query follows no relation.
    NoRelation,
    /// The relation is not one the plan knows.
    UnknownRelation(String),
    /// The name to start from is empty.
    EmptyTarget,
    /// A depth of zero reaches nothing.
    ZeroDepth,
    /// A top-k of zero keeps nothing.
    ZeroTopK,
    /// A budget of zero allows nothing to be read.
    ZeroBudget,
    /// The module to keep is empty.
    EmptyModule,
}

impl std::fmt::Display for InvalidQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvalidQuery::NoRelation => write!(f, "the query follows no relation"),
            InvalidQuery::UnknownRelation(name) => write!(
                f,
                "`{name}` is not a relation: the relations are `callers`, `callees`, \
                 `implementors`, `contracts` and `dependents`"
            ),
            InvalidQuery::EmptyTarget => write!(f, "the query names no symbol"),
            InvalidQuery::ZeroDepth => write!(f, "a depth of zero reaches nothing"),
            InvalidQuery::ZeroTopK => write!(f, "a top-k of zero keeps nothing"),
            InvalidQuery::ZeroBudget => write!(f, "a budget of zero allows nothing to be read"),
            InvalidQuery::EmptyModule => write!(f, "the module to keep is empty"),
        }
    }
}

/// Why a query gave no answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    /// The query is not one the plan runs.
    Invalid(InvalidQuery),
    /// The target is ambiguous or unknown.
    Graph(GraphQueryError),
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryError::Invalid(invalid) => invalid.fmt(f),
            QueryError::Graph(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for QueryError {}

/// One symbol the query reached, and how.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The symbol.
    pub id: SymbolId,
    /// How many hops from the target it was first reached.
    pub depth: u32,
    /// The relation that reached it.
    pub relation: Relation,
    /// The type of the edge that carries the most confidence.
    pub via: DependencyType,
    /// Where that edge came from.
    pub provenance: Provenance,
    /// How much that edge is trusted.
    pub confidence: f64,
}

/// How the graph is read to follow an incoming relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Through the reverse index: the sources of the target, and only their
    /// edges.
    Index,
    /// By examining every edge of the graph.
    Scan,
}

/// The operators of the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    /// Turn the target into a symbol.
    Lookup,
    /// Follow the relations.
    Expand,
    /// Keep the rows that match.
    Filter,
    /// Rank the rows and keep the first `k`.
    TopK,
}

/// What one operator did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Step {
    /// The operator.
    pub operator: Operator,
    /// The index or access the operator read through.
    pub index: String,
    /// The route the planner weighed and did not take, with its estimate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected: Option<String>,
    /// Rows the operator received.
    pub rows_in: usize,
    /// Rows the operator passed on.
    pub rows_out: usize,
    /// Fact visits the operator spent.
    pub visits: u64,
}

/// Whether the query looked at everything its plan called for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Completeness {
    /// The plan ran to its end.
    Complete,
    /// The plan stopped early: the rows are the ones found before it did.
    Partial {
        /// Why it stopped.
        reasons: Vec<String>,
    },
}

/// Where the estimate of the cost came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EstimateBasis {
    /// Computed from the graph: the first hop exactly, later hops by the
    /// mean out-degree.
    Structural,
    /// The mean of the spends measured for queries of the same shape.
    Measured,
}

/// What a query did, published next to its rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Explain {
    /// The operators that ran, in order.
    pub steps: Vec<Step>,
    /// The fact visits the plan expected to spend.
    pub estimated_visits: u64,
    /// Where that expectation came from.
    pub estimate_basis: EstimateBasis,
    /// The fact visits the plan spent.
    pub spent_visits: u64,
    /// The ceiling the query declared, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<u64>,
    /// Whether the plan ran to its end.
    pub completeness: Completeness,
    /// The rows the expansion reached.
    pub reached: usize,
    /// The rows that matched the filter.
    pub matched: usize,
    /// The rows the filter left out.
    pub filtered_out: usize,
    /// The rows listed, after the top-k.
    pub listed: usize,
}

/// The rows of a query and the account of how they were found.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    /// The symbol the query started from.
    pub root: SymbolId,
    /// The rows, nearest first, then most trusted first, then by id.
    pub rows: Vec<Row>,
    /// What the plan did.
    pub explain: Explain,
}

/// Runs a query, letting the planner choose how to read the graph.
///
/// When an estimator is given, the spend of the run is observed into it, so
/// the next query of the same shape is estimated from what this one cost.
pub fn run(
    graph: &CallGraph,
    query: &TypedQuery,
    estimator: Option<&CostEstimator>,
) -> Result<Answer, QueryError> {
    execute(graph, query, None, estimator)
}

/// Runs a query reading the graph the way the caller says, not the way the
/// planner would. The answer is the same; the spend is what differs.
pub fn run_with_access(
    graph: &CallGraph,
    query: &TypedQuery,
    access: Access,
    estimator: Option<&CostEstimator>,
) -> Result<Answer, QueryError> {
    execute(graph, query, Some(access), estimator)
}

/// The symbols a word names, ordered by symbol id.
///
/// This is the lookup of a keyword: every symbol the graph's name index
/// holds under the word, whatever the case, without choosing between them.
pub fn symbols_named(graph: &CallGraph, word: &str) -> Vec<SymbolId> {
    let mut ids: Vec<SymbolId> = graph
        .find_by_name(word)
        .into_iter()
        .map(|symbol| SymbolId::new(symbol.fully_qualified_name()))
        .collect();
    ids.sort();
    ids
}

/// The spend of a query, against the ceiling it declared.
struct Meter {
    authorizer: BudgetAuthorizer,
    state: BudgetState,
    spent: u64,
}

impl Meter {
    fn new(ceiling: Option<u64>) -> Self {
        let declaration = ceiling
            .and_then(std::num::NonZeroU64::new)
            .map(BudgetDeclaration::fact_visits)
            .unwrap_or_else(BudgetDeclaration::none);
        Self {
            state: BudgetState::new(&declaration, 0),
            authorizer: BudgetAuthorizer::new(declaration),
            spent: 0,
        }
    }

    /// Pays for one visit, or says the ceiling does not allow it.
    fn visit(&mut self) -> Result<(), Stopped> {
        self.authorizer
            .check(&self.state, BudgetKind::FactVisits, 1)
            .map_err(|_| Stopped)?;
        self.authorizer
            .commit(&mut self.state, BudgetKind::FactVisits, 1);
        self.spent += 1;
        Ok(())
    }
}

/// The budget did not allow the next visit.
struct Stopped;

fn execute(
    graph: &CallGraph,
    query: &TypedQuery,
    forced: Option<Access>,
    estimator: Option<&CostEstimator>,
) -> Result<Answer, QueryError> {
    query.validate().map_err(QueryError::Invalid)?;
    let mut meter = Meter::new(query.budget);
    let mut steps = Vec::new();

    let Some(root) = lookup(graph, query, &mut meter, &mut steps)? else {
        return Ok(stopped_before_the_root(query, meter, steps));
    };

    let plan = Plan::of(graph, &root, query, forced);
    let estimate = estimate_of(graph, &root, query, &plan, estimator);
    let visits_before = meter.spent;
    let (reached, stopped) = expand(graph, &root, query, plan.access, &mut meter);
    steps.push(Step {
        operator: Operator::Expand,
        index: plan.describe(),
        rejected: plan.rejected(),
        rows_in: 1,
        rows_out: reached.len(),
        visits: meter.spent - visits_before,
    });

    let reached_count = reached.len();
    let kept = filter(graph, query, reached);
    steps.push(Step {
        operator: Operator::Filter,
        index: filter_description(query),
        rejected: None,
        rows_in: reached_count,
        rows_out: kept.len(),
        visits: 0,
    });

    let matched = kept.len();
    let rows = top_k(query, kept);
    steps.push(Step {
        operator: Operator::TopK,
        index: "rank: depth, confidence, id".to_string(),
        rejected: None,
        rows_in: matched,
        rows_out: rows.len(),
        visits: 0,
    });

    if let Some(work) = query.work()
        && let Some(estimator) = estimator
    {
        estimator.observe(&work, meter.spent);
    }

    let completeness = match stopped {
        None => Completeness::Complete,
        Some(reason) => Completeness::Partial {
            reasons: vec![reason],
        },
    };
    Ok(Answer {
        root,
        explain: Explain {
            steps,
            estimated_visits: estimate.visits,
            estimate_basis: estimate.basis,
            spent_visits: meter.spent,
            budget: query.budget,
            completeness,
            reached: reached_count,
            matched,
            filtered_out: reached_count - matched,
            listed: rows.len(),
        },
        rows,
    })
}

/// The answer of a query whose budget did not even pay for the lookup.
fn stopped_before_the_root(query: &TypedQuery, meter: Meter, steps: Vec<Step>) -> Answer {
    let root = match &query.target {
        Target::Id(id) => id.clone(),
        Target::Name(name) => SymbolId::new(name.clone()),
    };
    Answer {
        root,
        rows: Vec::new(),
        explain: Explain {
            steps,
            estimated_visits: 0,
            estimate_basis: EstimateBasis::Structural,
            spent_visits: meter.spent,
            budget: query.budget,
            completeness: Completeness::Partial {
                reasons: vec![
                    "budget: the ceiling did not allow the lookup of the target".to_string(),
                ],
            },
            reached: 0,
            matched: 0,
            filtered_out: 0,
            listed: 0,
        },
    }
}

/// Resolves the target; `None` when the budget did not allow the visit.
fn lookup(
    graph: &CallGraph,
    query: &TypedQuery,
    meter: &mut Meter,
    steps: &mut Vec<Step>,
) -> Result<Option<SymbolId>, QueryError> {
    if meter.visit().is_err() {
        return Ok(None);
    }
    let (root, index) = match &query.target {
        Target::Name(name) => (
            resolve(graph, name).map_err(QueryError::Graph)?,
            "name_index",
        ),
        Target::Id(id) => {
            if graph.get_symbol(id).is_none() {
                return Err(QueryError::Graph(GraphQueryError::NotFound {
                    name: id.as_str().to_string(),
                }));
            }
            (id.clone(), "symbol_id")
        }
    };
    steps.push(Step {
        operator: Operator::Lookup,
        index: index.to_string(),
        rejected: None,
        rows_in: 0,
        rows_out: 1,
        visits: 1,
    });
    Ok(Some(root))
}

/// How the plan reads the graph, and what it weighed.
struct Plan {
    access: Access,
    incoming: bool,
    index_cost: u64,
    scan_cost: u64,
}

impl Plan {
    /// The cheaper way to read the first hop. The index touches only the
    /// edges of the target's sources; the scan touches every edge.
    fn of(graph: &CallGraph, root: &SymbolId, query: &TypedQuery, forced: Option<Access>) -> Self {
        let incoming = query.relations.iter().any(|r| r.side() == Side::Incoming);
        let index_cost = index_cost(graph, root);
        let scan_cost = graph.edge_count() as u64;
        let chosen = if index_cost <= scan_cost {
            Access::Index
        } else {
            Access::Scan
        };
        Self {
            access: forced.unwrap_or(chosen),
            incoming,
            index_cost,
            scan_cost,
        }
    }

    fn describe(&self) -> String {
        match (self.incoming, self.access) {
            (false, _) => "forward edges".to_string(),
            (true, Access::Index) => "reverse index + forward edges of the sources".to_string(),
            (true, Access::Scan) => "edge scan".to_string(),
        }
    }

    fn rejected(&self) -> Option<String> {
        if !self.incoming {
            return None;
        }
        Some(match self.access {
            Access::Index => format!("edge scan, estimated {} visits", self.scan_cost),
            Access::Scan => format!("reverse index, estimated {} visits", self.index_cost),
        })
    }
}

/// The edges the reverse index makes the first hop examine: all the edges
/// of every source of the target.
fn index_cost(graph: &CallGraph, root: &SymbolId) -> u64 {
    graph
        .callers(root)
        .iter()
        .map(|source| graph.dependencies(source).count() as u64)
        .sum()
}

struct Estimate {
    visits: u64,
    basis: EstimateBasis,
}

fn estimate_of(
    graph: &CallGraph,
    root: &SymbolId,
    query: &TypedQuery,
    plan: &Plan,
    estimator: Option<&CostEstimator>,
) -> Estimate {
    let measured = estimator
        .zip(query.work())
        .and_then(|(estimator, work)| estimator.estimate(&work));
    if let Some(visits) = measured {
        return Estimate {
            visits,
            basis: EstimateBasis::Measured,
        };
    }
    let first_hop: u64 = query
        .relations
        .iter()
        .map(|relation| match (relation.side(), plan.access) {
            (Side::Outgoing, _) => graph.dependencies(root).count() as u64,
            (Side::Incoming, Access::Index) => plan.index_cost,
            (Side::Incoming, Access::Scan) => plan.scan_cost,
        })
        .sum();
    let symbols = graph.symbol_count().max(1) as u64;
    let mean_degree = (graph.edge_count() as u64).div_ceil(symbols).max(1);
    let mut visits = 1 + first_hop;
    let mut hop = first_hop;
    for _ in 1..query.depth {
        hop = hop.saturating_mul(mean_degree);
        visits = visits.saturating_add(hop);
    }
    Estimate {
        visits,
        basis: EstimateBasis::Structural,
    }
}

/// Breadth-first from the root. Returns the rows reached and, when the
/// budget stopped the walk, the reason.
fn expand(
    graph: &CallGraph,
    root: &SymbolId,
    query: &TypedQuery,
    access: Access,
    meter: &mut Meter,
) -> (Vec<Row>, Option<String>) {
    let mut rows: Vec<Row> = Vec::new();
    let mut seen: HashSet<SymbolId> = HashSet::from([root.clone()]);
    let mut frontier: VecDeque<SymbolId> = VecDeque::from([root.clone()]);
    for depth in 1..=query.depth {
        let mut next: VecDeque<SymbolId> = VecDeque::new();
        for node in &frontier {
            for relation in &query.relations {
                let hits = match neighbours(graph, node, *relation, access, meter) {
                    Ok(hits) => hits,
                    Err(Stopped) => {
                        return (
                            rows,
                            Some(format!(
                                "budget: the ceiling of {} fact visits was reached at depth {depth}; \
                                 the rows are the ones found before it",
                                query.budget.unwrap_or_default()
                            )),
                        );
                    }
                };
                for hit in hits {
                    if seen.insert(hit.id.clone()) {
                        next.push_back(hit.id.clone());
                        rows.push(Row {
                            id: hit.id,
                            depth,
                            relation: *relation,
                            via: hit.via,
                            provenance: hit.provenance,
                            confidence: hit.confidence,
                        });
                    }
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    (rows, None)
}

struct Hit {
    id: SymbolId,
    via: DependencyType,
    provenance: Provenance,
    confidence: f64,
}

/// The symbols one relation reaches from one node, each once, with the
/// edge that carries the most confidence, ordered by symbol id.
fn neighbours(
    graph: &CallGraph,
    node: &SymbolId,
    relation: Relation,
    access: Access,
    meter: &mut Meter,
) -> Result<Vec<Hit>, Stopped> {
    let mut candidates: Vec<Hit> = Vec::new();
    match (relation.side(), access) {
        (Side::Outgoing, _) => {
            for (target, kind, provenance, confidence) in graph.dependencies_with_metadata(node) {
                meter.visit()?;
                if relation.admits(kind) {
                    candidates.push(Hit {
                        id: target.clone(),
                        via: *kind,
                        provenance,
                        confidence,
                    });
                }
            }
        }
        (Side::Incoming, Access::Index) => {
            for source in graph.callers(node) {
                for (target, kind, provenance, confidence) in
                    graph.dependencies_with_metadata(&source)
                {
                    meter.visit()?;
                    if target == node && relation.admits(kind) {
                        candidates.push(Hit {
                            id: source.clone(),
                            via: *kind,
                            provenance,
                            confidence,
                        });
                    }
                }
            }
        }
        (Side::Incoming, Access::Scan) => {
            for (source, target, kind, provenance, confidence) in
                graph.all_dependencies_with_metadata()
            {
                meter.visit()?;
                if target == node && relation.admits(kind) {
                    candidates.push(Hit {
                        id: source.clone(),
                        via: *kind,
                        provenance,
                        confidence,
                    });
                }
            }
        }
    }
    Ok(one_edge_per_symbol(candidates))
}

/// A symbol reached through several edges is one row, carried by the edge
/// with the most confidence; the first of them when they are equal.
fn one_edge_per_symbol(mut candidates: Vec<Hit>) -> Vec<Hit> {
    candidates.sort_by(|a, b| {
        a.id.cmp(&b.id)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
    });
    candidates.dedup_by(|later, earlier| later.id == earlier.id);
    candidates
}

fn filter(graph: &CallGraph, query: &TypedQuery, rows: Vec<Row>) -> Vec<Row> {
    rows.into_iter()
        .filter(|row| {
            let Some(symbol) = graph.get_symbol(&row.id) else {
                return false;
            };
            if query.exclude_tests && symbol.is_test() {
                return false;
            }
            query
                .module
                .as_deref()
                .is_none_or(|module| in_module(symbol.location().file(), module))
        })
        .collect()
}

/// Whether a file is declared in a module: the directory that holds it
/// ends with the module, component by component.
fn in_module(file: &str, module: &str) -> bool {
    std::path::Path::new(file)
        .parent()
        .is_some_and(|directory| directory.ends_with(module.trim()))
}

fn filter_description(query: &TypedQuery) -> String {
    let mut parts = Vec::new();
    if let Some(module) = &query.module {
        parts.push(format!("module `{}`", module.trim()));
    }
    if query.exclude_tests {
        parts.push("no tests".to_string());
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

fn top_k(query: &TypedQuery, mut rows: Vec<Row>) -> Vec<Row> {
    rows.sort_by(|a, b| {
        a.depth
            .cmp(&b.depth)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
            .then_with(|| a.id.cmp(&b.id))
    });
    if let Some(k) = query.top_k {
        rows.truncate(k);
    }
    rows
}
