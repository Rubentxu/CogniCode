//! El goal `context`: el workspace como contexto de trabajo.
//!
//! Dice qué hay en el workspace: los ficheros que la lectura encontró, los
//! lenguajes que se detectan y la cobertura de la lectura. Es el goal que se
//! llama antes de tocar nada.
//!
//! ## Lo que NO dice
//!
//! La cobertura que publica es la de la **lectura**: los ficheros que no se
//! pudieron leer. Las referencias que el grafo no pudo atar y las relaciones
//! que el repositorio rechazó las mide el grafo, y este goal no lo
//! construye.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::infrastructure::graph::source_files::inventory;
use crate::interface::coverage::CoverageInfo;
use crate::interface::goals::common::coverage_of;
use crate::interface::workspace::detect_languages;

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
/// Input for the `context` goal
pub struct ContextInput {
    /// Include the coverage of the read, with the files it could not read
    #[serde(default = "default_true")]
    pub include_coverage: bool,
}

impl Default for ContextInput {
    fn default() -> Self {
        ContextInput {
            include_coverage: default_true(),
        }
    }
}

fn default_true() -> bool {
    true
}

/// La respuesta del goal `context`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GoalContext {
    /// Una línea que resume el contexto.
    pub summary: String,
    /// La versión del esquema de esta respuesta.
    pub schema_version: String,
    /// El goal que contesta.
    pub goal: String,
    /// La raíz del workspace.
    pub path: String,
    /// Los ficheros que la lectura encontró y el producto puede leer.
    pub files: usize,
    /// Los lenguajes que se han detectado.
    pub languages: Vec<String>,
    /// La cobertura de la lectura, cuando se pide.
    pub coverage: Option<CoverageInfo>,
}

/// La respuesta del goal `context` para ese workspace.
pub fn context(input: &ContextInput, workspace: &Path) -> GoalContext {
    let inventory = inventory(workspace);
    let files = inventory.source_files().count();
    let languages: Vec<String> = detect_languages(workspace)
        .iter()
        .map(|language| language.name().to_string())
        .collect();
    let coverage = if input.include_coverage {
        Some(coverage_of(workspace))
    } else {
        None
    };

    let summary = format!(
        "{} file(s) readable, {} lenguaje(s) detectado(s).",
        files,
        languages.len()
    );

    GoalContext {
        summary,
        schema_version: "cognicode.goal.context/v1".to_string(),
        goal: "context".to_string(),
        path: workspace.to_string_lossy().to_string(),
        files,
        languages,
        coverage,
    }
}

/// La implementación del goal: lee sus argumentos y devuelve su respuesta.
pub fn run(workspace: &Path, arguments: &Value) -> Result<Value, String> {
    let input: ContextInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `context` are not valid: {error}"))?;
    let answer = context(&input, workspace);
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `context` is not serialisable: {error}"))
}

/// La respuesta del goal `context` en texto.
pub fn render(answer: &Value) -> String {
    let context: GoalContext = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `context` is the answer its own type describes");
    let mut text = String::new();
    text.push_str(&context.summary);
    text.push('\n');
    text.push_str(&format!("  workspace: {}\n", context.path));
    text.push_str(&format!("  files:    {}\n", context.files));
    text.push_str(&format!(
        "  languages: {}\n",
        if context.languages.is_empty() {
            "(none detected)".to_string()
        } else {
            context.languages.join(", ")
        }
    ));
    if let Some(coverage) = &context.coverage {
        text.push_str(&format!(
            "  coverage: {} ({} file(s) unreadable)\n",
            coverage.status,
            coverage.skipped_files.len()
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_answer_counts_the_files_the_read_found() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");
        std::fs::write(dir.path().join("notes.md"), "not code").expect("write");

        let answer = context(&ContextInput::default(), dir.path());

        assert_eq!(answer.goal, "context");
        assert_eq!(
            answer.files, 1,
            "the markdown file is not a source the product can read"
        );
        assert!(answer.coverage.is_some());
    }

    #[test]
    fn the_coverage_can_be_left_out() {
        let dir = tempfile::tempdir().expect("tempdir");
        let answer = context(
            &ContextInput {
                include_coverage: false,
            },
            dir.path(),
        );
        assert!(answer.coverage.is_none());
    }
}
