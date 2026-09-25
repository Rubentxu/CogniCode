//! L1.4.W1 — Caracterización E2E del handler MCP `handle_find_usages`.
//!
//! Estado pre-L1.4:
//!   - `AnalysisService::find_symbol_usages` está pineado por 4 tests
//!     internos (R1.1..R1.4) en `analysis_service.rs:find_symbol_usages_tests`.
//!   - El handler MCP `handle_find_usages` envuelve ese service pero NO
//!     está pineado por test de caracterización E2E. Solo se enumera
//!     en `mcp_roundtrip_tests.rs` como nombre de tool.
//!
//! Riesgo que cierra este test:
//!   - drift entre el contrato del handler y el del service
//!   - regresiones en la conversión `UsageResult → UsageEntry`
//!   - regresiones en el input validation / error handling
//!
//! L1.4.W2 (CLI `cognicode find-usages`) reutilizará los mismos
//! fixtures para verificar equivalencia con el handler MCP.
//!
//! Política L1.4: NO duplicar lógica del handler en el CLI.
//! El CLI debe llamar al MISMO use case (`AnalysisService::find_symbol_usages`)
//! y la equivalencia es:
//!   mismo (project_dir, symbol_name, include_declaration, context_lines)
//!   → mismo set (file, line, column, is_definition) en mismo orden.

use cognicode_core::interface::mcp::handlers::{HandlerContext, handle_find_usages};
use cognicode_core::interface::mcp::schemas::{FindUsagesInput, FindUsagesOutput};

/// Construye un workspace temporal con código fuente de muestra donde
/// `alpha` está definido una vez (línea 1) y usado dos veces (líneas 5 y 6).
/// Crea `target/` para verificar que se ignora.
fn fixture_corpus() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create tempdir");
    std::fs::create_dir_all(dir.path().join("src")).expect("mkdir src");
    std::fs::create_dir_all(dir.path().join("target")).expect("mkdir target");

    std::fs::write(
        dir.path().join("src/lib.rs"),
        "fn alpha() -> i32 {\n    42\n}\n\nfn main() {\n    let x = alpha();\n    let y = alpha();\n    println!(\"{} {}\", x, y);\n}\n",
    )
    .expect("write lib.rs");

    std::fs::write(dir.path().join("target/distractor.rs"), "fn alpha() {}")
        .expect("write distractor");

    dir
}

/// HandlerContext mínimo para los tests: solo working_dir. El resto
/// de los campos opcionales caen a defaults razonables (sin graph,
/// sin file_ops, etc.).
fn make_ctx(dir: &tempfile::TempDir) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(dir.path().to_path_buf())
        .build()
}

/// RED→GREEN (caracterización): pinea el contrato observable del
/// handler. Si esto cambia, el CLI debe cambiar también o el test de
/// equivalencia L1.4.W4 fallará.
#[tokio::test]
async fn handler_include_declaration_returns_definition_plus_call_sites() {
    let dir = fixture_corpus();
    let ctx = make_ctx(&dir);

    let out: FindUsagesOutput = handle_find_usages(
        &ctx,
        FindUsagesInput {
            symbol_name: "alpha".to_string(),
            include_declaration: true,
            context_lines: None,
        },
    )
    .await
    .expect("handler no debe fallar en workspace válido");

    assert_eq!(out.symbol, "alpha", "el handler devuelve el símbolo pedido");
    assert!(
        out.total >= 3,
        "esperábamos >=3 (definición + 2 calls), obtuvimos {}",
        out.total
    );
    assert_eq!(
        out.usages.len(),
        out.total,
        "total debe coincidir con usages.len()"
    );

    let defs = out.usages.iter().filter(|u| u.is_definition).count();
    assert_eq!(
        defs, 1,
        "exactamente 1 definición (first_only_definition=true)"
    );

    // Solo src/lib.rs debe aparecer (target/ está en USAGE_SKIP_DIRS).
    assert!(
        out.usages.iter().all(|u| u.file.ends_with("src/lib.rs")),
        "todas las usages deben venir de src/lib.rs, no de target/: {:?}",
        out.usages
    );
}

/// Contrato del input: `include_declaration=false` excluye la
/// definición. Esto es la semántica de "usages only" — distinto del
/// caso `include_declaration=true`.
#[tokio::test]
async fn handler_exclude_declaration_omits_definition() {
    let dir = fixture_corpus();
    let ctx = make_ctx(&dir);

    let out = handle_find_usages(
        &ctx,
        FindUsagesInput {
            symbol_name: "alpha".to_string(),
            include_declaration: false,
            context_lines: None,
        },
    )
    .await
    .expect("handler no debe fallar");

    assert!(
        out.usages.iter().all(|u| !u.is_definition),
        "ningún usage debe ser marcado is_definition=true"
    );
    assert_eq!(
        out.usages.len(),
        2,
        "solo los 2 call sites, sin definición: {:?}",
        out.usages
    );
}

/// Símbolo inexistente: resultado vacío, NO error. Esto pinea la
/// tolerancia del handler (no es un bug encontrar cero resultados).
#[tokio::test]
async fn handler_unknown_symbol_returns_empty() {
    let dir = fixture_corpus();
    let ctx = make_ctx(&dir);

    let out = handle_find_usages(
        &ctx,
        FindUsagesInput {
            symbol_name: "no_existe_este_symbol".to_string(),
            include_declaration: true,
            context_lines: None,
        },
    )
    .await
    .expect("handler no debe fallar");

    assert_eq!(out.total, 0, "total debe ser 0 para símbolo desconocido");
    assert!(
        out.usages.is_empty(),
        "usages debe estar vacío para símbolo desconocido"
    );
}

/// Caracterización del handler con símbolo vacío: el validator
/// `validate_query` actual SOLO pinea longitud máxima, no pinea
/// no-vacío. Por tanto un símbolo vacío pasa validación y devuelve
/// 0 resultados. Esto pinea el COMPORTAMIENTO REAL; si en el futuro
/// se cierra el gap de "no-vacío", este test falla y obliga a
/// actualizar también el CLI y los tests pineados.
#[tokio::test]
async fn handler_empty_symbol_returns_empty_results() {
    let dir = fixture_corpus();
    let ctx = make_ctx(&dir);

    let out = handle_find_usages(
        &ctx,
        FindUsagesInput {
            symbol_name: String::new(),
            include_declaration: true,
            context_lines: None,
        },
    )
    .await
    .expect("el handler no rechaza símbolo vacío: validate_query solo pinea max length");

    assert_eq!(
        out.total, 0,
        "símbolo vacío debe devolver 0 results (no hay coincidencias para '')"
    );
    assert!(out.usages.is_empty(), "símbolo vacío no debe tener usages");
}
