//! Third-party action pinning contract (audit QW-05, PR-SEC).
//!
//! ## What this prevents
//!
//! Every `uses:` in a workflow is executable code pulled in at run time by
//! the runner, at a ref the workflow author chooses. A tag or a branch is a
//! mutable name: the upstream owner can move it, and every job that has
//! already merged will pick up the new content on its next run. `checkout`
//! and `rust-toolchain` run before the project's own build, so this is
//! supply-chain execution, not a dependency to bump at leisure.
//!
//! SHA pinning makes the coupling auditable: the exact bytes that ran are
//! recoverable from the workflow file itself.
//!
//! ## Why this file exists, given the pins were already there
//!
//! Almost every `uses:` in this repository was already SHA-pinned, and the
//! audit that produced QW-05 counted only what it found in the *release*
//! workflows: "7 pinned, 0 mutable" was a true statement about
//! `release.yml` and `release-validate.yml` and a false statement about the
//! repository. Twelve more mutable refs were live in `ci.yml` (8) and
//! `pr-ci.yml` (4) — including the four in the workflow that gates every
//! merge.
//!
//! The twelve carried this comment:
//!
//! ```text
//! - uses: dtolnay/rust-toolchain@1.96.0  # pinned 2026-09-26 by CR-08-debt-verify
//! ```
//!
//! Read it plainly: it asserts the ref is pinned, and the ref is a branch.
//! The comment is not a pin, and it is worse than no comment, because a
//! reader scanning for `pinned` concludes the work is done. That is why the
//! second assertion below exists: it makes "claims to be pinned" and "is
//! pinned" the same claim, so the two can no longer disagree.
//!
//! ## Scope
//!
//! Repository-wide, not release-only. A security property that is measured
//! over a subset is reported as `Partial`, never as done, and the subset has
//! to be a deliberate, stated narrowing. `1.96.0` is genuinely a branch of
//! `dtolnay/rust-toolchain` (its head is the `toolchain: 1.96.0` commit,
//! `ebb3d1676050bfd0971c36c1e215b5751473994d`, signed upstream), so the
//! workflows were not merely at the mercy of a tag moving — they were
//! re-resolving that branch on every run.
//!
//! `rust-toolchain.toml` is a separate matter and is deliberately out of
//! scope: it pins the *toolchain version* the local toolchain selects, not
//! the bytes of the action that installs it. The old comment used it to
//! justify a mutable ref, which is the reasoning error this contract removes.

use std::path::PathBuf;

/// `uses:` refs allowed to be something other than a full commit SHA.
///
/// Empty by design. A local action (`./path`) and a template expression
/// (`${{ matrix.x }}`) cannot be SHA-pinned, and both are legitimate. If one
/// is ever needed it gets added here with a reason in the commit — not
/// quietly permitted by a wildcard in the parser.
const NON_SHA_ALLOWED: &[(&str, &str)] = &[];

/// Workflow files that existed when this contract was calibrated.
///
/// Also the anti-vacuity floor: if the glob ever stops matching, or the
/// directory is read from the wrong place, every other assertion in this
/// file would happily pass over an empty set.
const BASELINE_WORKFLOWS: usize = 6;
const BASELINE_USES: usize = 72;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/cognicode-core has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

fn workflow_paths() -> Vec<PathBuf> {
    let dir = repo_root().join(".github/workflows");
    let entries =
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|s| s.to_str()),
                Some("yml") | Some("yaml")
            )
        })
        .collect();
    paths.sort();
    paths
}

#[derive(Debug)]
struct Use {
    workflow: String,
    line_no: usize,
    /// The whole source line, so failures can quote it verbatim.
    source: String,
    /// The part after `uses:`, with any trailing `# comment` removed.
    value: String,
    /// The trailing `# comment`, if any.
    comment: Option<String>,
}

impl Use {
    fn is_sha_pinned(&self) -> bool {
        match self.value.split_once('@') {
            Some((_, reference)) => {
                reference.len() == 40 && reference.chars().all(|c| c.is_ascii_hexdigit())
            }
            // A bare `uses: ./local-action` has no `@` at all and is a
            // local path, governed by NON_SHA_ALLOWED rather than by this.
            None => false,
        }
    }

    fn is_explicitly_allowed(&self) -> bool {
        NON_SHA_ALLOWED.iter().any(|(name, _)| self.value == *name)
    }
}

fn parse_uses(workflow: &str, text: &str) -> Vec<Use> {
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let Some(rest) = line.split_once("uses:").map(|(_, r)| r) else {
            continue;
        };
        // Strip a trailing comment, but only when it is one: a `#` inside a
        // quoted string is part of the value. No workflow here quotes the
        // value, and a ref cannot contain `#`, so the simple split is the
        // honest one — and if someone starts quoting values this test says
        // so instead of silently skipping their refs.
        let (value, comment) = match rest.split_once('#') {
            Some((v, c)) => (v.trim().to_string(), Some(c.trim().to_string())),
            None => (rest.trim().to_string(), None),
        };
        if value.is_empty() {
            continue;
        }
        out.push(Use {
            workflow: workflow.to_string(),
            line_no: idx + 1,
            source: line.trim_end().to_string(),
            value,
            comment,
        });
    }
    out
}

