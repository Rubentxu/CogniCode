//! Renderers — output C4 diagrams in various formats

pub mod mermaid;
pub mod mermaid_c4;
pub mod plantuml;
pub mod sequence;
pub mod structurizr_dsl;

pub use mermaid::{render_class_diagram, MermaidOptions};
pub use mermaid_c4::{render_component_diagram, render_container_diagram, C4MermaidOptions};
pub use plantuml::{render_plantuml_c4, PlantUmlOptions, PlantUmlViewType};
pub use sequence::{render_sequence_diagram, find_entry_points, SequenceDiagramOptions};
pub use structurizr_dsl::{render_structurizr_dsl, StructurizrDslOptions};
