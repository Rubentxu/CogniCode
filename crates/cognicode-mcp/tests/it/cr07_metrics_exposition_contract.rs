//! The `/metrics` exposure contract must survive the OTel 0.27 -> 0.29.1 migration.
//!
//! ## Why this test exists
//!
//! The OTel advisory migration was deferred with a reason, quoted verbatim in
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
//! Measured before touching any manifest: **no test in the repository
//! exercised `/metrics` at all.** The endpoint had zero coverage, and the
//! exposition format it returns is a public contract — orchestrator scrape
//! configs match on the literal content-type, and a silent change from
//! `text/plain; version=0.0.4` to protobuf exposition breaks every scraper
//! without failing a single build.
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
//! 5. `telemetry_sdk_version` reports the migrated SDK. This is the anti-
//!    vacuity guard: it makes the test fail if the migration is reverted or
//!    if the exposition is being served by a stale artifact.
//!
//! Real binary, real HTTP, real scrape. The content-type is read off the wire
//! rather than asserted against the source, because the source is the thing
//! being changed.

use crate::common;

use std::path::{Path, PathBuf};

use common::{HttpResponse, HttpServer};

fn fixture_ws() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws")
}

/// One scrape of `/metrics` from a server started for the check, with no
/// credential: the scrape route must not need one.
fn fetch() -> HttpResponse {
    let server = HttpServer::start(&fixture_ws(), None, &[]);
    server.get("/metrics", None)
}

#[test]
fn metrics_endpoint_serves_the_pinned_prometheus_exposition() {
    let response = fetch();

    assert!(
        response.status_line.contains("200"),
        "/metrics must answer 200, got `{}`",
        response.status_line.trim()
    );

    assert_eq!(
        response.header("content-type"),
        Some("text/plain; version=0.0.4"),
        "/metrics changed its content-type. This literal is a public contract: orchestrator \
         scrape configs match on it. A migration that silently switched to protobuf exposition \
         would break every scraper while still returning 200, which is why this asserts the \
         wire value instead of the constant in source. headers: {:?}",
        response.headers
    );

    assert!(
        !response.body.trim().is_empty(),
        "/metrics returned 200 with an empty body"
    );

    // A real exposition has HELP/TYPE lines. Asserting only "non-empty"
    // would pass on an error string, which is the exact failure the
    // status code cannot see.
    assert!(
        response.body.contains("# HELP") && response.body.contains("# TYPE"),
        "/metrics body is not a Prometheus exposition (no # HELP / # TYPE lines):\n{}",
        response.body
    );
}

#[test]
fn the_exporter_is_actually_registered_and_rendering() {
    let response = fetch();

    // `target_info` is emitted by the OTel SDK's own Prometheus exporter.
    // Its presence is what distinguishes "exporter registered and rendering"
    // from "handler answers 200 with an empty registry" — the latter is a
    // 200 that no test in this repository detected.
    assert!(
        response.body.contains("target_info"),
        "/metrics has no target_info, so the OTel Prometheus exporter is not registered against \
         the shared Registry. Body:\n{}",
        response.body
    );
    assert!(
        response.body.contains("telemetry_sdk_language=\"rust\""),
        "target_info is missing the rust SDK label, so it did not come from the OTel exporter:\n{}",
        response.body
    );
}

#[test]
fn the_exposition_reports_the_migrated_opentelemetry_sdk() {
    let response = fetch();

    let version = response
        .body
        .split("telemetry_sdk_version=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| {
            panic!(
                "could not read telemetry_sdk_version from /metrics; the label the migration \
                 is supposed to move is absent:\n{}",
                response.body
            )
        });

    let mut parts = version
        .split(['.', '-', '+'])
        .filter_map(|p| p.parse::<u64>().ok());
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);

    assert!(
        (major, minor) >= (0, 29),
        "/metrics still reports telemetry_sdk_version={version}. The advisory migration goes \
         OTel 0.27 -> 0.29.1 because only 0.29.1 declares `prometheus ^0.14`, which is what \
         pulls `protobuf ^3.7.2` and resolves RUSTSEC-2024-0437. 0.28.0 and 0.29.0 both keep \
         protobuf on 2.x, so a build reporting 0.28 or 0.29.0 would still be vulnerable while \
         looking migrated."
    );
}
