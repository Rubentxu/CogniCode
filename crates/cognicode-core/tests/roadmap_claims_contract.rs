//! Roadmap-claims contract for `docs/roadmap/ROADMAP.md`.
//!
//! ## Why a gate on prose
//!
//! `ROADMAP.md` declares itself *"la única autoridad de agenda de desarrollo
//! para CogniCode en adelante"*. A document that people make decisions from
//! is a contract, and a contract that can silently become false is a trap.
//!
//! This is not hypothetical. On 2026-09-30 the PRODUCT-1.0 row stated that
//! PR #309 was still open, and used that to explain why work item
//! `8fec95db` could not be closed. PR #309 had in fact been merged at
//! `2026-09-30T10:55:17Z` (squash `70132ae6`). The ROADMAP commit that
//! reaffirmed the claim is timestamped `10:56:01 +0200` — 44 seconds
//! after the merge it denies.
//!
//! The identical failure had already been recorded once. N+65.5, earlier
//! the same day, corrected two aging assertions in this same file and wrote
//! down why: *"una fila de tabla escrita en pasado queda convertida en
//! presente, y `docs/roadmap/ROADMAP.md` es un documento que la gente
//! cita. Un diario puede contar lo que pasó; un roadmap afirma lo que es,
//! y por eso los dos tienen presupuesto distinto para la obsolescencia."*
//!
//! It happened again within nine hours. A document-level correction does
//! not hold, because nothing re-checks it.
//!
//! ## What this asserts, and what it deliberately does not
//!
//! It does not parse the whole file or validate its formatting. It asserts
//! one thing that is cheap to check and expensive to get wrong: **the file
//! must not assert that a pull request is open when that pull request is
//! merged.**
//!
//! The check is deliberately narrow. `gh` is not available inside a test
//! sandbox and a test must not make a network call, so the source of truth
//! is a pinned table of merge states, and what is asserted is that the
//! document does not contradict it. A PR absent from the table is not
//! constrained — an unknown PR is not a false claim, and inventing a rule
//! for it would be the same prose-as-verdict mistake this file is about.
//!
//! ## Calibration
//!
//! Every assertion was watched fail against a mutated copy of the document
//! before being accepted (see the mutations table in the cycle's
//! exploration report). A contract on a document that has never been seen
//! red is a description with `assert` in it.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/cognicode-core has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

fn read_roadmap() -> String {
    let path = repo_root().join("docs/roadmap/ROADMAP.md");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Pull requests whose merge state this contract knows, with the evidence
/// that established it.
///
/// Pinned deliberately. A test that queried GitHub would be a network call
/// in a unit test, which is the wrong place for one, and would make the
/// gate's result depend on connectivity rather than on the repository. The
/// table is the audit trail: each row records how the state was observed
/// and when, so a stale row is visible as a stale row rather than as a
/// silent pass.
const KNOWN_PR_STATES: &[(u32, &str, &str)] = &[
    // (number, "MERGED" | "OPEN", evidence)
    (
        309,
        "MERGED",
        "gh pr view 309 --json state,mergedAt -> MERGED 2026-09-30T10:55:17Z, squash 70132ae6",
    ),
    (
        310,
        "OPEN",
        "gh pr view 310 --json state -> OPEN, ci/pipelinek-kotlin-gate",
    ),
];

/// The ROADMAP must not claim a merged PR is still open.
///
/// The claim is matched as "PR #N" plus a nearby open-ness assertion, so a
/// document that merely mentions a merged PR number while describing it as
/// merged is not flagged. The window is bounded because these rows are
/// long: the PRODUCT-1.0 row is a single physical line several kilobytes
/// long, and an unbounded window would match across unrelated sentences.
#[test]
fn no_merged_pull_request_is_claimed_open_in_the_roadmap() {
    let roadmap = read_roadmap();

    for (number, state, evidence) in KNOWN_PR_STATES {
        if *state != "MERGED" {
            continue;
        }
        let needle = format!("PR #{number}");
        // Only the first occurrence is examined: the ROADMAP mentions a PR
        // number once per row, and scanning every occurrence would make a
        // single correct historical reference fail the gate.
        let Some(idx) = roadmap.find(&needle) else {
            continue;
        };
        // Bounded window around the reference.
        let start = idx.saturating_sub(120);
        let end = (idx + 260).min(roadmap.len());
        let window = &roadmap[start..end];

        let claims_open = ["sigue OPEN", "sigue abierto", "OPEN sin mergear"]
            .iter()
            .any(|claim| window.contains(claim));

        assert!(
            !claims_open,
            "ROADMAP.md refers to PR #{number} in a window that still claims it \
             is open, but the PR is MERGED.\n\
             Observed: {evidence}\n\
             A roadmap asserts what IS. When the evidence changes, the row that \
             asserts is what gets corrected — not the diary that recorded the \
             error. This is the same failure N+65.5 corrected and that recurred \
             within nine hours: the ROADMAP commit of 2026-09-30 10:56:01 \
             reaffirmed 'PR #309 sigue OPEN sin mergear' 44 seconds after the \
             merge at 10:55:17Z."
        );
    }
}

/// The corrected claim must keep its evidence, or it degrades back to prose.
///
/// "PR #309 was merged" is a claim. "PR #309 was merged, per
/// `gh pr view 309 --json state,mergedAt`, at this timestamp" is an auditable
/// one. The distinction is the whole point: a future reader can re-run the
/// command and find out whether the document is still true.
///
/// The assertion requires the *command*, not the word `MERGED`. Accepting
/// the state word was the first version of this test, and it was
/// tautological: the sentence asserting the merge contains `MERGED`, so
/// stripping the evidence left the check green. A check that can be
/// satisfied by the claim it is supposed to be auditing is not a check —
/// this is the N+53 tautology that N+60 also had to remove.
#[test]
fn the_roadmap_records_how_pr_states_were_observed() {
    let roadmap = read_roadmap();
    for (number, state, evidence) in KNOWN_PR_STATES {
        if *state != "MERGED" {
            continue;
        }
        let needle = format!("PR #{number}");
        let Some(idx) = roadmap.find(&needle) else {
            // If the ROADMAP stops mentioning this PR entirely there is
            // nothing to keep evidenced. Removing the claim is a legitimate
            // outcome; removing the evidence while keeping the claim is not.
            continue;
        };
        let start = idx.saturating_sub(200);
        let end = (idx + 700).min(roadmap.len());
        let window = &roadmap[start..end];

        assert!(
            window.contains("gh pr view"),
            "ROADMAP.md mentions PR #{number} but carries no command that \
             establishes its state. The corrected row must say how the state \
             was observed — `{evidence}` — or the correction decays back into \
             an assertion nobody can re-check. Naming the state word is not \
             enough: the sentence claiming the merge contains that word."
        );
    }
}

/// The file under audit must still be the one that calls itself the
/// authority.
///
/// If the header changes, every claim this contract makes about the
/// document's role is stale, and the next reader would be misled about what
/// authority it carries. N+58 established the precedent: pin the scope of a
/// scan, or it silently grows or shrinks.
#[test]
fn the_roadmap_still_declares_itself_the_agenda_authority() {
    let roadmap = read_roadmap();
    assert!(
        roadmap.contains("única autoridad de agenda de desarrollo"),
        "ROADMAP.md no longer declares itself the single agenda authority. This \
         contract asserts properties that follow from that role; if the role \
         changed, re-evaluate whether a prose gate is still the right \
         instrument rather than assuming it is."
    );
}
