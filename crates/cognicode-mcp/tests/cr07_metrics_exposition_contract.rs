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

/// A port the OS says is free. Binding it and releasing it before the
/// server starts is not atomic, but the server is started immediately
/// after and this is the only way to keep the test parallel-safe without
/// pinning a port that a concurrent CI runner would collide with.
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

impl Scrape {
    fn fetch() -> (Self, Child) {
        let port = free_port();
        let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().expect("addr");

        let mut child = Command::new(server_binary())
            .arg("--cwd")
            .arg(fixture_ws())
            .arg("--listen")
            .arg(addr.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn cognicode-mcp-server");

        let (status_line, headers, body) = wait_for_scrape(addr, &mut child);
        (
            Scrape {
                status_line,
                headers,
                body,
            },
            child,
        )
    }
}

#[test]
fn metrics_endpoint_serves_the_pinned_prometheus_exposition() {
    let (scrape, mut child) = Scrape::fetch();
    let _ = child.kill();
    let _ = child.wait();

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
    let (scrape, mut child) = Scrape::fetch();
    let _ = child.kill();
    let _ = child.wait();

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
    let (scrape, mut child) = Scrape::fetch();
    let _ = child.kill();
    let _ = child.wait();

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
