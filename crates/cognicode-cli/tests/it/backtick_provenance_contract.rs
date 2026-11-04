//! PR-SEC / docs hygiene: a provenance path cited in backticks must exist.
//!
//! ## Why this test exists
//!
//! `sddk lint` extracts markdown links, so it only sees `[text](path)`. A
//! reference written as inline code — `` `openspec/changes/e40-…/spec.md` `` —
//! is invisible to it. Receipt N+73 verified 87 relative links in the ADR tree
//! and found none broken; receipt N+74 then swept for backticked paths and found
//! 27 more dead references in the very tree that had just reported clean. A
//! third sweep, in N+75, found four more.
//!
//! Three sweeps in one session, each finding more, because nothing covered this
//! case. `87/87` was a floor, not a count: a number of verifications, not a
//! number of references.
//!
//! ## Why it is scoped to the decision record
//!
//! The naive sweep is not a gate, it is noise. Measured over the decision
//! record and the entrypoint that pointed at it:
//!
//! | set | broken |
//! |---|---|
//! | every backticked path | **238** |
//! | of those, starting with `docs/` or `openspec/` | **9** |
//!
//! The other 229 are `tools/list`, `references/`, `domain/`, `src/x.rs`,
//! `tests/integration.rs`, `tmp/.cognicode/versions/0.92.0/…`: partial paths
//! ending in `/`, skill names, code identifiers and template examples. A
//! "does this exist" check over all backticks would produce 229 false positives
//! per sweep, and gating that is worse than not gating it — the same decision
//! taken for `sddk lint` in receipt N+73.4.
//!
//! The paths that carry provenance are the ones rooted at `docs/` and
//! `openspec/`, and those are exactly the ones a broken reference makes worse:
//! an ADR that cites a decision record which is not there.
//!
//! ## What is deliberately allowed to stay missing
//!
//! Every entry below is a measured case, not a sweep to make the test pass.
//! Three of them are **GIVEN-clause fixtures** whose meaning depends on the
//! path *not* existing; creating the target would invert the test.
//!
//! | path | where | why it stays missing |
//! |---|---|---|
//! | `openspec/` | the decision record | the specification tree was removed with the documentation tree. The decision cites the specification it implemented; rewriting a decision to hide that provenance would falsify it. |
//! | `docs/prf/` | the decision record | the closed programme's dossier was removed with the documentation tree. The decision cites the artefacts that programme produced; the programme is closed and its dossier is gone. |
//! | `docs/parked-crates/` | `ADR-001-parked-crates.md` | the parked crates were removed with the documentation tree. The decision that parked them cites where it parked them. |
//! | `docs/RELEASE-1.0.0-PLAN.md` | `ADR-031`, `ADR-032` | the release plan was removed with the documentation tree. The decisions that define the release cite the plan they were based on. |
//! | `docs/analysis/release-1.0.0-scorecard.md` | `ADR-031-release-1.0.0-definition.md` | prospective: *"se archiva en … al publicar"*. The file is created at release time. |
//! | `docs/adr/E32-cognicode-distribution.md` | `ADR-034` | E32 is a ROADMAP *program* with sub-units E32-A..I, not an ADR, so this path never named a decision record. Choosing `ADR-034` or the ROADMAP E32 section would be invention. |
//!
//! A row whose path ends in `/` is a root: any citation under it is allowed
//! too. That is how a removed tree is declared — once, with the reason it
//! went — instead of one row per file that used to live in it.
//!
//! Adding a row here is a claim that the path should not exist. The claim is in
//! the diff, and `every_allowed_missing_states_a_reason` fails if the reason is
//! blank or if nothing cites the row any more.
//!
//! ## What `merge-gate` caught in this contract's first CI run
//!
//! The first version of this file shipped with the governed *set* read through
//! `git ls-files` but each citation resolved against `Path::exists()`. Those are
//! two different questions, and only the first one is the same everywhere.
//!
//! `merge-gate` failed on the first run in CI on
//! `docs/CogniCode_Living_Software_Intelligence/RETIREMENT-LEDGER.md`: present
//! in the maintainer's working tree, absent from the repository, because `docs/`
//! is gitignored and only nested paths under that package are force-added. The
//! local run was green for a reason the runner could not reproduce.
//!
//! So a gate can be deterministic about *what it reads* and still be
//! environment-dependent about *what it concludes*. Resolution now asks what is
//! committed, and `resolution_asks_what_is_committed_not_what_is_on_disk` pins
//! it against the exact path that failed.
//!
//! The flip side is that this file can no longer be validated by "it passes on
//! my machine" — which was never evidence, and here was actively wrong.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Directories whose backticked paths are provenance references.
const ROOTS: &[&str] = &["docs/", "openspec/"];

