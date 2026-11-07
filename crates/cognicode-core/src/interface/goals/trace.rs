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

use crate::application::services::analysis_service::AnalysisService;
use crate::application::services::graph_queries::Direction;
use crate::interface::goals::GoalEnvironment;
use crate::interface::project_graph::service_with_graph;

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
/// Input for the `trace` goal
pub struct TraceInput {
    /// The name of the symbol to trace
    pub symbol: String,
    /// Which side to follow: `callers`, `callees` or `both`
    #[serde(default = "default_direction")]
    pub direction: String,
    /// How many hops to follow
    #[serde(default = "default_depth")]
    pub depth: u32,
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
    /// Los vecinos que se han encontrado.
    pub entries: Vec<TraceEntry>,
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

/// El lado que se sigue, con el nombre que el dueño usa.
fn direction_of(name: &str) -> Result<Direction, String> {
    match name {
        "callers" => Ok(Direction::Callers),
        "callees" => Ok(Direction::Callees),
        "both" => Ok(Direction::Both),
        other => Err(format!(
            "`{other}` is not a side to follow: the sides are `callers`, `callees` and `both`."
        )),
    }
}

/// La respuesta del goal `trace` para ese símbolo.
pub fn trace(
    workspace: &Path,
    symbol: &str,
    direction: &str,
    depth: u32,
) -> Result<GoalTrace, String> {
    let side = direction_of(direction)?;
    let (_service, built) = service_with_graph(workspace)?;
    let entries = AnalysisService::call_hierarchy(&built.graph, symbol, depth, side)
        .map_err(|error| format!("`{symbol}` could not be traced: {error}"))?;

    let summary = format!(
        "`{symbol}` has {} neighbour(s) on the `{direction}` side.",
        entries.len()
    );

    Ok(GoalTrace {
        summary,
        schema_version: "cognicode.goal.trace/v1".to_string(),
        goal: "trace".to_string(),
        symbol: symbol.to_string(),
        direction: direction.to_string(),
        entries: entries
            .into_iter()
            .map(|entry| TraceEntry {
                name: entry.name,
                file: entry.file,
                line: entry.line + 1,
                column: entry.column + 1,
                depth: entry.depth,
                direction: entry.direction.as_str().to_string(),
            })
            .collect(),
    })
}

/// La implementación del goal: lee sus argumentos y devuelve su respuesta.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let workspace = ctx.workspace;
    let input: TraceInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `trace` are not valid: {error}"))?;
    let answer = trace(workspace, &input.symbol, &input.direction, input.depth)?;
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
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_side_that_does_not_exist_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let answer = trace(dir.path(), "helper", "sideways", 1);

        assert!(answer.is_err(), "an unknown side is not a quiet answer");
    }
}
