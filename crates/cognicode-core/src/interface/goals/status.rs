//! El goal `status`: qué ofrece este build y qué se ve del workspace.
//!
//! Es el primero de los goals porque es la raíz: un cliente lo llama antes de
//! decidir, y `cognicode` sin argumentos es este goal.
//!
//! ## Lo que responde
//!
//! - **las fases** del producto y lo que cada una significa;
//! - **los lenguajes** que se han detectado en el workspace;
//! - **la cobertura** de la lectura: los ficheros que no se pudieron leer;
//! - **los goals** que este build ofrece, con sus plantillas de argumentos.
//!
//! ## Lo que NO dice
//!
//! La cobertura que publica es la de la **lectura**. Las referencias que el
//! grafo no pudo atar y las relaciones que el repositorio rechazó las mide
//! el grafo, y este goal no lo construye: publicarlas vacías sería publicar
//! una cuenta que nadie midió.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::interface::coverage::CoverageInfo;
use crate::interface::goals::GoalEnvironment;
use crate::interface::goals::common::coverage_of;
use crate::interface::goals::{GoalInfo, GoalPhase, goal_info, registry};
use crate::interface::workspace::detect_languages;

// El struct es la declaración de los argumentos: de él sale el
// `inputSchema` que publica el MCP, y de él salen los argumentos del
// comando clap. El doc que lleva es corto a propósito: el esquema lo
// publica, y un esquema con un párrafo dentro es un esquema que nadie
// lee.
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
/// Input for the `status` goal
pub struct StatusInput {
    /// Include the goals that this build offers, with their argument templates
    #[serde(default = "default_true")]
    pub include_goals: bool,
}

impl Default for StatusInput {
    /// El valor por defecto es el mismo que el que se deriva de los
    /// argumentos ausentes: si el struct por defecto dijera otra cosa,
    /// un llamante que no manda el argumento y otro que lo manda vacío
    /// no serían la misma pregunta.
    fn default() -> Self {
        StatusInput {
            include_goals: default_true(),
        }
    }
}

fn default_true() -> bool {
    true
}

/// La respuesta del goal `status`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GoalStatus {
    /// Una línea que resume el estado.
    pub summary: String,
    /// La versión del esquema de esta respuesta.
    pub schema_version: String,
    /// El goal que contesta.
    pub goal: String,
    /// La versión del producto que contesta.
    pub version: String,
    /// Las fases del producto y lo que cada una significa.
    pub phases: Vec<PhaseInfo>,
    /// El workspace que se ha mirado.
    pub workspace: WorkspaceStatus,
    /// La cobertura de la lectura del workspace.
    pub coverage: CoverageInfo,
    /// Los goals que este build ofrece.
    pub goals: Vec<GoalInfo>,
}

/// Una fase del producto.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PhaseInfo {
    /// El nombre de la fase.
    pub name: String,
    /// Lo que la fase significa.
    pub description: String,
}

/// El workspace que se ha mirado.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkspaceStatus {
    /// La raíz del workspace.
    pub path: String,
    /// Los lenguajes que se han detectado en él.
    pub languages: Vec<String>,
}

/// La respuesta del goal `status` para ese workspace.
///
/// La misma función la llaman el MCP y el CLI: el sobre que devuelve una
/// superficie es el mismo sobre que devuelve la otra.
pub fn status(input: &StatusInput, workspace: &Path) -> GoalStatus {
    let languages: Vec<String> = detect_languages(workspace)
        .iter()
        .map(|language| language.name().to_string())
        .collect();

    let phases: Vec<PhaseInfo> = GoalPhase::ALL
        .iter()
        .map(|phase| PhaseInfo {
            name: phase.as_str().to_string(),
            description: phase.description().to_string(),
        })
        .collect();

    let goals = if input.include_goals {
        registry().iter().map(goal_info).collect()
    } else {
        Vec::new()
    };

    let coverage = coverage_of(workspace);
    let version = env!("CARGO_PKG_VERSION");
    let summary = format!(
        "CogniCode {version}: {} goal(s), workspace con {} lenguaje(s) detectado(s), \
cobertura {}.",
        goals.len(),
        languages.len(),
        coverage.status
    );

    GoalStatus {
        summary,
        schema_version: "cognicode.goal.status/v1".to_string(),
        goal: "status".to_string(),
        version: version.to_string(),
        phases,
        workspace: WorkspaceStatus {
            path: workspace.to_string_lossy().to_string(),
            languages,
        },
        coverage,
        goals,
    }
}

/// La implementación del goal: lee sus argumentos y devuelve su respuesta.
///
/// La firma la firma el registro ([`crate::interface::goals::GoalSpec::run`]):
/// el workspace donde se mira y los argumentos del llamante.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let workspace = ctx.workspace;
    let input: StatusInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `status` are not valid: {error}"))?;
    let answer = status(&input, workspace);
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `status` is not serialisable: {error}"))
}

/// La respuesta del goal `status` en texto.
///
/// La respuesta llega como el JSON que el goal produce; aquí se vuelve a
/// leer con su propio tipo, que es el único que sabe qué significa cada
/// campo. Un goal que devuelve una respuesta que no puede explicar es un
/// goal que no se puede leer.
pub fn render(answer: &Value) -> String {
    let status: GoalStatus = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `status` is the answer its own type describes");
    render_text(&status)
}

/// La respuesta del goal `status` en texto.
///
/// El texto lo compone el core y lo escribe el presenter: un core que
/// escribe su propia respuesta puede intercalarla con lo que el consumidor
/// está parseando.
pub fn render_text(answer: &GoalStatus) -> String {
    let mut text = String::new();
    text.push_str(&answer.summary);
    text.push('\n');
    text.push_str(&format!("  workspace: {}\n", answer.workspace.path));
    if answer.workspace.languages.is_empty() {
        text.push_str("  languages: (none detected)\n");
    } else {
        text.push_str(&format!(
            "  languages: {}\n",
            answer.workspace.languages.join(", ")
        ));
    }
    text.push_str(&format!(
        "  phases:   {}\n",
        answer
            .phases
            .iter()
            .map(|phase| phase.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    text.push_str(&format!(
        "  coverage: {} ({} file(s) unreadable)\n",
        answer.coverage.status,
        answer.coverage.skipped_files.len()
    ));
    for goal in &answer.goals {
        text.push_str(&format!(
            "  goal {} ({}): {}\n",
            goal.name, goal.authority, goal.title
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_answer_carries_the_goals_and_the_phases() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\n").expect("write");

        let answer = status(
            &StatusInput {
                include_goals: true,
            },
            dir.path(),
        );

        assert_eq!(answer.goal, "status");
        assert_eq!(answer.workspace.languages, vec!["Rust".to_string()]);
        assert_eq!(answer.phases.len(), GoalPhase::ALL.len());
        assert_eq!(answer.goals.len(), registry().len());
        assert!(!answer.summary.is_empty());
    }

    #[test]
    fn the_answer_carries_the_coverage_of_the_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");

        let answer = status(
            &StatusInput {
                include_goals: false,
            },
            dir.path(),
        );

        assert_eq!(
            answer.coverage.status, "complete",
            "a tree that can be read has a complete coverage"
        );
        assert!(answer.coverage.skipped_files.is_empty());
    }

    #[test]
    fn the_goals_can_be_left_out() {
        let dir = tempfile::tempdir().expect("tempdir");
        let answer = status(
            &StatusInput {
                include_goals: false,
            },
            dir.path(),
        );
        assert!(answer.goals.is_empty());
        assert!(!answer.summary.is_empty());
    }
}
