//! TrialExecutor (e72 WU3 — M9).
//!
//! A [`TrialExecutor`] runs the policy-gate evaluation step of a
//! trial: it takes a [`TrialInput`] with an [`EvidenceBundle`], evaluates
//! that bundle against a configured [`PolicySpec`], and assembles a
//! [`TrialEvidence`] with the resulting [`PolicyDecision`].
//!
//! ## `TrialExecutor` ≠ `WorkExecutor`
//!
//! These are deliberately separate traits:
//!
//! - [`WorkExecutor`](crate::application::local_ci::InMemoryWorkExecutor)
//!   (e70) runs a piece of logical work; its output is a
//!   [`PerWorkReport`].
//! - [`TrialExecutor`] (this module) assembles the trial envelope
//!   from pre-computed evidence, evaluating the policy gate; its
//!   output is a [`TrialEvidence`].
//!
//! The two may compose (a future impl may invoke a `WorkExecutor`
//! inside `run_trial`), but they are not the same trait. The envelope
//! is explicit about this separation: do not pre-emptively merge
//! the two.
//!
//! ## Reusing e69 (no parallel verdict model)
//!
//! The executor evaluates the bundle via e69's
//! [`evaluate`](crate::application::policy_gate::evaluate) function.
//! It does NOT introduce a parallel verdict model. The
//! [`PolicyDecision`] returned is exactly the one e69 produces.
//!
//! ## Pure / deterministic / no I/O
//!
//! The default implementation does not touch the filesystem, the
//! clock, the network, or any graph store. It is a pure composition
//! over the inputs. The trait itself does not constrain this — a
//! custom impl could add I/O — but the default impl does not, and
//! the tests assert this.
//!
//! ## Feature gate
//!
//! This module requires the `evidence-kernel` feature because
//! `TrialInput` lives behind that gate.

use crate::application::change_proposal::trial::{
    TrialEvidence, TrialId, TrialInput, assemble_trial_evidence,
};
use crate::application::policy_gate::{PolicySpec, evaluate};

/// A type that runs the policy-gate step of a trial and assembles the
/// resulting [`TrialEvidence`].
///
/// Implementations are responsible for:
/// 1. Receiving a [`TrialInput`] with an [`EvidenceBundle`] and any
///    other pre-computed artefacts.
/// 2. Evaluating the bundle against their configured policy.
/// 3. Producing a [`TrialEvidence`] via
///    [`assemble_trial_evidence`].
///
/// Implementations MUST NOT:
/// - Synthesise a new verdict that bypasses
///   [`evaluate`](crate::application::policy_gate::evaluate).
/// - Add fields to the [`TrialEvidence`] envelope that the assembler
///   does not produce. The envelope shape is fixed by e72 WU2.
pub trait TrialExecutor {
    /// Run a trial and produce the lineage-labelled envelope.
    fn run_trial(&self, trial_id: TrialId, input: TrialInput) -> TrialEvidence;
}

/// Default [`TrialExecutor`] implementation: pure composition, no IO.
///
/// The executor is constructed with a [`PolicySpec`]. On each
/// `run_trial`, it:
/// 1. Evaluates the input's evidence bundle against the spec via e69's
///    [`evaluate`] function (pure, deterministic, total).
/// 2. Assembles the [`TrialEvidence`] via
///    [`assemble_trial_evidence`].
///
/// This is the only logic; there is no clock, no IO, no graph store.
/// The default impl is the canonical reference for "what running a
/// trial means" in M9.
#[derive(Debug, Clone)]
pub struct DefaultTrialExecutor {
    policy: PolicySpec,
}

impl DefaultTrialExecutor {
    /// Construct a default executor with the given policy spec.
    pub fn new(policy: PolicySpec) -> Self {
        Self { policy }
    }

    /// Borrow the configured policy spec.
    pub fn policy(&self) -> &PolicySpec {
        &self.policy
    }
}

impl TrialExecutor for DefaultTrialExecutor {
    fn run_trial(&self, trial_id: TrialId, input: TrialInput) -> TrialEvidence {
        let gate = evaluate(&input.evidence_bundle, &self.policy);
        assemble_trial_evidence(trial_id, input, gate)
    }
}