/// The trees this contract governs. A citation inside one of them has to
/// resolve: these are the documents a reader follows to find out what the
/// product does and how it is put together.
const GOVERNED_TREES: &[&str] = &["docs/adr/", "docs/roadmap/consumer-contracts/"];

/// Length in bytes of the backtick run starting at `i`.
fn backtick_run(bytes: &[u8], i: usize) -> usize {
    let mut n = 0;
    while i + n < bytes.len() && bytes[i + n] == b'`' {
        n += 1;
    }
    n
}

/// Extract the backticked paths in `text` that are rooted at `ROOTS`.
///
/// The scan is deliberately narrow. Everything else in a backtick — partial
/// paths, skill names, code identifiers — is not a provenance reference, and
/// treating it as one is what produces 229 false positives per sweep.
///
/// ## Why this walks the bytes instead of pairing delimiters
///
/// The obvious implementation is `text.split('`').skip(1).step_by(2)`, and it
/// fails silently. A markdown code fence is **three** backticks, so a document
/// that embeds one has an odd total and every position after it is off by one.
/// Measured on a specification document before the tree was pruned: 373 backticks, and
/// the pairing extractor returned **zero** paths for the whole file — including
/// the three broken ones this contract exists to catch. A scanner that returns
/// an empty set passes the gate below without checking anything, which is the
/// exact failure mode of the N+66 ghost filter.
///
/// So a span is delimited by runs of exactly one backtick on each side, found
/// by walking forward from an opening run. That reads `` `docs/adr/0001.md` `` as
/// a citation and skips the bare `` ```json `` of a fence, whatever the rest of
/// the file does — parity is never consulted.
fn provenance_paths(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut out = BTreeSet::new();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] != b'`' || backtick_run(bytes, i) != 1 {
            i += if bytes[i] == b'`' {
                backtick_run(bytes, i)
            } else {
                1
            };
            continue;
        }
        let start = i + 1;
        match closing_backtick(bytes, start) {
            Some(end) => {
                consider(&mut out, &text[start..end]);
                i = end + 1;
            }
            None => {
                // Unterminated span: skip the opening tick and keep going.
                i = start;
            }
        }
    }
    out
}

/// Index of the single backtick that closes a span opened just before `start`,
/// or `None` when the span runs past the end of the line without closing.
fn closing_backtick(bytes: &[u8], start: usize) -> Option<usize> {
    let mut k = start;
    while k < bytes.len() {
        match bytes[k] {
            b'\n' => return None,
            b'`' if backtick_run(bytes, k) == 1 => return Some(k),
            _ => k += 1,
        }
    }
    None
}

/// Apply the provenance filter to one span body, or ignore it.
fn consider(out: &mut BTreeSet<String>, body: &str) {
    let candidate = body.trim();
    if !ROOTS.iter().any(|r| candidate.starts_with(r)) {
        return;
    }
    // A line range, a glob or a written-out ellipsis is not a file citation.
    if candidate.contains(':') || candidate.contains('*') || candidate.contains('…') {
        return;
    }
    for expansion in expand_braces(candidate) {
        if expansion
            .split('/')
            .all(|s| s.is_empty() || s == "." || s == "..")
        {
            continue;
        }
        out.insert(expansion);
    }
}

/// Ceiling on brace expansion, so a pathological span cannot explode the scan.
const MAX_EXPANSIONS: usize = 16;

