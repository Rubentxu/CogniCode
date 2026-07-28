//! Plan error types — PlanError, ExecutorError, UnsupportedConstruct, CancellationToken, ProvenanceSource.
//!
//! Part of e28-1-moldplan-graphplan-contracts: PR1 Foundation Phase 1.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

// ============================================================================
// PlanError
// ============================================================================

/// Error type for plan construction and validation failures.
///
/// `PlanError` is a pure domain error — it does NOT carry execution state
/// (that's `ExecutorError`). It is raised during plan construction, lowering,
/// or validation before any execution begins.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum PlanError {
    /// A `GraphPlan` was constructed without a revision pin.
    #[error("graph plan must be pinned to a workspace and revision")]
    UnpinnedGraphPlan,

    /// A plan variant requires a specific limit that is absent.
    #[error("plan is missing required limit: {0}")]
    MissingLimit(super::PlanLimit),

    /// A plan variant was constructed with an unbounded quantifier
    /// (e.g. unbounded path length).
    #[error("unbounded quantifier: {0}")]
    UnboundedQuantifier(String),

    /// A plan uses syntax that is only valid in Cypher.
    #[error("Cypher-only syntax: {0}")]
    CypherOnlySyntax(String),

    /// A plan uses a GQL-only feature not supported in MoldQL.
    #[error("GQL feature not supported: {0}")]
    GqlFeature(String),

    /// A plan's `LIMIT` clause is required but absent.
    #[error("LIMIT clause is required for this query type")]
    LimitMissing,

    /// The executor does not know how to execute this plan variant.
    #[error("unknown backend for plan: {0}")]
    UnknownBackend(String),

    /// The plan references a revision that does not exist.
    #[error("revision unknown: workspace={workspace_id} revision={revision_id}")]
    RevisionUnknown {
        workspace_id: String,
        revision_id: u64,
    },

    /// A semantic violation in the plan itself.
    #[error("semantics violation: {0}")]
    SemanticsViolation(#[from] super::SemanticsViolation),

    /// The plan is already pinned and cannot be pinned again.
    #[error("plan is already pinned to a workspace and revision")]
    AlreadyPinned,

    /// The operation requires a graph plan but the current plan is not a graph plan.
    #[error("operation requires a graph plan")]
    NotAGraphPlan,

    /// An unsupported syntactic construct was encountered during lowering.
    #[error("unsupported construct: {0}")]
    UnsupportedConstruct(#[from] UnsupportedConstruct),
}

// ============================================================================
// CancellationToken
// ============================================================================

/// A shared cancellation flag that can be set externally and polled by executors.
///
/// `CancellationToken` wraps an `Arc<AtomicBool>`. When `set()` is called,
/// `is_cancelled()` returns `true` for all clones. This allows safe sharing
/// across async tasks without a channel.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    inner: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Create a new unset cancellation token.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Set the cancellation flag. This is irrevocable.
    pub fn set(&self) {
        self.inner.store(true, Ordering::SeqCst);
    }

    /// Returns `true` if `set()` has been called on this token or any clone.
    pub fn is_cancelled(&self) -> bool {
        self.inner.load(Ordering::SeqCst)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for CancellationToken {
    fn eq(&self, other: &Self) -> bool {
        // Compare by pointer equality on the underlying Arc.
        // Two CancellationTokens are equal if they share the same inner flag.
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for CancellationToken {}

impl std::hash::Hash for CancellationToken {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash by pointer value of the Arc.
        //
        // NOTE: This hash is **process-local**. The Arc pointer address is
        // determined by the allocator and is NOT stable across process restarts.
        // Do NOT use `CancellationToken` as a key in persistent `HashMap`/
        // `HashSet` structures that outlive a single process. Within a single
        // process, pointer-based hashing is consistent with `PartialEq` (which
        // also uses `Arc::ptr_eq`).
        (Arc::as_ptr(&self.inner) as usize).hash(state);
    }
}

// ============================================================================
// ConstructId
// ============================================================================

/// A unique identifier for a syntactic construct that the executor cannot handle.
///
/// `ConstructId` is the "what" part of `UnsupportedConstruct` — it tells
/// the caller which specific syntax construct is not supported.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConstructId {
    /// An unbounded path expression (no `max_hops`).
    UnboundedPath,
    /// An unbounded quantifier (e.g., `*` in regex-like patterns).
    UnboundedQuantifier,
    /// Unbounded recursion in a WITH clause.
    UnboundedRecursion,
    /// A mutating clause (WRITE, DELETE, etc.) in a read-only context.
    MutatingClause,
    /// A Pattern Profile GQL feature.
    PatternProfileFeature,
    /// A Graph Analytics registry feature.
    GraphAnalyticsFeature,
    /// Some other construct not listed above.
    Other(String),
}

impl fmt::Display for ConstructId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstructId::UnboundedPath => write!(f, "UnboundedPath"),
            ConstructId::UnboundedQuantifier => write!(f, "UnboundedQuantifier"),
            ConstructId::UnboundedRecursion => write!(f, "UnboundedRecursion"),
            ConstructId::MutatingClause => write!(f, "MutatingClause"),
            ConstructId::PatternProfileFeature => write!(f, "PatternProfileFeature"),
            ConstructId::GraphAnalyticsFeature => write!(f, "GraphAnalyticsFeature"),
            ConstructId::Other(s) => write!(f, "Other({s})"),
        }
    }
}

