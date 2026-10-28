//! The outcome of naming a target — including the outcomes that are not a
//! target.
//!
//! ## Why this exists
//!
//! The resolver used to answer `Option<SymbolId>`, which is honest about
//! *not* naming a target but dishonest about *why*: a name that matched
//! nothing and a name that matched two homonyms both collapsed to `None`.
//! For impact analysis those are different facts. "This call goes to
//! either `a.rs:foo` or `b.rs:foo`, and we could not prove which" tells
//! the user where the uncertainty is; "there is no `foo`" tells them
//! nothing is unknown. Collapsing them means a consumer cannot surface
//! a gap it never learned exists.
//!
//! Naming a target stays the privilege of `Resolved` alone. That is what
//! keeps the "no invented edges" guarantee structural rather than a rule
//! someone has to remember: an ambiguous call cannot become an edge
//! because there is no variant that both says "ambiguous" and carries a
//! target.
//!
//! ## The two producers
//!
//! One shape, two questions:
//!
//! - The graph builder resolves a *callee name* and has the caller's file
//!   as context. Two homonyms of the same module can be told apart there,
//!   and only what it cannot tell apart stays ambiguous.
//! - The name resolver ([`crate::domain::services::resolve_target`]) takes
//!   a name somebody typed and has no context at all: two homonyms are
//!   ambiguous, and the consumer is told which ones they are.
//!
//! Both answer with this shape, so a consumer learns the same way in both
//! places: only `Resolved` names a target.
//!
//! ## Relation to `ingest::types::TargetRef`
//!
//! Not a duplicate of it. `TargetRef` is an extraction-stage *work item* —
//! `Unresolved(name)` means "a later stage will try to match this". This
//! is the *result* of that matching, produced against a built index of
//! `SymbolId`s. Different stage, different payload, different lifetime.
//!
//! ## Why no `External` variant
//!
//! `ADR-K003` proposes a fourth variant for callees known to live outside
//! the analyzed project. Nothing in the current pipeline can produce it —
//! there is no external-symbol table — so it would be a variant no code
//! can construct and no test could exercise. The rule that pays is the one
//! that forbids representing a fact you cannot observe: an unused variant
//! is an assumption that will survive longer than the code that should
//! have filled it in.

use crate::domain::aggregates::call_graph::SymbolId;

/// The outcome of resolving one name to one target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetResolution {
    /// Exactly one target, proven by a disambiguation rule.
    Resolved(SymbolId),
    /// The name matched, but the target cannot be proven. The candidates
    /// are retained so a consumer can report the uncertainty instead of
    /// silently shrinking the graph.
    Ambiguous {
        /// The lowercased name that was being resolved.
        name: String,
        /// Every candidate the name matched, in the order the producer
        /// found them.
        candidates: Vec<SymbolId>,
    },
    /// The name matched nothing in the project.
    Unresolved(String),
}
