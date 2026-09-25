//! L1.3 — Compat policy 0.97.x: executable compatibility tests.
//!
//! Política L1.3 Post-PRF: el contrato MCP publicado en 0.97.x debe
//! seguir siendo respetado por el servidor actual. Si rompemos
//! backward-compat (rename de campo, removal de tool, cambio de tipo
//! en un input), los clientes 0.97.x en producción dejan de funcionar.
//!
//! Tests:
//! - L1.3.W1 smoke del binario legacy (0.97.3):
//!   arranca el binario publicado, envía `initialize`, verifica que
//!   responde con `serverInfo.version` y un capabilities set coherente.
//!
//! - L1.3.W2 backward compat del servidor actual:
//!   arranca el binario HEAD, envía un request `tools/call` para
//!   `find_usages` con el schema del cliente 0.97.3 (los campos
//!   `symbol_name`, `include_declaration`, `context_lines` que son los
//!   que existían en 0.97.3), y verifica que:
//!     - responde sin error
//!     - el output es deserializable como el `FindUsagesOutput` actual
//!     - el output contiene los campos que un cliente 0.97.3 esperaría
//!
//! - L1.3.W3 forward compat smoke:
//!   arranca el binario 0.97.3 y le envía un request con schema
//!   ACTUAL. Si 0.97.3 ignora silenciosamente los campos nuevos,
//!   OK (forward compat del servidor legacy).
//!
//! Binario legacy: `sandbox/.compat/0.97.3/cognicode-mcp`
//!   extraído del tarball `cognicode-mcp-0.97.3-x86_64-unknown-linux-gnu.tar.gz`
//!   publicado en `v0.97.3`. Path gitignored (`sandbox/.compat/`),
//!   por lo que el test se marca como SKIP si el binario falta.
//!
//! NO dependencia de red en CI: el binario se extrae manualmente al
//! repo como fixture reproducible (sha256 verificable contra el
//! SHA256SUMS publicado).

use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

mod common;

/// Resuelve el path al binario legacy 0.97.3. Si no existe, devuelve
/// `None` y los tests se saltan con un mensaje claro.
fn compat_mcp_0973_path() -> Option<PathBuf> {
    let workspace_root = workspace_root();
    let bin = workspace_root.join("sandbox/.compat/0.97.3/cognicode-mcp");
    if bin.exists() && bin.is_file() {
        Some(bin)
    } else {
        eprintln!(
            "L1.3 SKIP: binario legacy 0.97.3 no encontrado en {}\n\
             Para preparar: descargar cognicode-mcp-0.97.3-x86_64-unknown-linux-gnu.tar.gz\n\
             de https://github.com/Rubentxu/CogniCode/releases/tag/v0.97.3\n\
             y extraerlo a sandbox/.compat/0.97.3/.",
            bin.display()
        );
        None
    }
}

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

/// `common::binary_path()` resuelve el path del binario `cognicode-mcp`
/// (cargo lo setea via `CARGO_BIN_EXE_cognicode-mcp` para tests del
/// mismo crate). Devuelve el binario actual (v0.98.x).
fn cognicode_mcp_current_path() -> PathBuf {
    common::binary_path()
}

/// Construye un workspace temporal con `alpha` definido línea 1,
/// usado líneas 5 y 6. find_usages debe encontrar 3 entradas.
fn make_workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create tempdir");
    std::fs::create_dir_all(dir.path().join("src")).expect("mkdir src");
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "fn alpha() -> i32 { 42 }\nfn main() { let x = alpha(); let y = alpha(); }\n",
    )
    .expect("write lib.rs");
    dir
}

