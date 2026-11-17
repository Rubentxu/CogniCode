//! The review contract run over the in-memory forge, and over the
//! broken forges that make the review approve what it should not. The
//! use case is the same in every run; only the forge changes.

use super::clause;
use super::run_review_contract;
use crate::domain::forge::ForgeCapabilities;
use crate::domain::forge::conformance_tests::{BOTH, Flaw, FlawedHarness};
use crate::domain::forge::fake::FakeForgeHarness;

const NEITHER: ForgeCapabilities = ForgeCapabilities::NONE;

/// Each flaw, the clause of the review contract it must break and the
/// capabilities the forge declares while it misbehaves.
const CONTROLS: &[(Flaw, &str, ForgeCapabilities)] = &[
    (Flaw::FrozenHead, clause::PR_DIFF_STALE_HEAD, BOTH),
    (Flaw::FrozenHead, clause::MERGED_STALE_HEAD, BOTH),
    (
        Flaw::PublishesOnBaseTip,
        clause::PR_DIFF_PUBLISHED_AT_HEAD,
        BOTH,
    ),
    (
        Flaw::PublishesOnBaseTip,
        clause::MERGED_PUBLISHED_AT_HEAD,
        BOTH,
    ),
    (Flaw::PublishesOnBaseTip, clause::UNSUPPORTED, NEITHER),
    (Flaw::MissingBaseAsFound, clause::UNRELATED_HISTORIES, BOTH),
    (Flaw::PendingAsReady, clause::MERGED_PENDING, BOTH),
    (Flaw::ConflictHidden, clause::MERGED_CONFLICT, BOTH),
    (Flaw::WrongCandidate, clause::MERGED_PUBLISHED_AT_HEAD, BOTH),
];

#[test]
fn the_review_meets_its_contract_over_the_in_memory_forge() {
    for capabilities in [
        BOTH,
        NEITHER,
        ForgeCapabilities {
            merge_base: true,
            merge_candidate: false,
        },
        ForgeCapabilities {
            merge_base: false,
            merge_candidate: true,
        },
    ] {
        run_review_contract(&FakeForgeHarness::new(capabilities)).assert_conforms(&format!(
            "the review over FakeForge declaring {capabilities:?}"
        ));
    }
}

#[test]
fn the_review_contract_rejects_every_forge_that_makes_the_review_approve_wrongly() {
    for (flaw, expected, capabilities) in CONTROLS {
        let report = run_review_contract(&FlawedHarness {
            capabilities: *capabilities,
            flaw: *flaw,
        });
        report.assert_detects(&format!("{flaw:?}"), expected);
    }
}

#[test]
fn every_clause_of_the_review_contract_has_a_negative_control() {
    let reportable = [
        clause::PR_DIFF_PUBLISHED_AT_HEAD,
        clause::PR_DIFF_STALE_HEAD,
        clause::UNRELATED_HISTORIES,
        clause::MERGED_PUBLISHED_AT_HEAD,
        clause::MERGED_STALE_HEAD,
        clause::MERGED_CONFLICT,
        clause::MERGED_PENDING,
        clause::UNSUPPORTED,
    ];
    let uncontrolled: Vec<&str> = reportable
        .into_iter()
        .filter(|reportable| !CONTROLS.iter().any(|(_, clause, _)| clause == reportable))
        .collect();
    assert!(
        uncontrolled.is_empty(),
        "no negative control for {uncontrolled:?}"
    );
}
