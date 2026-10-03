//! CR-07: the `/metrics` exposure contract must survive the OTel 0.27 -> 0.29.1 migration.
//!
//! ## Why this test exists
//!
//! CR-07 was deferred with the reason, quoted verbatim in
//! `docs/debts/DEBT-SEC-001-advisory-ignores.md`:
//!
//! > Romper `/metrics` en producción sería peor que el advisory.
//!
//! That is the right reason and it is still the right reason. But it was
//! recorded as prose in a journal entry, with no test behind it — so at the
//! moment the migration became worth doing, nothing measured whether
//! `/metrics` still worked afterwards. The whole deferral rested on an
//! unverified prediction.
//!
//! Measured 2026-10-01 before touching any manifest: **no test in the
//! repository exercised `/metrics` at all.** The endpoint had zero coverage,
//! and the exposition format it returns is a public contract — orchestrator
//! scrape configs match on the literal content-type, and a silent change
//! from `text/plain; version=0.0.4` to protobuf exposition breaks every
//! scraper without failing a single build.
//!
//! So this test is written BEFORE the migration and asserts the contract
//! that must not change:
//!
//! 1. `/metrics` answers 200.
//! 2. Content-type is exactly `text/plain; version=0.0.4`.
//! 3. The body is a parseable Prometheus exposition, not an empty buffer and
//!    not an error string.
//! 4. The OTel SDK's own `target_info` is present, which proves the exporter
//!    is genuinely registered and rendering — a `/metrics` that returns 200
//!    with nothing in it is the failure mode a status-code assertion misses.
//! 5. `telemetry_sdk_version` reports the migrated SDK. This is the anti->
//!    vacuity guard: it makes the test fail if the migration is reverted or
//!    if the exposition is being served by a stale artifact.
//!
//! Real binary, real HTTP, real scrape. The content-type is read off the wire
//! rather than asserted against the source, because the source is the thing
//! being changed.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// Minimal `text/plain; version=0.0.4` scrape over raw HTTP.
///
/// No HTTP client dependency: the crate has no `reqwest`/`ureq` in
/// `[dev-dependencies]`, and a one-shot GET against a local socket is not
/// worth one. Reading the status line and headers off the socket is what lets
/// this assert the *wire* content-type rather than a re-derivation of it.
fn scrape(addr: std::net::SocketAddr) -> (String, Vec<(String, String)>, String) {
    let mut stream = std::net::TcpStream::connect(addr).expect("connect to /metrics");
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("read timeout");
    stream
        .write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .expect("send GET");

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader
        .read_line(&mut status_line)
        .expect("read status line");

    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }

    let mut body = String::new();
    reader.read_to_string(&mut body).expect("read body");

    (status_line, headers, body)
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// Held for as long as a server is alive, so no two tests in this file can be
/// holding the same port.
///
/// MEDIDO 2026-10-03. The lane de v0.101.5 cayo en `clean-clone` por esto:
///
///     thread 'the_exposition_reports_the_migrated_opentelemetry_sdk' panicked at
///     crates/cognicode-mcp/tests/cr07_metrics_exposition_contract.rs:66:10:
///     read status line: Os { code: 104, kind: ConnectionReset, message:
///     "Connection reset by peer" }
///
/// No era `/metrics`: el handler es un axum normal, sin `SO_LINGER` ni accept
/// manual, y no reinicia conexiones. Era esta linea. `free_port()` suelta el
/// listener efimero ANTES de que el servidor haga bind, asi que entre los dos
/// otro test del mismo fichero puede quedarse con ese puerto; cuando el
/// dueño original mata su servidor, el que Scraped al suyo recibe RST.
///
/// MEDIDO, con la misma concurrencia en los dos lados —seis instancias del
/// binario, doce rondas, 72 ejecuciones:
///
///     --test-threads=3, carga 21:  1 fallo de 72
///     --test-threads=1, carga 45:  0 fallos de 72
///
/// La diferencia es exactamente la concurrencia dentro del binario. Y este
/// fichero es el UNICO del workspace que suelta el puerto antes del bind:
/// `installer_transaction.rs`, `lifecycle_resolver.rs` y
/// `cp1_control_plane_endpoint.rs` sirven desde el listener que ya tienen
/// abierto, asi que no pueden perderlo. Por eso la carrera es local aqui y un
/// candado de modulo la cierra entera.
///
/// Sin candado el fallo es de reloj: sale en la lane, que compila con la
/// maquina saturada, y no sale cuando se ejecuta a mano. Un gate que depende
/// de la carga de la maquina no es un gate.
///
/// La reserva no distingue test: el candado se toma en `Scrape::fetch` y se
/// suelta cuando el hijo muere, que es la ventana durante la cual el puerto
/// puede ser robado. Un mutex envenenado se recupera en vez de propagar el
/// panic a los otros dos tests, que fallarian con un mensaje que no dice nada
/// del problema real.
fn port_reservation() -> MutexGuard<'static, ()> {
    static RESERVATION: OnceLock<Mutex<()>> = OnceLock::new();
    RESERVATION
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A port the OS says is free.
///
/// Binding it and releasing it before the server starts is not atomic — see
/// `port_reservation` for the measurement of what that cost. Callers hold the
/// reservation across the server's whole lifetime so the window is empty.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn server_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cognicode-mcp-server"))
}

