//! Self-hosting validation (e76).
//!
//! "CogniCode evaluating CogniCode" — a deterministic baseline over
//! the project's own source, plus (in future WUs) sealed predictions
//! and prediction-vs-observation scoring.
//!
//! The module is platform-neutral and pure. It composes existing
//! canonical ingestion primitives (`content_digest`,
//! `ingest_rust_facts`, `DetectorDigest`) rather than introducing a
//! parallel digest scheme.
//!
//! Sub-modules:
//! - `baseline.rs` — WU1 (deterministic self-model baseline).
//! - `prediction.rs` — WU2 (sealed prediction vs observation scoring).
//! - `mutation_corpus.rs` — WU3 (controlled mutation corpus with
//!   pre-declared expected outcomes).
//! - `platform_equivalence.rs` — WU4 (cross-platform equivalence
//!   comparison + historical replay bootstrap).
//! - `closure.rs` — closure gate (the structural proof that the
//!   pieces compose end-to-end).
//! - `acceptance.rs` — end-to-end acceptance tests that walk the
//!   real `cognicode-core/src/` directory at test time, exercising
//!   the real public API (`compute_baseline`) on real file bytes.
//! - `platform_equivalence_acceptance.rs` — end-to-end acceptance
//!   for the WU4 platform equivalence public API.
//! - `prediction_acceptance.rs` — end-to-end acceptance for the
//!   WU2 prediction public API.

pub mod acceptance;
pub mod baseline;
pub mod closure;
pub mod mutation_corpus;
pub mod platform_equivalence;
pub mod platform_equivalence_acceptance;
pub mod prediction;
pub mod prediction_acceptance;
