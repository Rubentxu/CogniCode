//! `LlmPort` — provider-neutral inference capability.
//!
//! The port is **not** a conversation surface. A conversation surface
//! (Explorer, MCP, CLI) may consume the port, but the port itself
//! is one bounded inference call: a request in, a response out.
//!
//! ## No live external provider is required
//!
//! The default in-memory adapter (`FakeLlmPort`, in
//! `application::ai::fake`) is what the acceptance suite uses. Real
//! provider adapters (OpenAI, Anthropic, Ollama, …) are future work
//! and are gated by DEBT-SDDK-003 (provider/worker outage). They are
//! out of scope for e79's deterministic closure.

use std::fmt;

use crate::domain::ai::frame::InvestigationFrame;
use crate::domain::ai::request::InvestigationRequest;
use crate::domain::ai::response::LlmResponse;

/// Errors that the port may surface.
///
/// These are port-contract errors, not provider errors. A real
/// provider adapter will translate HTTP / SDK errors into these
/// variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmPortError {
    /// The port refused the request because it exceeded the frame's
    /// budget (e.g. instruction too long, too many tools).
    BudgetExceeded {
        /// What exceeded the budget.
        kind: &'static str,
        /// The observed value.
        observed: u64,
        /// The budget limit.
        limit: u64,
    },
    /// The port refused the request because the tool surface is
    /// inconsistent (e.g. duplicate tool names).
    InvalidToolSurface(&'static str),
    /// The port cannot produce a response for the given frame (e.g.
    /// the deterministic fake has no scripted response for this
    /// frame).
    NoResponseForFrame,
    /// Generic port-level failure.
    Other(String),
}

impl fmt::Display for LlmPortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BudgetExceeded { kind, observed, limit } => {
                write!(f, "budget exceeded for {kind}: observed={observed}, limit={limit}")
            }
            Self::InvalidToolSurface(s) => write!(f, "invalid tool surface: {s}"),
            Self::NoResponseForFrame => f.write_str("no scripted response for this frame"),
            Self::Other(s) => write!(f, "port error: {s}"),
        }
    }
}

impl std::error::Error for LlmPortError {}

/// Provider-neutral inference capability.
pub trait LlmPort {
    /// Submit one bounded request, get one bounded response.
    ///
    /// Implementations MUST:
    /// 1. Echo the request's `frame_id` and `provenance` verbatim in
    ///    the response.
    /// 2. Honour the frame's budget.
    /// 3. Produce a response whose `observed_read_set` is auditable.
    fn complete(&self, request: &InvestigationRequest, frame: &InvestigationFrame) -> Result<LlmResponse, LlmPortError>;
}
