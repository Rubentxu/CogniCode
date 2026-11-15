//! El plazo del despacho corta una llamada real, y el llamante sabe lo que pasó.
//!
//! ## El hueco
//!
//! El repositorio demuestra las dos mitades por separado, pero nunca
//! juntas:
//!
//! - `dispatch_timeout_cannot_interrupt_a_blocking_handler` prueba que un
//!   cuerpo que bloquea su hilo NO puede ser interrumpido.
//! - `slow_tool_call_is_bounded_and_does_not_wedge_the_transport` prueba
//!   que una llamada lenta dentro de su ventana se responde con su
//!   respuesta, no con un timeout.
//!
//! Falta la del medio: una llamada cuyo trabajo dura MÁS que la ventana de
//! su categoría, respondida con el error del plazo en lugar de con su
//! respuesta. Ese caso sólo es alcanzable con un trabajo fuera del
//! runtime: dentro del hilo, el plazo no puede observarse.
//!
//! ## El banco
//!
//! Hoy ninguna herramienta publicada tiene un trabajo más largo que su
//! ventana: la ventana se deriva del presupuesto del trabajo y de lo que
//! el catálogo declara (`requires_graph`), y hay contratos que lo
//! vigilan. Así que el caso se alcanza nombrando la ventana —el despacho
//! la acepta— y el test la nombra en un instante que ninguna respuesta
//! alcanza. La herramienta es real, el trabajo es real, y el trabajo se
//! queda fuera del runtime: `search_content` camina el árbol en el pool
//! de bloqueo.
//!
//! ## Lo que se mide
//!
//! 1. **El plazo corta**: la llamada se responde con el error del plazo,
//!    no con el resultado del trabajo.
//! 2. **El transporte no se queda wedged**: la llamada siguiente se
//!    sirve.
//! 3. **El llamante sabe lo que pasó**: la respuesta dice que el trabajo
//!    NO se canceló —una tarea del pool de bloqueo no se aborta— y que su
//!    resultado se tira. Sin ese dato, un llamante que reintenta no sabe
//!    que el intento anterior puede seguir corriendo.

use cognicode_core::interface::mcp::error::{ErrorKind, InterfaceError};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_mcp::rmcp_adapter::call_tool_handler_with_window;
use cognicode_mcp::tool_response::error_result;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

/// Una ventana que ninguna respuesta alcanza.
///
/// El plazo se cumple en el primer instante: el trabajo se corta casi
/// siempre, y el test no depende de la velocidad de la máquina.
const A_DEADLINE_NO_ANSWER_REACHES: Duration = Duration::from_nanos(1);

/// Un contexto sobre el workspace.
fn ctx_for(workspace: &Path) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build()
}

/// Un workspace con un fichero que el recorrido puede encontrar.
fn workspace_with_a_file() -> tempfile::TempDir {
    let workspace = tempfile::tempdir().expect("workspace");
    std::fs::write(workspace.path().join("a.rs"), "fn the_marker() {}\n").expect("fixture file");
    workspace
}

/// Una llamada por el despacho de verdad, con la ventana que el llamante
/// nombra.
async fn call_with(
    ctx: &HandlerContext,
    tool: &str,
    arguments: Value,
    window: Option<Duration>,
) -> Result<String, InterfaceError> {
    let request = CallToolRequestParams::new(tool.to_string())
        .with_arguments(arguments.as_object().cloned().unwrap_or_default());
    call_tool_handler_with_window(ctx, request, window).await
}

/// Una llamada con la ventana que el producto publica.
async fn call(
    ctx: &HandlerContext,
    tool: &str,
    arguments: Value,
) -> Result<String, InterfaceError> {
    call_with(ctx, tool, arguments, None).await
}

/// Los argumentos del recorrido que camina el árbol.
fn search_arguments() -> Value {
    json!({"pattern": "the_marker", "max_results": 1})
}

/// El plazo corta la llamada, y el llamante lee lo que pasó.
#[tokio::test]
async fn the_deadline_cuts_the_call_and_says_the_work_was_not_cancelled() {
    let workspace = workspace_with_a_file();
    let ctx = ctx_for(workspace.path());

    let error = call_with(
        &ctx,
        "search_content",
        search_arguments(),
        Some(A_DEADLINE_NO_ANSWER_REACHES),
    )
    .await
    .expect_err("the deadline cuts a call whose work outlives it");

    // (1) El corte: el error del plazo, no el resultado del trabajo.
    assert_eq!(
        error.kind(),
        ErrorKind::Timeout,
        "the call was cut by the deadline and the answer says another thing: {error}"
    );

    // (3) Lo que el llamante lee: el trabajo no se canceló y su resultado
    // se tira. Sin este dato, un llamante que reintenta no sabe que el
    // intento anterior puede seguir corriendo.
    let wire = error_result(&error);
    let text = wire.content[0]
        .raw
        .as_text()
        .expect("the first block is text")
        .text
        .clone();
    assert!(
        text.contains("timeout"),
        "the answer of a cut call does not name the timeout: {text}"
    );
    assert!(
        text.contains("not cancelled"),
        "the answer of a cut call does not say that the work was not cancelled: {text}"
    );
    assert!(
        text.contains("discarded"),
        "the answer of a cut call does not say that the result of the work is discarded: {text}"
    );
}

/// El corte no deja el transporte wedged.
///
/// La llamada siguiente, con la ventana que el producto publica, se
/// sirve —y se sirve en el MISMO contexto que la llamada cortada: un
/// plazo que corta una llamada no puede dejar la sesión inservible.
#[tokio::test]
async fn the_transport_serves_the_next_call_after_a_cut() {
    let workspace = workspace_with_a_file();
    let ctx = ctx_for(workspace.path());
    call_with(
        &ctx,
        "search_content",
        search_arguments(),
        Some(A_DEADLINE_NO_ANSWER_REACHES),
    )
    .await
    .expect_err("the deadline cuts a call whose work outlives it");

    let answer = call(&ctx, "search_content", search_arguments())
        .await
        .expect("the next call is served");
    assert!(
        answer.contains("the_marker"),
        "the next call is served with its answer: {answer}"
    );
}
