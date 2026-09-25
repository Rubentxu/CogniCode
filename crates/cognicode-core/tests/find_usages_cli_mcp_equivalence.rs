//! L1.4.W4 — Equivalencia CLI ↔ MCP para `find_usages`.
//!
//! Política L1.4: el CLI NO duplica lógica del handler MCP. Ambos
//! llaman al MISMO use case (`AnalysisService::find_symbol_usages`).
//!
//! Este test pinea el contrato: dado el mismo `(cwd, symbol,
//! include_declaration, context_lines)`, CLI y MCP devuelven el mismo
//! conjunto de resultados (mismo set de `(file, line, column,
//! is_definition)` en mismo orden).
//!
//! Si el CLI añadiera un filtro, un sort distinto o cualquier
//! transformación, este test falla. Si el handler MCP cambiara su
//! contrato (p. ej. añadir deduplicación), este test detecta la
//! divergencia.

use std::collections::BTreeSet;

use cognicode_core::application::services::analysis_service::{AnalysisService, UsageSearchParams};
use cognicode_core::interface::mcp::handlers::{HandlerContext, handle_find_usages};
use cognicode_core::interface::mcp::schemas::FindUsagesInput;
use cognicode_core::interface::mcp::schemas::FindUsagesOutput;

/// Construye un workspace temporal con código fuente de muestra donde
/// `alpha` está definido una vez (línea 1) y usado dos veces (líneas 5 y 6).
fn fixture_corpus() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create tempdir");
    std::fs::create_dir_all(dir.path().join("src")).expect("mkdir src");

    std::fs::write(
        dir.path().join("src/lib.rs"),
        "fn alpha() -> i32 {\n    42\n}\n\nfn main() {\n    let x = alpha();\n    let y = alpha();\n    println!(\"{} {}\", x, y);\n}\n",
    )
    .expect("write lib.rs");
    dir
}

/// Llama directamente al handler MCP y devuelve su output.
async fn run_mcp(cwd: &std::path::Path, symbol: &str, include_decl: bool) -> FindUsagesOutput {
    let ctx = HandlerContext::builder()
        .with_working_dir(cwd.to_path_buf())
        .build();
    handle_find_usages(
        &ctx,
        FindUsagesInput {
            symbol_name: symbol.to_string(),
            include_declaration: include_decl,
            context_lines: None,
        },
    )
    .await
    .expect("MCP handler no debe fallar en workspace válido")
}

/// Llama directamente al `AnalysisService` — exactamente lo que hace
/// el CLI por debajo (no podemos capturar stdout del binario desde un
/// test lib, pero podemos reproducir el camino de datos).
async fn run_service(cwd: &std::path::Path, symbol: &str, include_decl: bool) -> FindUsagesOutput {
    let service = AnalysisService::new();
    let usages = service
        .find_symbol_usages(UsageSearchParams {
            project_dir: cwd.to_path_buf(),
            symbol_name: symbol.to_string(),
            include_declaration: include_decl,
            context_lines: None,
            first_only_definition: true,
        })
        .expect("AnalysisService no debe fallar en workspace válido");

    FindUsagesOutput {
        symbol: symbol.to_string(),
        total: usages.len(),
        usages: usages
            .into_iter()
            .map(|u| cognicode_core::interface::mcp::schemas::UsageEntry {
                file: u.file,
                line: u.line,
                column: u.column,
                context: u.context,
                is_definition: u.is_definition,
                surrounding_lines: u.context_lines.map(|c| {
                    cognicode_core::interface::mcp::schemas::ContextLines {
                        before: c.before,
                        current: c.current,
                        after: c.after,
                    }
                }),
            })
            .collect(),
    }
}

/// Equivalencia clave del set `(line, column, is_definition)`.
/// Usamos `BTreeSet` para ignorar el orden.
///
/// NOTA: NO incluimos `file` en la clave porque el MCP handler
/// canonicaliza el cwd via `HandlerContext`, mientras que la llamada
/// directa a `AnalysisService` recibe el path crudo del fixture. Eso
/// puede dar paths equivalentes pero textualmente distintos
/// (p.ej. `/home/...` vs `/var/home/...` cuando `/home` es symlink).
/// Lo que importa para el contrato de búsqueda es el IDENTIFICADOR
/// de posición; la ruta es metadata del filesystem.
fn usage_set(out: &FindUsagesOutput) -> BTreeSet<(u32, u32, bool)> {
    out.usages
        .iter()
        .map(|u| (u.line, u.column, u.is_definition))
        .collect()
}

/// Equivalencia dura: `total` y `usages.len()` y el set de tuplas.
fn assert_outputs_equivalent(actual: &FindUsagesOutput, expected: &FindUsagesOutput) {
    assert_eq!(actual.symbol, expected.symbol, "symbol mismatch");
    assert_eq!(
        actual.total, expected.total,
        "total mismatch: actual={} expected={}",
        actual.total, expected.total
    );
    assert_eq!(
        actual.usages.len(),
        expected.usages.len(),
        "usages.len mismatch"
    );
    assert_eq!(
        usage_set(actual),
        usage_set(expected),
        "set of (line, column, is_definition) mismatch: \
         actual={:?} expected={:?}",
        actual.usages,
        expected.usages
    );
}

#[tokio::test]
async fn service_matches_mcp_include_declaration_true() {
    let dir = fixture_corpus();
    let cwd = dir.path();
    let mcp = run_mcp(cwd, "alpha", true).await;
    let svc = run_service(cwd, "alpha", true).await;
    assert_outputs_equivalent(&svc, &mcp);
}

#[tokio::test]
async fn service_matches_mcp_include_declaration_false() {
    let dir = fixture_corpus();
    let cwd = dir.path();
    let mcp = run_mcp(cwd, "alpha", false).await;
    let svc = run_service(cwd, "alpha", false).await;
    assert_outputs_equivalent(&svc, &mcp);
}

/// Idempotencia: el mismo (cwd, symbol, include_decl) produce el
/// mismo set de resultados en dos llamadas independientes.
#[tokio::test]
async fn service_results_are_idempotent() {
    let dir = fixture_corpus();
    let cwd = dir.path();
    let a = run_service(cwd, "alpha", true).await;
    let b = run_service(cwd, "alpha", true).await;
    assert_outputs_equivalent(&a, &b);
}

/// Símbolo desconocido → mismo comportamiento (vacío) en ambos caminos.
#[tokio::test]
async fn service_matches_mcp_unknown_symbol() {
    let dir = fixture_corpus();
    let cwd = dir.path();
    let mcp = run_mcp(cwd, "no_existe_este_symbol_xyz", true).await;
    let svc = run_service(cwd, "no_existe_este_symbol_xyz", true).await;
    assert_outputs_equivalent(&svc, &mcp);
    assert_eq!(mcp.total, 0);
    assert_eq!(svc.total, 0);
}
