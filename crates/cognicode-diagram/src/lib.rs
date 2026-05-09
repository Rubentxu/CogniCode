//! CogniCode Diagram — Inferred diagramming and C4 Model reverse engineering
//!
//! Generates C4 Model diagrams (Context, Container, Component, Code),
//! UML class diagrams, and architecture visualizations from code analysis.
//!
//! # Architecture
//!
//! The crate is organized in three phases:
//! 1. **Inference** — extracts C4 model elements from `CallGraph` and project config
//! 2. **Layout** — computes node positions using Sugiyama algorithm with port extensions
//! 3. **Render** — outputs to Mermaid, PlantUML, Structurizr DSL, or SVG
//!
//! # Example
//!
//! ```ignore
//! use cognicode_diagram::inference::InferenceEngine;
//! use cognicode_core::domain::aggregates::call_graph::CallGraph;
//!
//! let engine = InferenceEngine::new(&call_graph);
//! let elements = engine.infer_code_elements("src/domain", 2);
//! let mermaid = cognicode_diagram::render::render_class_diagram(&elements);
//! ```

pub mod model;
pub mod inference;
pub mod render;

// Layout module will be enabled in Phase 4
// pub mod layout;

// MCP integration handlers — used by cognicode-mcp to register tools
pub mod mcp;

pub use model::c4_types::{
    C4Element, Person, SoftwareSystem, Container, ContainerType,
    Component, ComponentType, CodeElement, CodeElementKind,
    ElementId, ElementLocation, Visibility,
};
pub use model::relationships::{C4Relationship, C4RelationshipKind};
pub use model::workspace::C4Workspace;
pub use inference::engine::InferenceEngine;
