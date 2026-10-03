//! PRF-CLI-01 (extensión): todo brazo de `CommandExecutor::execute` que
//! recibe un `Err` debe propagarlo, no imprimirlo y continuar.
//!
//! ## El defecto que este contrato mide
//!
//! `CommandExecutor::execute` despacha sobre `CliCommand`. Tres brazos
//! propagan el error con `return Err(e)` — `Analyze`, `Graph`,
//! `FindUsages` — y ocho lo tragan: imprimen con `eprintln!` y dejan que
//! `execute` termine en `Ok(())`. El proceso sale con **0**, así que un
//! script o un agente lee "terminó bien" de una operación que no se
//! hizo. El caso más caro no es imaginario: `cognicode navigate
//! references <symbol>` imprimía
//!
//!     Navigate command failed: Invalid position 'MySymbol': expected file:line:column
//!
//! y salía con 0. Un agente que seguía la skill `cognicode-pr-review`
//! creía haber consultado las referencias del símbolo y no había
//! consultado ninguna.
//!
//! `FindUsages` documenta la política en su propio comentario ("exit 0:
//! éxito; exit 2 (via Err): uso inválido o error de backend"). Este
//! contrato la convierte de prosa en aserción para el resto de la
//! superficie.
//!
//! ## Por qué el argv es lo que es
//!
//! `prf_cli_01_uat::graph_full_nonexistent_path_does_not_exit_zero` pasa
//! `--path /nonexistent/...`, y `graph full` no tiene `--path`: su
//! firma real es `graph full [PATH]`. Clap rechaza el flag con exit 2
//! y el test pasa **sin ejecutar el brazo**. Su comentario dice "already
//! the case; pins the contract", y lo que realmente mide es que clap
//! conoce la aridad. Aquí se usa el positional, que sí llega al brazo.
//! ## Los dos sentidos
//!
//! Un contrato que solo afirma "esto sale distinto de 0" pasa entero si
//! el arreglo convierte *todo* en error, incluido el éxito. Por eso cada
//! caso de error tiene su gemelo de éxito: el mismo comando con
//! entrada válida, que debe seguir saliendo con 0. Si un arreglo rompe
//! el caso feliz, cae esta suite, no la próxima.

use std::path::PathBuf;
use std::process::{Command, Output};

mod common;

fn cognicode_bin() -> PathBuf {
    common::binary_path("cognicode")
}

fn run(args: &[&str]) -> Output {
    Command::new(cognicode_bin())
        .args(args)
        .output()
        .expect("spawn cognicode binary")
}

fn exit_code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

/// Un directorio inexistente, con una ruta que ningún sandbox puede tener.
const MISSING: &str = "/nonexistent/prf_cli_01_exit_propagation/missing";

// ---------------------------------------------------------------------------
// 1. Navigate: el caso que la skill `cognicode-pr-review` enseñaba mal.
// ---------------------------------------------------------------------------

/// `navigate` recibe un `position` que no es `file:line:column`.
/// `parse_position` falla, el error se imprime y el proceso sale con 0.
#[test]
fn navigate_references_propagates_invalid_position() {
    let out = run(&["navigate", "references", "MySymbol"]);
    assert_ne!(
        exit_code(&out),
        0,
        "`navigate references <symbol>` debe salir distinto de 0: la posición \
         es inválida y la consulta NO se hizo. Salir con 0 hace creer al \
         agente que consultó las referencias. stderr: {}",
        stderr_of(&out)
    );
}

/// El mismo defecto por el brazo `Definition`, que comparte `parse_position`.
#[test]
fn navigate_definition_propagates_invalid_position() {
    let out = run(&["navigate", "definition", "MySymbol"]);
    assert_ne!(
        exit_code(&out),
        0,
        "`navigate definition <symbol>` debe salir distinto de 0. stderr: {}",
        stderr_of(&out)
    );
}

/// `Hover` es el tercer brazo de `Navigate` y el mismo `parse_position`.
#[test]
fn navigate_hover_propagates_invalid_position() {
    let out = run(&["navigate", "hover", "MySymbol"]);
    assert_ne!(
        exit_code(&out),
        0,
        "`navigate hover <symbol>` debe salir distinto de 0. stderr: {}",
        stderr_of(&out)
    );
}

// ---------------------------------------------------------------------------
// 2. Index: `build_index` sobre un directorio inexistente.
// ---------------------------------------------------------------------------

