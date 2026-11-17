//! Semantic SAST models: one catalog of source / sink / sanitizer API models,
//! per-language adapters, and the shared CWE-89 detector.
//!
//! This module owns **what an API means** for taint analysis. It does not own
//! **how taint flows**: reachability stays in the M5 engine behind
//! [`M5DataflowBackend`](crate::application::findings::M5DataflowBackend), and
//! the rule is an ordinary [`DetectorIr`](super::DetectorIr) that goes through
//! the unchanged admission, planning and verification path. There is no second
//! IR, rule engine or policy syntax here.
//!
//! ```text
//! ObservedFunction ─ classify ─▶ DataflowFunction (subjects) ─▶ M5 taint
//!   (per language)    (adapter)       security.user_input / sql_execution / sanitizer
//! ```
//!
//! ## Honesty contract
//!
//! - A language without an adapter is [`Classification::Unsupported`]; it is
//!   never analysed against an empty model.
//! - The shipped backend is **intraprocedural**. A run that finds nothing while
//!   calls remain unmodeled is [`SastVerdict::Unknown`]; only a run with every
//!   call modeled is [`SastVerdict::NoFlowObserved`], and that verdict names its
//!   [`AnalysisReach`] instead of claiming the code is clean.
//! - An interprocedural rule declares the capabilities it really needs
//!   (resolution and graph queries on top of dataflow). With no backend that
//!   provides them the planner refuses the run, so no evidence is admitted.
//!
//! ## Extension points
//!
//! A new language is one more [`LanguageAdapter`] table; a new weakness is a
//! new [`Weakness`] variant plus its sink/sanitizer rows and a rule builder
//! next to [`cwe89_sql_injection`]. Flow logic is never copied.
//!
//! Pure domain: no I/O.

mod assessment;
mod catalog;
mod classify;
mod rule;

pub use assessment::{Assessment, PrecisionRecall, SastVerdict, assess};
pub use catalog::{
    ApiModel, LanguageAdapter, ModelRole, SemanticModelCatalog, SourceLanguage, Weakness,
};
pub use classify::{
    CallArg, Classification, ModeledFunction, Observation, ObservationKind, ObservedCall,
    ObservedFunction,
};
pub use rule::{
    AnalysisReach, SANITIZER_SUBJECT, SQL_EXECUTION_SUBJECT, SQL_INJECTION_KIND,
    USER_INPUT_SUBJECT, cwe89_sql_injection,
};
