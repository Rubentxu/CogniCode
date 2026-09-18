//! e80a WU7 — adversarial authority tests.
//!
//! The invariant under test:
//!
//! ```text
//! automated author
//! + perfect trial
//! + PolicyGate::Pass
//! + CleanPromotionReady
//! + forged "human" ActorRef
//! + NO trusted external verification
//! = NO PromotionPermit
//! ```
//!
//! Cases A–L mirror the cycle directive. A few are compile-time or source-level
//! properties (J, K, L); those are audited against the source text, which is
//! the strongest executable check available without adding a compile-fail
//! harness.

use crate::application::change_proposal::proposal::RequestedBy;
use crate::application::promotion_authority::authority_test_support::{
    clean_dry_run, drifted_dry_run, dry_run_for_candidate, human, llm_agent, passing_trial_for,
    plugin, proposal, world,
};
use crate::application::promotion_authority::authorization::{
    ExternalApprovalAuthority, ExternalApprovalError, ExternalApprovalRequest,
    ExternalApprovalVerifier, PromotionApprovalTarget, PromotionAuthorizationError,
    PromotionAuthorizationPolicy, RejectAllExternalApprovals, VerifiedExternalApproval,
};
use crate::application::promotion_authority::permit::{
    PromotionApplyError, PromotionApplyOutcome, PromotionPermitId, apply_with_permit,
    issue_promotion_permit,
};
use crate::domain::execution::actor::ActorRef;
use std::path::{Path, PathBuf};

/// A deterministic verifier that approves exactly one `(approver, target)`.
struct ApproveExact {
    approver: String,
    target: PromotionApprovalTarget,
}

impl ExternalApprovalVerifier for ApproveExact {
    fn verify(&self, request: &ExternalApprovalRequest) -> bool {
        request.claimed_approver.id == self.approver && request.target == self.target
    }
}

fn target_of(
    run: &crate::application::promotion_authority::evaluation::PromotionDryRun,
) -> PromotionApprovalTarget {
    PromotionApprovalTarget::from_dry_run(run)
}

fn approved(
    approver: &str,
    run: &crate::application::promotion_authority::evaluation::PromotionDryRun,
) -> VerifiedExternalApproval {
    let target = target_of(run);
    ExternalApprovalAuthority::verify(
        &ApproveExact {
            approver: approver.to_string(),
            target,
        },
        ExternalApprovalRequest::new(target_of(run), ActorRef::human(approver)),
    )
    .expect("fixture verifier approves the exact target")
}

// --- A / B: automated author, no approval ---------------------------

#[test]
fn a_llmagent_with_clean_dry_run_and_no_approval_is_refused() {
    let p = proposal("p-1", llm_agent());
    let trial = passing_trial_for(&p);
    let run = clean_dry_run("p-1", &trial);
    let err = PromotionAuthorizationPolicy::authorize(&p, run, None).unwrap_err();
    assert!(matches!(
        err,
        PromotionAuthorizationError::AutomatedAuthorRequiresExternalApproval { .. }
    ));
}

#[test]
fn b_plugin_with_clean_dry_run_and_no_approval_is_refused() {
    let p = proposal("p-1", plugin());
    let trial = passing_trial_for(&p);
    let run = clean_dry_run("p-1", &trial);
    let err = PromotionAuthorizationPolicy::authorize(&p, run, None).unwrap_err();
    assert!(matches!(
        err,
        PromotionAuthorizationError::AutomatedAuthorRequiresExternalApproval { .. }
    ));
}

// --- C: forged human claim, verifier rejects ------------------------

#[test]
fn c_forged_human_actorref_with_rejecting_verifier_yields_no_approval() {
    let p = proposal("p-1", llm_agent());
    let trial = passing_trial_for(&p);
    let run = clean_dry_run("p-1", &trial);
    let target = target_of(&run);

    // The caller can name a human...
    let request = ExternalApprovalRequest::new(target, ActorRef::human("alice"));
    assert_eq!(
        request.claimed_approver.kind,
        crate::domain::execution::actor::ActorKind::Human
    );

    // ...but the trusted verifier refuses, so no approval is minted.
    let err = ExternalApprovalAuthority::verify(&RejectAllExternalApprovals, request).unwrap_err();
    assert_eq!(err, ExternalApprovalError::NotVerified);

    // Without an approval, the automated author is refused.
    assert!(PromotionAuthorizationPolicy::authorize(&p, run, None).is_err());
}

