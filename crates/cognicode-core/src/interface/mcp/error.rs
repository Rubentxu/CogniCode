//! Interface Error Types
//!
//! Errors at the MCP interface layer. Preserves causal chain via thiserror's
//! `#[source]` attribute.

use crate::interface::mcp::handlers::HandlerError;
use crate::interface::mcp::security::SecurityError;
use thiserror::Error;

/// What the text channel says when a call outlives its budget.
const TIMEOUT_DETAIL: &str = "timeout";

/// What the text channel says when the rate limiter refuses a call.
const RATE_LIMITED_DETAIL: &str = "rate_limit_exceeded";

/// What the text channel says when a read-only server refuses a mutating
/// tool. One sentence, shared by the typed error and by the conversion to
/// the handler error, so the two routes cannot drift.
fn read_only_detail(tool: &str) -> String {
    format!(
        "read_only_mode: tool `{tool}` mutates workspace state and is disabled; restart the server without --read-only to enable it"
    )
}

#[derive(Error, Debug)]
pub enum InterfaceError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("security: {0}")]
    Security(#[from] SecurityError),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("internal: {0}")]
    Internal(String),

    /// The name is carried by more than one symbol, and the caller has to
    /// say which one it means. The candidates are data: a client that wants
    /// to ask again picks one of them.
    ///
    /// The sentence keeps the words it had when the refusal was an
    /// [`InterfaceError::InvalidInput`]: it is what the consumers of the
    /// text channel read.
    #[error("invalid input: ambiguous symbol '{name}': {}", .candidates.join(", "))]
    Ambiguous {
        name: String,
        candidates: Vec<String>,
    },

    /// The capability behind the tool is not configured in this build or
    /// session. The call is well-formed; the product cannot serve it.
    #[error("internal: {0}")]
    NotConfigured(String),

    /// The server runs read-only and the tool mutates workspace state.
    #[error("internal: {}", read_only_detail(.tool))]
    ReadOnly { tool: String },

    /// The call did not finish inside the budget of its category.
    #[error("internal: {}", TIMEOUT_DETAIL)]
    Timeout,

    /// The call was refused by the rate limiter before it ran.
    #[error("internal: {}", RATE_LIMITED_DETAIL)]
    RateLimited,

    /// Catch-all for domain and application errors, preserving the causal chain
    #[error("domain: {0}")]
    Domain(#[source] crate::domain::error::DomainError),

    /// Application-level errors that don't map to domain errors
    #[error("application: {0}")]
    Application(#[source] crate::application::error::AppError),
}

/// What kind of failure an [`InterfaceError`] is.
///
/// The one answer to "what happened", decided by the type and never by the
/// words of the message: a message that happens to say `timeout` is not one.
/// Every consumer that sorts failures (the status label, the metric, the
/// wire) reads this and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    ToolNotFound,
    InvalidInput,
    Ambiguous,
    Security,
    NotFound,
    Internal,
    NotConfigured,
    ReadOnly,
    Timeout,
    RateLimited,
    Domain,
    Application,
}

impl InterfaceError {
    /// The kind of this failure. Total: a new variant does not compile until
    /// it says which kind it is.
    pub fn kind(&self) -> ErrorKind {
        match self {
            InterfaceError::ToolNotFound(_) => ErrorKind::ToolNotFound,
            InterfaceError::InvalidInput(_) => ErrorKind::InvalidInput,
            InterfaceError::Ambiguous { .. } => ErrorKind::Ambiguous,
            InterfaceError::Security(_) => ErrorKind::Security,
            InterfaceError::NotFound(_) => ErrorKind::NotFound,
            InterfaceError::Internal(_) => ErrorKind::Internal,
            InterfaceError::NotConfigured(_) => ErrorKind::NotConfigured,
            InterfaceError::ReadOnly { .. } => ErrorKind::ReadOnly,
            InterfaceError::Timeout => ErrorKind::Timeout,
            InterfaceError::RateLimited => ErrorKind::RateLimited,
            InterfaceError::Domain(_) => ErrorKind::Domain,
            InterfaceError::Application(_) => ErrorKind::Application,
        }
    }
}

impl From<crate::domain::error::DomainError> for InterfaceError {
    fn from(err: crate::domain::error::DomainError) -> Self {
        match err {
            crate::domain::error::DomainError::SymbolNotFound(s) => {
                InterfaceError::NotFound(format!("symbol not found: {}", s))
            }
            crate::domain::error::DomainError::InvalidSymbolKind(s) => {
                InterfaceError::InvalidInput(format!("invalid symbol kind: {}", s))
            }
            crate::domain::error::DomainError::CycleDetected => {
                InterfaceError::InvalidInput("cycle detected in dependency graph".into())
            }
            crate::domain::error::DomainError::LocationOutOfBounds(s) => {
                InterfaceError::InvalidInput(format!("location out of bounds: {}", s))
            }
            crate::domain::error::DomainError::ParseError(s) => {
                InterfaceError::InvalidInput(format!("parse error: {}", s))
            }
            crate::domain::error::DomainError::AnalysisError(_) => InterfaceError::Domain(err),
            crate::domain::error::DomainError::RefactoringError(_) => InterfaceError::Domain(err),
            crate::domain::error::DomainError::InvalidRange(s) => {
                InterfaceError::InvalidInput(format!("invalid range: {}", s))
            }
        }
    }
}

impl From<crate::application::error::AppError> for InterfaceError {
    fn from(err: crate::application::error::AppError) -> Self {
        match err {
            crate::application::error::AppError::SymbolNotFound(s) => {
                InterfaceError::NotFound(format!("symbol not found: {}", s))
            }
            crate::application::error::AppError::AmbiguousSymbol { name, candidates } => {
                InterfaceError::Ambiguous { name, candidates }
            }
            crate::application::error::AppError::LocationNotFound(s) => {
                InterfaceError::NotFound(format!("location not found: {}", s))
            }
            crate::application::error::AppError::NavigationError(s) => {
                InterfaceError::InvalidInput(format!("navigation error: {}", s))
            }
            crate::application::error::AppError::AnalysisError(_) => {
                InterfaceError::Application(err)
            }
            crate::application::error::AppError::CycleDetected => {
                InterfaceError::InvalidInput("cycle detected in dependency graph".into())
            }
            crate::application::error::AppError::InvalidCallGraph(s) => {
                InterfaceError::InvalidInput(format!("invalid call graph: {}", s))
            }
            crate::application::error::AppError::RefactorError(_) => {
                InterfaceError::Application(err)
            }
            crate::application::error::AppError::InvalidRefactorAction(s) => {
                InterfaceError::InvalidInput(format!("invalid refactor action: {}", s))
            }
            crate::application::error::AppError::RefactorTargetNotFound(s) => {
                InterfaceError::NotFound(format!("refactor target not found: {}", s))
            }
            crate::application::error::AppError::StrategyNotFound(s) => {
                InterfaceError::NotFound(format!("strategy not found: {}", s))
            }
            crate::application::error::AppError::SafetyCheckFailed(s) => {
                InterfaceError::InvalidInput(format!("safety check failed: {}", s))
            }
            crate::application::error::AppError::SearchError(s) => {
                InterfaceError::InvalidInput(format!("search error: {}", s))
            }
            crate::application::error::AppError::InspectionError(s) => {
                InterfaceError::InvalidInput(format!("inspection error: {}", s))
            }
            crate::application::error::AppError::RepositoryError(s) => {
                InterfaceError::Internal(format!("repository error: {}", s))
            }
            crate::application::error::AppError::InvalidParameter(s) => {
                InterfaceError::InvalidInput(format!("invalid parameter: {}", s))
            }
            crate::application::error::AppError::InternalError(s) => InterfaceError::Internal(s),
            crate::application::error::AppError::NotImplemented(s) => {
                InterfaceError::Internal(format!("not implemented: {}", s))
            }
        }
    }
}

// From<SecurityError> for InterfaceError is auto-generated by #[from] SecurityError

impl From<crate::domain::traits::CodeIntelligenceError> for InterfaceError {
    fn from(err: crate::domain::traits::CodeIntelligenceError) -> Self {
        InterfaceError::InvalidInput(format!("navigation error: {}", err))
    }
}

impl From<crate::domain::traits::RefactorError> for InterfaceError {
    fn from(err: crate::domain::traits::RefactorError) -> Self {
        InterfaceError::Application(crate::application::error::AppError::RefactorError(
            err.to_string(),
        ))
    }
}

impl From<crate::domain::traits::DependencyError> for InterfaceError {
    fn from(err: crate::domain::traits::DependencyError) -> Self {
        InterfaceError::Internal(format!("dependency error: {}", err))
    }
}

impl From<crate::domain::traits::SearchError> for InterfaceError {
    fn from(err: crate::domain::traits::SearchError) -> Self {
        InterfaceError::InvalidInput(format!("search error: {}", err))
    }
}

// Note: From<SecurityError> for InterfaceError is auto-generated by #[from] SecurityError

impl From<serde_json::Error> for InterfaceError {
    fn from(err: serde_json::Error) -> Self {
        InterfaceError::InvalidInput(format!("JSON error: {}", err))
    }
}

impl From<HandlerError> for InterfaceError {
    fn from(err: HandlerError) -> Self {
        // HandlerError is a wrapper enum that can contain InterfaceError-convertible types
        use crate::interface::mcp::handlers::HandlerError as HE;
        match err {
            HE::Security(e) => InterfaceError::Security(e),
            HE::App(e) => InterfaceError::Application(e),
            HE::Domain(e) => InterfaceError::Domain(e),
            HE::InvalidInput(s) => InterfaceError::InvalidInput(s),
            HE::NotFound(s) => InterfaceError::NotFound(s),
            HE::Ambiguous { name, candidates } => InterfaceError::Ambiguous { name, candidates },
            HE::NotConfigured(s) => InterfaceError::NotConfigured(s),
            HE::Internal(s) => InterfaceError::Internal(s),
        }
    }
}

/// The way back: the handler layer speaks `HandlerError`, and a failure that
/// began on the client route (a refusal of the server, a capability that is
/// not configured) crosses to it keeping its meaning.
///
/// Both directions live in this module so a new variant is decided once, next
/// to its inverse.
impl From<InterfaceError> for HandlerError {
    fn from(err: InterfaceError) -> Self {
        match err {
            InterfaceError::ToolNotFound(s) => {
                HandlerError::Internal(format!("tool not found: {}", s))
            }
            InterfaceError::InvalidInput(s) => HandlerError::InvalidInput(s),
            InterfaceError::Security(e) => HandlerError::Security(e),
            InterfaceError::NotFound(s) => HandlerError::NotFound(s),
            InterfaceError::Internal(s) => HandlerError::Internal(s),
            InterfaceError::Ambiguous { name, candidates } => {
                HandlerError::Ambiguous { name, candidates }
            }
            InterfaceError::NotConfigured(s) => HandlerError::NotConfigured(s),
            InterfaceError::ReadOnly { tool } => HandlerError::Internal(read_only_detail(&tool)),
            InterfaceError::Timeout => HandlerError::Internal(TIMEOUT_DETAIL.to_string()),
            InterfaceError::RateLimited => HandlerError::Internal(RATE_LIMITED_DETAIL.to_string()),
            InterfaceError::Domain(e) => HandlerError::Domain(e),
            InterfaceError::Application(e) => HandlerError::App(e),
        }
    }
}

/// Result type for interface operations
pub type InterfaceResult<T> = Result<T, InterfaceError>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::error::AppError;

    fn two_homonyms() -> (String, Vec<String>) {
        (
            "helper".to_string(),
            vec![
                "src/a.rs:helper:1".to_string(),
                "src/b.rs:helper:1".to_string(),
            ],
        )
    }

    #[test]
    fn an_ambiguous_name_keeps_its_candidates_as_data() {
        let (name, candidates) = two_homonyms();
        let error = InterfaceError::from(AppError::AmbiguousSymbol {
            name: name.clone(),
            candidates: candidates.clone(),
        });

        match &error {
            InterfaceError::Ambiguous {
                name: n,
                candidates: c,
            } => {
                assert_eq!(n, &name);
                assert_eq!(c, &candidates);
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
        assert_eq!(error.kind(), ErrorKind::Ambiguous);
    }

    #[test]
    fn the_ambiguity_survives_the_round_trip_through_the_handler_error() {
        let (name, candidates) = two_homonyms();
        let handler = HandlerError::Ambiguous {
            name: name.clone(),
            candidates: candidates.clone(),
        };

        let error = InterfaceError::from(handler);
        assert_eq!(error.kind(), ErrorKind::Ambiguous);

        // And back: the handler route and the client route say the same thing.
        match HandlerError::from(error) {
            HandlerError::Ambiguous {
                name: n,
                candidates: c,
            } => {
                assert_eq!(n, name);
                assert_eq!(c, candidates);
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn the_sentence_a_client_already_reads_did_not_change() {
        let (name, candidates) = two_homonyms();
        let error = InterfaceError::Ambiguous { name, candidates };

        assert_eq!(
            error.to_string(),
            "invalid input: ambiguous symbol 'helper': src/a.rs:helper:1, src/b.rs:helper:1"
        );
    }

    #[test]
    fn the_server_refusals_say_what_they_always_said() {
        assert_eq!(InterfaceError::Timeout.to_string(), "internal: timeout");
        assert_eq!(
            InterfaceError::RateLimited.to_string(),
            "internal: rate_limit_exceeded"
        );
        assert_eq!(
            InterfaceError::ReadOnly {
                tool: "write_file".to_string()
            }
            .to_string(),
            "internal: read_only_mode: tool `write_file` mutates workspace state and is \
             disabled; restart the server without --read-only to enable it"
        );
        assert_eq!(
            InterfaceError::NotConfigured("generate_contract requires a persistence layer".into())
                .to_string(),
            "internal: generate_contract requires a persistence layer"
        );
    }

    #[test]
    fn a_message_that_spells_a_sentinel_is_not_one() {
        // The kind is the type. Words in a message decide nothing.
        for text in [
            "timeout",
            "rate_limit_exceeded",
            "read_only_mode",
            "not configured",
        ] {
            assert_eq!(
                InterfaceError::Internal(text.to_string()).kind(),
                ErrorKind::Internal,
                "{text}"
            );
            assert_eq!(
                InterfaceError::InvalidInput(text.to_string()).kind(),
                ErrorKind::InvalidInput,
                "{text}"
            );
        }
    }

    #[test]
    fn every_typed_refusal_has_its_own_kind() {
        let kinds = [
            InterfaceError::Timeout.kind(),
            InterfaceError::RateLimited.kind(),
            InterfaceError::ReadOnly {
                tool: "t".to_string(),
            }
            .kind(),
            InterfaceError::NotConfigured("x".to_string()).kind(),
            InterfaceError::ToolNotFound("t".to_string()).kind(),
            InterfaceError::NotFound("x".to_string()).kind(),
        ];
        for (i, a) in kinds.iter().enumerate() {
            for b in &kinds[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn a_capability_that_is_not_configured_keeps_the_internal_code_on_the_wire() {
        let mcp: crate::interface::mcp::schemas::McpError =
            HandlerError::from(InterfaceError::NotConfigured("no store".to_string())).into();
        assert_eq!(mcp.code, -32005);
        assert!(mcp.data.is_none());
    }

    #[test]
    fn the_server_refusals_cross_to_the_handler_error_with_their_words() {
        for (error, words) in [
            (InterfaceError::Timeout, "timeout"),
            (InterfaceError::RateLimited, "rate_limit_exceeded"),
        ] {
            match HandlerError::from(error) {
                HandlerError::Internal(text) => assert_eq!(text, words),
                other => panic!("expected Internal, got {other:?}"),
            }
        }
    }
}
