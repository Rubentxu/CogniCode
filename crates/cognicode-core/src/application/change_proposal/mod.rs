//! ChangeProposal (e72 WU1 — M9).
//!
//! A [`ChangeProposal`] is the *intent* to apply a change to a
//! derivation context. It is **not** an authority to apply it.
//!
//! The umbrella rule is unchanged from e69/e70: **creation is not
//! authority**. A `ChangeProposal` is created by an author (human,
//! plugin, or LLM agent) and then evaluated through a trial (e72 WU2)
//! whose evidence feeds a policy gate (e69) and, only with an explicit
//! promotion permit (e73 WU2), can be applied.
//!
//! This module defines the proposal shape only. The proposal does not
//! contain:
//!
//! - A [`PromotionPermit`](crate::application::promotion_authority::PromotionPermit)
//!   (that lives in e73).
//! - A [`TrialEvidence`](crate::application::change_proposal::trial::TrialEvidence)
//!   (that lives in e72 WU2).
//! - An [`EvidenceBundle`](crate::application::evidence_bundle::EvidenceBundle)
//!   (that lives in e69).
//!
//! Anything resembling authority attached to the proposal would
//! reintroduce the e67-style detector self-promotion anti-pattern. We
//! keep the proposal minimal.
//!
//! ## Pure / deterministic / no I/O
//!
//! - No clock: the proposal does not carry a timestamp. Time audit is a
//!   concern of the trial record / promotion audit (e72 WU3, e73 WU2).
//! - No UUID mint at construction. The caller supplies the id.
//! - No graph store interaction. The proposal references a
//!   [`SoftwareWorldId`]; the trial resolves it.
//!
//! ## Feature gate
//!
//! This module requires the `evidence-kernel` feature because
//! [`SoftwareWorldId`] lives behind that gate.

#[cfg(feature = "evidence-kernel")]
pub mod proposal;

#[cfg(all(test, feature = "evidence-kernel"))]
#[path = "proposal_tests.rs"]
mod proposal_tests;
