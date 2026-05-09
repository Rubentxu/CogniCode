//! Renderers — output C4 diagrams in various formats

pub mod mermaid;
pub mod mermaid_c4;

pub use mermaid::{render_class_diagram, MermaidOptions};
pub use mermaid_c4::{render_component_diagram, render_container_diagram, C4MermaidOptions};
