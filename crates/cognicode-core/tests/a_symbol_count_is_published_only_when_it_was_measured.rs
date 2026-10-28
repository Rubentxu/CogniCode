//! Un recuento de símbolos se publica solo si se midió.
//!
//! ## El hueco
//!
//! La herramienta `reparse_on_edit` dice cuántos símbolos añadió y quitó un
//! fichero. Los números que publicaba no eran medidas: cada fichero
//! modificado sumaba "uno" como sustituto de un recuento que el método de
//! indexado no devuelve, y un fichero borrado publicaba los símbolos que
//! tenía según un manifiesto que guarda cero en todos los ficheros.
//!
//! ## Lo que se mide aquí
//!
//! El número publicado tiene que ser la diferencia entre lo que el grafo
//! tenía del fichero antes y lo que tiene después. Un fichero que crece
//! publica los símbolos que ganó; uno que encoge, los que perdió; uno que
//! se borra, los que tenía. Y cuando el cambio no se puede medir, la
//! herramienta lo dice en vez de publicar un cero que parece medido.
//!
//! ## Por qué se aserta sobre el JSON
//!
//! La aserción va por lo que el cliente ve, no por el campo tipado: así el
//! test compila contra el código anterior y falla por el motivo correcto
//! —el número que sale— y no porque el campo todavía no exista.

#![cfg(feature = "persistence")]

use cognicode_core::interface::mcp::handlers::aix_handlers::handle_reparse_on_edit;
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};
use cognicode_core::interface::mcp::schemas::ReparseOnEditInput;
use std::path::Path;
use std::time::Duration;

/// Un contexto con el workspace en un directorio temporal.
fn ctx_for(workspace: &Path) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build()
}

/// Un fichero Rust con el número de funciones pedido.
fn source_with_functions(count: usize) -> String {
    let mut body = String::new();
    for index in 0..count {
        body.push_str(&format!("fn funcion_{index}() {{}}\n"));
    }
    body
}

/// Los símbolos que el grafo tiene de un fichero.
///
/// Se cuenta desde el grafo, no desde la salida de la herramienta: el
/// oráculo no puede ser el código bajo prueba.
fn symbols_in_graph(ctx: &HandlerContext, file: &str) -> usize {
    ctx.analysis_service()
        .get_project_graph()
        .symbols()
        .filter(|symbol| symbol.location().file().ends_with(file))
        .count()
}

/// Lo que la herramienta publica, tal y como lo ve el cliente.
async fn reparse(ctx: &HandlerContext, file: &str) -> serde_json::Value {
    let output = handle_reparse_on_edit(
        ctx,
        ReparseOnEditInput {
            file_paths: vec![file.to_string()],
            edit_ranges: None,
        },
    )
    .await
    .expect("reparse_on_edit responde");
    serde_json::to_value(&output).expect("la salida es JSON")
}

/// Un workspace con un fichero Rust analysed y su grafo construido.
async fn analysed_workspace(functions: usize) -> (tempfile::TempDir, HandlerContext) {
    let workspace = tempfile::tempdir().expect("workspace");
    let path = workspace.path().join("src/main.rs");
    std::fs::create_dir_all(path.parent().expect("padre")).expect("se crea el directorio");
    std::fs::write(&path, source_with_functions(functions)).expect("se escribe el fichero");

    let ctx = ctx_for(workspace.path());
    let build = handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("build_graph responde");
    assert_eq!(
        build.status, "complete",
        "el grafo tiene que estar completo: si no, el manifiesto no se guarda y \
         el fichero se trata como nuevo en vez de como modificado"
    );

    (workspace, ctx)
}

#[tokio::test]
async fn a_file_that_grew_publishes_the_symbols_it_gained() {
    let (workspace, ctx) = analysed_workspace(2).await;

    let before = symbols_in_graph(&ctx, "src/main.rs");
    assert!(
        before >= 2,
        "el grafo tiene que tener los símbolos del fichero antes de empezar: {before}"
    );

    // Una fecha distinta: sin ella el producto no ve el cambio.
    tokio::time::sleep(Duration::from_millis(20)).await;
    std::fs::write(
        workspace.path().join("src/main.rs"),
        source_with_functions(6),
    )
    .expect("se reescribe el fichero");

    let published = reparse(&ctx, "src/main.rs").await;
    let after = symbols_in_graph(&ctx, "src/main.rs");
    assert!(
        after > before,
        "el grafo tiene que haberse reconstruido con el contenido nuevo: {after} contra {before}"
    );

    assert_eq!(
        published["symbols_added"].as_u64(),
        Some((after - before) as u64),
        "el número publicado tiene que ser la diferencia medida entre el grafo de \
         antes y el de después, no un uno de sustitución"
    );
    assert_eq!(
        published["symbols_removed"].as_u64(),
        Some(0),
        "crecer no quita símbolos"
    );
}

#[tokio::test]
async fn a_file_that_shrank_publishes_the_symbols_it_lost() {
    let (workspace, ctx) = analysed_workspace(6).await;

    let before = symbols_in_graph(&ctx, "src/main.rs");
    assert!(
        before >= 6,
        "el grafo tiene que tener los símbolos del fichero antes de empezar: {before}"
    );

    tokio::time::sleep(Duration::from_millis(20)).await;
    std::fs::write(
        workspace.path().join("src/main.rs"),
        source_with_functions(2),
    )
    .expect("se reescribe el fichero");

    let published = reparse(&ctx, "src/main.rs").await;
    let after = symbols_in_graph(&ctx, "src/main.rs");
    assert!(
        after < before,
        "el grafo tiene que haberse reconstruido con el contenido nuevo: {after} contra {before}"
    );

    assert_eq!(
        published["symbols_removed"].as_u64(),
        Some((before - after) as u64),
        "un fichero que encoge publica los símbolos que perdió"
    );
    assert_eq!(
        published["symbols_added"].as_u64(),
        Some(0),
        "encoger no añade símbolos"
    );
}

#[tokio::test]
async fn a_deleted_file_publishes_the_symbols_it_had() {
    let (workspace, ctx) = analysed_workspace(4).await;

    let before = symbols_in_graph(&ctx, "src/main.rs");
    assert!(
        before >= 4,
        "el grafo tiene que tener los símbolos del fichero antes de borrarlo: {before}"
    );

    std::fs::remove_file(workspace.path().join("src/main.rs")).expect("se borra el fichero");

    let published = reparse(&ctx, "src/main.rs").await;

    assert_eq!(
        published["symbols_removed"].as_u64(),
        Some(before as u64),
        "un fichero borrado publica los símbolos que tenía, contados en el grafo"
    );
}

#[tokio::test]
async fn a_change_that_cannot_be_measured_is_published_as_unmeasured() {
    // Sin grafo anterior no hay contra qué medir: el cambio es desconocido,
    // y decirlo es distinto de publicar un cero que parece medido.
    let workspace = tempfile::tempdir().expect("workspace");
    let path = workspace.path().join("src/main.rs");
    std::fs::create_dir_all(path.parent().expect("padre")).expect("se crea el directorio");
    std::fs::write(&path, source_with_functions(3)).expect("se escribe el fichero");

    let ctx = ctx_for(workspace.path());
    let published = reparse(&ctx, "src/main.rs").await;

    assert_eq!(
        published["symbols_unmeasured"].as_u64(),
        Some(1),
        "un cambio que no se puede medir se declara como tal"
    );
    assert_eq!(
        published["symbols_added"].as_u64(),
        Some(0),
        "un cambio que no se puede medir no publica símbolos añadidos"
    );
}
