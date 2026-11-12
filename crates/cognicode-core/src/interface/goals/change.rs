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
use std::path::Path;

use crate::application::commands::RenameSymbolCommand;
use crate::application::services::refactor_service::RefactorService;
use crate::interface::goals::GoalEnvironment;
use crate::interface::path_authorisation::authorise_workspace_file;
use crate::interface::project_graph::service_with_graph;

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

/// La respuesta del goal `change` para ese símbolo.
pub fn change(
    workspace: &Path,
    symbol: &str,
    new_name: &str,
    file: Option<&str>,
) -> Result<GoalChange, String> {
    // Un solo grafo para todo el goal: el que el índice consulta para
    // saber dónde vive el símbolo es el que mide el impacto del rename
    // (TASK-223).
    let (service, built) = service_with_graph(workspace)?;
    let path = match file {
        Some(given) => authorise_workspace_file(workspace, given)?,
        None => {
            let locations = service.find_symbol(symbol, workspace).locations;
            match locations.first() {
                Some(location) => authorise_workspace_file(workspace, &location.file)?,
                None => {
                    return Err(format!(
                        "`{symbol}` is not declared anywhere, so there is nothing to rename."
                    ));
                }
            }
        }
    };
    let basis = crate::application::services::refactor_service::RefactorBasis {
        graph: built.graph,
        coverage: built.coverage,
        workspace_root: workspace.to_path_buf(),
    };
    let refactor = RefactorService::new();
    let command = RenameSymbolCommand::new(symbol, new_name, path.to_string_lossy().to_string());
    let preview = refactor
        .rename_symbol(command, &basis)
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

    // TASK-218 AC #4 — the legitimate route keeps working: a file inside
    // the workspace, named by the caller, is still renamed.
    #[test]
    fn a_file_inside_the_workspace_is_still_changed() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn helper() {}").expect("write");

        let answer = change(dir.path(), "helper", "renamed", Some("main.rs"))
            .expect("a file inside the workspace is renamed");

        assert_eq!(answer.file, dir.path().join("main.rs").to_string_lossy());
        assert_eq!(answer.change_count, 1);
    }

    // TASK-218 — the file the caller names is confined to the workspace:
    // `../` and an absolute path outside are refused before the refactor
    // service reads a byte, and the refusal names the workspace boundary.
    #[test]
    fn a_file_outside_the_workspace_cannot_be_changed() {
        let parent = tempfile::tempdir().expect("tempdir");
        let work = parent.path().join("work");
        let outside = parent.path().join("outside");
        std::fs::create_dir(&work).expect("work dir");
        std::fs::create_dir(&outside).expect("outside dir");
        std::fs::write(work.join("main.rs"), "fn helper() {}").expect("write");
        std::fs::write(outside.join("helper.rs"), "fn helper() {}").expect("write");

        let relative = change(&work, "helper", "renamed", Some("../outside/helper.rs"));
        let absolute = change(
            &work,
            "helper",
            "renamed",
            Some(&outside.join("helper.rs").to_string_lossy()),
        );

        assert!(
            relative.is_err(),
            "a relative escape cannot be renamed: {relative:?}"
        );
        assert!(
            absolute.is_err(),
            "an absolute path outside cannot be renamed: {absolute:?}"
        );
        // La absoluta es el caso de frontera pura: el rechazo dice qué
        // mundo se dejó, y no es un accidente de I/O.
        let message = absolute.expect_err("already asserted");
        assert!(
            message.contains("outside the workspace"),
            "the refusal names the workspace boundary: {message}"
        );
    }
}
