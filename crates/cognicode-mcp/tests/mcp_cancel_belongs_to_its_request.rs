//! Una cancelacion pertenece a la peticion que la pidio.
//!
//! El servidor MCP guarda una bandera de cancelacion para saber si una
//! peticion en vuelo debe abandonar. Esa bandera es de la peticion, no de
//! la sesion: una peticion cancelada no puede cancelar a las que van
//! detras, porque el cliente que las lanzo no las cancelo.
//!
//! Aqui se comprueba por el camino que de verdad usa un cliente: el binario
//! real, por stdio. Se llama a una herramienta, se manda despues la
//! notificacion `notifications/cancelled` de esa peticion —el protocolo
//! admite que llegue cuando la peticion ya termino— y se llama a la misma
//! herramienta otra vez. La segunda llamada tiene que salir bien.
//!
//! Lo que se mide es el texto de la respuesta, que es lo que lee el
//! cliente: un `Cancelled` ahi significa que la segunda peticion observo la
//! cancelacion de la primera.

mod common;

use common::McpSession;
use serde_json::json;
use std::path::{Path, PathBuf};

/// Copia el fixture a un directorio temporal: `build_graph` escribe el
/// grafo en el workspace, y el fixture es del repositorio.
fn workspace_copy() -> (PathBuf, tempfile::TempDir) {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws");
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path().join("ws");
    copy_dir(&fixture, &ws);
    (ws, tmp)
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// Un workspace con suficientes ficheros para que el indice tarde.
///
/// La cancelacion se manda mientras el servidor trabaja, asi que el trabajo
/// tiene que durar lo suficiente para que la notificacion caiga en mitad.
fn corpus_workspace(files: usize) -> (PathBuf, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path().join("ws");
    let src = ws.join("src");
    std::fs::create_dir_all(&src).expect("create src");
    for i in 0..files {
        std::fs::write(
            src.join(format!("f{i}.rs")),
            format!("fn sym_{i}() -> u32 {{ {i} }}\nfn main() {{}}\n"),
        )
        .expect("write source");
    }
    (ws, tmp)
}

/// El texto de la respuesta de una llamada a herramienta.
fn tool_text(response: &serde_json::Value) -> String {
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelar_una_peticion_no_deja_cancelada_la_siguiente() {
    let (ws, _tmp) = workspace_copy();
    let mut session = McpSession::spawn(&ws).await.expect("spawn");

    let arguments = json!({"directory": ws.to_str().unwrap()});

    // 1. Una peticion que termina bien.
    let (id, first) = session
        .request_with_id("tools/call", json!({"name": "build_graph", "arguments": arguments}))
        .await
        .expect("primera llamada");
    let first_text = tool_text(&first);
    assert!(
        !first_text.contains("Cancel"),
        "la primera llamada no deberia estar cancelada, respondio: {first_text}"
    );

    // 2. La cancelacion de esa peticion. El protocolo admite que la
    //    notificacion llegue despues de que la peticion haya terminado.
    session
        .notify_cancelled(id)
        .await
        .expect("notificacion de cancelacion");

    // 3. La peticion siguiente, que el cliente no cancelo.
    let (_id, second) = session
        .request_with_id("tools/call", json!({"name": "build_graph", "arguments": arguments}))
        .await
        .expect("segunda llamada");
    let second_text = tool_text(&second);
    assert!(
        !second_text.contains("Cancel"),
        "la cancelacion de la peticion {id} enveneno la sesion: la siguiente \
         peticion respondio: {second_text}"
    );

    session.shutdown().await;
}

/// The cancellation reaches the request that asked for it while that
/// request is still running.
///
/// The request goes out and the cancellation follows it without waiting
/// for the answer, so the server is still working on it. The answer has
/// to say that the request was cancelled —and only that request: the one
/// that comes after has to be served normally.
///
/// This one was green before the change and has to stay green: it is the
/// guard for what the fix must not break. The server keeps the ability to
/// stop a request that is in flight; the flag that says so now belongs to
/// the request, and the request that owns it is the one that stops.
#[tokio::test(flavor = "multi_thread")]
async fn la_cancelacion_en_vuelo_llega_a_su_peticion() {
    let (ws, _tmp) = corpus_workspace(3000);
    let mut session = McpSession::spawn(&ws).await.expect("spawn");

    let arguments = json!({"symbol_name": "main", "directory": ws.to_str().unwrap()});

    // The request goes out, and the cancellation names it right away.
    let id = session
        .send_request("tools/call", json!({"name": "build_call_subgraph", "arguments": arguments.clone()}))
        .await
        .expect("la peticion sale");
    session
        .notify_cancelled(id)
        .await
        .expect("notificacion de cancelacion");

    let cancelled = session
        .await_response(id)
        .await
        .expect("la respuesta de la peticion cancelada");
    let cancelled_text = tool_text(&cancelled);
    assert!(
        cancelled_text.contains("Cancel"),
        "la peticion {id} estaba en vuelo y su cancelacion tiene que llegarle, \
         respondio: {cancelled_text}"
    );

    // The request that comes after is not cancelled.
    let (_id, after) = session
        .request_with_id("tools/call", json!({"name": "build_call_subgraph", "arguments": arguments}))
        .await
        .expect("la peticion siguiente");
    let after_text = tool_text(&after);
    assert!(
        !after_text.contains("Cancel"),
        "la cancelacion de la peticion {id} enveneno la sesion: la siguiente \
         peticion respondio: {after_text}"
    );

    session.shutdown().await;
}
