//! # cognicode-diagram
//!
//! Inferred diagramming and C4 Model reverse engineering for CogniCode.
//!
//! ## Features
//! - **C4 Model**: L1-L4 inference and rendering (Context, Container, Component, Code)
//! - **Deployment Diagrams**: Dockerfile/docker-compose inference
//! - **ER Diagrams**: SQL schema inference
//! - **Multiple Formats**: Mermaid, PlantUML, D2, SVG, Structurizr DSL
//! - **Layout Engine**: Sugiyama hierarchical layout algorithm
//!
//! ## Architecture
//!
//! The crate is organized in four main modules:
//!
//! 1. **model** — C4 model types, deployment model, ER model, workspace
//! 2. **inference** — extracts model elements from code analysis, configs, schemas
//! 3. **layout** — computes node positions using Sugiyama algorithm
//! 4. **render** — outputs diagrams in various formats (Mermaid, PlantUML, D2, SVG)
//!
//! ## Quick Start
//!
//! ```ignore
//! use cognicode_diagram::model::workspace::C4Workspace;
//! use cognicode_diagram::render::d2::{render_d2, D2Options};
//!
//! let workspace = C4Workspace::new("MySystem");
//! let d2_source = render_d2(&workspace, &D2Options::default());
//! ```

pub mod model;
pub mod inference;
pub mod render;
pub mod layout;

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
