//! El goal `trace`: quién llama a un símbolo y a quién llama.
//!
//! Sigue el grafo de llamadas alrededor de un símbolo. El grafo es el que el
//! producto ya calculó —el que sirve la terminal y el cable—, no uno que el
//! goal construye por su cuenta.
//!
//! ## De quién es la respuesta
//!
//! De la del servicio: el goal no recorre el grafo por su cuenta, pregunta
//! al dueño y publica lo que dijo.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::application::services::graph_operators::{
    Completeness, Explain, QueryError, Relation, Target, TypedQuery,
};
use crate::application::services::graph_queries::neighbours;
use crate::interface::coverage::CoverageInfo;
use crate::interface::goals::GoalEnvironment;
use crate::interface::goals::common::{GoalBasis, graph_basis};
use crate::interface::project_graph::service_with_graph;

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
/// Input for the `trace` goal
pub struct TraceInput {
    /// The name of the symbol to trace
    pub symbol: String,
    /// Which side to follow: `callers`, `callees`, `both`, `implementors`,
    /// `contracts` or `dependents`
    #[serde(default = "default_direction")]
    pub direction: String,
    /// How many hops to follow (at least 1)
    #[serde(default = "default_depth")]
    pub depth: u32,
    /// List only the symbols declared in this module (a directory, matched
    /// by path suffix). It restricts what is listed, not what is followed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// List only the best `top_k` neighbours: nearest first, then the most
    /// trusted edge
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<usize>,
    /// Spend at most this many fact visits (one per edge examined). A trace
    /// that reaches the ceiling stops and says it is partial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<u64>,
}

fn default_direction() -> String {
    "both".to_string()
}

fn default_depth() -> u32 {
    1
}

/// La respuesta del goal `trace`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GoalTrace {
    /// Una línea que resume la respuesta.
    pub summary: String,
    /// La versión del esquema de esta respuesta.
    pub schema_version: String,
    /// El goal que contesta.
    pub goal: String,
    /// El símbolo que se ha seguido.
    pub symbol: String,
    /// El lado que se ha seguido.
    pub direction: String,
    /// El veredicto de la respuesta: `partial` si la consulta se paró antes
    /// de mirar todo lo que su plan pedía, y si no, el de la cobertura del
    /// grafo que se leyó.
    pub status: String,
    /// Los vecinos que se han encontrado.
    pub entries: Vec<TraceEntry>,
    /// What the query did to find the entries: the operators it ran, the
    /// index it read through, what it estimated against what it spent, and
    /// whether it looked at everything its plan called for.
    pub explain: Explain,
    /// What the graph this answer was computed over covers: the files the
    /// walk could not read, the references without a declaration and the
    /// relations the repository refused.
    ///
    /// A neighbour that lives in a file the walk could not read is not in
    /// the answer, so the consumer needs the holes next to the trace.
    pub coverage: CoverageInfo,
    /// What the graph was built from, when the load kept it.
    ///
    /// `None` when the load kept no manifest: there is no digest to
    /// publish, and an empty one would read as a measured identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basis: Option<GoalBasis>,
}

/// Un vecino del símbolo, y a qué distancia está.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TraceEntry {
    /// El nombre del símbolo.
    pub name: String,
    /// El fichero donde está declarado.
    pub file: String,
    /// La línea (contada desde uno).
    pub line: u32,
    /// La columna (contada desde uno).
    pub column: u32,
    /// A qué distancia del símbolo está.
    pub depth: u32,
    /// Por qué lado se ha encontrado.
    pub direction: String,
}

/// Las relaciones que se siguen por un lado, con el nombre que el dueño usa.
fn relations_of(name: &str) -> Result<Vec<Relation>, String> {
    if name == "both" {
        return Ok(vec![Relation::Callers, Relation::Callees]);
    }
    Relation::parse(name)
        .map(|relation| vec![relation])
        .map_err(|_| {
            format!(
                "`{name}` is not a side to follow: the sides are `callers`, `callees`, `both`, \
             `implementors`, `contracts` and `dependents`."
            )
        })
}

/// Los límites de una traza: lo que se lista, cuánto y a qué coste.
#[derive(Debug, Clone, Default)]
pub struct TraceLimits {
    pub module: Option<String>,
    pub top_k: Option<usize>,
    pub budget: Option<u64>,
}