/// Expand `{a,b}` into its alternatives.
///
/// A citation like `` `specs/{alpha, beta}/spec.md` `` is one reference to two
/// files, not one reference to a file whose name contains braces. Treating it as a literal path would put a permanent,
/// unexplainable failure in the gate; skipping braces entirely would let a real
/// broken citation hide behind the notation.
fn expand_braces(candidate: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut frontier = vec![candidate.to_string()];

    while let Some(current) = frontier.pop() {
        if out.len() >= MAX_EXPANSIONS {
            break;
        }
        let Some(open) = current.find('{') else {
            out.push(current);
            continue;
        };
        let Some(close) = current[open..].find('}').map(|i| i + open) else {
            out.push(current);
            continue;
        };
        let prefix = &current[..open];
        let suffix = &current[close + 1..];
        let inner = &current[open + 1..close];

        for alt in inner.split(',') {
            let piece = format!("{prefix}{}{suffix}", alt.trim());
            // Nested braces are not expanded; the literal is kept so the finding
            // stays visible rather than being silently reshaped.
            if piece.contains('{') {
                out.push(piece);
            } else {
                frontier.push(piece);
            }
        }
    }
    out
}

/// Paths that are allowed not to exist, with the reason each one stays that way.
///
/// A row whose path ends in `/` is a root: any citation under it is allowed
/// too. That is how a removed tree is declared — once, with the reason it
/// went — instead of one row per file that used to live in it.
///
/// See the table in the module docs. A blank reason fails the contract.
const ALLOWED_MISSING: &[(&str, &str)] = &[
    (
        "openspec/",
        "the specification tree was removed with the documentation tree. The decision record cites the specification each decision implemented; rewriting a decision to hide that provenance would falsify it, and the citation states what was implemented rather than a path a reader can follow today",
    ),
    (
        "docs/prf/",
        "the closed programme's dossier was removed with the documentation tree. The decision record cites the artefacts that programme produced; the programme is closed and its dossier is gone, so the citation states what was decided rather than a path a reader can follow today",
    ),
    (
        "docs/roadmap/MAINTENANCE.md",
        "the maintenance backlog closed its 13 units and was retired with the documentation tree. The decision record cites where the correction was recorded; the receipt of that correction remains in the Journal, and a backlog whose every row is closed is history, not a working document",
    ),
    (
        "docs/parked-crates/",
        "the parked crates were removed with the documentation tree. The decision that parked them cites where it parked them; the crates are gone and the decision that made them is not",
    ),
    (
        "docs/RELEASE-1.0.0-PLAN.md",
        "the release plan was removed with the documentation tree. The decisions that define the release cite the plan they were based on; the plan is gone and the decisions are not",
    ),
    (
        "docs/analysis/release-1.0.0-scorecard.md",
        "cited prospectively in ADR-031: the scorecard is archived here at release time, so it is absent before the release",
    ),
    (
        "docs/adr/E32-cognicode-distribution.md",
        "E32 is a ROADMAP program with sub-units E32-A..I, not an ADR; this path never named a decision record and picking a substitute would be invention",
    ),
    (
        "docs/cognicode-kotlin-roadmap/",
        "the upstream Kotlin package lives outside version control by operator decision, so the consumer contract cites where the package came from rather than a path a reader can follow today; the integrated copies are under docs/roadmap/consumer-contracts/kotlin-evolution/ and those resolve",
    ),
    (
        "docs/roadmap/kotlin-evolution/",
        "the upstream package deposits its documents at this gitignored path; the consumer contract's integration table names this path to record where each file came from, and the integrated copies live under docs/roadmap/consumer-contracts/kotlin-evolution/ where they resolve",
    ),
    (
        "docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md",
        "the consumer contract records that this path was cited by the roadmap while it did not exist, and states that the integration deliberately does not create the file to turn a broken reference green; the citation names a known absence rather than a document a reader can follow today",
    ),
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

/// The trees whose tracked files this contract governs: the decision record,
/// the operating contract, and the consumer contract that tells a consumer
/// what the product does.
///
/// Read through `git ls-files` so an untracked scratch file cannot fail the
/// build.
///
/// The consumer contract is governed for the same reason the decision record
/// is: it is the entrance door for whoever arrives, so its citations are the
/// first thing they will follow. It was left out of this set, and the
/// consequence was nineteen citations that resolve to nothing — nobody
/// noticed because the guard was never looking there.
fn governed_files(root: &Path) -> Vec<PathBuf> {
    let tracked = tracked_paths(root);

    let mut files: Vec<PathBuf> = tracked
        .iter()
        .filter(|p| GOVERNED_TREES.iter().any(|prefix| p.starts_with(prefix)))
        .map(PathBuf::from)
        .collect();

    files.sort();
    files
}

/// Every path a clean checkout of this repository would contain.
fn tracked_paths(root: &Path) -> BTreeSet<String> {
    let listed = Command::new("git")
        .current_dir(root)
        .args(["ls-files", "-z"])
        .output()
        .expect("cannot run `git ls-files` (is git on PATH?)");
    assert!(
        listed.status.success(),
        "`git ls-files` failed: {}",
        String::from_utf8_lossy(&listed.stderr)
    );

    listed
        .stdout
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

/// Whether `path` resolves for someone who checked the repository out.
///
/// **Tracked, not present on disk.** `docs/` is gitignored and 598 files under it
/// are force-added selectively, so a file can sit in one maintainer's working
/// directory and not exist in a clean checkout at all. Asking the filesystem
/// therefore answers a different question depending on who runs the test: the
/// same commit passes on a laptop and fails on the runner that is supposed to
/// enforce it.
///
/// This is not hypothetical. Receipt N+76 shipped exactly that gate: it read the
/// governed set through `git ls-files` but resolved each citation against the
/// filesystem, and `merge-gate` failed on the first run in CI on
/// `docs/CogniCode_Living_Software_Intelligence/RETIREMENT-LEDGER.md` — present
/// in the working tree, absent from the repository. The set being scanned was
/// deterministic; the verdict was not.
fn resolves_in_a_checkout(tracked: &BTreeSet<String>, path: &str) -> bool {
    let path = path.trim_end_matches('/');
    if tracked.contains(path) {
        return true;
    }
    // A citation may name a directory rather than a file.
    let prefix = format!("{path}/");
    tracked.iter().any(|p| p.starts_with(&prefix))
}

/// Every broken provenance path, as `(path, citing file)`.
fn broken_provenance_paths(root: &Path) -> Vec<(String, String)> {
    let tracked = tracked_paths(root);
    let mut broken = Vec::new();
    for rel in governed_files(root) {
        let text = match std::fs::read_to_string(root.join(&rel)) {
            Ok(t) => t,
            Err(e) => panic!("cannot read governed file {}: {e}", rel.display()),
        };
        for candidate in provenance_paths(&text) {
            if resolves_in_a_checkout(&tracked, &candidate) {
                continue;
            }
            broken.push((candidate, rel.display().to_string()));
        }
    }
    broken.sort();
    broken.dedup();
    broken
}

fn is_allowed(path: &str) -> bool {
    ALLOWED_MISSING.iter().any(|(declared, _)| {
        *declared == path || (declared.ends_with('/') && path.starts_with(declared))
    })
}

#[test]
fn every_backticked_provenance_path_resolves() {
    let root = repo_root();
    let broken = broken_provenance_paths(&root);
    let undeclared: Vec<_> = broken
        .iter()
        .filter(|(path, _)| !is_allowed(path))
        .collect();

    assert!(
        undeclared.is_empty(),
        "these provenance paths are cited in backticks but do not exist, and are not \
         declared in ALLOWED_MISSING:\n{}\n\nA backticked path is invisible to `sddk lint`, \
         which extracts only markdown links, so nothing else catches this. Either point the \
         citation at a real target or add a row to ALLOWED_MISSING stating why the path should \
         not exist — a blank reason fails the contract.",
        undeclared
            .iter()
            .map(|(p, f)| format!("  {p}  <-  {f}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn a_broken_citation_in_the_governed_tree_is_reported() {
    // The negative control: a citation that does not resolve has to be
    // reported, from inside the tree the contract governs.
    //
    // The gate above proves the real tree is clean. This proves the opposite
    // direction: that "clean" is a fact about the tree and not about a
    // scanner that stopped looking. Without it, a guard whose filter
    // matched nothing would pass forever, and the failure would look
    // exactly like a healthy repository.
    let root = repo_root();
    let tracked = tracked_paths(&root);
    let governed = governed_files(&root);

    assert!(
        !governed.is_empty(),
        "the governed tree is empty, so there is nothing to scan"
    );

    // A citation to a document that does not exist, seen by the same code
    // that reads the real tree.
    let synthetic = "Source: `docs/roadmap/consumer-contracts/no-existe.md`.\n";
    let found = provenance_paths(synthetic);
    assert!(
        found.contains("docs/roadmap/consumer-contracts/no-existe.md"),
        "the scanner missed a broken citation inside the governed tree; it found {found:?}"
    );

    // And the same citation, judged by the same resolution, does not resolve.
    let broken: Vec<_> = found
        .iter()
        .filter(|p| !resolves_in_a_checkout(&tracked, p))
        .collect();
    assert!(
        !broken.is_empty(),
        "a citation to a document that does not exist was reported as resolving; \
         the resolution is not asking what is actually there"
    );
}

#[test]
fn the_scan_actually_finds_a_broken_reference() {
    // Non-vacuity. A scanner that returns an empty set passes the gate above
    // without checking anything, which is the failure mode this contract exists
    // to avoid — the same shape as the N+66 ghost filter and the N+70 allowlist
    // rationale.
    let root = repo_root();
    let governed = governed_files(&root);
    assert!(
        governed.len() > 20,
        "only {} governed files found; the scan is not covering the tree it claims to cover",
        governed.len()
    );

    // Non-vacuity over the *real* tree, not only over the fixture below.
    //
    // The fixture proves the scanner can find a path when handed one. It says
    // nothing about whether the scan of the governed tree finds any at all --
    // and an empty result makes `every_backticked_provenance_path_resolves`
    // pass with nothing checked. That is the fence-parity defect this contract
    // already shipped once, and it would look identical from here.
    //
    // Measured: 26 of the governed files contribute a path, 44 distinct ones
    // once deduplicated across files. (A quick script counted 78 because it
    // summed per-file sets and double-counted citations that appear in several
    // ADRs; this accumulates into one set, so 44 is the real figure.) The floors
    // sit well under both so that legitimately pruning a citation does not fail
    // the build, while still being unreachable by a scanner that stopped
    // finding anything.
    let root = repo_root();
    let mut contributors = 0usize;
    let mut found_paths = BTreeSet::new();
    for rel in governed_files(&root) {
        let Ok(text) = std::fs::read_to_string(root.join(&rel)) else {
            continue;
        };
        let hits = provenance_paths(&text);
        if !hits.is_empty() {
            contributors += 1;
        }
        found_paths.extend(hits);
    }
    assert!(
        found_paths.len() >= 25 && contributors >= 15,
        "the scan of the real governed tree found only {} provenance paths across {} files; \
         a scanner that has stopped finding citations makes the gate pass with nothing \
         checked. If citations were legitimately removed, lower these floors deliberately.",
        found_paths.len(),
        contributors
    );

    let fixture = "Source: `openspec/changes/does-not-exist/proposal.md` and \
                   `openspec/specs/real/spec.md`.\n";
    let found = provenance_paths(fixture);
    assert!(
        found.contains("openspec/changes/does-not-exist/proposal.md"),
        "the scanner missed a well-formed broken backticked path; it found {found:?}"
    );
    assert!(
        !found.iter().any(|p| p == "src/x.rs"),
        "the scanner picked up a code identifier as a provenance path"
    );
    assert!(
        !found.iter().any(|p| p.contains("…") || p.contains('*')),
        "the scanner picked up a placeholder or glob as a provenance path"
    );
}

#[test]
fn a_code_fence_does_not_shift_every_later_span() {
    // The defect this pins. A markdown fence is three backticks, so the file has
    // an odd total and a `split('`').skip(1).step_by(2)` pairing is off by one for
    // the rest of the document. Measured on
    // `openspec/specs/docs-source-adapter/spec.md`, that extractor returned zero
    // paths for the whole file.
    //
    // The failure is silent and it is the dangerous kind: the gate above sees an
    // empty set and passes.
    let fixture = "3. Parse Markdown to extract: headings, links, code fences \
                   ```json\n{}\n```\n\nSource: `openspec/specs/does-not-exist/spec.md`\n";
    let found = provenance_paths(fixture);
    assert_eq!(
        found.iter().cloned().collect::<Vec<_>>(),
        vec!["openspec/specs/does-not-exist/spec.md".to_string()],
        "an odd number of backticks shifted the scan; found {found:?}"
    );

    // A doubled backtick is a fence edge or escaped tick, never a span edge.
    let doubled = "``openspec/specs/not-a-citation/spec.md`` \
                   and `openspec/specs/real/spec.md`";
    let found = provenance_paths(doubled);
    assert_eq!(
        found.iter().cloned().collect::<Vec<_>>(),
        vec!["openspec/specs/real/spec.md".to_string()],
        "a doubled backtick was treated as a span delimiter; found {found:?}"
    );
}

#[test]
fn brace_notation_is_expanded_not_taken_literally() {
    // `docs/ROADMAP.md` cites `openspec/specs/{cognicode-cli, cognicode-ide-adapter}/spec.md`:
    // one reference to two files. Both targets exist, so a scanner that expands
    // braces passes, and a scanner that takes the braces literally fails forever
    // on a citation that is not broken.
    let found =
        provenance_paths("en `openspec/specs/{cognicode-cli, cognicode-ide-adapter}/spec.md`");
    assert_eq!(
        found.iter().cloned().collect::<Vec<_>>(),
        vec![
            "openspec/specs/cognicode-cli/spec.md".to_string(),
            "openspec/specs/cognicode-ide-adapter/spec.md".to_string(),
        ],
        "brace notation was not expanded into its alternatives; found {found:?}"
    );

    // The expansion must reach a genuinely broken target, or it only ever proves
    // the happy case.
    let broken = provenance_paths("en `openspec/specs/{nope-one, nope-two}/spec.md`");
    assert_eq!(
        broken.len(),
        2,
        "expansion did not surface both broken alternatives; found {broken:?}"
    );
}

#[test]
fn resolution_asks_what_is_committed_not_what_is_on_disk() {
    // The defect receipt N+76 shipped and `merge-gate` caught on its first run in
    // CI. The governed *set* was read through `git ls-files`, but each citation
    // was resolved against the filesystem, so the verdict depended on whose
    // checkout the gate ran in: green on a laptop that holds local-only files,
    // red on the runner meant to enforce it.
    //
    // The first version of this test asserted that the failing path was *present
    // in the working tree*. `merge-gate` failed that too, with "the fixture is
    // no longer present in this working tree" — which is the point, stated
    // backwards: the file is absent from a checkout, so a test cannot require it.
    //
    // So the properties below are asserted against a synthetic tracked set.
    // Resolution must be a function of that set alone; nothing on disk may enter
    // into it. Every assertion here is therefore identical on a laptop and on a
    // runner.
    let mut tracked = BTreeSet::new();
    tracked.insert("docs/adr/ADR-001-example.md".to_string());
    tracked.insert("openspec/specs/demo/docs/nested.md".to_string());

    // A committed path resolves.
    assert!(resolves_in_a_checkout(
        &tracked,
        "docs/adr/ADR-001-example.md"
    ));
    // Nothing else does. `docs/roadmap/JOURNAL.md` is present in every checkout
    // of this repository, so it cannot be used as the counter-example; the point
    // is that the function has no channel through which the disk could answer.
    assert!(
        !resolves_in_a_checkout(&tracked, "docs/roadmap/JOURNAL.md"),
        "a path absent from the tracked set resolved"
    );
    assert!(
        !resolves_in_a_checkout(&tracked, "docs/adr/ADR-999-absent.md"),
        "an invented path resolved"
    );
    // A citation may name a directory rather than a file, with or without a
    // trailing slash.
    assert!(resolves_in_a_checkout(&tracked, "openspec/specs/demo/"));
    assert!(resolves_in_a_checkout(&tracked, "openspec/specs/demo/docs"));
    assert!(!resolves_in_a_checkout(&tracked, "openspec/specs/other/"));

    // Grounded in the real repository by the one fact that is stable everywhere:
    // the ledger that broke the first CI run is not committed. Whether it is
    // sitting in someone's working tree is precisely the thing that varies.
    let ledger = "docs/CogniCode_Living_Software_Intelligence/RETIREMENT-LEDGER.md";
    let real = tracked_paths(&repo_root());
    assert!(
        !real.contains(ledger),
        "the ledger is now committed, so it no longer distinguishes the two questions \
         and this test no longer describes the defect it exists for"
    );
    assert!(
        !resolves_in_a_checkout(&real, ledger),
        "a path absent from the repository resolved as if it existed for a reader with \
         a checkout; this is the N+76 defect"
    );
}

#[test]
fn the_scan_covers_exactly_what_ci_can_see() {
    // The governed set is read through `git ls-files` on purpose. `docs/adr/` is
    // git-ignored (`.gitignore:182`) and ADRs are force-added selectively, so on
    // this machine there are ADRs on disk that are **not** in git: 35 tracked
    // against 53 present.
    //
    // That is the right basis for a CI gate rather than a compromise. A CI
    // runner gets a checkout, so a file that is not tracked does not exist there.
    // A check that scanned the working directory would therefore be non-vacuous
    // on a developer machine and vacuous in the run that is supposed to enforce
    // it — the ghost-filter shape this work item is about.
    //
    // What is tracked is what CI can enforce; what sits untracked is local
    // reference material, and `git check-ignore` is how to see which is which.
    let root = repo_root();
    let governed = governed_files(&root);
    let on_disk = root.join("docs/adr");
    let tracked_here = governed
        .iter()
        .filter(|p| p.starts_with("docs/adr/"))
        .count();

    assert!(
        tracked_here > 0,
        "no ADR is tracked, so the gate would pass on an empty set"
    );
    // Safe to assert on disk here, unlike in `resolution_asks_...`: the
    // directory holds tracked files, so a checkout has it too. (A `PathBuf` is
    // never empty, so this used to assert nothing.)
    assert!(
        on_disk.is_dir(),
        "docs/adr/ does not exist even though {} ADRs are tracked; the scan is pointed \
         at the wrong tree",
        tracked_here
    );

    // Every tree this contract claims to govern has to be governed. Without
    // this, dropping a tree from the filter is indistinguishable from a tree
    // with nothing to say: the gate stays green and the tree stops being
    // checked.
    for tree in GOVERNED_TREES {
        let count = governed.iter().filter(|p| p.starts_with(tree)).count();
        assert!(
            count > 0,
            "no tracked file is governed under {tree}, so that tree is not being scanned"
        );
    }
}

#[test]
fn every_allowed_missing_states_a_reason() {
    // Each row is a claim that the path should not exist. A blank or trivial
    // reason turns the allowlist back into the thing this work item set out to
    // remove: rows that make a test pass without saying why.
    for (path, reason) in ALLOWED_MISSING {
        assert!(
            reason.trim().len() >= 30,
            "ALLOWED_MISSING row `{path}` has no real reason ({:?}). Every allowed-missing \
             path must say what it is and why it should stay missing.",
            reason
        );
    }

    // Every allowed path must still be cited somewhere, or the row is stale and
    // the allowlist has started accumulating entries nobody needs.
    let root = repo_root();
    let tracked = tracked_paths(&root);
    let cited: BTreeSet<String> = governed_files(&root)
        .iter()
        .flat_map(|rel| {
            std::fs::read_to_string(root.join(rel))
                .map(|t| provenance_paths(&t))
                .unwrap_or_default()
        })
        .collect();
    for (path, _) in ALLOWED_MISSING {
        // A root row is alive while something under it is still cited.
        let still_cited = if path.ends_with('/') {
            cited.iter().any(|c| c.starts_with(path))
        } else {
            cited.contains(*path)
        };
        assert!(
            still_cited,
            "ALLOWED_MISSING row `{path}` is stale: nothing in the governed tree cites it. \
             Remove the row rather than leaving an allowlist entry that no longer applies."
        );

        // The other direction, and the one this contract was missing until a
        // mutation exposed it: a row must also still be *needed*. If the path
        // now resolves, the allowlist is excusing a citation that is in fact
        // fine, and nothing else would ever notice -- the row is still cited,
        // so the staleness check above passes.
        //
        // Proven by adding a row pointing at a path that exists and is cited:
        // all seven tests still passed.
        assert!(
            !resolves_in_a_checkout(&tracked, path),
            "ALLOWED_MISSING row `{path}` is obsolete: that path now resolves in a checkout, \
             so it no longer needs to be excused. Delete the row."
        );
    }
}
