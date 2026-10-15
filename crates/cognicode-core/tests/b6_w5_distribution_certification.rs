//! B6.W5 — Certificación de distribución.
//!
//! This integration suite pins the distribution-certification surface for
//! the B6 program. It enforces three properties that are easy to lose
//! silently and very expensive to recover from: every `SKIP_NOT_APPLICABLE`
//! must have a reason, every `NOT_RUN` must have a reason, and every
//! executed gate must agree with what the workspace actually contains.
//!
//! This suite exists to make sure the certification file at
//! `docs/roadmap/production-ready/certifications/c6/cp2-cp4-cp5.json`
//! cannot drift into a "PASS" claim for a platform that was not
//! exercised. Per PRF-DIST-05 in
//! `docs/prf/specs/SPEC-DISTRIBUTION.md`, support for a platform is
//! only announced with build, native-runner execution, corpus, and a
//! real UAT. Any other state is `NOT_RUN` or `SKIP_NOT_APPLICABLE`
//! with a justification. The contract below enforces exactly that.
//!
//! Suite coverage (>= 7 tests):
//!   1. `cp2_published_tarballs_resolve`
//!   2. `cp4_attestation_script_exists_and_runs`
//!   3. `cp5_release_tag_coherence`
//!   4. `no_silent_skips`
//!   5. `no_unjustified_not_run`
//!   6. `cert_file_exists_and_is_valid_json`
//!   7. `cert_covers_all_three_topics`
//!   8. `cp5_workspace_version_consistent`
//!
//! Esquema de entrada por certificado:
//! ```text
//! { "id": "CP2-01", "status": "PASS|NOT_RUN|SKIP_NOT_APPLICABLE|FAIL",
//!   "command": "...", "exit_code": 0, "evidence_path": "...",
//!   "result": "...", "reason": "..." }
//! ```
//!
//! Política dura:
//!   - Un entry `SKIP_NOT_APPLICABLE` sin `reason` NO es válido: es un
//!     skip silencioso, exactamente lo que este suite prohíbe.
//!   - Un entry `NOT_RUN` sin `reason` NO es válido por la misma razón.
//!   - Un entry `PASS` se exige coherente con el comando y el
//!     exit_code registrados (exit_code == 0).

use std::path::{Path, PathBuf};
use std::process::Command;

// ============================================================================
// Helpers
// ============================================================================

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/cognicode-core has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

fn cert_path() -> PathBuf {
    repo_root()
        .join("docs/roadmap/production-ready/certifications/c6/cp2-cp4-cp5.json")
}

fn read_cert_text() -> String {
    let p = cert_path();
    std::fs::read_to_string(&p).unwrap_or_else(|e| {
        panic!(
            "B6.W5 certification file missing at {p:?}: {e}. \
             B6.W5 closes only after the cert file is populated; \
             a missing file is the RED state of `no_silent_skips`."
        )
    })
}

fn parse_cert() -> serde_json::Value {
    let text = read_cert_text();
    serde_json::from_str::<serde_json::Value>(&text).unwrap_or_else(|e| {
        panic!("B6.W5 certification file is not valid JSON: {e}")
    })
}

fn entries(cert: &serde_json::Value) -> &[serde_json::Value] {
    cert["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("cert file has no top-level `entries` array: {cert:?}"))
        .as_slice()
}

fn entry_reason(entry: &serde_json::Value) -> Option<&str> {
    entry["reason"].as_str()
}

fn entry_status(entry: &serde_json::Value) -> &str {
    entry["status"]
        .as_str()
        .unwrap_or_else(|| panic!("entry is missing string `status`: {entry:?}"))
}

fn entry_id(entry: &serde_json::Value) -> &str {
    entry["id"]
        .as_str()
        .unwrap_or_else(|| panic!("entry is missing string `id`: {entry:?}"))
}

fn entry_topic(entry: &serde_json::Value) -> &str {
    // Convention used in the cert: id starts with "CP2-", "CP4-", "CP5-".
    let id = entry_id(entry);
    if let Some(rest) = id.strip_prefix("CP") {
        rest.split('-').next().unwrap_or("")
    } else {
        ""
    }
}

fn entry_reason_is_non_empty(entry: &serde_json::Value) -> bool {
    entry_reason(entry)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

/// Parse `[workspace.package].version` from a Cargo.toml text.
///
/// Walks line-by-line, keeping a small state machine: inside
/// `[workspace.package]` the first `version = "..."` is the
/// answer; any other `[section]` closes the block. The parser is
/// deliberately restrictive — it only matches `version = "x.y.z"`
/// on its own line — so it cannot accidentally accept a `version`
/// key nested under a member crate or under a different table.
fn parse_workspace_version(text: &str, source: &Path) -> String {
    let mut in_block = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            in_block = trimmed.starts_with("[workspace.package]");
            continue;
        }
        if !in_block {
            continue;
        }
        // Match `version = "..."` strictly: the key has to be the
        // first non-whitespace token and the value has to be a
        // quoted string on the same line.
        let Some(rest) = trimmed.strip_prefix("version") else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('"') else {
            continue;
        };
        let Some(end) = rest.find('"') else {
            continue;
        };
        return rest[..end].to_string();
    }
    panic!("could not parse [workspace.package].version from {source:?}")
}

