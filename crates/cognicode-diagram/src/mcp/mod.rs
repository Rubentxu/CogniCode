//! MCP tool integration — handlers for cognicode-mcp

pub mod tools;

pub use tools::{
    GenerateC4CodeInput, GenerateC4CodeOutput, handle_generate_c4_code,
    GenerateC4ContainersInput, GenerateC4ContainersOutput, handle_generate_c4_containers,
    GenerateC4ComponentsInput, GenerateC4ComponentsOutput, handle_generate_c4_components,
};
