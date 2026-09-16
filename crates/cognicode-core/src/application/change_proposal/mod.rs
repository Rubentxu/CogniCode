//! ChangeProposal and Trial assembly (e72 — M9).
//!
//! e72 splits the "experiment → prove" surface into two layers:
//!
//! - [`proposal`] (e72 WU1): intent-only. A [`ChangeProposal`]
//!   describes *what* someone wants to change; it carries no
//!   authority to apply it.
//! - [`trial`] (e72 WU2+WU3): evaluation. A [`TrialEvidence`] wraps
//!   the e68/e69/e70 outputs under one lineage header, with no
//!   parallel verdict model.
//!
//! ## The umbrella invariant
//!
//! **Creation is not authority.** A `ChangeProposal` does not contain
//! any field that grants apply power. The promotion path is reserved
//! for e73's [`PromotionPermit`](crate::application::promotion_authority::PromotionPermit).
//!
//! [`proposal`]: crate::application::change_proposal::proposal
//! [`trial`]: crate::application::change_proposal::trial
//! [`ChangeProposal`]: crate::application::change_proposal::proposal::ChangeProposal
//! [`TrialEvidence`]: crate::application::change_proposal::trial::TrialEvidence

#[cfg(feature = "evidence-kernel")]
pub mod executor;
#[cfg(feature = "evidence-kernel")]
pub mod proposal;
#[cfg(feature = "evidence-kernel")]
pub mod trial;

#[cfg(all(test, feature = "evidence-kernel"))]
#[path = "executor_tests.rs"]
mod executor_tests;
#[cfg(all(test, feature = "evidence-kernel"))]
#[path = "proposal_tests.rs"]
mod proposal_tests;
#[cfg(all(test, feature = "evidence-kernel"))]
#[path = "trial_tests.rs"]
mod trial_tests;