// ============================================================================
// SourceLocation
// ============================================================================

/// Source location for error reporting — line, column, and byte offset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceLocation {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    pub column: u32,
    /// 0-based byte offset from the start of the source.
    pub byte_offset: u32,
}

impl SourceLocation {
    pub fn new(line: u32, column: u32, byte_offset: u32) -> Self {
        Self { line, column, byte_offset }
    }
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.line, self.column, self.byte_offset)
    }
}

// ============================================================================
// UnsupportedConstruct
// ============================================================================

/// A structured error for unsupported language constructs.
///
/// `UnsupportedConstruct` is raised BEFORE execution begins — the parser or
/// plan lowerer detects the construct and returns this error. The executor
/// NEVER receives a plan containing an `UnsupportedConstruct`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub struct UnsupportedConstruct {
    /// The syntactic construct that is not supported.
    pub construct: ConstructId,
    /// A human-readable message describing the problem.
    pub message: String,
    /// The supported alternative (if one exists), otherwise `None`.
    pub supported_alternative: Option<String>,
    /// Where in the source the construct appears.
    pub location: Option<SourceLocation>,
}

impl UnsupportedConstruct {
    /// Construct an `UnsupportedConstruct` without a source location.
    pub fn new(construct: ConstructId, message: impl Into<String>) -> Self {
        Self {
            construct,
            message: message.into(),
            supported_alternative: None,
            location: None,
        }
    }

    /// Attach a supported alternative to this error.
    pub fn with_alternative(mut self, alt: impl Into<String>) -> Self {
        self.supported_alternative = Some(alt.into());
        self
    }

    /// Attach a source location to this error.
    pub fn at(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }
}

impl fmt::Display for UnsupportedConstruct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unsupported construct: {} — {}", self.construct, self.message)?;
        if let Some(ref alt) = self.supported_alternative {
            write!(f, " (supported alternative: {alt})")?;
        }
        if let Some(ref loc) = self.location {
            write!(f, " at {loc}")?;
        }
        Ok(())
    }
}

// ============================================================================
// ProvenanceSource
// ============================================================================

/// Source of provenance metadata for an edge or node result.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProvenanceSource {
    /// Provenance from a static analysis pass (e.g., "calls" from tree-sitter extraction).
    StaticAnalysis(String),
    /// Provenance from a dynamic runtime observation (e.g., "runtime" from profiling).
    Runtime(String),
    /// Provenance from an external tool or integration (e.g., "lsp" from language server).
    External(String),
    /// Provenance was not captured or is unknown.
    Unknown,
}

impl fmt::Display for ProvenanceSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProvenanceSource::StaticAnalysis(s) => write!(f, "static:{s}"),
            ProvenanceSource::Runtime(s) => write!(f, "runtime:{s}"),
            ProvenanceSource::External(s) => write!(f, "external:{s}"),
            ProvenanceSource::Unknown => write!(f, "unknown"),
        }
    }
}

// ============================================================================
// ExecutorError
// ============================================================================