/// La respuesta del goal `trace` para ese símbolo.
pub fn trace(
    workspace: &Path,
    cache_root: Option<&Path>,
    symbol: &str,
    direction: &str,
    depth: u32,
    limits: &TraceLimits,
) -> Result<GoalTrace, String> {
    let relations = relations_of(direction)?;
    let (_service, built) = service_with_graph(workspace, cache_root)?;
    let mut query = TypedQuery::new(Target::Name(symbol.to_string()), relations);
    query = query.with_depth(depth);
    if let Some(module) = &limits.module {
        query = query.in_module(module.clone());
    }
    if let Some(k) = limits.top_k {
        query = query.with_top_k(k);
    }
    if let Some(visits) = limits.budget {
        query = query.with_budget(visits);
    }
    let (entries, explain) =
        neighbours(&built.graph, &query, None).map_err(|error| refusal(symbol, &error))?;

    let mut summary = format!(
        "`{symbol}` has {} neighbour(s) on the `{direction}` side.",
        entries.len()
    );
    if explain.matched > explain.listed {
        summary.push_str(&format!(
            " {} matched and the best {} are listed.",
            explain.matched, explain.listed
        ));
    }
    if let Completeness::Partial { reasons } = &explain.completeness {
        summary.push_str(&format!(" The answer is partial: {}.", reasons.join("; ")));
    }

    let coverage = CoverageInfo::of(&built.coverage);
    let status = match explain.completeness {
        Completeness::Partial { .. } => "partial".to_string(),
        Completeness::Complete => coverage.status.clone(),
    };

    Ok(GoalTrace {
        summary,
        status,
        schema_version: "cognicode.goal.trace/v1".to_string(),
        goal: "trace".to_string(),
        symbol: symbol.to_string(),
        direction: direction.to_string(),
        entries: entries
            .into_iter()
            .map(|entry| TraceEntry {
                name: entry.name,
                file: entry.file,
                line: entry.line,
                column: entry.column,
                depth: entry.depth,
                direction: entry.relation.as_str().to_string(),
            })
            .collect(),
        explain,
        coverage,
        basis: graph_basis(workspace, &built),
    })
}

fn refusal(symbol: &str, error: &QueryError) -> String {
    match error {
        QueryError::Invalid(invalid) => format!("the trace of `{symbol}` is not valid: {invalid}"),
        QueryError::Graph(error) => format!("`{symbol}` could not be traced: {error}"),
    }
}

/// La implementación del goal: lee sus argumentos y devuelve su respuesta.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let workspace = ctx.workspace;
    let input: TraceInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `trace` are not valid: {error}"))?;
    let limits = TraceLimits {
        module: input.module,
        top_k: input.top_k,
        budget: input.budget,
    };
    let answer = trace(
        workspace,
        ctx.cache_root,
        &input.symbol,
        &input.direction,
        input.depth,
        &limits,
    )?;
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `trace` is not serialisable: {error}"))
}

/// La respuesta del goal `trace` en texto.
pub fn render(answer: &Value) -> String {
    let traced: GoalTrace = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `trace` is the answer its own type describes");
    let mut text = String::new();
    text.push_str(&traced.summary);
    text.push('\n');
    for entry in &traced.entries {
        text.push_str(&format!(
            "  {}:{}:{} {} ({} side, depth {})\n",
            entry.file, entry.line, entry.column, entry.name, entry.direction, entry.depth
        ));
    }
    text.push_str(&plan_line(&traced.explain));
    text
}

/// La línea que dice cómo se encontró la respuesta y si es entera.
fn plan_line(explain: &Explain) -> String {
    let operators: Vec<String> = explain
        .steps
        .iter()
        .map(|step| format!("{:?}", step.operator).to_lowercase())
        .collect();
    let ceiling = explain
        .budget
        .map(|budget| format!(" of a budget of {budget}"))
        .unwrap_or_default();
    let state = match explain.completeness {
        Completeness::Complete => "complete",
        Completeness::Partial { .. } => "partial",
    };
    format!(
        "  plan: {}; {} visit(s) spent{ceiling}, {} estimated; {state}\n",
        operators.join(" > "),
        explain.spent_visits,
        explain.estimated_visits
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_side_that_does_not_exist_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let answer = trace(
            dir.path(),
            None,
            "helper",
            "sideways",
            1,
            &TraceLimits::default(),
        );

        assert!(answer.is_err(), "an unknown side is not a quiet answer");
    }
}
