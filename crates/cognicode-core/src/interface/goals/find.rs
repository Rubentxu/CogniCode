//! El goal `find`: dónde está un símbolo.
//!
//! Resuelve un nombre a los sitios donde está declarado. Es la pregunta del
//! homónimo: el mismo nombre puede estar en dos ficheros, y un goal que
//! devolviera sólo uno publicaría media verdad.
//!
//! ## De quién es la respuesta
//!
//! De la índices del servicio: el goal no cuenta los símbolos por su cuenta,
//! pregunta al dueño y publica lo que dijo.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::infrastructure::graph::source_files::inventory;
use crate::interface::cli::project_graph::service_with_graph;
use crate::interface::goals::GoalEnvironment;

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
/// Input for the `find` goal
pub struct FindInput {
    /// The name of the symbol to find
    pub symbol: String,
}

/// La respuesta del goal `find`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GoalFind {
    /// Una línea que resume la respuesta.
    pub summary: String,
    /// La versión del esquema de esta respuesta.
    pub schema_version: String,
    /// El goal que contesta.
    pub goal: String,
    /// El nombre que se buscó.
    pub symbol: String,
    /// Los sitios donde el nombre está declarado.
    pub locations: Vec<FoundSymbol>,
}

/// Un sitio donde el nombre está declarado.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FoundSymbol {
    /// El nombre del símbolo.
    pub name: String,
    /// El tipo del símbolo, con el nombre que le da el dominio.
    pub kind: String,
    /// El fichero donde está declarado.
    pub file: String,
    /// La línea (contada desde uno).
    pub line: u32,
    /// La columna (contada desde uno).
    pub column: u32,
}

/// La respuesta del goal `find` para ese símbolo.
pub fn find(workspace: &Path, symbol: &str) -> Result<GoalFind, String> {
    let (service, _built) = service_with_graph(workspace)?;
    let locations: Vec<FoundSymbol> = service
        .find_symbol(symbol)
        .into_iter()
        .map(|location| FoundSymbol {
            name: symbol.to_string(),
            kind: location.symbol_kind.to_string(),
            file: location.file.clone(),
            line: location.line + 1,
            column: location.column + 1,
        })
        .collect();

    let summary = if locations.is_empty() {
        let mut text = format!("`{symbol}` is not in the project's symbols.");
        text.push_str(&unindexed_note(workspace));
        text
    } else {
        format!("`{symbol}` is declared in {} place(s).", locations.len())
    };

    Ok(GoalFind {
        summary,
        schema_version: "cognicode.goal.find/v1".to_string(),
        goal: "find".to_string(),
        symbol: symbol.to_string(),
        locations,
    })
}

/// Lo que una respuesta vacía no puede afirmar: los tipos que el índice no
/// lee en los lenguajes del proyecto. Vacío si no falta ninguno.
fn unindexed_note(workspace: &Path) -> String {
    let mut gaps: Vec<String> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for file in inventory(workspace).source_files() {
        let Some(language) = file.language else {
            continue;
        };
        let kinds = language.unread_class_kinds();
        if kinds.is_empty() || !seen.insert(language.name()) {
            continue;
        }
        gaps.push(format!("{} ({})", language.name(), kinds.join(", ")));
    }
    if gaps.is_empty() {
        return String::new();
    }
    format!(
        " The answer is not complete: these declarations are not indexed: {}.",
        gaps.join("; ")
    )
}

/// La implementación del goal: lee sus argumentos y devuelve su respuesta.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let workspace = ctx.workspace;
    let input: FindInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `find` are not valid: {error}"))?;
    let answer = find(workspace, &input.symbol)?;
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `find` is not serialisable: {error}"))
}

/// La respuesta del goal `find` en texto.
pub fn render(answer: &Value) -> String {
    let found: GoalFind = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `find` is the answer its own type describes");
    let mut text = String::new();
    text.push_str(&found.summary);
    text.push('\n');
    for location in &found.locations {
        text.push_str(&format!(
            "  {}:{}:{} {} ({})\n",
            location.file, location.line, location.column, location.name, location.kind
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_symbol_that_is_not_there_says_so() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");

        let answer = find(dir.path(), "nowhere").expect("the goal answers");

        assert!(answer.locations.is_empty());
        assert!(answer.summary.contains("not in the project's symbols"));
    }

    #[test]
    fn a_declared_type_is_found_or_the_answer_does_not_claim_completeness() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("lib.rs"),
            "pub enum SurfaceProfile { A, B }\npub struct Cfg { pub p: SurfaceProfile }\n",
        )
        .expect("write");

        for name in ["SurfaceProfile", "Cfg"] {
            let answer = find(dir.path(), name).expect("the goal answers");
            assert!(
                !answer.locations.is_empty(),
                "`{name}` is declared in lib.rs but find says: {}",
                answer.summary
            );
        }
    }

    #[test]
    fn an_empty_answer_names_the_kinds_the_index_does_not_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("a.c"),
            "typedef int T;\nint f(void){return 0;}\n",
        )
        .expect("write");

        let answer = find(dir.path(), "nowhere").expect("the goal answers");

        assert!(answer.locations.is_empty());
        assert!(
            answer.summary.contains("not indexed") && answer.summary.contains("C"),
            "an empty answer over a language with unread kinds must say so: {}",
            answer.summary
        );
    }

    #[test]
    fn an_empty_answer_over_a_fully_read_language_does_not_hedge() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("k.kt"), "class K\n").expect("write");

        let answer = find(dir.path(), "nowhere").expect("the goal answers");

        assert!(
            !answer.summary.contains("not indexed"),
            "{}",
            answer.summary
        );
    }

    #[test]
    fn a_symbol_that_is_there_says_where() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn helper() {}\nfn main() {}").expect("write");

        let answer = find(dir.path(), "helper").expect("the goal answers");

        assert_eq!(answer.locations.len(), 1);
        assert!(answer.locations[0].file.ends_with("main.rs"));
        assert_eq!(answer.locations[0].line, 1);
    }
}