// ============================================================================
// Test 1 — CP2 published tarballs / catalog references real artefacts.
// ============================================================================

/// CP2 — `product/tools.json` does not declare an `install_instructions`
/// field on its tools yet. The B6.W5 cert must reflect that honestly:
/// the catalog has not been extended to advertise per-tool install paths,
/// so the CP2 sub-claim "every tool's `install_instructions` references a
/// real path or URL" is `NOT_RUN` (not implemented) rather than `PASS`
/// (which would be a false positive) or a silent `SKIP_NOT_APPLICABLE`
/// (which would hide the gap from the agent consuming the catalog).
///
/// This test pins the source side of that claim by reporting whether the
/// field is present at all. The status of the corresponding cert entry
/// (`CP2-01` by convention) is the authoritative answer; this test only
/// checks that the source file is parseable and that the catalog still
/// does not promise install instructions that have not been authored.
#[test]
fn cp2_published_tarballs_resolve() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let catalog_path = manifest
        .ancestors()
        .find_map(|a| {
            let candidate = a.join("product/tools.json");
            if candidate.is_file() {
                Some(candidate)
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("could not locate product/tools.json above {manifest:?}"));

    let text = std::fs::read_to_string(&catalog_path)
        .unwrap_or_else(|e| panic!("cannot read catalog {catalog_path:?}: {e}"));
    let catalog: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("catalog {catalog_path:?} is not valid JSON: {e}"));

    let tools = catalog["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("catalog has no `tools` array"));

    let mut with_install_instructions = 0usize;
    for tool in tools {
        if tool.get("install_instructions").is_some() {
            with_install_instructions += 1;
            // When the field is present, every entry must reference a
            // non-empty path or URL — empty string is silently broken.
            let value = tool["install_instructions"].as_str();
            assert!(
                value.is_some() && !value.unwrap_or("").trim().is_empty(),
                "tool {:?} declares install_instructions but the value is empty",
                tool.get("name")
            );
        }
    }

    // The B6.W5 cert must reflect the actual catalog shape. If the
    // catalog has zero entries with install_instructions, the CP2-01
    // cert entry must be NOT_RUN (or SKIP_NOT_APPLICABLE with a real
    // reason) — never PASS. PASS is reserved for a catalog that has
    // actually been extended and that resolves to a real artefact.
    let cert = parse_cert();
    let cp2_01 = entries(&cert)
        .iter()
        .find(|e| entry_id(e) == "CP2-01")
        .unwrap_or_else(|| {
            panic!(
                "B6.W5 cert is missing CP2-01; entries present: {:?}",
                entries(&cert)
                    .iter()
                    .map(|e| entry_id(e).to_string())
                    .collect::<Vec<_>>()
            )
        });
    let status = entry_status(cp2_01);
    if with_install_instructions == 0 {
        assert!(
            matches!(status, "NOT_RUN" | "SKIP_NOT_APPLICABLE"),
            "CP2-01 status is {status:?} but the catalog declares 0 tools with \
             install_instructions — only NOT_RUN or SKIP_NOT_APPLICABLE with a \
             non-empty reason is honest here"
        );
        assert!(
            entry_reason_is_non_empty(cp2_01),
            "CP2-01 is {status} but `reason` is empty or missing: \
             a silent reason is the failure this contract pins"
        );
    }
    // The "with_install_instructions > 0" branch is not exercised today
    // because the catalog has not been extended; once it is, the
    // assertion above (non-empty value per element) is the one that
    // would catch a regressed install_instructions.
}

// ============================================================================
// Test 2 — CP4 attestation script exists and runs.
// ============================================================================

/// CP4 — `scripts/ci/attest-provenance.sh` is the GENERATION half of
/// release provenance. Its own contract says it must never create a
/// signing key (the custody decision is human-owned per
/// `docs/adr/ADR-RELEASE-PROVENANCE-PIPELINEK.md`), so the script
/// always refuses to run without `--key <existing.key>`. A `--help`
/// invocation, however, exercises the entire argument parser, the
/// `set -uo pipefail` policy, the `cosign`-on-PATH check, and the
/// help-text dispatch, all of which are part of the script's
/// contract.
#[test]
fn cp4_attestation_script_exists_and_runs() {
    let script = repo_root().join("scripts/ci/attest-provenance.sh");
    assert!(
        script.is_file(),
        "CP4 attest-provenance.sh missing at {script:?}"
    );

    let out = Command::new("bash")
        .arg(&script)
        .arg("--help")
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn bash on {script:?}: {e}"));

    assert!(
        out.status.success(),
        "attest-provenance.sh --help must succeed (exit 0); got {:?} \
         with stderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    // `cosign` is the signer the script is built on; a missing cosign
    // would surface only on a real run, but the help path also
    // documents the dependency. We check the binary is on PATH so a
    // misconfigured machine surfaces here rather than at release time.
    let cosign_check = Command::new("bash")
        .arg("-c")
        .arg("command -v cosign >/dev/null 2>&1")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(
        cosign_check,
        "cosign is not on PATH; CP4 cannot attest any artefact without it. \
         Install cosign (>= 2.0) or mark CP4 SKIP_NOT_APPLICABLE with a reason"
    );

    // The CP4 cert entry must agree: if the script ran successfully,
    // CP4-01 (script existence + --help) is PASS; CP4-02 (actual
    // attestation against a release artefact) is a separate claim
    // recorded independently in the cert file.
    let cert = parse_cert();
    let cp4_01 = entries(&cert)
        .iter()
        .find(|e| entry_id(e) == "CP4-01")
        .unwrap_or_else(|| panic!("B6.W5 cert is missing CP4-01"));
    assert_eq!(
        entry_status(cp4_01),
        "PASS",
        "attest-provenance.sh ran end-to-end (exit 0); CP4-01 must be PASS"
    );
}

// ============================================================================
// Test 3 — CP5 release-tag coherence.
// ============================================================================

/// CP5 — `scripts/ci/release-tag-coherence.sh` in validate mode
/// (`--from-version 0.101.15 HEAD`) reads the workspace Cargo.toml
/// from the given SHA without checking it out, parses
/// `[workspace.package].version`, and asserts the requested version
/// matches the workspace. This is the same check the tag-coherence
/// gate runs at release time, minus the identity check (which only
/// applies when a tag is named).
///
/// Exit code 0 + the `OK: validate-mode` line on stdout is the
/// observable signal that the workspace version is internally
/// consistent at HEAD.
#[test]
fn cp5_release_tag_coherence() {
    let script = repo_root().join("scripts/ci/release-tag-coherence.sh");
    assert!(
        script.is_file(),
        "CP5 release-tag-coherence.sh missing at {script:?}"
    );

    // Read the workspace version directly so we have an independent
    // expectation — the script could misparse and still exit 0 if its
    // failure mode were `silent agreement with a wrong value`.
    let cargo_toml_path = repo_root().join("Cargo.toml");
    let cargo_toml = std::fs::read_to_string(&cargo_toml_path)
        .unwrap_or_else(|e| panic!("cannot read {cargo_toml_path:?}: {e}"));
    let workspace_version: String = parse_workspace_version(&cargo_toml, &cargo_toml_path);

    let head_sha = {
        let out = Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .current_dir(repo_root())
            .output()
            .expect("git rev-parse HEAD");
        String::from_utf8(out.stdout)
            .expect("git rev-parse HEAD produced non-UTF-8")
            .trim()
            .to_string()
    };

    let out = Command::new("bash")
        .arg(&script)
        .arg("--from-version")
        .arg(&workspace_version)
        .arg(&head_sha)
        .current_dir(repo_root())
        .output()
        .unwrap_or_else(|e| panic!("failed to release spawn {script:?}: {e}"));

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "release-tag-coherence.sh --from-version {workspace_version} {head_sha} \
         must exit 0; got {:?}\nstdout:\n{stdout}\nstderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("OK: validate-mode"),
        "release-tag-coherence.sh exited 0 but did not emit the OK line; \
         stdout was:\n{stdout}"
    );

    // The cert entry must agree with the observed outcome.
    let cert = parse_cert();
    let cp5_01 = entries(&cert)
        .iter()
        .find(|e| entry_id(e) == "CP5-01")
        .unwrap_or_else(|| panic!("B6.W5 cert is missing CP5-01"));
    assert_eq!(
        entry_status(cp5_01),
        "PASS",
        "release-tag-coherence.sh --from-version {workspace_version} {head_sha} \
         exited 0; CP5-01 must be PASS"
    );
}

// ============================================================================
// Test 4 — no silent SKIP_NOT_APPLICABLE.
// ============================================================================

/// Every `SKIP_NOT_APPLICABLE` entry must carry a non-empty `reason`.
/// A `SKIP` without a reason is a silent skip: it looks like a
/// deliberate decision in the summary but cannot be reconstructed from
/// the cert alone. That is the failure mode this contract pins — the
/// PRF-DIST-05 clause that says support is only announced with real
/// UAT, and the corresponding `SKIP_NOT_APPLICABLE` carries the
/// human-readable justification that lets a reviewer verify the claim.
#[test]
fn no_silent_skips() {
    let cert = parse_cert();
    let mut silent: Vec<&str> = Vec::new();
    for entry in entries(&cert) {
        if entry_status(entry) == "SKIP_NOT_APPLICABLE" && !entry_reason_is_non_empty(entry) {
            silent.push(entry_id(entry));
        }
    }
    assert!(
        silent.is_empty(),
        "SKIP_NOT_APPLICABLE entries without a `reason`: {silent:?}. \
         PRF-DIST-05 requires every silent skip to be replaced by an \
         explicit, machine-readable justification in the cert file."
    );
}

// ============================================================================
// Test 5 — no unjustified NOT_RUN.
// ============================================================================

/// Every `NOT_RUN` entry must carry a non-empty `reason`. The
/// B6.W5 contract says: an item not run in this environment is
/// declared `NOT_RUN`, but the declaration itself must say *why* it
/// was not run (no release artefact, no signing key, no native
/// runner, etc.). A `NOT_RUN` without a reason is silent in the
/// other direction: it looks like a known gap but cannot be
/// distinguished from an oversight.
#[test]
fn no_unjustified_not_run() {
    let cert = parse_cert();
    let mut unjustified: Vec<&str> = Vec::new();
    for entry in entries(&cert) {
        if entry_status(entry) == "NOT_RUN" && !entry_reason_is_non_empty(entry) {
            unjustified.push(entry_id(entry));
        }
    }
    assert!(
        unjustified.is_empty(),
        "NOT_RUN entries without a `reason`: {unjustified:?}. \
         B6.W5 requires each NOT_RUN to declare the gap it represents \
         (missing artefact, missing key, missing native runner, etc.)."
    );
}

// ============================================================================
// Test 6 — cert file exists, is valid JSON, has an `entries` array.
// ============================================================================

/// Sanity pin: the cert file must exist, parse as JSON, and expose an
/// `entries` array. This is the gate the other tests stand on; if it
/// fails here, the other tests fail with a less actionable message.
#[test]
fn cert_file_exists_and_is_valid_json() {
    let path = cert_path();
    assert!(
        path.is_file(),
        "B6.W5 cert file missing at {path:?}; the cert is the deliverable \
         that B6.W5 exists to produce, so a missing file is the failure \
         this contract pins"
    );
    let text = read_cert_text();
    let parsed: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("B6.W5 cert at {path:?} is not valid JSON: {e}"));
    let arr = parsed["entries"].as_array().unwrap_or_else(|| {
        panic!("B6.W5 cert at {path:?} has no top-level `entries` array: {parsed:?}")
    });
    assert!(
        !arr.is_empty(),
        "B6.W5 cert at {path:?} has an empty `entries` array; \
         empty == silent == NOT_RUN-by-default is not acceptable here"
    );
    for entry in arr {
        assert!(
            entry.get("id").and_then(|v| v.as_str()).is_some(),
            "cert entry missing string `id`: {entry:?}"
        );
        assert!(
            entry.get("status").and_then(|v| v.as_str()).is_some(),
            "cert entry {:?} missing string `status`",
            entry.get("id")
        );
        let status = entry["status"].as_str().unwrap_or("");
        assert!(
            ["PASS", "NOT_RUN", "SKIP_NOT_APPLICABLE", "FAIL"].contains(&status),
            "cert entry {:?} has unknown status {status:?}; \
             only PASS, NOT_RUN, SKIP_NOT_APPLICABLE, FAIL are accepted",
            entry["id"]
        );
    }
}

