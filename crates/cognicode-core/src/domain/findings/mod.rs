//! Findings & Detector IR — M6 domain foundation.
//!
//! Umbrella task 7.2: *Define Detector IR schema/parser/validator.*
//!
//! This module is additive pure domain (no I/O, no `sqlx`/`tokio`),
//! mirroring the M5 `domain::analytics::program_analysis` precedent. It
//! defines the declarative Detector IR that later M6 cycles compile into
//! AST/semantic/graph/dataflow/symbolic backends.
//!
//! See `openspec/changes/e52-lsi-detector-ir/design.md` for the contract.
//!
//! ## Module map
//!
//! - [`detector_ir`] — [`AnalysisLevel`], [`DetectorStep`], [`DetectorIr`],
//!   [`DetectorAuthority`] and the fail-loud validator.

pub mod detector_ir;

pub use detector_ir::{
    AnalysisLevel, DetectorAuthority, DetectorId, DetectorIr, DetectorIrError, DetectorStep,
    FindingKind, SubjectPattern,
};
