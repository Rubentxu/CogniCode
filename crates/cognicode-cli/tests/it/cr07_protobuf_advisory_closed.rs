//! CR-07: `RUSTSEC-2024-0437` is fixed, not deferred.
//!
//! ## Why this test exists
//!
//! CR-07 was parked on a false premise. The record said the fix required
//! `opentelemetry-prometheus 0.28`, so bumping to 0.28 leaves protobuf on 2.x,
//! and an agent that trusted the record would read that as "deferral
//! confirmed" rather than "the record was wrong". Measured against the
//! crates.io index on 2026-10-01: 0.28.0 and 0.29.0 both declare
//! `prometheus ^0.13` + `protobuf ^2.14`; only **0.29.1** declares
//! `prometheus ^0.14`, and `prometheus 0.14.0` is what pulls `protobuf ^3.7.2`.
//!
//! A parked item whose stated reason is wrong is worse than an open one,
//! because it reads as a decision rather than a gap. This test closes that
//! gap by asserting the *resolved dependency graph*, which is the thing the
//! advisory actually applies to. Not the comment in `deny.toml`. Not the
//! manifest requirement. The lockfile.
//!
//! ## What it does NOT assert
//!
//! It does not run `cargo deny`. That is the gate, and it runs in CI at
//! `release.yml:128` and `release-validate.yml:123`. This test is the local
//! half: it is the thing that fails the moment the premise is wrong, without
//! a network fetch and without cargo-deny installed, so the correction cannot
//! be deferred again behind an unavailable tool.
//!
//! It also does not assert the OTel version directly. OTel is the *route* to
//! the fix, not the fix. `prometheus 0.14` is reachable other ways, and a test
//! that pinned the route instead of the destination would fail for a correct
//! change and pass for a wrong one.
//!
//! ## Parsing
//!
//! Dependency-free line scanner over `Cargo.lock`, which is a flat
//! `[[package]]` format with `name = ` / `version = ` on adjacent lines.

use std::path::PathBuf;

/// The advisory this contract closes.
const ADVISORY: &str = "RUSTSEC-2024-0437";

/// The minimum protobuf version that carries the fix.
const MIN_FIXED: (u64, u64, u64) = (3, 7, 2);

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

/// Version of `crate` as resolved in `Cargo.lock`, or `None` when absent.
fn resolved_version(crate_name: &str) -> Option<String> {
    let lock =
        std::fs::read_to_string(repo_root().join("Cargo.lock")).expect("Cargo.lock is readable");

    let mut lines = lock.lines().peekable();
    while let Some(line) = lines.next() {
        if line.trim() != format!("name = \"{crate_name}\"") {
            continue;
        }
        // `name` is followed by `version` in the same [[package]] block.
        for inner in lines.by_ref() {
            let trimmed = inner.trim();
            if trimmed.starts_with("name = ") || trimmed.starts_with("[[") {
                break;
            }
            if let Some(rest) = trimmed.strip_prefix("version = \"") {
                return rest.strip_suffix('"').map(|v| v.to_string());
            }
        }
    }
    None
}

fn parse_version(v: &str) -> (u64, u64, u64) {
    let mut parts = v
        .split(['.', '-', '+'])
        .filter_map(|p| p.parse::<u64>().ok());
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

#[test]
fn protobuf_in_the_lockfile_is_not_vulnerable() {
    let resolved = resolved_version("protobuf")
        .expect("Cargo.lock pins a protobuf version; it is reachable via opentelemetry-otlp");

    assert!(
        parse_version(&resolved) >= MIN_FIXED,
        "Cargo.lock still resolves protobuf {resolved}, which is below {MIN_FIXED:?} and so \
         still affected by {ADVISORY} (uncontrolled recursion -> stack overflow on untrusted \
         input). {ADVISORY} is not a hygiene notice; it is a vulnerability. The migration is \
         `opentelemetry 0.27 -> 0.29.1`, which moves `opentelemetry-prometheus` to `prometheus \
         ^0.14`, which is what pulls `protobuf ^3.7.2`. Bumping to 0.28 or 0.29.0 does NOT \
         resolve it: both declare `prometheus ^0.13` + `protobuf ^2.14`."
    );
}

#[test]
fn the_advisory_is_not_still_ignored_in_deny_toml() {
    let deny = std::fs::read_to_string(repo_root().join("deny.toml")).expect("deny.toml readable");

    // The ignore block, not the whole file: the corrected comment block
    // deliberately names the advisory while explaining the fix.
    let start = deny
        .find("ignore = [")
        .expect("deny.toml has an ignore array under [advisories]");
    let body_start = start + "ignore = [".len();
    let end = deny[body_start..]
        .find(']')
        .map(|i| body_start + i)
        .expect("the ignore array is closed");

    let still_ignored = deny[body_start..end].contains(ADVISORY);

    assert!(
        !still_ignored,
        "deny.toml still ignores {ADVISORY} while Cargo.lock resolves a protobuf that is \
         already fixed. That is a dead ignore: it suppresses nothing and hides the fact that \
         the migration landed. `unused-ignored-advisory = \"deny\"` is meant to make this \
         impossible; if this test fires, that setting is not doing its job."
    );
}

#[test]
fn the_exporter_route_is_the_one_that_actually_resolves_the_advisory() {
    // Guards against the record regressing to the version that cannot work.
    // opentelemetry-prometheus 0.28.0 and 0.29.0 both still resolve protobuf 2;
    // only 0.29.1 moved to prometheus ^0.14.
    let otel_prom = resolved_version("opentelemetry-prometheus")
        .expect("Cargo.lock pins opentelemetry-prometheus");

    let (major, minor, patch) = parse_version(&otel_prom);
    assert!(
        (major, minor, patch) >= (0, 29, 1),
        "opentelemetry-prometheus resolved to {otel_prom}. Versions 0.28.0 and 0.29.0 declare \
         `prometheus ^0.13` + `protobuf ^2.14` and therefore do NOT resolve {ADVISORY}. Only \
         0.29.1+ declares `prometheus ^0.14`, which pulls `protobuf ^3.7.2`. A migration that \
         stopped at 0.29.0 would have looked finished."
    );
}
