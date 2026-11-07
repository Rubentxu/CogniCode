//! El goal `overview`: cómo es el proyecto.
//!
//! Resume el proyecto que hay en el workspace: cuántos símbolos tiene,
//! cuántas aristas, cuántos ficheros y en qué lenguajes.
//!
//! ## De quién es la respuesta
//!
//! De la del servicio: las estadísticas las calcula el dueño —
//! [`crate::application::services::analysis_service::AnalysisService`]— y el
//! goal las publica. Los lenguajes se ordenan por nombre: una respuesta que
//! cambia de orden entre dos llamadas no es la misma respuesta.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::interface::cli::project_graph::service_with_graph;
use crate::interface::coverage::CoverageInfo;
use crate::interface::goals::GoalEnvironment;
use crate::interface::goals::common::coverage_of;

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
/// Input for the `overview` goal
pub struct OverviewInput {}

/// La respuesta del goal `overview`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GoalOverview {
    /// Una línea que resume el proyecto.
    pub summary: String,
    /// La versión del esquema de esta respuesta.
    pub schema_version: String,
    /// El goal que contesta.
    pub goal: String,
    /// Los símbolos que el proyecto tiene.
    pub symbols: usize,
    /// Las aristas que el proyecto tiene.
    pub edges: usize,
    /// Los ficheros donde viven esos símbolos.
    pub files: usize,
    /// Los símbolos por lenguaje, ordenados por nombre.
    pub languages: Vec<LanguageCount>,
    /// La cobertura de la lectura del workspace.
    pub coverage: CoverageInfo,
}

/// Cuántos símbolos hay en un lenguaje.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LanguageCount {
    /// El nombre del lenguaje.
    pub name: String,
    /// Cuántos símbolos tiene.
    pub symbols: usize,
}

/// La respuesta del goal `overview` para ese workspace.
pub fn overview(workspace: &Path) -> Result<GoalOverview, String> {
    let (service, _built) = service_with_graph(workspace)?;
    let stats = service.get_graph_stats();

    let mut languages: Vec<LanguageCount> = stats
        .language_breakdown
        .into_iter()
        .map(|(name, symbols)| LanguageCount { name, symbols })
        .collect();
    languages.sort_by(|a, b| a.name.cmp(&b.name));

    let summary = format!(
        "{} symbol(s), {} edge(s), {} file(s), {} lenguaje(s).",
        stats.symbol_count,
        stats.edge_count,
        stats.file_count,
        languages.len()
    );

    Ok(GoalOverview {
        summary,
        schema_version: "cognicode.goal.overview/v1".to_string(),
        goal: "overview".to_string(),
        symbols: stats.symbol_count,
        edges: stats.edge_count,
        files: stats.file_count,
        languages,
        coverage: coverage_of(workspace),
    })
}

/// La implementación del goal: lee sus argumentos y devuelve su respuesta.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let workspace = ctx.workspace;
    serde_json::from_value::<OverviewInput>(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `overview` are not valid: {error}"))?;
    let answer = overview(workspace)?;
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `overview` is not serialisable: {error}"))
}

/// La respuesta del goal `overview` en texto.
pub fn render(answer: &Value) -> String {
    let overview: GoalOverview = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `overview` is the answer its own type describes");
    let mut text = String::new();
    text.push_str(&overview.summary);
    text.push('\n');
    for language in &overview.languages {
        text.push_str(&format!(
            "  {}: {} symbol(s)\n",
            language.name, language.symbols
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_answer_counts_what_the_project_has() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("main.rs"),
            "fn helper() {}\nfn main() { helper() }",
        )
        .expect("write");

        let answer = overview(dir.path()).expect("the goal answers");

        assert_eq!(answer.goal, "overview");
        assert_eq!(answer.symbols, 2);
        assert_eq!(answer.edges, 1);
        assert_eq!(answer.languages.len(), 1);
        assert_eq!(answer.languages[0].name, "Rust");
    }

    #[test]
    fn the_languages_come_in_the_same_order_every_time() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");
        std::fs::write(dir.path().join("app.py"), "def main():\n    pass\n").expect("write");

        let first = overview(dir.path()).expect("the goal answers");
        let second = overview(dir.path()).expect("the goal answers");

        let names = |overview: &GoalOverview| {
            overview
                .languages
                .iter()
                .map(|language| language.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&first), names(&second));
    }
}
