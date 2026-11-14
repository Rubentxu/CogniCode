//! El goal `impact`: qué rompe un cambio.
//!
//! Es la pregunta que se hace antes de tocar nada: si se cambia este
//! símbolo, qué se ve afectado y con qué riesgo.
//!
//! ## De quién es la respuesta
//!
//! De la del servicio: el impacto tiene un solo dueño —
//! [`crate::application::services::impact_of::impact_of`]— y el goal
//! publica lo que dijo. Un goal que contara los dependientes por su cuenta
//! publicaría una segunda regla de impacto, y dos reglas de impacto dan dos
//! verdades.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::application::services::analysis_service::AnalysisService;
use crate::interface::coverage::CoverageInfo;
use crate::interface::goals::GoalEnvironment;
use crate::interface::goals::common::{GoalBasis, graph_basis};
use crate::interface::project_graph::service_with_graph;

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
/// Input for the `impact` goal
pub struct ImpactInput {
    /// The name of the symbol whose impact is asked for
    pub symbol: String,
}

/// La respuesta del goal `impact`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GoalImpact {
    /// Una línea que resume la respuesta.
    pub summary: String,
    /// La versión del esquema de esta respuesta.
    pub schema_version: String,
    /// El goal que contesta.
    pub goal: String,
    /// El símbolo que se ha preguntado.
    pub symbol: String,
    /// Cuántos símbolos del grafo llevan ese nombre.
    pub matched: usize,
    /// Los símbolos que dependen del que se cambió.
    pub symbols: Vec<String>,
    /// Los ficheros en los que viven esos dependientes.
    pub files: Vec<String>,
    /// El riesgo que el dueño le da al cambio.
    pub risk: String,
    /// What the graph this answer was computed over covers: the files the
    /// walk could not read, the references without a declaration and the
    /// relations the repository refused.
    ///
    /// An impact over a tree that was not read whole is an impact nobody can
    /// trust: a dependent that lives in a file the walk could not read is
    /// not in the answer, and the consumer needs the holes next to the
    /// verdict to know that.
    pub coverage: CoverageInfo,
    /// What the graph was built from, when the load kept it.
    ///
    /// `None` when the load kept no manifest: there is no digest to
    /// publish, and an empty one would read as a measured identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basis: Option<GoalBasis>,
}

/// La respuesta del goal `impact` para ese símbolo.
pub fn impact(workspace: &Path, symbol: &str) -> Result<GoalImpact, String> {
    let (_service, built) = service_with_graph(workspace)?;
    // The refusal names the symbol, and for a homonym it names the
    // candidates: the caller writes the identity it means and asks again.
    let impact =
        AnalysisService::impact_of(&built.graph, symbol).map_err(|refusal| refusal.to_string())?;

    // El riesgo lo publica el dueño con su propia palabra: el goal no
    // decide qué nivel es «alto».
    let risk = serde_json::to_value(impact.risk)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string());

    let summary = format!(
        "changing `{symbol}` touches {} symbol(s) in {} file(s) (risk: {risk}).",
        impact.symbols.len(),
        impact.files.len()
    );

    Ok(GoalImpact {
        summary,
        schema_version: "cognicode.goal.impact/v1".to_string(),
        goal: "impact".to_string(),
        symbol: symbol.to_string(),
        matched: impact.matched,
        symbols: impact.symbols,
        files: impact.files,
        risk,
        coverage: CoverageInfo::of(&built.coverage),
        basis: graph_basis(workspace, &built),
    })
}

/// La implementación del goal: lee sus argumentos y devuelve su respuesta.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let workspace = ctx.workspace;
    let input: ImpactInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `impact` are not valid: {error}"))?;
    let answer = impact(workspace, &input.symbol)?;
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `impact` is not serialisable: {error}"))
}

/// La respuesta del goal `impact` en texto.
pub fn render(answer: &Value) -> String {
    let impacted: GoalImpact = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `impact` is the answer its own type describes");
    let mut text = String::new();
    text.push_str(&impacted.summary);
    text.push('\n');
    for file in &impacted.files {
        text.push_str(&format!("  {file}\n"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_symbol_that_is_not_in_the_graph_says_so() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");

        let answer = impact(dir.path(), "nowhere");

        assert!(
            answer.is_err(),
            "a symbol that is not there is not an empty impact: {answer:?}"
        );
    }
}
