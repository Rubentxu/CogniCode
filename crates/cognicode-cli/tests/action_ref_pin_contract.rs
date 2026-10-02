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
//! Every `.yml` in `.github/workflows/`, discovered from the directory. There
//! is deliberately **no list of files to check**: an earlier revision of this
//! test carried a hand-written `GUARDED_WORKFLOWS` array, which quietly fell
//! behind — `regression-check.yml` was never added to it, so that workflow's
//! refs were unguarded while the test's doc comment implied they were not. A
//! hand-maintained allow/deny list is the N+66 ghost-filter shape again, so
//! the list is gone and coverage is structural: a workflow added tomorrow is
//! covered tomorrow, with no edit here.
//!
//! `sandbox-nightly.yml` used to be excluded for a second reason: it carried
//! a `rootful/setup-podman@v4` ref to a repository deleted from GitHub
//! (HTTP 404, re-verified 2026-10-01), which cannot be pinned because it does
//! not exist. That step is now a `run:` step that exits 1 with the reason
//! attached (SDDK WorkItem 016a4f03, decision 402f2771) — pinning is
//! impossible against a missing repository, and substituting an unaudited
//! fork would trade a loud, verifiable failure for a silent dependency on a
//! third party. So there is nothing left to exclude, and no `uses:` anywhere
//! in this repository is now a tag, a branch, or a dead repository name.
//!
//! ## Parser limits, stated rather than hidden
//!
//! `third_party_refs` scans line by line; it does not parse YAML. It skips
//! fully-commented lines, but it cannot tell a `uses:` key from the same text
//! inside a `run: |` block scalar. That direction of error is deliberate: a
//! false positive makes this test fail and a human look, whereas a false
//! negative would let a mutable ref through unnoticed. It over-approximates
//! on purpose.

mod common;

use std::path::{Path, PathBuf};

/// The action this repository installs the toolchain with, pinned to the
/// head of `dtolnay/rust-toolchain`'s `1.96.0` branch.
const RUST_TOOLCHAIN_ACTION: &str = "dtolnay/rust-toolchain";
const RUST_TOOLCHAIN_PIN: &str = "ebb3d1676050bfd0971c36c1e215b5751473994d";
/// The version that pin corresponds to. Must track `rust-toolchain.toml`.
const RUST_TOOLCHAIN_VERSION: &str = "1.96.0";

/// Every workflow in `.github/workflows/`, sorted, discovered from disk.
fn workflow_files(root: &Path) -> Vec<PathBuf> {
    let dir = root.join(".github/workflows");
    let entries =
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
    let mut out: Vec<PathBuf> = entries
        .map(|e| e.expect("readable dir entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "yml"))
        .collect();
    out.sort();
    assert!(
        !out.is_empty(),
        "no .yml workflow found in {} — this guard would be vacuous",
        dir.display()
    );
    out
}

/// Extract every third-party `uses:` ref as `(line_number, ref)`.
///
/// Skips fully-commented lines: in YAML a `#` preceded only by whitespace
/// starts a comment, and GitHub never executes it. Local actions (`./path`)
/// and `docker://` refs are not third-party and carry nothing to pin.
fn third_party_refs(workflow: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (idx, line) in workflow.lines().enumerate() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        let Some(pos) = line.find("uses:") else {
            continue;
        };
        // A trailing `# comment` documents the ref; it is not part of it.
        let value = line[pos + "uses:".len()..]
            .split('#')
            .next()
            .unwrap_or("")
            .trim();
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
    rev.len() == 40
        && rev
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[test]
fn every_action_ref_in_every_workflow_is_sha_pinned() {
    let root = common::repo_root();
    let mut checked = 0usize;
    let mut offenders: Vec<String> = Vec::new();

    for path in workflow_files(&root) {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for (line_no, reference) in third_party_refs(&content) {
            checked += 1;
            if !is_sha_pinned(&reference) {
                offenders.push(format!(
                    ".github/workflows/{name}:{line_no}: {reference} is not pinned to a \
                     40-hex SHA. A tag or branch ref is mutable: it can be moved under us \
                     without any change to this repository, and CI would execute whatever \
                     it points at after the move. A ref to a repository that no longer exists \
                     (HTTP 404) cannot be pinned either — replace it with an audited action or \
                     with a run step that fails loudly, do not point at a dead or unverified name."
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
        "no action refs were found in any workflow — the guard would be vacuous if the \
         workflows were emptied"
    );
}

#[test]
fn commented_and_non_third_party_refs_are_not_counted() {
    // This parser rule was not hypothetical: while removing the dead
    // `rootful/setup-podman@v4` ref, the explanatory comment written above
    // the replacement step quoted the old ref verbatim — `uses:
    // rootful/setup-podman@v4` — and the previous line scanner reported it as
    // a live, unpinned third-party ref and failed the suite on it.
    let yaml = r#"
jobs:
  demo:
    steps:
      # 2026-10-01 — this step used to be `uses: rootful/setup-podman@v4`.
      #   uses: some/action@v1
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # pinned
      - uses: ./local-action
      - uses: docker://alpine:3.19
      - uses: dtolnay/rust-toolchain@1.96.0
"#;
    let refs = third_party_refs(yaml);
    let values: Vec<&str> = refs.iter().map(|(_, r)| r.as_str()).collect();

    assert_eq!(
        values,
        vec![
            "actions/checkout@11d5960a326750d5838078e36cf38b85af677262",
            "dtolnay/rust-toolchain@1.96.0",
        ],
        "only the two live third-party refs should be collected; comments, local actions \
         and docker:// refs are not refs to pin. Got: {values:?}"
    );
    assert_eq!(
        refs[0].0, 7,
        "line numbers must stay 1-based for the failure message"
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

    let mut seen = 0usize;
    let mut offenders: Vec<String> = Vec::new();

    for path in workflow_files(&root) {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
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

    // 20 as of 2026-10-02: the 20th is the `supply-chain` job added to put
    // cargo-deny on the merge path (PR-SEC), pinned to the same verified
    // 1.96.0 commit as the other 19. The count is a tripwire, not a budget:
    // adding or removing a ref means saying so here, so that the diff shows
    // the surface changed rather than growing quietly.
    assert_eq!(
        seen, 20,
        "expected 20 {RUST_TOOLCHAIN_ACTION} refs across all workflows, found {seen}. \
         A ref was added or removed without updating this test."
    );
    assert!(
        offenders.is_empty(),
        "toolchain action drifted:\n  - {}",
        offenders.join("\n  - ")
    );
}
