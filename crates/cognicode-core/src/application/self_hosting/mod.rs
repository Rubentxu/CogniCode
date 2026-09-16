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

pub mod baseline;
pub mod closure;
pub mod mutation_corpus;
pub mod platform_equivalence;
pub mod prediction;