// --- D: approval for proposal X reused for Y ------------------------

#[test]
fn d_approval_for_one_proposal_cannot_authorise_another() {
    let p1 = proposal("p-1", llm_agent());
    let trial1 = passing_trial_for(&p1);
    let run1 = clean_dry_run("p-1", &trial1);
    let approval_p1 = approved("alice", &run1);

    let p2 = proposal("p-2", llm_agent());
    let trial2 = passing_trial_for(&p2);
    let run2 = clean_dry_run("p-2", &trial2);

    let err = PromotionAuthorizationPolicy::authorize(&p2, run2, Some(approval_p1)).unwrap_err();
    assert!(matches!(
        err,
        PromotionAuthorizationError::ApprovalTargetMismatch { .. }
    ));
}

// --- E: approval for candidate B1 reused for B2 ---------------------

#[test]
fn e_approval_for_one_candidate_cannot_authorise_another() {
    let p = proposal("p-1", llm_agent());
    let trial = passing_trial_for(&p);
    let run_b1 = dry_run_for_candidate("p-1", &world("w-B1", 10), &trial);
    let approval_b1 = approved("alice", &run_b1);

    let run_b2 = dry_run_for_candidate("p-1", &world("w-B2", 10), &trial);
    let err = PromotionAuthorizationPolicy::authorize(&p, run_b2, Some(approval_b1)).unwrap_err();
    assert!(matches!(
        err,
        PromotionAuthorizationError::ApprovalTargetMismatch { .. }
    ));
}

// --- F: approval from a stale current snapshot ----------------------

#[test]
fn f_stale_current_snapshot_is_refused() {
    let p = proposal("p-1", llm_agent());
    let trial = passing_trial_for(&p);

    // Approval was obtained against the clean attempt (current snapshot 10).
    let clean = clean_dry_run("p-1", &trial);
    let stale_approval = approved("alice", &clean);

    // The world has since moved (current snapshot 12). The dry-run is no longer
    // clean, so it is refused before the approval is even consulted.
    let drifted = drifted_dry_run("p-1", &trial);
    let err =
        PromotionAuthorizationPolicy::authorize(&p, drifted, Some(stale_approval)).unwrap_err();
    assert!(
        matches!(err, PromotionAuthorizationError::DryRunNotClean { .. }),
        "a stale world must fail closed at the cleanliness gate, got {err:?}"
    );
}

// --- G: approval + later world drift preserves fail-closed apply ----

#[test]
fn g_world_drift_after_permit_is_preserved() {
    let p = proposal("p-1", llm_agent());
    let trial = passing_trial_for(&p);
    let run = clean_dry_run("p-1", &trial);
    let approval = approved("alice", &run);
    let authorization =
        PromotionAuthorizationPolicy::authorize(&p, run, Some(approval)).expect("authorised");
    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), authorization);

    let drifted = world("w-C", 12);
    assert_eq!(
        apply_with_permit(&drifted, &permit),
        PromotionApplyOutcome::Rejected(PromotionApplyError::WorldDriftedSincePermit {
            permit_snapshot: crate::domain::evidence_kernel::ids::SnapshotId::new(10),
            current_snapshot: crate::domain::evidence_kernel::ids::SnapshotId::new(12),
        })
    );
}

// --- H: human path preserved ----------------------------------------

#[test]
fn h_human_author_promotion_is_preserved() {
    let p = proposal("p-1", human());
    let trial = passing_trial_for(&p);
    let run = clean_dry_run("p-1", &trial);
    let authorization =
        PromotionAuthorizationPolicy::authorize(&p, run, None).expect("human path preserved");
    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), authorization);
    assert!(matches!(
        apply_with_permit(&world("w-C", 10), &permit),
        PromotionApplyOutcome::Applied { .. }
    ));
}

// --- I: legitimate verified approval authorises ---------------------

