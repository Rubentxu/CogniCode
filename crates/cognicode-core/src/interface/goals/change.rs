//! El goal `change`: el cambio que se va a hacer.
//!
//! Es el único goal que escribe, y por eso es el único que no aparece en el
//! modo de sólo lectura.
//!
//! ## Qué hace hoy
//!
//! Renombra un símbolo. La acción la calcula el [`RefactorService`], que es
//! el dueño de los cambios: el goal no edita ficheros por su cuenta, pide el
//! cambio y publica lo que el dueño calculó —los ficheros que tocaría, los
//! símbolos que afecta y con qué riesgo—.
//!
//! Las demás acciones (extraer, mover, cambiar una firma) siguen en la tool
//! `safe_refactor`, que las calcula con el mismo servicio. Cuando el goal las
//! recoja, la tool se retira con su contrato de deprecación.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::application::commands::RenameSymbolCommand;
use crate::application::services::refactor_service::RefactorService;
use crate::interface::cli::project_graph::service_with_graph;
use crate::interface::goals::GoalEnvironment;

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
/// Input for the `change` goal
pub struct ChangeInput {
    /// The name of the symbol to change
    pub symbol: String,
    /// The new name of the symbol
    pub new_name: String,
    /// The file that declares the symbol, relative to the workspace or absolute
    pub file: Option<String>,
}

/// La respuesta del goal `change`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GoalChange {
    /// Una línea que resume el cambio.
    pub summary: String,
    /// La versión del esquema de esta respuesta.
    pub schema_version: String,
    /// El goal que contesta.
    pub goal: String,
    /// El símbolo que se cambia.
    pub symbol: String,
    /// El nombre nuevo.
    pub new_name: String,
    /// El fichero donde está declarado.
    pub file: String,
    /// Los ficheros que el cambio tocaría.
    pub files_to_modify: Vec<String>,
    /// Los símbolos que el cambio afecta.
    pub symbols_affected: Vec<String>,
    /// Cuántos cambios son.
    pub change_count: usize,
    /// El riesgo que el dueño le da al cambio.
    pub risk: String,
}

/// El fichero donde el símbolo está declarado.
///
/// Si el llamante no lo dice, lo dice el índice: el símbolo vive en un
/// fichero, y el índice es el que lo sabe.
fn file_of(workspace: &Path, symbol: &str, given: Option<&str>) -> Result<PathBuf, String> {
    if let Some(file) = given {
        let path = Path::new(file);
        return Ok(if path.is_absolute() {
            path.to_path_buf()
        } else {
            workspace.join(path)
        });
    }

    let (service, _built) = service_with_graph(workspace)?;
    let locations = service.find_symbol(symbol);
    match locations.first() {
        Some(location) => {
            let path = Path::new(&location.file);
            Ok(if path.is_absolute() {
                path.to_path_buf()
            } else {
                workspace.join(path)
            })
        }
        None => Err(format!(
            "`{symbol}` is not declared anywhere, so there is nothing to rename."
        )),
    }
}

/// La respuesta del goal `change` para ese símbolo.
pub fn change(
    workspace: &Path,
    symbol: &str,
    new_name: &str,
    file: Option<&str>,
) -> Result<GoalChange, String> {
    let path = file_of(workspace, symbol, file)?;
    let service = RefactorService::new();
    let command = RenameSymbolCommand::new(symbol, new_name, path.to_string_lossy().to_string());
    let preview = service
        .rename_symbol(command)
        .map_err(|error| format!("`{symbol}` could not be renamed: {error}"))?;

    let summary = format!(
        "renaming `{symbol}` to `{new_name}` touches {} file(s) and {} symbol(s) (risk: {}).",
        preview.files_to_modify.len(),
        preview.symbols_affected.len(),
        preview.risk_assessment
    );

    Ok(GoalChange {
        summary,
        schema_version: "cognicode.goal.change/v1".to_string(),
        goal: "change".to_string(),
        symbol: symbol.to_string(),
        new_name: new_name.to_string(),
        file: path.to_string_lossy().to_string(),
        files_to_modify: preview.files_to_modify,
        symbols_affected: preview.symbols_affected,
        change_count: preview.change_count,
        risk: preview.risk_assessment,
    })
}

/// La implementación del goal: lee sus argumentos y devuelve su respuesta.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let workspace = ctx.workspace;
    let input: ChangeInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `change` are not valid: {error}"))?;
    let answer = change(
        workspace,
        &input.symbol,
        &input.new_name,
        input.file.as_deref(),
    )?;
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `change` is not serialisable: {error}"))
}

/// La respuesta del goal `change` en texto.
pub fn render(answer: &Value) -> String {
    let changed: GoalChange = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `change` is the answer its own type describes");
    let mut text = String::new();
    text.push_str(&changed.summary);
    text.push('\n');
    for file in &changed.files_to_modify {
        text.push_str(&format!("  {file}\n"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_symbol_that_is_nowhere_cannot_be_changed() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn helper() {}").expect("write");

        let answer = change(dir.path(), "nowhere", "elsewhere", None);

        assert!(
            answer.is_err(),
            "a symbol that is not there is not an empty change: {answer:?}"
        );
    }
}
