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
//! See `baseline.rs` for WU1 (deterministic self-model baseline).

pub mod baseline;
