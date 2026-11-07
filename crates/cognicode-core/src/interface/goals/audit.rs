//! El goal `audit`: qué está mal en el proyecto.
//!
//! Mira el proyecto y dice qué encuentra: el código que no llega a usarse
//! y los ciclos de llamadas.
//!
//! ## De quién es la respuesta
//!
//! De la del servicio: la detección de código muerto y la de ciclos tienen
//! un dueño cada una, y el goal publica lo que dijeron. El goal no recalcula
//! ninguna de las dos.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::interface::goals::GoalEnvironment;
use crate::interface::project_graph::service_with_graph;

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
/// Input for the `audit` goal
pub struct AuditInput {}

/// La respuesta del goal `audit`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GoalAudit {
    /// Una línea que resume la respuesta.
    pub summary: String,
    /// La versión del esquema de esta respuesta.
    pub schema_version: String,
    /// El goal que contesta.
    pub goal: String,
    /// Los símbolos del proyecto.
    pub symbols: usize,
    /// Los símbolos a los que no llega ninguna llamada.
    pub dead_code: Vec<DeadSymbol>,
    /// Cuántos símbolos del proyecto son código muerto.
    pub dead_code_count: usize,
    /// Si el proyecto tiene ciclos de llamadas.
    pub has_cycles: bool,
    /// Cuántos símbolos están en un ciclo.
    pub symbols_in_cycles: usize,
    /// Cuántos componentes fuertemente conexos tiene el grafo.
    pub total_sccs: usize,
}

/// Un símbolo al que no llega ninguna llamada.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeadSymbol {
    /// El nombre del símbolo.
    pub symbol: String,
    /// El fichero donde está declarado.
    pub file: String,
    /// La línea (contada desde uno).
    pub line: u32,
}

/// La respuesta del goal `audit` para ese workspace.
pub fn audit(workspace: &Path) -> Result<GoalAudit, String> {
    let (service, built) = service_with_graph(workspace)?;
    let dead = service.detect_dead_code();
    let cycles = service.check_cycles(&built.graph);

    let dead_code: Vec<DeadSymbol> = dead
        .dead_code
        .iter()
        .map(|entry| DeadSymbol {
            symbol: entry.symbol.clone(),
            file: entry.file.clone(),
            line: entry.line,
        })
        .collect();

    let summary = format!(
        "{} dead symbol(s) of {} and {} symbol(s) in a cycle.",
        dead_code.len(),
        dead.total_symbols,
        cycles.symbols_in_cycles()
    );

    Ok(GoalAudit {
        summary,
        schema_version: "cognicode.goal.audit/v1".to_string(),
        goal: "audit".to_string(),
        symbols: dead.total_symbols,
        dead_code,
        dead_code_count: dead.total_dead,
        has_cycles: cycles.has_cycles,
        symbols_in_cycles: cycles.symbols_in_cycles(),
        total_sccs: cycles.total_sccs,
    })
}

/// La implementación del goal: lee sus argumentos y devuelve su respuesta.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let workspace = ctx.workspace;
    serde_json::from_value::<AuditInput>(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `audit` are not valid: {error}"))?;
    let answer = audit(workspace)?;
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `audit` is not serialisable: {error}"))
}

/// La respuesta del goal `audit` en texto.
pub fn render(answer: &Value) -> String {
    let audited: GoalAudit = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `audit` is the answer its own type describes");
    let mut text = String::new();
    text.push_str(&audited.summary);
    text.push('\n');
    for symbol in &audited.dead_code {
        text.push_str(&format!(
            "  {}:{} {} (nothing calls it)\n",
            symbol.file, symbol.line, symbol.symbol
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_answer_counts_what_the_owner_counted() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn helper() {}\nfn main() {}").expect("write");

        let answer = audit(dir.path()).expect("the goal answers");

        assert_eq!(answer.goal, "audit");
        assert_eq!(answer.symbols, 2);
        // El recuento y la lista son la misma cuenta: un goal que publica
        // «2 símbolos» y «3 muertos» publica dos verdades.
        assert_eq!(answer.dead_code_count, answer.dead_code.len());
        // Y el resumen dice la misma cuenta que los campos.
        assert!(
            answer.summary.contains(&answer.dead_code_count.to_string()),
            "the summary says another count than the fields: {answer:?}"
        );
    }
}
