//! MCP tool handler subsystem.
//!
//! This module provides the ISP-segregated tool handler architecture
//! that replaces the monolithic 34-arm `dispatch()` match in `explorer.rs`.
//!
//! # Module layout
//!
//! - [`explorer`] — [`ExplorerMcpHandler`] and the original dispatch logic
//! - [`context`]  — [`McpContext`] — shared execution context passed to handlers
//! - [`error`]    — [`ToolError`] — structured error types for handler failures
//! - [`handler`]  — [`ToolHandler`](handler::ToolHandler) trait + [`ToolHandlerRegistry`](handler::ToolHandlerRegistry)
//! - [`handler::sessions`] — session-family handlers (9 tools)

pub mod envelope;
pub mod explorer;
pub mod context;
pub mod error;
pub mod handler;

// Re-export all public names from explorer.rs so `lib.rs` can continue
// using `pub use mcp::ExplorerMcpHandler` etc. without changes.
pub use explorer::{
    ExplorerMcpHandler, McpResultEnvelope, ProvenanceMetadata, FollowUp,
    EnvelopeError,
    TOOL_OPEN_WORKSPACE, TOOL_SPOTTER_SEARCH, TOOL_INSPECT_OBJECT,
    TOOL_GET_VIEWS, TOOL_GET_VIEW, TOOL_GET_LENSES, TOOL_APPLY_LENS, TOOL_QUERY_MOLDQL,
    TOOL_IMPACT_RADIUS, TOOL_IMPACT_FORWARD_RADIUS, TOOL_IMPACT_HAS_PATH,
    TOOL_IMPACT_SHORTEST_PATH, TOOL_IMPACT_DETECT_CYCLES, TOOL_IMPACT_COMPONENT,
    TOOL_GRAPH_SUBGRAPH, TOOL_GRAPH_CLUSTER, TOOL_GRAPH_EXPLAIN,
    TOOL_ASK,
    TOOL_BRAIN_OPEN, TOOL_BRAIN_ATTACH, TOOL_BRAIN_ASK,
    TOOL_BRAIN_FOCUS, TOOL_BRAIN_STATUS, TOOL_BRAIN_CLOSE,
    TOOL_VIEW_SAVE, TOOL_VIEW_LOAD, TOOL_VIEW_LIST, TOOL_VIEW_DELETE,
    TOOL_NAMES, tool_names,
    DEFAULT_IMPACT_RADIUS_DEPTH, DEFAULT_SUBGRAPH_DEPTH,
};

#[cfg(feature = "multimodal")]
pub use explorer::{
    TOOL_BRAIN_ADD_SPACE, TOOL_BRAIN_REMOVE_SPACE, TOOL_BRAIN_SPACES,
    TOOL_DOCS_INGEST, TOOL_GRAPH_SEARCH, TOOL_ISSUES_INGEST,
    DEFAULT_GRAPH_SEARCH_LIMIT, MAX_GRAPH_SEARCH_LIMIT,
};

// Re-export key types for convenience.
pub use context::McpContext;
pub use error::ToolError;
pub use handler::{ToolHandler, ToolHandlerRegistry};