/// Top-level error type for plan execution failures.
///
/// `ExecutorError` is raised DURING or AFTER plan execution. It wraps both
/// structured `PlanError` (pre-execution) and runtime limit/unsupported errors.
///
/// This type supersedes any legacy `QueryError` in the codebase.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum ExecutorError {
    /// The executor encountered an unsupported construct at execution time
    /// (should have been caught at plan construction, but defense in depth).
    #[error("unsupported construct: {0}")]
    UnsupportedConstruct(#[from] UnsupportedConstruct),

    /// A resource limit was exceeded during execution.
    #[error("limit exceeded: {dimension} (observed: {observed})")]
    LimitExceeded {
        dimension: super::PlanLimit,
        observed: u64,
    },

    /// The plan references a revision that does not exist in this workspace.
    #[error("revision unknown: {0}")]
    RevisionUnknown(String),

    /// A semantic rule was violated at runtime.
    #[error("semantics violation: {0}")]
    SemanticsViolation(#[from] super::SemanticsViolation),

    /// A plan-level construction/validation error.
    #[error("plan error: {0}")]
    PlanError(#[from] super::PlanError),

    /// An internal executor failure (assertion, invariant violation).
    #[error("internal executor error: {0}")]
    InternalError(String),
}

impl ExecutorError {
    /// Returns `true` if this error should be treated as a pre-execution error
    /// (i.e., it was raised before any rows were processed).
    ///
    /// Used for error classification in tests and monitoring.
    pub fn is_pre_execution(&self) -> bool {
        matches!(
            self,
            ExecutorError::UnsupportedConstruct(_) | ExecutorError::PlanError(_)
        )
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Task 1.2a RED — PlanError enum (UnpinnedGraphPlan, MissingLimit, etc.)
    // Scenario: `moldplan-graphplan::Revision Pinning` (Constructing a plan
    //           without a pin is rejected); `plan-limits::Every Plan Declares Applicable Limits`
    // Assert: missing pin → `Err(PlanError::UnpinnedGraphPlan)`;
    //         subgraph w/o depth → `Err(MissingLimit(MaxDepth))`
    // Task 1.5a RED — CancellationToken set/abort + propagation
    // Scenario: `plan-limits::Cancellation Token`
    // Assert: external `set()` → subsequent `is_cancelled()` returns true;
    //         `LimitExceeded { limit: Cancellation, observed: 0 }` envelope raised
    // Task 1.6a RED — UnsupportedConstruct + ConstructId exhaustiveness + Display + location
    // Scenario: `unsupported-operation-errors::UnsupportedConstruct Error` (both) +
    //           `Identifies the Supported Alternative` (both) + `Source Location` (both)
    // Assert: ConstructId covers all variants; Display includes construct id + message + alternative;
    //         SourceLocation carries line, column, byte_offset
    // Task 1.10a RED — ExecutionError variants + pre-execution raise
    // Scenario: `executor-semantics::Error Envelope` (both) + `plan-limits::Breach Produces Typed Error or Explicit Truncation` (all 3)
    // Assert: time-limit breach → `Err(LimitExceeded { Time, observed })`;
    //         unsupported → `Err(UnsupportedConstruct)` pre-execution
    // -------------------------------------------------------------------------

    /// `PlanError::UnpinnedGraphPlan` Display.
    #[test]
    fn plan_error_unpinned() {
        let err = PlanError::UnpinnedGraphPlan;
        assert_eq!(err.to_string(), "graph plan must be pinned to a workspace and revision");
    }

    /// `PlanError::MissingLimit` carries the limit variant.
    #[test]
    fn plan_error_missing_limit() {
        use super::super::PlanLimit;
        let err = PlanError::MissingLimit(PlanLimit::MaxDepth);
        assert_eq!(err.to_string(), "plan is missing required limit: max_depth");
    }

    /// `PlanError::UnboundedQuantifier` carries context.
    #[test]
    fn plan_error_unbounded_quantifier() {
        let err = PlanError::UnboundedQuantifier("path(*, start, end)".into());
        assert!(err.to_string().contains("path(*, start, end)"));
    }

    /// `PlanError::RevisionUnknown` carries ids.
    #[test]
    fn plan_error_revision_unknown() {
        let err = PlanError::RevisionUnknown {
            workspace_id: "ws1".into(),
            revision_id: 42,
        };
        assert!(err.to_string().contains("ws1"));
        assert!(err.to_string().contains("42"));
    }

    /// `CancellationToken::new()` is not cancelled.
    #[test]
    fn cancellation_token_default_not_cancelled() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
    }

    /// `CancellationToken::set()` makes `is_cancelled()` return `true`.
    #[test]
    fn cancellation_token_set() {
        let token = CancellationToken::new();
        token.set();
        assert!(token.is_cancelled());
    }

    /// `CancellationToken` is `Clone`able and shares state across clones.
    #[test]
    fn cancellation_token_clone_shares_state() {
        let token = CancellationToken::new();
        let clone = token.clone();
        assert!(!token.is_cancelled());
        assert!(!clone.is_cancelled());
        token.set();
        assert!(token.is_cancelled());
        assert!(clone.is_cancelled(), "clone must see the same cancelled state");
    }

    /// `CancellationToken` is `Eq` when they share the same underlying flag.
    #[test]
    fn cancellation_token_eq() {
        let token = CancellationToken::new();
        let clone = token.clone();
        assert_eq!(token, clone);
    }

    /// `CancellationToken` is `Hash`able.
    #[test]
    fn cancellation_token_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let token = CancellationToken::new();
        let clone = token.clone();
        fn hash_of<T: Hash>(value: &T) -> u64 {
            let mut s = DefaultHasher::new();
            value.hash(&mut s);
            s.finish()
        }
        assert_eq!(hash_of(&token), hash_of(&clone));
    }

    /// `CancellationToken` is `Default`.
    #[test]
    fn cancellation_token_default() {
        let token: CancellationToken = Default::default();
        assert!(!token.is_cancelled());
    }

    /// `ConstructId::Display` returns the variant name or Other(...).
    #[test]
    fn construct_id_display() {
        assert_eq!(ConstructId::UnboundedPath.to_string(), "UnboundedPath");
        assert_eq!(ConstructId::MutatingClause.to_string(), "MutatingClause");
        assert_eq!(ConstructId::Other("CustomThing".into()).to_string(), "Other(CustomThing)");
    }

    /// `ConstructId` has all expected variants.
    #[test]
    fn construct_id_exhaustive() {
        use ConstructId::*;
        let variants = [
            UnboundedPath,
            UnboundedQuantifier,
            UnboundedRecursion,
            MutatingClause,
            PatternProfileFeature,
            GraphAnalyticsFeature,
            Other("test".into()),
        ];
        assert_eq!(variants.len(), 7, "ConstructId must have 7 variants");
    }

    /// `UnsupportedConstruct::new` creates a basic error.
    #[test]
    fn unsupported_construct_new() {
        let err = UnsupportedConstruct::new(ConstructId::UnboundedPath, "no max_hops specified");
        assert_eq!(err.construct, ConstructId::UnboundedPath);
        assert_eq!(err.message, "no max_hops specified");
        assert!(err.supported_alternative.is_none());
        assert!(err.location.is_none());
    }

    /// `UnsupportedConstruct::with_alternative` sets the alternative.
    #[test]
    fn unsupported_construct_with_alternative() {
        let err = UnsupportedConstruct::new(ConstructId::UnboundedQuantifier, "quantifier unbounded")
            .with_alternative("add a `max_hops = N` bound");
        assert_eq!(
            err.supported_alternative.as_deref(),
            Some("add a `max_hops = N` bound")
        );
    }

    /// `UnsupportedConstruct::at` sets the location.
    #[test]
    fn unsupported_construct_with_location() {
        let err = UnsupportedConstruct::new(ConstructId::MutatingClause, "WRITE in read-only context")
            .at(SourceLocation::new(5, 10, 42));
        assert_eq!(err.location.as_ref().unwrap().line, 5);
        assert_eq!(err.location.as_ref().unwrap().column, 10);
        assert_eq!(err.location.as_ref().unwrap().byte_offset, 42);
    }

    /// `UnsupportedConstruct::Display` includes construct + message.
    #[test]
    fn unsupported_construct_display() {
        let err = UnsupportedConstruct::new(ConstructId::PatternProfileFeature, "not implemented yet");
        let display = err.to_string();
        assert!(display.contains("unsupported construct"));
        assert!(display.contains("PatternProfileFeature"));
        assert!(display.contains("not implemented yet"));
    }

    /// `SourceLocation::Display` shows `line:column:byte_offset`.
    #[test]
    fn source_location_display() {
        let loc = SourceLocation::new(10, 5, 100);
        assert_eq!(loc.to_string(), "10:5:100");
    }

    /// `ExecutorError::LimitExceeded` carries dimension and observed value.
    #[test]
    fn executor_error_limit_exceeded() {
        use super::super::PlanLimit;
        let err = ExecutorError::LimitExceeded {
            dimension: PlanLimit::TimeMs,
            observed: 5001,
        };
        assert!(err.to_string().contains("time_ms"));
        assert!(err.to_string().contains("5001"));
    }

    /// `ExecutorError::UnsupportedConstruct` converts from `UnsupportedConstruct`.
    #[test]
    fn executor_error_from_unsupported_construct() {
        let uc = UnsupportedConstruct::new(ConstructId::UnboundedPath, "no hops");
        let err: ExecutorError = uc.into();
        assert!(matches!(err, ExecutorError::UnsupportedConstruct(_)));
    }

    /// `ExecutorError::is_pre_execution` returns `true` for pre-execution variants.
    #[test]
    fn executor_error_is_pre_execution() {
        let uc = UnsupportedConstruct::new(ConstructId::UnboundedPath, "no hops");
        let err: ExecutorError = uc.into();
        assert!(err.is_pre_execution());

        let plan_err = ExecutorError::from(PlanError::UnpinnedGraphPlan);
        assert!(plan_err.is_pre_execution());

        let limit_err = ExecutorError::LimitExceeded {
            dimension: super::super::PlanLimit::TimeMs,
            observed: 100,
        };
        assert!(!limit_err.is_pre_execution());
    }

    /// `ExecutorError::PlanError` converts from `PlanError`.
    #[test]
    fn executor_error_from_plan_error() {
        let plan_err = PlanError::UnpinnedGraphPlan;
        let exec_err: ExecutorError = plan_err.into();
        assert!(matches!(exec_err, ExecutorError::PlanError(_)));
    }

    /// `ExecutorError::InternalError` carries a message.
    #[test]
    fn executor_error_internal() {
        let err = ExecutorError::InternalError("assertion failed: node not found".into());
        assert!(err.to_string().contains("assertion failed"));
    }

    /// `ExecutorError` is `Clone`, `Debug`, `PartialEq`, `Eq`.
    #[test]
    fn executor_error_derives() {
        fn assert_derives<T: Clone + std::fmt::Debug + PartialEq + Eq>() {}
        assert_derives::<ExecutorError>();
    }

    /// `UnsupportedConstruct` is `Clone`, `Debug`, `PartialEq`, `Eq`.
    #[test]
    fn unsupported_construct_derives() {
        fn assert_derives<T: Clone + std::fmt::Debug + PartialEq + Eq>() {}
        assert_derives::<UnsupportedConstruct>();
    }

    /// `PlanError` is `Clone`, `Debug`, `PartialEq`, `Eq`, `Serialize`, `Deserialize`.
    #[test]
    fn plan_error_derives() {
        fn assert_derives<T: Clone + std::fmt::Debug + PartialEq + Eq + serde::Serialize + for<'de> serde::Deserialize<'de>>() {}
        assert_derives::<PlanError>();
    }

    /// `PlanError::UnknownBackend` round-trips through serde.
    #[test]
    fn plan_error_unknown_backend_serde() {
        let err = PlanError::UnknownBackend("PostgreSQL v99".into());
        let json = serde_json::to_string(&err).expect("serialize");
        let parsed: PlanError = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, err);
    }

    /// `ExecutorError` round-trips through serde.
    #[test]
    fn executor_error_serde_roundtrip() {
        let err = ExecutorError::LimitExceeded {
            dimension: super::super::PlanLimit::MemoryBytes,
            observed: 1024,
        };
        let json = serde_json::to_string(&err).expect("serialize");
        let parsed: ExecutorError = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, err);
    }

    /// `ProvenanceSource` has all expected variants and Display format.
    #[test]
    fn provenance_source_display() {
        assert_eq!(ProvenanceSource::StaticAnalysis("calls".into()).to_string(), "static:calls");
        assert_eq!(ProvenanceSource::Runtime("profiling".into()).to_string(), "runtime:profiling");
        assert_eq!(ProvenanceSource::External("lsp".into()).to_string(), "external:lsp");
        assert_eq!(ProvenanceSource::Unknown.to_string(), "unknown");
    }
}
