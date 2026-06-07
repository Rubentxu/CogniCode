//! MoldQL — the explorer's query language.
//!
//! MoldQL is a small, declarative language that combines a target
//! (what to find), a scope filter (where to look), a set of AND-chained
//! conditions, and an optional lens, into a single query. EXPLORE adds
//! BFS traversal over the call graph.
//!
//! ## Pipeline
//!
//! ```text
//!   str ── parse ──► MoldQLQuery (AST)
//!                  ── execute ─► MoldQLResult
//! ```
//!
//! The executor never reaches outside the existing service ports — it
//! only consumes [`crate::ports::SymbolRepository`], the optional
//! [`crate::ports::QualityRepository`], and the registered lenses.
//!
//! ## Public surface
//!
//! - [`MoldQLQuery`], [`MoldQLResult`], [`MoldQLItem`] — the AST / DTO
//!   types callers interact with
//! - [`parser::parse`] — entry point for `&str → MoldQLQuery`
//! - [`ParseError`] — diagnostic with line + column
//! - [`MoldQLExecutor`] — drives an [`crate::service::ExplorerService`]
//!   against a parsed query
//!
//! Most callers should go through
//! [`crate::service::ExplorerService::execute_query`] — it combines
//! parse + execute into a single call.

pub mod ast;
pub mod executor;
pub mod parser;

pub use ast::{
    Condition, Direction, ExploreQuery, Field, FindQuery, MoldQLQuery, Op, TargetType, Value,
};
pub use executor::{MoldQLExecutor, MoldQLItem, MoldQLResult, MoldQLView};
pub use parser::{parse, ParseError};
