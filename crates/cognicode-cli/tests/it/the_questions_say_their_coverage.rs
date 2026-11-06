//! TASK-49 — las preguntas dicen su cobertura.
//!
//! Una pregunta se responde con un grafo, y el grafo puede no cubrir el
//! árbol entero. Cuando eso pasa, la respuesta tiene que decirlo: un
//! "0 símbolos afectados" sobre un grafo que no leyó un fichero no es un
//! hallazgo, es un hueco publicado como un cero.
//!
//! Este fichero mide esa publicación en las DOS superficies y en las DOS
//! preguntas, por los caminos REALES:
//!
//! - la CLI, como el binario que el usuario ejecuta (documento JSON);
//! - el MCP, como el handler vivo (documento JSON).
//!
//! Y por los TRES casos que un árbol puede darle:
//!
//! - `complete`: el árbol se leyó entero;
//! - `partial` por un fichero que no se pudo leer;
//! - `partial` por una referencia que nadie declaró.
//!
//! Los dos últimos son los DOS primeros agujeros que el producto cuenta:
//! un fichero que no se pudo leer y una referencia que no se pudo atar a
//! una declaración. Un consumidor que lee uno y no el otro lee media
//! verdad, así que la respuesta los lleva los dos.
//!
//! La comparación es del documento publicado, campo por campo: las dos
//! superficies son adaptadores de un servicio, y un consumidor no puede
//! tener que adivinar cuál de las dos midió.

use crate::common;

use cognicode_core::interface::mcp::handlers::{
    HandlerContext, handle_analyze_impact, handle_get_call_hierarchy,
};
use cognicode_core::interface::mcp::schemas::{
    AnalyzeImpactInput, CallDirection, GetCallHierarchyInput,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The symbol the questions ask about.
///
/// It is in the graph and somebody calls it, so the answer is a real one
/// and not the "no such symbol" refusal.
const SYMBOL: &str = "callee";

// ---------------------------------------------------------------------------
// Fixture: a workspace, and a cache of its own.
// ---------------------------------------------------------------------------

/// A workspace, the cache the product keeps its calculations in, and the
/// temporary root that holds both.
///
/// The source declares two symbols and the call between them, so the
/// questions have a real answer to give.
fn workspace() -> (tempfile::TempDir, PathBuf, PathBuf) {
    workspace_with("fn callee() -> u32 { 1 }\nfn caller() -> u32 { callee() }\n")
}

/// The same workspace, plus a Terraform file that names something nobody
/// declares.
///
/// The reference cannot be tied to a declaration, so the graph holds the
/// statement without holding its far end: that is the second hole the
/// product counts, and it is a hole even when every file was read whole.
fn workspace_with_a_pending_reference() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let (root, ws, cache) = workspace();
    std::fs::write(
        ws.join("main.tf"),
        "resource \"aws_s3_bucket_policy\" \"ghost_policy\" {\n  bucket = aws_s3_bucket.no_existe.id\n  policy = \"{}\"\n}\n",
    )
    .expect("the terraform source is written");
    (root, ws, cache)
}

fn workspace_with(source: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let root = tempfile::tempdir().expect("a temporary root");
    let ws = root.path().join("ws");
    std::fs::create_dir_all(&ws).expect("the workspace is created");
    std::fs::write(ws.join("a.rs"), source).expect("the source is written");
    std::fs::write(ws.join("b.rs"), "fn other() -> u32 { 2 }\n")
        .expect("the second source is written");
    let cache = root.path().join("cache");
    (root, ws, cache)
}

/// A file the walk cannot read: a hole in the coverage.
fn make_unreadable(root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let unreadable = root.join("b.rs");
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000))
        .expect("set_permissions 000");
}

// ---------------------------------------------------------------------------
// The CLI: the real binary, with a cache of its own.
// ---------------------------------------------------------------------------