// ============================================================================
// Test 7 — cert covers CP2, CP4, CP5.
// ============================================================================

/// The B6.W5 scope is exactly CP2, CP4, CP5 (CP6 web/landing is out of
/// scope per the task spec). Every topic must have at least one entry
/// in the cert; a missing topic is a silent omission that the gate
/// would otherwise let through.
#[test]
fn cert_covers_all_three_topics() {
    let cert = parse_cert();
    let mut topics: std::collections::BTreeSet<String> = Default::default();
    for entry in entries(&cert) {
        let t = entry_topic(entry);
        if !t.is_empty() {
            topics.insert(t.to_string());
        }
    }
    for required in ["2", "4", "5"] {
        assert!(
            topics.contains(required),
            "cert is missing CP{required} entries; topics present: {topics:?}. \
             B6.W5 scope requires CP2 (binary distribution), CP4 \
             (provenance attestation), and CP5 (release artifact \
             verification); CP6 web/landing is out of scope."
        );
    }
}

// ============================================================================
// Test 8 — CP5 cert's recorded workspace version matches reality.
// ============================================================================

/// The cert is honest about which SHA and which workspace version it
/// was generated against. If the cert's `cp5_workspace_version`
/// (free-form field at the top level) drifts away from what the
/// workspace Cargo.toml actually says, the cert is stale — and a
/// reviewer reading it months later would be told the workspace is at
/// a version it is not.
///
/// This test reads both and compares, so the only way for the cert
/// to claim a version is for the workspace to actually be at it.
#[test]
fn cp5_workspace_version_consistent() {
    let cert = parse_cert();
    let recorded = cert["cp5_workspace_version"]
        .as_str()
        .unwrap_or_else(|| {
            panic!(
                "B6.W5 cert has no top-level `cp5_workspace_version`; \
                 the cert must record the workspace version it was \
                 generated against, so the claim is checkable later"
            )
        });

    let cargo_toml_path = repo_root().join("Cargo.toml");
    let cargo_toml = std::fs::read_to_string(&cargo_toml_path)
        .unwrap_or_else(|e| panic!("cannot read {cargo_toml_path:?}: {e}"));
    let workspace_version: String = parse_workspace_version(&cargo_toml, &cargo_toml_path);

    assert_eq!(
        recorded, workspace_version,
        "cert claims cp5_workspace_version={recorded:?} but Cargo.toml says \
         [workspace.package].version={workspace_version:?}. The cert must be \
         regenerated in the same commit as any version bump, otherwise the \
         recorded evidence is stale and a reviewer cannot trust it."
    );

    // Bonus: the cert's recorded head_sha (if present) must be a
    // commit reachable in the repo's history. The cert is a
    // historical record of "what was true at this SHA", and once
    // committed, HEAD moves past it (e.g. the commit that added the
    // cert is one ahead of the SHA it records). What the contract
    // actually requires is that the SHA is real and points at a
    // commit the repo knows about — not that it equals the current
    // HEAD, which would force the cert to be re-issued on every
    // commit and would make it impossible to read after release.
    if let Some(stored_sha) = cert["head_sha"].as_str() {
        let out = Command::new("git")
            .arg("cat-file")
            .arg("-e")
            .arg(stored_sha)
            .current_dir(repo_root())
            .output()
            .unwrap_or_else(|e| panic!("failed to spawn git cat-file -e: {e}"));
        assert!(
            out.status.success(),
            "cert.head_sha={stored_sha:?} is not a reachable commit in this repo; \
             the cert records a non-existent object, which means it was \
             generated against an unrecorded state and cannot be reviewed"
        );
    }

    // Suppress the unused import when Path is not directly referenced
    // in this test; the path import is shared across the suite.
    let _ = Path::new(&cargo_toml_path);
}