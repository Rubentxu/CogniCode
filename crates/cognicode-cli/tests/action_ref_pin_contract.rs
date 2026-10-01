//! PR-SEC: every third-party action ref is pinned to an immutable SHA.
//!
//! ## Why this test exists
//!
//! On 2026-10-01 an audit of `.github/workflows/*.yml` found 74 `uses:`
//! refs, 53 pinned by SHA and **21** not. Nineteen of those were
//! `dtolnay/rust-toolchain@1.96.0` — and every one of them carried the
//! comment *"pinned 2026-09-26 by CR-08-debt-verify"*, which was false.
//! A version tag is not a SHA pin.
//!
//! GitHub's API settles what `@1.96.0` actually is: `dtolnay/rust-toolchain`
//! has exactly one tag (`v1`). `1.96.0` is a **branch**, and the branch is
//! `protected: false` with `required_status_checks` off. So the active
//! PR-CI path resolved 19 refs through a mutable pointer while the file
//! claimed otherwise — the same defect class as the N+66 ghost filter and
//! the N+70 allowlist rationale: a written guarantee no mechanism applies.
//!
//! ## What it pins to
//!
//! `ebb3d1676050bfd0971c36c1e215b5751473994d` is the head of that branch
//! as of 2026-10-01, carrying a valid upstream PGP signature from the
//! maintainer (`verified: true`, commit message "toolchain: 1.96.0"). It is
//! the same commit the branch already pointed at, so pinning changes
//! nothing about what CI executes — only that the pointer stops being
//! movable. The signature is what makes this a verification rather than a
//! guess.
//!
//! ## Scope
//!
//! Only workflows that run on GitHub-hosted runners are guarded.
//! `sandbox-nightly.yml` is excluded on purpose: its `schedule` trigger is
//! disabled and the file is retained as documentation (E31-B5). Its
//! `rootful/setup-podman@v4` ref points at a repository that now returns
//! 404, and is tracked as a separate follow-up.

mod common;

use std::path::Path;

/// Workflows that actually execute on hosted runners.
const GUARDED_WORKFLOWS: &[&str] =
    &["ci.yml", "pr-ci.yml", "release-validate.yml", "release.yml"];

/// The action this repository installs the toolchain with, pinned to the
/// head of `dtolnay/rust-toolchain`'s `1.96.0` branch.
const RUST_TOOLCHAIN_ACTION: &str = "dtolnay/rust-toolchain";
const RUST_TOOLCHAIN_PIN: &str = "ebb3d1676050bfd0971c36c1e215b5751473994d";
/// The version that pin corresponds to. Must track `rust-toolchain.toml`.
const RUST_TOOLCHAIN_VERSION: &str = "1.96.0";

/// Extract every third-party `uses:` ref as `(line_number, ref)`.
///
/// Local actions (`./path`) are not third-party and carry no ref to pin.
fn third_party_refs(workflow: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (idx, line) in workflow.lines().enumerate() {
        let Some(pos) = line.find("uses:") else {
            continue;
        };
        // A trailing `# comment` documents the ref; it is not part of it.
        let value = line[pos + "uses:".len()..].split('#').next().unwrap_or("").trim();
        if value.is_empty() || value.starts_with("./") || value.starts_with("docker://") {
            continue;
        }
        out.push((idx + 1, value.to_string()));
    }
    out
}

fn is_sha_pinned(reference: &str) -> bool {
    let Some((_, rev)) = reference.rsplit_once('@') else {
        return false;
    };
    rev.len() == 40 && rev.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[test]
fn every_action_ref_in_active_workflows_is_sha_pinned() {
    let root = common::repo_root();
    let dir = root.join(".github/workflows");
    let mut checked = 0usize;
    let mut offenders: Vec<String> = Vec::new();

    for name in GUARDED_WORKFLOWS {
        let path = dir.join(name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for (line_no, reference) in third_party_refs(&content) {
            checked += 1;
            if !is_sha_pinned(&reference) {
                offenders.push(format!(
                    ".github/workflows/{name}:{line_no}: {reference} is not pinned to a \
                     40-hex SHA. A tag or branch ref is mutable: it can be moved under us \
                     without any change to this repository, and CI would execute whatever \
                     it points at after the move."
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "{} of {checked} action refs are mutable:\n  - {}",
        offenders.len(),
        offenders.join("\n  - "),
    );
    assert!(
        checked > 0,
        "no action refs were found in the guarded workflows — the guard would be \
         vacuous if the workflows were renamed or emptied"
    );
}

#[test]
fn rust_toolchain_action_matches_the_repository_toolchain() {
    let root = common::repo_root();

    // The action ref and the repo's own toolchain must agree. The old
    // comments asserted this in prose ("rust-toolchain.toml pins the same
    // version") with nothing checking it; here it is a fact or the test
    // fails. Bump rust-toolchain.toml -> this test tells you to re-pin.
    let manifest = std::fs::read_to_string(root.join("rust-toolchain.toml"))
        .expect("rust-toolchain.toml must exist");
    assert!(
        manifest.contains(&format!("channel = \"{RUST_TOOLCHAIN_VERSION}\"")),
        "rust-toolchain.toml no longer pins {RUST_TOOLCHAIN_VERSION}, but this test still \
         expects the action to be pinned to its {RUST_TOOLCHAIN_VERSION} commit. Re-resolve \
         the new version's branch head, re-pin every ref, and update \
         RUST_TOOLCHAIN_VERSION/RUST_TOOLCHAIN_PIN in this file together with \
         rust-toolchain.toml."
    );

    let dir = root.join(".github/workflows");
    let mut seen = 0usize;
    let mut offenders: Vec<String> = Vec::new();

    for name in GUARDED_WORKFLOWS {
        let path = dir.join(name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for (line_no, reference) in third_party_refs(&content) {
            let Some((action, _)) = reference.rsplit_once('@') else {
                continue;
            };
            if action != RUST_TOOLCHAIN_ACTION {
                continue;
            }
            seen += 1;
            if !reference.ends_with(RUST_TOOLCHAIN_PIN) {
                offenders.push(format!(
                    ".github/workflows/{name}:{line_no}: {action} is pinned to {reference}, \
                     expected the verified {RUST_TOOLCHAIN_VERSION} commit {RUST_TOOLCHAIN_PIN}"
                ));
            }
        }
    }

    assert_eq!(
        seen, 19,
        "expected 19 {RUST_TOOLCHAIN_ACTION} refs across the guarded workflows, found {seen}. \
         A ref was added or removed without updating this test."
    );
    assert!(
        offenders.is_empty(),
        "toolchain action drifted:\n  - {}",
        offenders.join("\n  - ")
    );
}
