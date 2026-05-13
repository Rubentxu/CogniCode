//! # Render Module
//!
//! Diagram renderers producing output in various formats.
//!
//! ## Supported Formats
//!
//! | Format | Description | Output Type |
//! |--------|-------------|-------------|
//! | Mermaid | Standard Mermaid flowchart syntax | Text |
//! | Mermaid C4 | Mermaid with C4-specific macros | Text |
//! | PlantUML | C4-PlantUML stdlib macros | Text |
//! | D2 | D2 diagram language | Text |
//! | SVG | Scalable Vector Graphics | XML |
//! | Structurizr DSL | Structurizr JSON DSL | Text |
//!
//! ## Rendering C4 Diagrams
//!
//! ```ignore
//! use cognicode_diagram::model::workspace::C4Workspace;
//! use cognicode_diagram::render::d2::{render_d2, D2Options};
//!
//! let workspace = C4Workspace::new("MySystem");
//! let d2 = render_d2(&workspace, &D2Options::default());
//! ```
//!
//! ## Rendering Deployment Diagrams
//!
//! ```ignore
//! use cognicode_diagram::model::deployment::DeploymentModel;
//! use cognicode_diagram::render::deployment::render_deployment_mermaid;
//!
//! let model = DeploymentModel::empty();
//! let mermaid = render_deployment_mermaid(&model);
//! ```
//!
//! ## Rendering ER Diagrams
//!
//! ```ignore
//! use cognicode_diagram::model::er_types::ErModel;
//! use cognicode_diagram::render::er::render_er_mermaid;
//!
//! let model = ErModel::default();
//! let mermaid = render_er_mermaid(&model);
//! ```

pub mod activity;
pub mod d2;
pub mod deployment;
pub mod er;
pub mod mermaid;
pub mod mermaid_c4;
pub mod plantuml;
pub mod sequence;
pub mod state_machine;
pub mod structurizr_dsl;
pub mod svg;

pub use activity::{
    render_activity_mermaid, render_activity_plantuml, render_empty_activity,
    ActivityRenderOptions,
};
pub use d2::{render_d2, D2Options, D2Theme, D2Direction};
pub use deployment::{render_deployment_mermaid, render_deployment_d2};
pub use er::{render_er_mermaid, render_er_d2};
pub use mermaid::{render_class_diagram, MermaidOptions};
pub use mermaid_c4::{render_component_diagram, render_container_diagram, C4MermaidOptions};
pub use plantuml::{render_plantuml_c4, PlantUmlOptions, PlantUmlViewType};
pub use sequence::{
    render_sequence_diagram, render_sequence_diagram_plantuml, render_sequence_diagram_svg,
    find_entry_points, SequenceDiagramOptions, SequenceSvgOptions,
};
pub use state_machine::{
    render_state_machine_mermaid, render_state_machine_plantuml,
    render_empty_state_machine, StateMachineRenderOptions,
};
pub use structurizr_dsl::{render_structurizr_dsl, StructurizrDslOptions};
pub use svg::{render_svg, render_svg_to_file, SvgTheme};
