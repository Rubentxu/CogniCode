//! e80a — WU0 characterization, flipped.
//!
//! WU0 (commit `test(e80a): characterize the automated-author authority gap`)
//! added four tests that ASSERTED THE HOLE: an LLM/plugin author with a clean
//! dry-run could mint a permit with no approval. Those tests passed on the
//! pre-e80a code by design.
//!
//! WU1–WU5 closed the hole. These are the same four scenarios, now asserting
//! the invariant e80a establishes. See `characterization.md` for the full
//! seam map and `authority_tests.rs` for the exhaustive WU7 suite.

use crate::application::change_proposal::proposal::RequestedBy;
use crate::application::promotion_authority::authorization::{
    PromotionAuthorizationError, PromotionAuthorizationPolicy, VerifiedExternalApproval,
};
use crate::application::promotion_authority::authority_test_support::{
    clean_dry_run, human, llm_agent, plugin, proposal, proposal_id,
};
use crate::application::promotion_authority::permit::{
    PromotionApplyOutcome, PromotionPermitId, apply_with_permit, issue_promotion_permit,
};
use crate::domain::execution::actor::ActorRef;

/// FLIPPED. WU0 asserted this succeeded. Now the automated author is refused
/// the authorization, so no permit can be minted.
#[test]
fn characterization_llmagent_author_is_refused_without_approval() {
    let p = proposal("p-1", llm_agent());
    let trial = crate::application::promotion_authority::authority_test_support::passing_trial_for(&p);
    let run = clean_dry_run("p-1", &trial);

    let err = PromotionAuthorizationPolicy::authorize(&p, run, None)
        .expect_err("automated author with no approval must not be authorised");
    assert!(matches!(
        err,
        PromotionAuthorizationError::AutomatedAuthorRequiresExternalApproval { .. }
    ));
}

/// FLIPPED. Same for a plugin author.
#[test]
fn characterization_plugin_author_is_refused_without_approval() {
    let p = proposal("p-1", plugin());
    let trial = crate::application::promotion_authority::authority_test_support::passing_trial_for(&p);
    let run = clean_dry_run("p-1", &trial);

    let err = PromotionAuthorizationPolicy::authorize(&p, run, None)
        .expect_err("automated author with no approval must not be authorised");
    assert!(matches!(
        err,
        PromotionAuthorizationError::AutomatedAuthorRequiresExternalApproval { .. }
    ));
}

/// PRESERVED. The human-authored path is unchanged: a clean dry-run still
/// promotes, with no new co-approval requirement.
#[test]
fn characterization_human_author_path_is_preserved() {
    let p = proposal("p-1", human());
    let trial = crate::application::promotion_authority::authority_test_support::passing_trial_for(&p);
    let run = clean_dry_run("p-1", &trial);

    let authorization = PromotionAuthorizationPolicy::authorize(&p, run, None)
        .expect("human author with a clean dry-run must keep promoting");
    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), authorization);
    let current = crate::application::promotion_authority::authority_test_support::world("w-C", 10);
    assert!(matches!(
        apply_with_permit(&current, &permit),
        PromotionApplyOutcome::Applied { .. }
    ));
}

/// UNCHANGED, and now the reason the sealing matters: an `ActorRef` is
/// caller-constructible data. It is still trivial to write, but it can no
/// longer be converted into authority — the conversion requires a verifier.
#[test]
fn characterization_actorref_human_is_still_only_a_claim() {
    let forged = ActorRef::human("alice");
    assert_eq!(forged.kind, crate::domain::execution::actor::ActorKind::Human);

    // The claim does not produce authority. The only path from a claim to
    // `VerifiedExternalApproval` runs through a verifier.
    let p = proposal("p-1", llm_agent());
    let trial = crate::application::promotion_authority::authority_test_support::passing_trial_for(&p);
    let run = clean_dry_run("p-1", &trial);
    let authorization = PromotionAuthorizationPolicy::authorize(&p, run, None);
    assert!(
        authorization.is_err(),
        "a forged ActorRef cannot substitute for a verified approval"
    );

    // `VerifiedExternalApproval` is not constructible here; naming it keeps the
    // import honest about what the test is about.
    let _: Option<VerifiedExternalApproval> = None;
    let _ = proposal_id("p-1");
    let _ = RequestedBy::Human {
        user_ref: "alice".to_string(),
    };
}
