//! El goal `inspect`: qué declara un fichero.
//!
//! Parsea el fichero que se le nombra y publica los símbolos que declara.
//! No necesita el grafo entero: el fichero es la unidad de trabajo.
//!
//! ## De quién es la respuesta
//!
//! De la del servicio: el goal no extrae símbolos por su cuenta, pregunta al
//! dueño y publica su payload. La forma de cada símbolo es la que el
//! producto publica para todos los consumidores.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::application::dto::SymbolDto;
use crate::application::services::analysis_service::AnalysisService;

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
/// Input for the `inspect` goal
pub struct InspectInput {
    /// The file to inspect, relative to the workspace or absolute
    pub file: String,
}

/// La respuesta del goal `inspect`.
///
/// Los símbolos son los del servicio (`SymbolDto`), sin copiarlos: el goal
/// publica la forma que el producto publica para todos los consumidores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalInspect {
    /// Una línea que resume la respuesta.
    pub summary: String,
    /// La versión del esquema de esta respuesta.
    pub schema_version: String,
    /// El goal que contesta.
    pub goal: String,
    /// El fichero que se ha inspeccionado.
    pub file: String,
    /// Los símbolos que el fichero declara.
    pub symbols: Vec<SymbolDto>,
}

/// El fichero que se nombra, resuelto contra el workspace.
///
/// Un camino relativo se resuelve contra el workspace: el goal habla del
/// workspace del llamante, no del directorio desde el que se teclea.
fn resolve(workspace: &Path, file: &str) -> PathBuf {
    let path = Path::new(file);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace.join(path)
    }
}

/// La respuesta del goal `inspect` para ese fichero.
pub fn inspect(workspace: &Path, file: &str) -> Result<GoalInspect, String> {
    let path = resolve(workspace, file);
    let service = AnalysisService::new();
    let symbols = service.get_file_symbols(&path).map_err(|error| {
        format!(
            "the file `{}` could not be read: {error}",
            path.to_string_lossy()
        )
    })?;

    let summary = format!(
        "`{}` declares {} symbol(s).",
        path.to_string_lossy(),
        symbols.len()
    );

    Ok(GoalInspect {
        summary,
        schema_version: "cognicode.goal.inspect/v1".to_string(),
        goal: "inspect".to_string(),
        file: path.to_string_lossy().to_string(),
        symbols,
    })
}

/// La implementación del goal: lee sus argumentos y devuelve su respuesta.
pub fn run(workspace: &Path, arguments: &Value) -> Result<Value, String> {
    let input: InspectInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `inspect` are not valid: {error}"))?;
    let answer = inspect(workspace, &input.file)?;
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `inspect` is not serialisable: {error}"))
}

/// La respuesta del goal `inspect` en texto.
pub fn render(answer: &Value) -> String {
    let inspected: GoalInspect = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `inspect` is the answer its own type describes");
    let mut text = String::new();
    text.push_str(&inspected.summary);
    text.push('\n');
    for symbol in &inspected.symbols {
        text.push_str(&format!(
            "  {}:{} {} ({})\n",
            symbol.file_path, symbol.line, symbol.name, symbol.kind
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_named_is_the_file_inspected() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn helper() {}\nfn main() {}").expect("write");

        let answer = inspect(dir.path(), "main.rs").expect("the goal answers");

        assert_eq!(answer.symbols.len(), 2);
        assert!(answer.file.ends_with("main.rs"));
        assert!(
            answer.symbols.iter().any(|symbol| symbol.name == "helper"),
            "the goal publishes what the file declares"
        );
    }

    #[test]
    fn a_file_that_cannot_be_read_says_so() {
        let dir = tempfile::tempdir().expect("tempdir");

        let answer = inspect(dir.path(), "nowhere.rs");

        assert!(
            answer.is_err(),
            "a file that is not there is not an empty answer: {answer:?}"
        );
    }
}
