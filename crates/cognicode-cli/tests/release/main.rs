//! One integration-test binary for these suites: every file below is a module.
//! Kept apart from `it` because the release pipeline runs them against host binaries.

#[path = "../common/mod.rs"]
mod common;

mod prf_f6_w1_release_coherence;
mod prf_f6_w2_staging_contract;
mod published_skill_bundles_are_required;
