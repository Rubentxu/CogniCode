//! Tool handler registry and trait definitions.
//!
//! This module defines the core abstractions for the ISP-segregated
//! tool handler architecture:
//!
//! - [`ToolHandler`] — the trait that each tool family implements
//! - [`ToolHandlerRegistry`] — the runtime registry that dispatches
//!   tools by name
//!
//! # Registration
//!
//! Each handler family (e.g. `sessions`, `views`, `ask`) lives in its
//! own module under [`super::handler`]. The registry is populated
//! at construction time using `OnceLock` for explicit, debuggable wiring.

use std::collections::HashMap;

// Declare handler submodules. Each file implements one tool family.
mod ask;
mod drift;
mod graph;
mod graph_analyze;
mod impact;
mod ingest;
mod named_views;
mod search;
mod sessions;
mod views;
mod workspace;

use async_trait::async_trait;
use rmcp::model::CallToolResult;
use serde_json::Value;

pub use super::context::McpContext;
pub use super::error::ToolError;

/// Abstract interface for a single MCP tool handler.
///
/// Implement this trait to add a new tool to the registry without
/// modifying the central dispatch match statement (Open/Closed Principle).
///
/// # Implementor notes
///
/// - `name()` must return a stable identifier that matches the tool
///   name used in `tools/list` and `tools/call` JSON-RPC requests.
/// - `arg_schema()` must return a valid JSON Schema `object` describing
///   the accepted parameters.
/// - `handle()` receives the shared [`McpContext`] and the raw
///   deserialised arguments. Return [`CallToolResult`] directly —
///   errors are converted to the structured envelope by the caller.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// Returns the tool's canonical name (e.g. `"brain_open"`).
    fn name(&self) -> &'static str;

    /// Returns a JSON Schema describing the tool's input parameters.
    fn arg_schema(&self) -> Value;

    /// Handle a `tools/call` invocation for this tool.
    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult;
}

/// In-memory registry of all registered tool handlers.
///
/// The registry maps tool names to boxed trait objects. Dispatch is a
/// single HashMap lookup — O(1) with no match arms.
///
/// # Construction
///
/// Callers build a registry by calling [`register`](Self::register) for
/// each handler family. The registry is typically built once at
/// server startup and shared as an `Arc<Self>` across all requests.
#[derive(Default)]
pub struct ToolHandlerRegistry {
    handlers: HashMap<&'static str, Box<dyn ToolHandler>>,
}

impl ToolHandlerRegistry {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler. Panics if a handler with the same name is
    /// already registered (duplicate registration is a programming bug,
    /// not a runtime error).
    pub fn register<H: ToolHandler + 'static>(&mut self, handler: H) {
        let name = handler.name();
        // `Into<Box<dyn ToolHandler>>` is implemented for all `T: ToolHandler`.
        let boxed: Box<dyn ToolHandler> = Box::new(handler);
        let old = self.handlers.insert(name, boxed);
        assert!(
            old.is_none(),
            "duplicate tool handler registration for `{name}`"
        );
    }

    /// Look up a handler by name. Returns `None` if no handler with
    /// that name is registered.
    pub fn get(&self, name: &str) -> Option<&dyn ToolHandler> {
        self.handlers.get(name).map(|b| b.as_ref())
    }

    /// List all registered handlers in registration order.
    pub fn list(&self) -> Vec<&dyn ToolHandler> {
        self.handlers.values().map(|b| b.as_ref()).collect()
    }

    /// Dispatch a tool call by name.
    ///
    /// Returns the result of calling the handler's `handle` method, or
    /// a structured "Unknown tool" error if no handler is registered.
    ///
    /// # Errors
    ///
    /// Returns a `CallToolResult::error` when:
    /// - No handler is registered for the given name (`UnknownTool`)
    /// - The handler itself returns an error (any [`ToolError`] variant)
    pub async fn dispatch(&self, name: &str, ctx: &McpContext, params: Value) -> CallToolResult {
        match self.get(name) {
            Some(handler) => handler.handle(ctx, params).await,
            None => unknown_tool_error(name),
        }
    }
}

/// Build a "Unknown tool" error result. Mirrors the existing
/// `_ => err(format!("Unknown tool: {name}"))` pattern in `mcp.rs`.
fn unknown_tool_error(name: &str) -> CallToolResult {
    use rmcp::model::Content;
    let msg = format!("Unknown tool: {name}");
    CallToolResult::error(vec![Content::text(msg)])
}

// Re-export registration functions from submodules for ergonomic external use.
pub use ask::register_ask_handlers;
pub use drift::register_drift_handlers;
pub use graph::register_graph_handlers;
pub use graph_analyze::register_graph_analyze_handlers;
pub use impact::register_impact_handlers;
pub use ingest::register_ingest_handlers;
pub use named_views::register_named_views_handlers;
pub use search::register_search_handlers;
pub use sessions::register_session_handlers;
pub use views::register_view_handlers;
pub use workspace::register_workspace_handlers;
