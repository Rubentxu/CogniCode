//! Relocated to `cognicode_core::domain::ports::quality_store`.
//!
//! The port trait was unified into a single 10-method `QualityStore`
//! trait (the original `QualityStore` + `QualityStore` split is
//! no longer needed at the port level — both groups are in the same
//! trait surface; callers that need the read-only invariant can use a
//! `&dyn QualityStore` projection that only exercises read methods).
//! The PostgreSQL adapter moved with the trait.
//!
//! This shim only re-exports the new names; existing call sites need
//! to migrate `crate::ports::QualityStore` / `QualityStore` /
//! `PostgresQualityStore` references to the unified
//! `QualityStore` / `PostgresQualityStore` names (PR2 WU2.3).

pub use cognicode_core::domain::ports::quality_store::{
    IssueFilter, NewIssue, PostgresQualityStore, QualityError, QualityGateSummary, QualityIssue,
    QualityStore, RuleSummary, UpsertSummary,
};
