//! Renderers — output C4 diagrams in various formats

pub mod mermaid;

pub use mermaid::{render_class_diagram, MermaidOptions};