fn all_uses() -> Vec<Use> {
    let mut out = Vec::new();
    for path in workflow_paths() {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .expect("workflow file name is UTF-8")
            .to_string();
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        out.extend(parse_uses(&name, &text));
    }
    out
}

/// The anti-vacuity guard. Runs first and, on failure, says why the rest of
/// the file cannot be trusted.
#[test]
fn the_workflow_surface_is_still_what_was_measured() {
    let workflows = workflow_paths();
    assert_eq!(
        workflows.len(),
        BASELINE_WORKFLOWS,
        "expected {BASELINE_WORKFLOWS} workflow files under .github/workflows, found {}. \
         If a workflow was added or removed, re-measure and update \
         BASELINE_WORKFLOWS and BASELINE_USES in the same commit — otherwise \
         every assertion in this file can pass over a surface it never read.",
        workflows.len()
    );

    let uses = all_uses();
    assert!(
        uses.len() >= BASELINE_USES,
        "expected at least {BASELINE_USES} `uses:` refs across the workflows, found {}. \
         A drop means the parser stopped seeing refs it used to see (quoting, \
         new line shape) — which would make `every_action_ref_is_sha_pinned` \
         pass without having looked at anything.",
        uses.len()
    );
}

/// The primary invariant.
#[test]
fn every_action_ref_is_sha_pinned() {
    let offenders: Vec<String> = all_uses()
        .iter()
        .filter(|u| !u.is_sha_pinned() && !u.is_explicitly_allowed())
        .map(|u| format!("{}:{}  {}", u.workflow, u.line_no, u.source))
        .collect();

    assert!(
        offenders.is_empty(),
        "{} mutable action ref(s) in .github/workflows — a tag or a branch can \
         be moved upstream and every job that already merged picks up the new \
         bytes on its next run:\n{}\n\nPin each to the full 40-hex commit SHA, \
         recording the tag or branch it came from in a trailing comment. If a \
         ref genuinely cannot be pinned (a local action, a template \
         expression), add it to NON_SHA_ALLOWED with a reason.",
        offenders.len(),
        offenders.join("\n")
    );
}

/// The invariant that failed the audit: a comment is not a pin.
///
/// A line may not describe its own ref as pinned, fixed, or locked unless
/// the ref actually is. This is the assertion that would have caught the
/// twelve `dtolnay/rust-toolchain@1.96.0  # pinned 2026-09-26 by
/// CR-08-debt-verify` lines, and it keeps catching the next person who fixes
/// a number in a comment and moves on.
#[test]
fn no_line_claims_to_be_pinned_while_its_ref_is_mutable() {
    let mut offenders = Vec::new();
    for u in all_uses() {
        let Some(comment) = &u.comment else { continue };
        let lowered = comment.to_lowercase();
        let claims_pinned = ["pinned", "pinea", "fijado", "locked", "sha-pin"]
            .iter()
            .any(|needle| lowered.contains(needle));
        if claims_pinned && !u.is_sha_pinned() && !u.is_explicitly_allowed() {
            offenders.push(format!("{}:{}  {}", u.workflow, u.line_no, u.source));
        }
    }

    assert!(
        offenders.is_empty(),
        "{} line(s) describe a mutable ref as pinned:\n{}\n\nA comment asserting \
         the work is done is worse than no comment: a reader scanning for \
         `pinned` will not look at the ref. Either pin the ref, or describe it \
         accurately.",
        offenders.len(),
        offenders.join("\n")
    );
}

/// `dtolnay/rust-toolchain` is the only action this repository pins across
/// every workflow, so it is the one most likely to drift back. Pinning it in
/// a single place and asserting that place is the cheap-loss check.
#[test]
fn rust_toolchain_pins_are_identical_across_workflows() {
    let mut seen: Vec<String> = Vec::new();
    for u in all_uses() {
        if !u.value.starts_with("dtolnay/rust-toolchain@") {
            continue;
        }
        let reference = u
            .value
            .split_once('@')
            .map(|(_, r)| r.to_string())
            .expect("dtolnay ref carries an @");
        if !seen.contains(&reference) {
            seen.push(reference);
        }
    }

    assert!(
        !seen.is_empty(),
        "no `dtolnay/rust-toolchain` ref found in any workflow. Either the \
         workflows stopped installing the toolchain through the action, or this \
         contract is reading the wrong directory. Re-measure before deleting \
         the assertion."
    );
    assert_eq!(
        seen.len(),
        1,
        "dtolnay/rust-toolchain is referenced at {} distinct refs: {seen:?}. \
         A per-workflow SHA is a version split waiting to happen: the next \
         toolchain bump fixes one workflow and silently leaves the other four \
         on an older toolchain.",
        seen.len()
    );
}
