//! Semantic SAST models: one catalog of source / sink / sanitizer API models,
//! per-language adapters, the shared taint detector (SQL injection, command
//! injection, path traversal), structural families (hardcoded secrets, `unsafe`
//! Rust), optional JVM context, and the capability matrix derived from them.
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
//! ## Pack and capability matrix
//!
//! [`CapabilityMatrix`] states, per language and family, whether the pack is
//! supported, through which backend, up to which evidence class and whether a
//! finding may block. It is derived from the catalog and the backend registry,
//! so it cannot drift from what a run does. `unsafe` Rust is a
//! [`FindingNature::Risk`] with [`BlockingLevel::AdvisoryOnly`], never an
//! automatic vulnerability.
//!
//! Java and Kotlin may carry a [`JvmContext`] (imports and classpath
//! supertypes) so receivers resolve to JDK types. It is plain data supplied by
//! the caller: no JVM or Detekt scanner is required, and without it an
//! unresolvable callee keeps the verdict at `Unknown`.
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
mod jvm;
mod pack;
mod rule;
mod structural;

pub use assessment::{Assessment, PrecisionRecall, SastVerdict, assess};
pub use catalog::{
    ApiModel, LanguageAdapter, ModelRole, SemanticModelCatalog, SourceLanguage, Weakness,
};
pub use classify::{
    CallArg, Classification, ModeledFunction, Observation, ObservationKind, ObservedCall,
    ObservedFunction,
};
pub use jvm::JvmContext;
pub use pack::{
    BlockingLevel, CapabilityEntry, CapabilityMatrix, FindingNature, ModelCounts, PACK_VERSION,
    PackVerdict, SecurityFamily, Support,
};
pub use rule::{
    AnalysisReach, COMMAND_EXECUTION_SUBJECT, COMMAND_INJECTION_KIND, FILE_ACCESS_SUBJECT,
    PATH_TRAVERSAL_KIND, SANITIZER_SUBJECT, SQL_EXECUTION_SUBJECT, SQL_INJECTION_KIND,
    USER_INPUT_SUBJECT, cwe89_sql_injection, taint_detector,
};
pub use structural::{
    HARDCODED_SECRET_KIND, HARDCODED_SECRET_SUBJECT, IAC_ENCRYPTION_KIND, IAC_ENCRYPTION_SUBJECT,
    IAC_PERMISSIONS_KIND, IAC_PERMISSIONS_SUBJECT, IAC_PUBLIC_EXPOSURE_KIND,
    IAC_PUBLIC_EXPOSURE_SUBJECT, IAC_WORKLOAD_HARDENING_KIND, IAC_WORKLOAD_HARDENING_SUBJECT,
    ObservedUnit, StructuralClassification, StructuralFact, StructuralFamily,
    StructuralObservation, UNSAFE_CODE_KIND, UNSAFE_CODE_SUBJECT, UnsafeConstruct,
    classify_structural, is_hardcoded_secret, unresolved_checks,
};