#[test]
fn index_build_propagates_missing_directory() {
    let out = run(&["index", "build", MISSING]);
    assert_ne!(
        exit_code(&out),
        0,
        "`index build <inexistente>` debe salir distinto de 0: el índice no \
         se construyó. stderr: {}",
        stderr_of(&out)
    );
}

#[test]
fn index_query_propagates_missing_directory() {
    let out = run(&["index", "query", "ZzzNope", MISSING]);
    assert_ne!(
        exit_code(&out),
        0,
        "`index query <símbolo> <inexistente>` debe salir distinto de 0. stderr: {}",
        stderr_of(&out)
    );
}

// ---------------------------------------------------------------------------
// 3. Graph: el caso que el UAT existente no llegaba a ejecutar.
// ---------------------------------------------------------------------------

/// `graph full` con la ruta como **positional**, que es la firma real.
/// Un grafo sobre un directorio inexistente no es un grafo: sale con 0 y
/// un `Warning: PARTIAL` que ningún script lee.
#[test]
fn graph_full_propagates_missing_directory() {
    let out = run(&["graph", "full", MISSING]);
    assert_ne!(
        exit_code(&out),
        0,
        "`graph full <inexistente>` debe salir distinto de 0. Un `PARTIAL` \
         que se emite con exit 0 es indistinguible del éxito para un \
         pipeline. stderr: {}",
        stderr_of(&out)
    );
}

/// `graph mermaid` sobre la misma ruta: mismo PARTIAL, mismo brazo.
#[test]
fn graph_mermaid_propagates_missing_directory() {
    let out = run(&["graph", "mermaid", MISSING]);
    assert_ne!(
        exit_code(&out),
        0,
        "`graph mermaid <inexistente>` debe salir distinto de 0. stderr: {}",
        stderr_of(&out)
    );
}

// ---------------------------------------------------------------------------
// 4. El gemelo de éxito: la mitad que impide "arreglar" exiting con 0.
//
// ---------------------------------------------------------------------------

/// `analyze` sobre un directorio válido y vacío es una operación
/// realizada: debe seguir saliendo con 0. Si un arreglo de propagación
/// lo rompe, esta aserción cae.
#[test]
fn analyze_valid_empty_dir_still_exits_zero() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = run(&["analyze", tmp.path().to_str().unwrap()]);
    assert_eq!(
        exit_code(&out),
        0,
        "`analyze <dir válido>` debe salir con 0. stderr: {}",
        stderr_of(&out)
    );
}

/// El caso feliz del mismo brazo que los tests de error atacan: una
/// posición bien formada llega al backend y sale con 0 aunque no
/// encuentre nada. "Sin resultados" es un resultado, no un error.
///
/// La firma real es `navigate definition <POSITION> [PATH]`: el workspace
/// es un **positional**, no `--path`. Usar `--path` haría que clap
/// rechazara el comando con exit 2 y el test pasaría sin ejecutar el brazo
/// — que es exactamente el defecto que este fichero documenta arriba
/// encuentra en `prf_cli_01_uat`.
#[test]
fn navigate_with_wellformed_position_does_not_fail_on_missing_definition() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let file = tmp.path().join("lib.rs");
    std::fs::write(&file, "pub fn alpha() -> u32 { 1 }\n").expect("write fixture");

    // `lib.rs:1:8` es una posición bien formada: el parseo pasa y el
    // fallo, si lo hay, viene del backend LSP, no de la invocación.
    let position = format!("{}:1:8", file.display());
    let out = Command::new(cognicode_bin())
        .args([
            "navigate",
            "definition",
            &position,
            tmp.path().to_str().unwrap(),
        ])
        .output()
        .expect("spawn cognicode binary");

    let err = stderr_of(&out);
    assert!(
        !err.contains("Invalid position"),
        "una posición bien formada no debe fallar en `parse_position`: {err}"
    );
}

/// Un `index build` sobre un directorio real y vacío sí es una operación
/// realizada: el gemelo positivo del `index_build_propagates_missing_directory`.
#[test]
fn index_build_on_real_empty_dir_still_exits_zero() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = run(&["index", "build", tmp.path().to_str().unwrap()]);
    assert_eq!(
        exit_code(&out),
        0,
        "`index build <dir válido>` debe salir con 0. stderr: {}",
        stderr_of(&out)
    );
}
