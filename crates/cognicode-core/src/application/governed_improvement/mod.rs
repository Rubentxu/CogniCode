//! Governed self-improvement path (e83 — M13 tasks 13.5/13.6).
//!
//! ```text
//! observed failure
//!       ↓
//! candidate proposal          (e80b FixAgent)
//!       ↓
//! OPTIMIZE shadow evaluation  (e81/e82)
//!       ↓
//! CandidateFreeze             candidate identity fixed
//!       ↓
//! CONFIRM shadow evaluation   (must match the freeze)
//!       ↓
//! HeldOutPromotionGate        + HeldOutPolicySpec
//!       ↓ PASS
//! HeldOutGatePass             sealed: CONFIRM did not block
//!       ↓
//! SoftwareWorld Trial → EvidenceBundle → e69 PolicyGate
//!       ↓
//! evaluate_promotion_lineage  full proposal/candidate/trial lineage
//!       ↓ CLEAN
//! external human approval     (e80a)
//!       ↓
//! PromotionAuthorization → PromotionPermit
//!       ↓
//! GovernedImprovementPermit   sealed: pass + permit for the same attempt
//!       ↓
//! apply_governed_improvement  delegates to e73 apply_with_permit
//!       ↓
//! GovernedImprovementReceipt  audit data (not authority)
//! ```
//!
//! ## The invariant this module establishes
//!
//! ```text
//! OPTIMIZE improvement  +  CONFIRM unacceptable regression  =  NO governed promotion
//! ```
//!
//! and, more strongly:
//!
//! ```text
//! HeldOut PASS + Trial PASS + CleanPromotionReady + external approval
//!   + FAILING CONFIRM
//! = NO GovernedImprovementPermit
//! ```
//!
//! ## Scope, stated precisely
//!
//! This module establishes the canonical **governed self-improvement** path. It
//! does NOT make every `ChangeProposal` in CogniCode use historical replay: the
//! generic M9 promotion surface remains available for changes that are not
//! continuous-improvement candidates (e83 WU10). The guarantee is narrower and
//! exact:
//!
//! ```text
//! any promotion performed THROUGH this module requires a sealed HeldOutGatePass
//! ```
//!
//! ## What this module does NOT claim
//!
//! * It does not attest that machine code corresponds to an analyzer descriptor.
//!   The binding proves identity/lineage inside the governed workflow, not binary
//!   attestation. A production source-patch → executable-analyzer verifier
//!   remains a future adapter seam.
//! * It does not synthesize canonical `Evidence` for the held-out result. The
//!   historical CONFIRM gate and the e69 trial `PolicyGate` stay separate: no
//!   synthetic `EvidenceId`, no placeholder `Fact`.
//! * It does not mutate source files. `PromotionApplyOutcome::Applied` is this
//!   architecture's authority/apply marker; the concrete mutation is a
//!   downstream adapter concern (unchanged since e73).
//! * It does not add an event bus. The typed outcomes and
//!   [`GovernedImprovementReceipt`] are the audit artifacts; the Intelligence
//!   Event Log seam is documented, not expanded.

pub mod binding;
pub mod permit;
pub mod policy;

#[cfg(all(test, feature = "evidence-kernel"))]
#[path = "governed_improvement_tests.rs"]
mod governed_improvement_tests;

pub use binding::{
    CandidateFreeze, FreezeError, GovernedBindingError, GovernedCandidateBinding, report_digest,
};
pub use permit::{
    GovernedImprovementPermit, GovernedImprovementReceipt, GovernedPermitError,
    apply_governed_improvement,
};
pub use policy::{
    HeldOutGateError, HeldOutGatePass, HeldOutPolicyDecision, HeldOutPolicySpec,
    HeldOutPromotionGate, HeldOutReason, HeldOutRejection,
};