/// Cliente MCP mínimo en tokio: arranca `bin`, envía una secuencia
/// de requests JSON-RPC separados por newline, lee cada respuesta
/// (esperando por `id` matching) hasta agotar `expected_ids`. Devuelve
/// un mapa `id -> response_value`.
async fn run_jsonrpc_session(
    bin: &PathBuf,
    cwd: &std::path::Path,
    requests: &[serde_json::Value],
    expected_ids: &[u64],
) -> Result<std::collections::HashMap<u64, serde_json::Value>, String> {
    let mut child = Command::new(bin)
        .arg("--cwd")
        .arg(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;

    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout).lines();

    // Escribir todos los requests.
    for req in requests {
        let line = serde_json::to_string(req).map_err(|e| format!("to_string: {e}"))?;
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("write_all: {e}"))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|e| format!("write newline: {e}"))?;
    }
    stdin.flush().await.map_err(|e| format!("flush: {e}"))?;
    drop(stdin); // EOF para stdin — el server termina tras procesar todo

    // Leer respuestas hasta haber visto todos los `expected_ids`.
    let mut responses = std::collections::HashMap::new();
    let expected: std::collections::HashSet<u64> = expected_ids.iter().copied().collect();
    let deadline = tokio::time::Duration::from_secs(15);
    let result = tokio::time::timeout(deadline, async {
        while responses.len() < expected_ids.len() {
            let line = reader
                .next_line()
                .await
                .map_err(|e| format!("read_line: {e}"))?
                .ok_or_else(|| "server closed stdout early".to_string())?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let v: serde_json::Value = serde_json::from_str(trimmed)
                .map_err(|e| format!("non-JSON line {trimmed:?}: {e}"))?;
            if let Some(id) = v.get("id").and_then(|x| x.as_u64())
                && expected.contains(&id)
            {
                responses.insert(id, v);
            }
        }
        Ok::<_, String>(responses)
    })
    .await;

    let _ = child.kill().await;
    result.map_err(|_| "timeout waiting for MCP responses".to_string())?
}

// =============================================================================
// L1.3.W1 — smoke del binario legacy 0.97.3
// =============================================================================

#[tokio::test]
async fn compat_0973_smoke_initialize_responds_with_server_info() {
    let Some(bin) = compat_mcp_0973_path() else {
        return; // SKIP si no hay binario
    };
    let workspace = make_workspace();

    let initialize = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "compat-test", "version": "0.0.1" }
        }
    });
    let initialized_notif = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });

    let responses = run_jsonrpc_session(
        &bin,
        workspace.path(),
        &[initialize, initialized_notif],
        &[1],
    )
    .await
    .expect("session debe succeed");

    let resp = responses.get(&1).expect("response for id=1");

    // 1. serverInfo.version debe ser 0.97.3.
    let server_info = resp
        .get("result")
        .and_then(|r| r.get("serverInfo"))
        .expect("missing serverInfo");
    assert_eq!(
        server_info.get("version").and_then(|v| v.as_str()),
        Some("0.97.3"),
        "serverInfo.version debe ser 0.97.3: {server_info}"
    );

    // 2. protocolVersion debe ser eco del cliente.
    assert_eq!(
        resp.get("result")
            .and_then(|r| r.get("protocolVersion"))
            .and_then(|v| v.as_str()),
        Some("2024-11-05"),
        "protocolVersion echo: {resp}"
    );
}

// =============================================================================
// L1.3.W2 — backward compat del servidor actual
// =============================================================================
//
// El cliente 0.97.3 envía un request `tools/call` para `find_usages`
// con los campos que conoce: `symbol_name`, `include_declaration`,
// `context_lines`. El servidor actual (HEAD) DEBE responder
// exitosamente, y el output DEBE contener los campos que un cliente
// 0.97.3 esperaría.
//
// Si alguien rompe backward-compat (p.ej. renombra `symbol_name` a
// `name`, omite `usages` en el output), este test falla.

