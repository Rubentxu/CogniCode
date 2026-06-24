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

pub mod context;
pub mod envelope;
pub mod error;
pub mod explorer;
pub mod handler;

// Re-export all public names from explorer.rs so `lib.rs` can continue
// using `pub use mcp::ExplorerMcpHandler` etc. without changes.
pub use explorer::{
    DEFAULT_IMPACT_RADIUS_DEPTH, DEFAULT_SUBGRAPH_DEPTH, EnvelopeError, ExplorerMcpHandler,
    FollowUp, McpResultEnvelope, ProvenanceMetadata, TOOL_APPLY_LENS, TOOL_ASK, TOOL_BRAIN_ASK,
    TOOL_BRAIN_ATTACH, TOOL_BRAIN_CLOSE, TOOL_BRAIN_FOCUS, TOOL_BRAIN_OPEN, TOOL_BRAIN_STATUS,
    TOOL_DETECT_ARCHITECTURE_DRIFT, TOOL_FIND_CYCLES, TOOL_FIND_DEAD_CODE, TOOL_FIND_DEAD_CODE_V2,
    TOOL_FIND_INTERSECTION, TOOL_GET_LENSES, TOOL_GET_VIEW, TOOL_GET_VIEWS,
    TOOL_GRAPH_ALL_SIMPLE_PATHS, TOOL_GRAPH_CLUSTER, TOOL_GRAPH_COMMUNITIES,
    TOOL_GRAPH_COMMUNITY_GOD_NODES, TOOL_GRAPH_EXPLAIN, TOOL_GRAPH_FEEDBACK_ARC_SET,
    TOOL_GRAPH_GOD_NODES, TOOL_GRAPH_PAGERANK, TOOL_GRAPH_SUBGRAPH,
    TOOL_GRAPH_SURPRISING_CONNECTIONS, TOOL_GRAPH_TRANSITIVE_REDUCTION, TOOL_HEALTH_DASHBOARD,
    TOOL_HOTSPOTS, TOOL_IMPACT_COMPONENT, TOOL_IMPACT_DETECT_CYCLES, TOOL_IMPACT_FORWARD_RADIUS,
    TOOL_IMPACT_HAS_PATH, TOOL_IMPACT_RADIUS, TOOL_IMPACT_SHORTEST_PATH, TOOL_INSPECT_OBJECT,
    TOOL_NAMES, TOOL_OPEN_WORKSPACE, TOOL_QUERY_MOLDQL, TOOL_SPOTTER_SEARCH, TOOL_VIEW_DELETE,
    TOOL_VIEW_LIST, TOOL_VIEW_LOAD, TOOL_VIEW_SAVE, tool_names,
};

#[cfg(feature = "multimodal")]
pub use explorer::{
    DEFAULT_GRAPH_SEARCH_LIMIT, MAX_GRAPH_SEARCH_LIMIT, TOOL_BRAIN_ADD_SPACE,
    TOOL_BRAIN_REMOVE_SPACE, TOOL_BRAIN_SPACES, TOOL_DOCS_INGEST, TOOL_GRAPH_SEARCH,
    TOOL_ISSUES_INGEST,
};

// Re-export key types for convenience.
pub use context::McpContext;
pub use error::ToolError;
pub use handler::{ToolHandler, ToolHandlerRegistry};