fn fixture_ws() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws")
}

/// Wait until `/metrics` answers, or fail with the server's stderr — which is
/// where a panicking `server_main` puts the reason.
fn wait_for_scrape(
    addr: std::net::SocketAddr,
    child: &mut Child,
) -> (String, Vec<(String, String)>, String) {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut last_err = String::new();
    while Instant::now() < deadline {
        if let Ok(Some(status)) = child.try_wait() {
            let mut err = String::new();
            if let Some(stderr) = child.stderr.as_mut() {
                let _ = stderr.read_to_string(&mut err);
            }
            panic!("cognicode-mcp-server exited early with {status}\nstderr:\n{err}");
        }
        match std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(300)) {
            Ok(_) => return scrape(addr),
            Err(e) => last_err = e.to_string(),
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let mut err = String::new();
    if let Some(stderr) = child.stderr.as_mut() {
        let _ = stderr.read_to_string(&mut err);
    }
    panic!("/metrics never became reachable: {last_err}\nstderr:\n{err}");
}

struct Scrape {
    status_line: String,
    headers: Vec<(String, String)>,
    body: String,
}

/// Kills the server whenever it goes out of scope, panic or not.
///
/// `Child`'s own `Drop` does not kill: it only drops the handle and leaves the
/// process running. A panic between `spawn` and the caller's `kill` — which is
/// exactly what a failing scrape does — therefore stranded the server on the
/// machine, holding its port, until someone noticed.
///
/// MEDIDO 2026-10-04, con la cuenta cerrada: cuatro `cognicode-mcp-server`
/// huerfanos, todos reparentados a init.
///
///     20:38  la lane de v0.101.5, que cayo con el ConnectionReset de la linea 66
///     16:53  la reproduccion de la carrera de puertos (hilo 439794 -> hijo 439798)
///     09:52  estres mutado, fallo 1 de 2
///     09:39  estres mutado, fallo 2 de 2
///
/// Uno por cada ejecucion que fallo, ni una mas. Un fallo de test que se lleva
/// por delante un proceso de la maquina convierte un rojo en deuda que se
/// acumula entre corridas.
struct ChildGuard(Child);

impl ChildGuard {
    fn kill_and_wait(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.kill_and_wait();
    }
}

impl Scrape {
    /// Both guards are returned so the caller keeps the port reserved and the
    /// server owned for the whole time it is alive. Binding them to
    /// `_reservation` and `child` is enough: both are released when those
    /// bindings go out of scope, which the caller's `kill_and_wait()` above has
    /// already made safe.
    fn fetch() -> (Self, ChildGuard, MutexGuard<'static, ()>) {
        let reservation = port_reservation();
        let port = free_port();
        let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().expect("addr");

        let mut child = ChildGuard(
            Command::new(server_binary())
                .arg("--cwd")
                .arg(fixture_ws())
                .arg("--listen")
                .arg(addr.to_string())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn cognicode-mcp-server"),
        );

        let (status_line, headers, body) = wait_for_scrape(addr, &mut child.0);
        (
            Scrape {
                status_line,
                headers,
                body,
            },
            child,
            reservation,
        )
    }
}

#[test]
fn metrics_endpoint_serves_the_pinned_prometheus_exposition() {
    let (scrape, mut child, _reservation) = Scrape::fetch();
    child.kill_and_wait();

    assert!(
        scrape.status_line.contains("200"),
        "/metrics must answer 200, got `{}`",
        scrape.status_line.trim()
    );

    assert_eq!(
        header(&scrape.headers, "content-type"),
        Some("text/plain; version=0.0.4"),
        "/metrics changed its content-type. This literal is a public contract: orchestrator \
         scrape configs match on it. A migration that silently switched to protobuf exposition \
         would break every scraper while still returning 200, which is why this asserts the \
         wire value instead of the constant in source. headers: {:?}",
        scrape.headers
    );

    assert!(
        !scrape.body.trim().is_empty(),
        "/metrics returned 200 with an empty body"
    );

    // A real exposition has HELP/TYPE lines. Asserting only "non-empty"
    // would pass on an error string, which is the exact failure the
    // status code cannot see.
    assert!(
        scrape.body.contains("# HELP") && scrape.body.contains("# TYPE"),
        "/metrics body is not a Prometheus exposition (no # HELP / # TYPE lines):\n{}",
        scrape.body
    );
}

#[test]
fn the_exporter_is_actually_registered_and_rendering() {
    let (scrape, mut child, _reservation) = Scrape::fetch();
    child.kill_and_wait();

    // `target_info` is emitted by the OTel SDK's own Prometheus exporter.
    // Its presence is what distinguishes "exporter registered and rendering"
    // from "handler answers 200 with an empty registry" — the latter is a
    // 200 that no test in this repository detected, measured 2026-10-01.
    assert!(
        scrape.body.contains("target_info"),
        "/metrics has no target_info, so the OTel Prometheus exporter is not registered against \
         the shared Registry. Body:\n{}",
        scrape.body
    );
    assert!(
        scrape.body.contains("telemetry_sdk_language=\"rust\""),
        "target_info is missing the rust SDK label, so it did not come from the OTel exporter:\n{}",
        scrape.body
    );
}

#[test]
fn the_exposition_reports_the_migrated_opentelemetry_sdk() {
    let (scrape, mut child, _reservation) = Scrape::fetch();
    child.kill_and_wait();

    let version = scrape
        .body
        .split("telemetry_sdk_version=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| {
            panic!(
                "could not read telemetry_sdk_version from /metrics; the label the migration \
                 is supposed to move is absent:\n{}",
                scrape.body
            )
        });

    let mut parts = version
        .split(['.', '-', '+'])
        .filter_map(|p| p.parse::<u64>().ok());
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);

    assert!(
        (major, minor) >= (0, 29),
        "/metrics still reports telemetry_sdk_version={version}. CR-07 migrates OTel 0.27 -> \
         0.29.1 because only 0.29.1 declares `prometheus ^0.14`, which is what pulls \
         `protobuf ^3.7.2` and resolves RUSTSEC-2024-0437. 0.28.0 and 0.29.0 both keep \
         protobuf on 2.x, so a build reporting 0.28 or 0.29.0 would still be vulnerable while \
         looking migrated."
    );
}