fn run_cli(args: &[&str], cache: &Path) -> Value {
    let binary = common::binary_path("cognicode");
    let output = Command::new(&binary)
        .arg("--format")
        .arg("json")
        .args(args)
        .env("XDG_CACHE_HOME", cache)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", binary.display()));
    assert!(
        output.status.success(),
        "the CLI exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "the CLI did not answer with JSON: {e}: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn cli_impact(root: &Path, cache: &Path) -> Value {
    run_cli(&["graph", "impact", SYMBOL, &root.to_string_lossy()], cache)
}

fn cli_hierarchy(root: &Path, cache: &Path) -> Value {
    run_cli(
        &[
            "graph",
            "hierarchy",
            SYMBOL,
            &root.to_string_lossy(),
            "--depth",
            "2",
            "--direction",
            "out",
        ],
        cache,
    )
}

// ---------------------------------------------------------------------------
// The MCP: the live handler, over the same cache.
// ---------------------------------------------------------------------------

fn mcp_context(root: &Path, cache: &Path) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(root.to_path_buf())
        .with_cache_root(cache.to_path_buf())
        .build()
}

async fn mcp_impact(ctx: &HandlerContext) -> Value {
    let output = handle_analyze_impact(
        ctx,
        AnalyzeImpactInput {
            symbol_name: SYMBOL.to_string(),
            compressed: false,
        },
    )
    .await
    .expect("the MCP answers the impact question");
    serde_json::to_value(&output).expect("the MCP's answer is a document")
}

async fn mcp_hierarchy(ctx: &HandlerContext) -> Value {
    let output = handle_get_call_hierarchy(
        ctx,
        GetCallHierarchyInput {
            symbol_name: SYMBOL.to_string(),
            direction: CallDirection::Outgoing,
            depth: 2,
            include_external: false,
            compressed: false,
        },
    )
    .await
    .expect("the MCP answers the hierarchy question");
    serde_json::to_value(&output).expect("the MCP's answer is a document")
}

// ---------------------------------------------------------------------------
// What a consumer reads.
// ---------------------------------------------------------------------------

/// The coverage the document publishes.
///
/// `null` when the document does not carry one: a document that says
/// nothing about its coverage is a document that claims it covers
/// everything.
fn coverage_of(document: &Value) -> Value {
    document.get("coverage").cloned().unwrap_or(Value::Null)
}

/// The status the document publishes, as the word the owner publishes.
fn status_of(document: &Value) -> String {
    coverage_of(document)["status"]
        .as_str()
        .unwrap_or_else(|| {
            panic!(
                "the answer does not say what it covers: {document:?}\n\
                 una respuesta que no dice su cobertura dice que lo cubre todo"
            )
        })
        .to_string()
}

/// The files the answer says it could not read.
///
/// The paths are the ones the product publishes —absolute, because the
/// product does not know where the workspace will be asked about from— and
/// what is compared is the file, not the directory it was copied to.
fn skipped_of(document: &Value, root: &Path) -> Vec<String> {
    coverage_of(document)["skipped_files"]
        .as_array()
        .unwrap_or_else(|| {
            panic!("the answer does not say which files it could not read: {document:?}")
        })
        .iter()
        .map(|file| {
            let path = file["path"].as_str().unwrap_or_default();
            Path::new(path)
                .strip_prefix(root)
                .unwrap_or_else(|_| Path::new(path))
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

/// The references the answer says it could not tie to a declaration.
fn pending_of(document: &Value) -> Vec<String> {
    coverage_of(document)["pending_references"]
        .as_array()
        .unwrap_or_else(|| {
            panic!("the answer does not say which references it could not tie: {document:?}")
        })
        .iter()
        .map(|reference| reference["target"].as_str().unwrap_or_default().to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Ley 1: la pregunta de impacto dice su cobertura, y dice la misma.
// ---------------------------------------------------------------------------

/// Un árbol que se leyó entero: las dos superficies dicen "complete".
#[tokio::test]
async fn the_impact_question_says_complete_over_a_tree_read_whole() {
    let (_root, ws, cache) = workspace();

    let cli = cli_impact(&ws, &cache);
    let mcp = mcp_impact(&mcp_context(&ws, &cache)).await;

    assert_eq!(
        status_of(&cli),
        "complete",
        "el árbol se leyó entero: la cobertura es completa: {cli:?}"
    );
    assert_eq!(
        coverage_of(&cli),
        coverage_of(&mcp),
        "las dos superficies dicen lo mismo sobre el mismo árbol: {cli:?} / {mcp:?}"
    );
}

/// Un árbol con un fichero que no se pudo leer: las dos superficies dicen
/// "partial" y NOMBRAN el fichero.
#[tokio::test]
async fn the_impact_question_says_partial_over_a_file_it_could_not_read() {
    let (_root, ws, cache) = workspace();
    make_unreadable(&ws);

    let cli = cli_impact(&ws, &cache);
    let ctx = mcp_context(&ws, &cache);
    let mcp = mcp_impact(&ctx).await;

    assert_eq!(
        status_of(&cli),
        "partial",
        "un fichero que no se pudo leer deja la cobertura incompleta: {cli:?}"
    );
    assert_eq!(
        coverage_of(&cli),
        coverage_of(&mcp),
        "las dos superficies dicen lo mismo sobre el mismo árbol: {cli:?} / {mcp:?}"
    );
    assert_eq!(
        skipped_of(&cli, &ws),
        vec!["b.rs".to_string()],
        "el hueco se nombra: {cli:?}"
    );

    // The second question of the same session is answered with the graph
    // the session already holds, and it has to publish the same coverage:
    // the graph was measured when it was loaded, and asking again does not
    // change what it covers.
    let second = mcp_impact(&ctx).await;
    assert_eq!(
        coverage_of(&second),
        coverage_of(&mcp),
        "la segunda pregunta de la misma sesión dice la misma cobertura: \
         {mcp:?} / {second:?}"
    );
}

/// Una referencia que nadie declaró: las dos superficies dicen "partial" y
/// nombran la referencia.
#[tokio::test]
async fn the_impact_question_says_partial_over_a_reference_it_could_not_tie() {
    let (_root, ws, cache) = workspace_with_a_pending_reference();

    let cli = cli_impact(&ws, &cache);
    let mcp = mcp_impact(&mcp_context(&ws, &cache)).await;

    assert_eq!(
        status_of(&cli),
        "partial",
        "una referencia sin declaración deja la cobertura incompleta: {cli:?}"
    );
    assert_eq!(
        coverage_of(&cli),
        coverage_of(&mcp),
        "las dos superficies dicen lo mismo sobre el mismo árbol: {cli:?} / {mcp:?}"
    );
    assert!(
        pending_of(&cli)
            .iter()
            .any(|target| target.contains("no_existe")),
        "la referencia se nombra: {cli:?}"
    );
}

// ---------------------------------------------------------------------------
// Ley 2: la pregunta de jerarquía dice su cobertura, y dice la misma.
// ---------------------------------------------------------------------------

/// Un árbol que se leyó entero: las dos superficies dicen "complete".
#[tokio::test]
async fn the_hierarchy_question_says_complete_over_a_tree_read_whole() {
    let (_root, ws, cache) = workspace();

    let cli = cli_hierarchy(&ws, &cache);
    let mcp = mcp_hierarchy(&mcp_context(&ws, &cache)).await;

    assert_eq!(
        status_of(&cli),
        "complete",
        "el árbol se leyó entero: la cobertura es completa: {cli:?}"
    );
    assert_eq!(
        coverage_of(&cli),
        coverage_of(&mcp),
        "las dos superficies dicen lo mismo sobre el mismo árbol: {cli:?} / {mcp:?}"
    );
}

/// Un árbol con un fichero que no se pudo leer: las dos superficies dicen
/// "partial" y lo nombran.
#[tokio::test]
async fn the_hierarchy_question_says_partial_over_a_file_it_could_not_read() {
    let (_root, ws, cache) = workspace();
    make_unreadable(&ws);

    let cli = cli_hierarchy(&ws, &cache);
    let ctx = mcp_context(&ws, &cache);
    let mcp = mcp_hierarchy(&ctx).await;

    assert_eq!(
        status_of(&cli),
        "partial",
        "un fichero que no se pudo leer deja la cobertura incompleta: {cli:?}"
    );
    assert_eq!(
        coverage_of(&cli),
        coverage_of(&mcp),
        "las dos superficies dicen lo mismo sobre el mismo árbol: {cli:?} / {mcp:?}"
    );
    assert_eq!(
        skipped_of(&cli, &ws),
        vec!["b.rs".to_string()],
        "el hueco se nombra: {cli:?}"
    );

    // The second question of the same session says the same coverage.
    let second = mcp_hierarchy(&ctx).await;
    assert_eq!(
        coverage_of(&second),
        coverage_of(&mcp),
        "la segunda pregunta de la misma sesión dice la misma cobertura: \
         {mcp:?} / {second:?}"
    );
}

/// Una referencia que nadie declaró: las dos superficies dicen "partial" y
/// nombran la referencia.
#[tokio::test]
async fn the_hierarchy_question_says_partial_over_a_reference_it_could_not_tie() {
    let (_root, ws, cache) = workspace_with_a_pending_reference();

    let cli = cli_hierarchy(&ws, &cache);
    let mcp = mcp_hierarchy(&mcp_context(&ws, &cache)).await;

    assert_eq!(
        status_of(&cli),
        "partial",
        "una referencia sin declaración deja la cobertura incompleta: {cli:?}"
    );
    assert_eq!(
        coverage_of(&cli),
        coverage_of(&mcp),
        "las dos superficies dicen lo mismo sobre el mismo árbol: {cli:?} / {mcp:?}"
    );
    assert!(
        pending_of(&cli)
            .iter()
            .any(|target| target.contains("no_existe")),
        "la referencia se nombra: {cli:?}"
    );
}
