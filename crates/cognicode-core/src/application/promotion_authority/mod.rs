//! Promotion authority (e73 — M9).
//!
//! This module defines the surface that decides whether a
//! [`ChangeProposal`](crate::application::change_proposal::proposal::ChangeProposal)
//! — backed by trial evidence — may be promoted (applied) to the
//! current world.
//!
//! ## Three-way evaluation
//!
//! ```text
//! ChangeProposal
//!      +
//! base world A    (world when the proposal was created)
//!      +
//! candidate world B (world after the trial ran)
//!      +
//! current world C  (world right now)
//!        ↓
//! three-way dry-run
//!        ↓
//! PromotionDryRun
//! ```
//!
//! ## Creation is not authority
//!
//! Following the umbrella rule, a `PromotionDryRun` is a *value*: it
//! describes the situation but does not authorise an apply. The
//! authority is granted separately by
//! [`PromotionPermit`](crate::application::promotion_authority::permit::PromotionPermit)
//! (e73 WU2). Direct apply without an explicit permit must fail
//! closed.
//!
//! ## Pure / deterministic / no I/O
//!
//! The evaluation function is total and pure: same inputs produce the
//! same output. It does not touch the filesystem, the clock, the
//! network, or any graph store. It does not "re-run" the trial; it
//! just inspects the snapshot ids and the trial envelope.

#[cfg(feature = "evidence-kernel")]
pub mod authorization;
#[cfg(feature = "evidence-kernel")]
pub mod evaluation;
#[cfg(feature = "evidence-kernel")]
pub mod permit;

#[cfg(all(test, feature = "evidence-kernel"))]
#[path = "evaluation_tests.rs"]
mod evaluation_tests;
#[cfg(all(test, feature = "evidence-kernel"))]
#[path = "permit_tests.rs"]
mod permit_tests;
#[cfg(all(test, feature = "evidence-kernel"))]
#[path = "pipeline_tests.rs"]
mod pipeline_tests;

// e80a WU0 — authority-gap characterization (asserts the pre-e80a hole).
#[cfg(all(test, feature = "evidence-kernel"))]
#[path = "authority_characterization_tests.rs"]
mod authority_characterization_tests;

// e80a — shared fixtures + the WU7 adversarial authority suite.
#[cfg(all(test, feature = "evidence-kernel"))]
mod authority_test_support;
#[cfg(all(test, feature = "evidence-kernel"))]
#[path = "authority_tests.rs"]
mod authority_tests;
