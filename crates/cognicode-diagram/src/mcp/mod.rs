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
    GenerateC4DynamicInput, GenerateC4DynamicOutput, handle_generate_c4_dynamic,
    GenerateSequenceDiagramInput, GenerateSequenceDiagramOutput, handle_generate_sequence_diagram,
    GenerateC4DeploymentInput, GenerateC4DeploymentOutput, handle_generate_c4_deployment,
    GenerateErDiagramInput, GenerateErDiagramOutput, handle_generate_er_diagram,
    GenerateStateMachineInput, GenerateStateMachineOutput, handle_generate_state_machine,
    GenerateActivityDiagramInput, GenerateActivityDiagramOutput, handle_generate_activity_diagram,
    GenerateMultiLangWorkspaceInput, GenerateMultiLangWorkspaceOutput, handle_generate_multi_lang_workspace,
    SummarizeDiagramInput, SummarizeDiagramOutput, handle_summarize_diagram,
    DiffDiagramsInput, DiffDiagramsOutput, handle_diff_diagrams,
};