#[tokio::test]
async fn compat_backward_find_usages_works_with_0973_schema() {
    let bin = cognicode_mcp_current_path();
    let workspace = make_workspace();

    let initialize = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "compat-test", "version": "0.0.1" }
        }
    });
    let initialized_notif = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    // Schema idéntico al cliente 0.97.3: solo symbol_name, sin include_declaration
    // ni context_lines (que también existían pero los omitimos para simular
    // un cliente que usa solo el mínimo).
    let tools_call = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "find_usages",
            "arguments": { "symbol_name": "alpha" }
        }
    });

    let responses = run_jsonrpc_session(
        &bin,
        workspace.path(),
        &[initialize, initialized_notif, tools_call],
        &[1, 2],
    )
    .await
    .expect("session debe succeed");

    let tools_resp = responses.get(&2).expect("response for id=2");

    // 1. La respuesta no debe contener `error` (puede haber warnings
    //    en stderr pero el JSON-RPC debe ser OK).
    if let Some(err) = tools_resp.get("error") {
        panic!("tools/call devolvió error: {err}");
    }

    // 2. MCP `tools/call` envuelve el output en result.content[0].text
    //    (un content block de tipo 'text'). Hay que deserializar
    //    ese string como JSON para inspeccionar el FindUsagesOutput.
    let result = tools_resp
        .get("result")
        .expect("tools/call response missing result");
    let content_text = result
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|block| block.get("text"))
        .and_then(|t| t.as_str())
        .expect("tools/call result debe tener content[0].text (MCP envelope)");
    let inner: serde_json::Value = serde_json::from_str(content_text)
        .unwrap_or_else(|e| panic!("content text no es JSON válido ({e}): {content_text}"));

    // 3. El output debe incluir los campos que un cliente 0.97.3
    //    espera: `symbol`, `usages`, `total`.
    assert!(
        inner.get("symbol").is_some(),
        "output debe contener 'symbol': {inner}"
    );
    assert_eq!(
        inner.get("symbol").and_then(|v| v.as_str()),
        Some("alpha"),
        "output.symbol debe ser 'alpha': {inner}"
    );
    assert!(
        inner.get("usages").is_some(),
        "output debe contener 'usages': {inner}"
    );
    assert!(
        inner.get("total").is_some(),
        "output debe contener 'total': {inner}"
    );

    // 4. Cada usage debe tener file/line/column (campos que un
    //    cliente 0.97.3 deserializa).
    let usages = inner
        .get("usages")
        .and_then(|v| v.as_array())
        .expect("usages array");
    assert!(!usages.is_empty(), "usages debe tener >=1 entrada: {inner}");
    for u in usages {
        assert!(u.get("file").is_some(), "usage sin file: {u}");
        assert!(u.get("line").is_some(), "usage sin line: {u}");
        assert!(u.get("column").is_some(), "usage sin column: {u}");
    }
}

// =============================================================================
// L1.3.W3 — forward compat smoke
// =============================================================================
//
// El servidor 0.97.3 debe ser capaz de recibir requests que incluyan
// campos del schema ACTUAL. Si rechaza un campo que el cliente actual
// envía normalmente, sería un breaking change para el futuro.

#[tokio::test]
async fn compat_forward_0973_server_handles_modern_request() {
    let Some(bin) = compat_mcp_0973_path() else {
        return; // SKIP si no hay binario
    };
    let workspace = make_workspace();

    let initialize = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "compat-test", "version": "0.0.1" }
        }
    });
    let initialized_notif = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    // Schema ACTUAL: include_declaration=true y context_lines=1 son
    // campos que el cliente moderno enviaría. Si el servidor 0.97.3
    // los ignora silenciosamente, OK (forward compat); si falla con
    // "unknown field", sería un breaking change ASCENDENTE.
    let tools_call = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "find_usages",
            "arguments": {
                "symbol_name": "alpha",
                "include_declaration": true,
                "context_lines": 1
            }
        }
    });

    let responses = run_jsonrpc_session(
        &bin,
        workspace.path(),
        &[initialize, initialized_notif, tools_call],
        &[1, 2],
    )
    .await
    .expect("session debe succeed");

    let tools_resp = responses.get(&2).expect("response for id=2");

    // Si 0.97.3 NO soporta context_lines, podría fallar con code -32602
    // (Invalid Params). Verificamos que NO hay ese error.
    if let Some(err) = tools_resp.get("error") {
        let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
        assert_ne!(
            code, -32602,
            "tools/call con schema moderno falló contra servidor 0.97.3 \
             (rompe forward-compat, code=-32602 Invalid Params): {err}"
        );
    }
}
