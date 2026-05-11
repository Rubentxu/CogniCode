//! # MCP Module
//!
//! Model Context Protocol (MCP) tool handlers for integration with cognicode-mcp.
//!
//! This module exposes diagram generation as MCP tools that can be invoked
//! by AI agents and editors that support the MCP protocol.
//!
//! ## Available Tools
//!
//! - **generate_c4_code** — Generate C4 L4 code component diagrams
//! - **generate_c4_containers** — Generate C4 L2 container diagrams
//! - **generate_c4_components** — Generate C4 L3 component diagrams
//!
//! ## Tool Registration
//!
//! Tools are registered with the MCP server via the `handle_*` functions.
//! Each tool has input/output types defined for JSON serialization.
//!
//! ## Usage
//!
//! ```ignore
//! use cognicode_diagram::mcp::{
//!     handle_generate_c4_code, GenerateC4CodeInput, GenerateC4CodeOutput
//! };
//!
//! let input = GenerateC4CodeInput {
//!     project_path: "/path/to/project".to_string(),
//!     scope: "src/domain".to_string(),
//!     max_depth: 2,
//! };
//! let output = handle_generate_c4_code(input);
//! ```

pub mod tools;

pub use tools::{
    GenerateC4CodeInput, GenerateC4CodeOutput, handle_generate_c4_code,
    GenerateC4ContainersInput, GenerateC4ContainersOutput, handle_generate_c4_containers,
    GenerateC4ComponentsInput, GenerateC4ComponentsOutput, handle_generate_c4_components,
};
