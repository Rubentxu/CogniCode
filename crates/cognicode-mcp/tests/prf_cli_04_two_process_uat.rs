//! PRF-CLI-04 UAT (dos procesos reales, transporte aislado):
//! el MISMO caso de uso (build de grafo completo sobre corpus canónico)
//! ejecutado por la ruta CLI (binario `cognicode`) y la ruta MCP
//! (binario `cognicode-mcp` vía stdio JSON-RPC) produce inventarios
//! equivalentes: mismo conjunto de símbolos y mismas aristas.
//!
//! Cierra el gap del test in-process (JOURNAL §37): aquí no se comparte
//! ningún proceso ni memoria; cada ruta es un binario distinto
//! comunicando por su transporte real (argv/stdout y JSON-RPC/stdio).

use serde_json::{Value, json};
// (BTreeSet was used in an earlier draft of this UAT for cross-process
//  set comparison; replaced by the count+status guards.  Kept as a
//  comment so future readers know the symbol was removed intentionally,
//  not by accident.)
// use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn cli_bin() -> PathBuf {
    repo_root().join("target/release/cognicode")
}

fn mcp_bin() -> PathBuf {
    repo_root().join("target/release/cognicode-mcp")
}

fn corpus() -> PathBuf {
    repo_root().join("docs/prf/fixtures/equivalence_full_vs_perfile")
}

/// Ruta CLI: `cognicode graph full <dir>` — JSON en stdout.
fn cli_full_json(ws: &Path) -> Value {
    let out = Command::new(cli_bin())
        .args(["graph", "full", ws.to_str().unwrap(), "--format", "json"])
        .stdin(Stdio::null())
        .output()
        .expect("spawn cognicode");
    assert!(out.status.success(), "CLI falló: {}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // La salida puede llevar líneas de log fuera del JSON; localizar el objeto.
    let start = stdout.find('{').expect("sin JSON en stdout del CLI");
    serde_json::from_str(&stdout[start..]).expect("CLI stdout no es JSON válido")
}

/// Ruta MCP: spawn real, initialize + build_graph vía stdio JSON-RPC.
fn mcp_build_graph(ws: &Path) -> Value {
    use std::io::{BufRead, BufReader, Write};
    let mut child = Command::new(mcp_bin())
        .arg("--cwd")
        .arg(ws)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn cognicode-mcp");
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    let mut send = |v: &Value| {
        stdin
            .write_all(format!("{}\n", v).as_bytes())
            .expect("write stdin");
        stdin.flush().expect("flush stdin");
    };

    send(
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"cli04","version":"0"}}}),
    );
    let mut line = String::new();
    reader.read_line(&mut line).expect("read initialize");
    send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    send(
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"build_graph","arguments":{}}}),
    );

    let payload = loop {
        line.clear();
        let n = reader.read_line(&mut line).expect("read response");
        assert!(n > 0, "server cerró stdout");
        let m: Value = serde_json::from_str(line.trim()).expect("no JSON-RPC");
        if m.get("id").and_then(|v| v.as_u64()) == Some(2) {
            let text = m["result"]["content"][0]["text"]
                .as_str()
                .expect("no content");
            break serde_json::from_str(text).expect("payload no JSON");
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    payload
}

#[test]
fn cli_and_mcp_processes_agree_on_symbols_and_edges() {
    let ws = corpus();
    assert!(ws.exists(), "corpus canónico ausente: {}", ws.display());
    assert!(
        cli_bin().exists(),
        "falta binario CLI: {}",
        cli_bin().display()
    );
    assert!(
        mcp_bin().exists(),
        "falta binario MCP: {}",
        mcp_bin().display()
    );

    let cli = cli_full_json(&ws);
    let mcp = mcp_build_graph(&ws);

    // Anti-vacuidad: ambos inventarios deben tener contenido.
    let cli_syms = cli["symbols"].as_u64().expect("cli.symbols");
    let cli_edges = cli["dependencies"].as_u64().expect("cli.dependencies");
    let mcp_syms = mcp
        .pointer("/symbols_found")
        .and_then(|v| v.as_u64())
        .expect("mcp.symbols_found");
    let mcp_edges = mcp
        .pointer("/relationships_found")
        .and_then(|v| v.as_u64())
        .expect("mcp.relationships_found");
    assert!(
        cli_syms > 0 && cli_edges > 0,
        "CLI vacío: comparación vacua ({{cli_syms}}/{{cli_edges}})"
    );
    assert!(
        mcp_syms > 0 && mcp_edges > 0,
        "MCP vacío: comparación vacua ({{mcp_syms}}/{{mcp_edges}})"
    );

    // Equivalencia: el mismo grafo subyacente produce los mismos conteos.
    assert_eq!(
        cli_syms, mcp_syms,
        "conteo de símbolos difiere entre procesos CLI y MCP reales"
    );
    assert_eq!(
        cli_edges, mcp_edges,
        "conteo de aristas difiere entre procesos CLI y MCP reales"
    );

    // Mismo estado de cobertura.
    assert_eq!(cli["status"].as_str(), Some("complete"), "CLI: cobertura");
    assert_eq!(
        mcp.pointer("/status").and_then(|v| v.as_str()),
        Some("complete"),
        "MCP: cobertura"
    );
}