#[test]
fn i_automated_author_with_verified_approval_can_be_authorised() {
    let p = proposal("p-1", llm_agent());
    let trial = passing_trial_for(&p);
    let run = clean_dry_run("p-1", &trial);
    let approval = approved("alice", &run);

    let authorization = PromotionAuthorizationPolicy::authorize(&p, run, Some(approval))
        .expect("verified external approval must authorise an automated author");
    assert!(matches!(
        authorization.author(),
        RequestedBy::LlmAgent { .. }
    ));
    assert_eq!(
        authorization
            .external_approval()
            .map(|a| a.approver().id.as_str()),
        Some("alice")
    );

    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), authorization);
    assert_eq!(
        permit.external_approver().map(|a| a.id.as_str()),
        Some("alice")
    );
    assert!(matches!(
        apply_with_permit(&world("w-C", 10), &permit),
        PromotionApplyOutcome::Applied { .. }
    ));
}

// --- perfect evidence is NOT authority ------------------------------

#[test]
fn perfect_evidence_is_not_authority() {
    // A passing trial + a clean evaluation is the strongest *technical* signal
    // there is, and it still confers no authority on an automated author.
    let p = proposal("p-1", plugin());
    let trial = passing_trial_for(&p);
    assert_eq!(
        trial.gate.outcome,
        crate::application::policy_gate::PolicyOutcome::Pass
    );
    let run = clean_dry_run("p-1", &trial);

    assert!(PromotionAuthorizationPolicy::authorize(&p, run, None).is_err());
}

// --- J / K / L: source-level authority audits -----------------------

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn read_src(rel: &str) -> String {
    std::fs::read_to_string(src_root().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// Collect every `#[derive(...)]` argument list in `src`.
fn derive_args(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(i) = rest.find("#[derive(") {
        let after = &rest[i + "#[derive(".len()..];
        match after.find(")]") {
            Some(j) => {
                out.push(after[..j].to_string());
                rest = &after[j..];
            }
            None => break,
        }
    }
    out
}

#[test]
fn j_sealed_authority_types_are_not_serializable() {
    let src = read_src("application/promotion_authority/authorization.rs");
    for args in derive_args(&src) {
        assert!(
            !args.contains("Serialize") && !args.contains("Deserialize"),
            "sealed authority types must not derive serde traits; found #[derive({args})]"
        );
    }
}

#[test]
fn k_sealed_authority_types_have_no_public_constructor() {
    let src = read_src("application/promotion_authority/authorization.rs");
    for name in ["VerifiedExternalApproval", "PromotionAuthorization"] {
        let body = struct_body(&src, name);
        assert!(
            !body.contains("pub "),
            "{name} must have no public fields (found a public field in its body)"
        );
        // The only constructors are the designated authority functions.
        assert!(
            !src.contains(&format!("impl {name} {{\n    /// Construct")),
            "{name} must not expose a constructor helper"
        );
    }
}

/// Extract the `{ ... }` body of a struct definition by name.
fn struct_body(src: &str, name: &str) -> String {
    let marker = format!("pub struct {name} ");
    let start = src
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} not found"));
    let brace = src[start..].find('{').expect("struct body") + start;
    let mut depth = 0usize;
    for (i, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return src[brace..brace + i + 1].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced braces for {name}");
}

#[test]
fn l_no_ai_module_imports_a_permit_minting_constructor() {
    // Production modules only. e80b's WU9 integration test under
    // `application/ai` deliberately references this surface to prove the
    // composition (an automated proposal cannot obtain a permit without a
    // verified approval), so test modules are excluded by design. The
    // production boundary is what must remain unreachable.
    let ai_dirs = ["application/ai", "domain/ai"];
    let forbidden = [
        "issue_promotion_permit",
        "ExternalApprovalAuthority",
        "PromotionAuthorizationPolicy",
        "promotion_authority::authorization",
    ];
    for dir in ai_dirs {
        let root = src_root().join(dir);
        if !root.exists() {
            continue;
        }
        for entry in walk_rs(&root) {
            let name = entry.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.ends_with("_tests.rs")
                || name.contains("test_support")
                || name == "boundary_tests.rs"
            {
                continue;
            }
            let text = std::fs::read_to_string(&entry).unwrap_or_default();
            // Ignore comment-only mentions: strip line comments before checking.
            let code: String = text
                .lines()
                .map(|l| l.split("//").next().unwrap_or(""))
                .collect::<Vec<_>>()
                .join("\n");
            for f in forbidden {
                assert!(
                    !code.contains(f),
                    "{} must not reference `{f}` (permit-minting surface)",
                    entry.display()
                );
            }
        }
    }
}

fn walk_rs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk_rs(&p));
        } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
            out.push(p);
        }
    }
    out
}
