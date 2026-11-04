//! Un nombre que nombran dos símbolos no se resuelve en silencio.
//!
//! ## El hueco
//!
//! La herramienta `trace_path` acepta un nombre —«helper»— y busca el
//! símbolo que lo lleva. El resolutor devolvía el **primero** que
//! encontraba, sin decir cuál. En un proyecto con dos ficheros que declaran
//! `helper`, la traza publicaba una ruta por uno de ellos: la que el
//! recorrido del grafo encontró antes. Y el recorrido del grafo es un mapa
//! hash, así que el «primero» cambiaba entre ejecuciones del mismo árbol.
//!
//! ## Lo que se mide aquí
//!
//! Un nombre que llevan varios símbolos es una **ambigüedad**, y una
//! ambigüedad se contesta: se dice qué nombre es ambiguo y qué símbolos
//! son los candidatos. No se elige uno. La ruta que el producto publicaría
//! a través de un símbolo que nadie nombró es peor que ninguna ruta.
//!
//! El nombre completo —la identidad del símbolo— sigue resolviendo como
//! siempre: es único, así que no hay nada que decidir.
//!
//! ## Por qué se aserta sobre lo que ve el cliente
//!
//! La aserción va por el error publicado, no por la variante tipada: así el
//! test compila contra el código anterior y falla por el motivo correcto
//! —publicó una ruta en vez de negar la ambigüedad— y no porque el campo
//! todavía no exista.

#![cfg(feature = "persistence")]

use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph, handle_trace_path,
};
use cognicode_core::interface::mcp::schemas::{McpError, TracePathInput};
use std::path::Path;

/// Un contexto con el workspace en un directorio temporal.
fn ctx_for(workspace: &Path) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build()
}

/// El contenido de un fichero que declara un homónimo y llama a un símbolo
/// propio.
fn source_with_homonym(called: &str) -> String {
    format!("fn helper() {{ {called}(); }}\nfn {called}() {{}}\n")
}

/// Un workspace con dos ficheros que declaran `helper`, cada uno con su
/// propio símbolo llamado, y el grafo construido.
async fn workspace_with_two_homonyms() -> (tempfile::TempDir, HandlerContext) {
    let workspace = tempfile::tempdir().expect("workspace");
    let src = workspace.path().join("src");
    std::fs::create_dir_all(&src).expect("se crea el directorio");
    std::fs::write(src.join("a.rs"), source_with_homonym("compute_a")).expect("se escribe a.rs");
    std::fs::write(src.join("b.rs"), source_with_homonym("compute_b")).expect("se escribe b.rs");

    let ctx = ctx_for(workspace.path());
    let build = handle_build_graph(&ctx, BuildGraphInput { directory: None })
        .await
        .expect("build_graph responde");
    assert_eq!(
        build.status, "complete",
        "el grafo tiene que estar completo: si no, la lectura de los ficheros \
         no es la que el test da por buena"
    );

    (workspace, ctx)
}

/// El nombre completo del símbolo que se llama así en ese fichero.
///
/// Se lee del grafo, no del código bajo prueba: el test no puede fijar la
/// línea que el lectordecidedá sin volverse una segunda implementación.
fn full_name_of(ctx: &HandlerContext, file: &str, name: &str) -> String {
    ctx.analysis_service()
        .get_project_graph()
        .symbols()
        .find(|symbol| symbol.location().file().ends_with(file) && symbol.name() == name)
        .unwrap_or_else(|| panic!("el grafo tiene que declarar {name} en {file}"))
        .fully_qualified_name()
        .to_string()
}

/// Los candidatos que el error publica.
fn candidates_of(error: &McpError) -> Vec<String> {
    let data = error
        .data
        .as_ref()
        .expect("la ambigüedad publica sus candidatos como datos");
    data["candidates"]
        .as_array()
        .expect("los candidatos son una lista")
        .iter()
        .map(|candidate| {
            candidate
                .as_str()
                .expect("cada candidato es un texto")
                .to_string()
        })
        .collect()
}

#[tokio::test]
async fn the_mcp_trace_refuses_an_ambiguous_name_and_publishes_its_candidates() {
    let (_workspace, ctx) = workspace_with_two_homonyms().await;

    let error = handle_trace_path(
        &ctx,
        TracePathInput {
            source: "helper".to_string(),
            target: "helper".to_string(),
            max_depth: 10,
            all: false,
        },
    )
    .await
    .expect_err("un nombre que nombran dos símbolos no se resuelve en silencio");

    let error = McpError::from(error);
    assert_eq!(
        error.code, -32006,
        "la ambigüedad tiene su propio código: el cliente tiene que poder \
         distinguirla de un símbolo que no existe"
    );
    assert!(
        error.message.contains("helper"),
        "el error dice qué nombre es ambiguo: {}",
        error.message
    );

    let candidates = candidates_of(&error);
    assert_eq!(
        candidates.len(),
        2,
        "el error dice todos los candidatos, no solo el primero: {candidates:?}"
    );
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.contains("a.rs")),
        "uno de los candidatos es el homónimo de a.rs: {candidates:?}"
    );
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.contains("b.rs")),
        "uno de los candidatos es el homónimo de b.rs: {candidates:?}"
    );
}

#[tokio::test]
async fn the_application_trace_refuses_an_ambiguous_name_too() {
    let (_workspace, ctx) = workspace_with_two_homonyms().await;

    let error = ctx
        .analysis_service()
        .trace_path("helper", "helper", 10)
        .expect_err("la capa de aplicación tampoco elige entre homónimos");

    let message = error.to_string();
    assert!(
        message.contains("helper"),
        "el error dice qué nombre es ambiguo: {message}"
    );
    assert!(
        message.contains("a.rs") && message.contains("b.rs"),
        "el error dice los dos candidatos: {message}"
    );
}

#[tokio::test]
async fn the_full_name_still_resolves_as_it_did() {
    let (_workspace, ctx) = workspace_with_two_homonyms().await;

    let source = full_name_of(&ctx, "a.rs", "helper");
    let target = full_name_of(&ctx, "a.rs", "compute_a");

    let output = handle_trace_path(
        &ctx,
        TracePathInput {
            source: source.clone(),
            target: target.clone(),
            max_depth: 10,
            all: false,
        },
    )
    .await
    .expect("el nombre completo resuelve: es único, no hay nada que decidir");

    assert!(
        output.path_found,
        "la ruta entre el símbolo y el que llama tiene que existir: {output:?}"
    );
    assert!(
        output.path.iter().any(|entry| entry.symbol == "compute_a"),
        "la ruta publicada llega al símbolo llamado: {output:?}"
    );
}
