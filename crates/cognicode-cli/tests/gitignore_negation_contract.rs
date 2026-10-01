//! PR-SEC / repo hygiene: a declared `!` negation must actually un-ignore its path.
//!
//! ## Why this test exists
//!
//! `.gitignore` declared `docs/`, which excludes the **directory**. Git does not
//! descend into an excluded directory, so every `!/docs/...` negation written
//! below it matched nothing. Measured 2026-10-01 with
//! `git check-ignore --no-index` on paths that do not exist:
//!
//! ```text
//! docs/ROADMAP.md                                    IGNORED
//! docs/adr/ADR-001-parked-crates.md                   IGNORED
//! docs/roadmap/production-ready/ROADMAP-ADDENDUM.md  IGNORED
//! docs/debts/probe.md                                IGNORED
//! ```
//!
//! `docs/ROADMAP.md` is the sharpest case: `.gitignore:171-172` declare it a
//! tracked PRF entrypoint, write the path and then immediately negate it, and
//! both lines are inert. The file asserted something and no mechanism applied —
//! the same shape as the N+66 ghost filter, the N+70 allowlist rationale and the
//! gate that no pipeline executed.
//!
//! ## The one-line fix is not enough
//!
//! Changing `docs/` to `docs/*` alone makes `!/docs/ROADMAP.md` work and leaves
//! `!/docs/roadmap/production-ready/` broken, because `docs/roadmap` is still
//! excluded and git never reaches the negation inside it. Every intermediate
//! level needs its own. That is asserted below rather than assumed.
//!
//! ## What is deliberately still ignored
//!
//! `.gitignore` has no `!/docs/adr/`. ADRs are first-class documentation that
//! the project relies on, and they are added with `git add -f`; the ones already
//! tracked predate the rule. That is declared policy, so this test holds it
//! rather than quietly changing it.
//!
//! ## The property that must not regress
//!
//! Making some paths trackable must not make everything trackable. If the
//! exclusions were relaxed by accident, the whole of `docs/` would start
//! staging itself into commits, so the untouched directories are asserted too.
//!
//! One of those is `docs/debts`, and it is the sharpest case. Fixing the
//! negations revealed four debt registers sitting untracked on disk
//! (DEBT-SDDK-002..005). Exempting the directory would have staged them by
//! default, and three of them carry the line "local only, NEVER pushed to
//! remote (ephemeral per AGENTS.md)". The obvious cleanup and the declared
//! policy point in opposite directions, so the policy wins here and the
//! question is registered rather than answered.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

/// Whether git's ignore machinery would skip this path. The path need not exist:
/// `--no-index` makes `check-ignore` read the patterns alone, so this is a fact
/// about the rules and not about what happens to be tracked today.
fn is_ignored(relative: &str) -> bool {
    let out = Command::new("git")
        .current_dir(repo_root())
        .args(["check-ignore", "-q", "--no-index", relative])
        .output()
        .unwrap_or_else(|e| panic!("cannot run `git check-ignore` (is git on PATH?): {e}"));
    match out.status.code() {
        Some(0) => true,
        Some(1) => false,
        other => panic!(
            "`git check-ignore -q --no-index {relative}` exited with {other:?}; \
             expected 0 (ignored) or 1 (not ignored). stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        ),
    }
}

/// Every directory `.gitignore` declares a negation for, with a probe path that
/// does not exist. These are the claims the file makes in prose.
const DECLARED_EXEMPT: &[&str] = &[
    "docs/ROADMAP.md",
    "docs/roadmap/production-ready/ROADMAP-ADDENDUM.md",
];

/// Directories under `docs/` with no negation. They must stay ignored: the local
/// -only decision of 2026-06-24 is the default, and the negations above are the
/// only exceptions to it.
const NOT_EXEMPT: &[&str] = &[
    "docs/analisis/probe.md",
    "docs/generated/probe.md",
    "docs/inventory/probe.md",
    "docs/evidence/probe.md",
    "docs/visual-deliverables/probe.md",
    "docs/distribution/probe.md",
    "docs/prf/probe.md",
    "docs/historico/probe.md",
    "docs/specs/probe.md",
    // Reachable only because `!/docs/roadmap/` had to be added for the
    // production-ready negations to match at all. Without the matching
    // `docs/roadmap/*` re-close, the whole directory opens up and ephemeral
    // files like docs/roadmap/INSTALL.md start staging themselves.
    "docs/roadmap/INSTALL.md",
    "docs/roadmap/probe.md",
    // docs/debts is here on purpose, and the reasoning is worth stating because
    // the opposite looks obviously right. `advisory_ignore_backing_contract`
    // reads DEBT-SEC-001 in CI, so exempting docs/debts/ looks like it should be
    // done. But it would also expose DEBT-SDDK-002..005, which exist on disk
    // and were never committed, and DEBT-SDDK-004/005/006 each carry the line
    // "local only, NEVER pushed to remote (ephemeral per AGENTS.md)". Widening
    // the exemption would quietly overturn the local-only rule of 2026-06-24
    // for four files whose authors said the opposite. That is a policy
    // decision, not a consequence of fixing the negations.
    "docs/debts/DEBT-SDDK-002.md",
];

#[test]
fn declared_negations_actually_unignore_their_paths() {
    for path in DECLARED_EXEMPT {
        assert!(
            !is_ignored(path),
            "`.gitignore` declares {path} exempt, but `git check-ignore` says it is \
             ignored. The negation is not reaching it — usually because an ancestor \
             directory is excluded outright. Exclude with `docs/*` rather than `docs/`, \
             and make sure every intermediate level has its own `!` rule."
        );
    }
}

#[test]
fn the_docs_exclusion_leaves_the_negations_reachable() {
    let text =
        std::fs::read_to_string(repo_root().join(".gitignore")).expect(".gitignore must exist");

    let excludes_directory = text
        .lines()
        .map(str::trim)
        .any(|l| l == "docs/" || l == "docs");
    assert!(
        !excludes_directory,
        "`.gitignore` excludes `docs/` as a directory. Git will not descend into an \
         excluded directory, so every `!/docs/...` below it is inert and the file \
         asserts exemptions it does not grant. Use `docs/*`, which excludes the same \
         paths but leaves `docs/` traversable."
    );

    assert!(
        text.lines().any(|l| l.trim() == "!/docs/roadmap/"),
        "`.gitignore` has no `!/docs/roadmap/`. Without it, `docs/roadmap` stays \
         excluded, git never descends into it, and the `!/docs/roadmap/production-ready/` \
         rules below match nothing — which is exactly what they did for months."
    );
}

#[test]
fn non_exempt_docs_directories_stay_ignored() {
    for path in NOT_EXEMPT {
        assert!(
            is_ignored(path),
            "{path} is not excluded any more. `docs/` is LOCAL ONLY per the user \
             decision of 2026-06-24; only the declared negations are exceptions. If \
             this exclusion is meant to be lifted, it is a policy change and belongs \
             in its own review, not as a side effect of fixing the negations."
        );
    }
}

#[test]
fn adrs_stay_force_added_by_declared_policy() {
    assert!(
        is_ignored("docs/adr/ADR-999-probe.md"),
        "docs/adr/ is no longer ignored. There is deliberately no `!/docs/adr/` \
         negation: the comment above the rule says ADRs are first-class and added \
         with `git add -f`, and the tracked ones predate the rule. If ADRs are meant \
         to be exempt by default now, that is a decision to state and test, not a \
         drift to arrive at."
    );
    assert!(
        is_ignored("docs/adr/"),
        "docs/adr/ is no longer excluded at all. It is the documented escape hatch \
         for the ADRs the project relies on."
    );
}
