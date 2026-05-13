//! # Model Module
//!
//! C4 Model types and workspace representation — agnostic to output format.
//!
//! ## Submodules
//!
//! - **c4_types** — Core C4 model types: Person, SoftwareSystem, Container, Component, CodeElement
//! - **deployment** — Deployment diagram model: nodes, relationships, networks, volumes
//! - **er_types** — Entity-Relationship diagram model: entities, columns, relationships
//! - **sequence_types** — Sequence diagram model: participants, messages, lifelines
//! - **state_machine_types** — State Machine diagram model: states, transitions
//! - **relationships** — C4 relationship types and kinds
//! - **workspace** — C4Workspace container for all model elements
//! - **views** — View definitions for filtered diagram perspectives
//! - **styles** — Visual styling options for diagrams
//!
//! ## C4 Model Levels
//!
//! | Level | Name | Description |
//! |-------|------|-------------|
//! | L1 | Context | People and software systems |
//! | L2 | Container | Applications, databases, services |
//! | L3 | Component | Code components within a container |
//! | L4 | Code | Actual code structures (classes, functions) |
//!
//! ## Diagram Types
//!
//! | Type | Description |
//! |-------|-------------|
//! | C4 | Standard C4 Model diagrams |
//! | Sequence | UML Sequence diagrams from call graphs |
//! | State Machine | UML State Machine diagrams |
//! | Activity | UML Activity diagrams |
//! | Deployment | Deployment infrastructure diagrams |
//! | ER | Entity-Relationship diagrams |

pub mod activity_types;
pub mod c4_types;
pub mod deployment;
pub mod er_types;
pub mod sequence_types;
pub mod state_machine_types;
pub mod relationships;
pub mod workspace;
pub mod views;
pub mod styles;

pub use activity_types::*;
pub use c4_types::*;
pub use deployment::*;
pub use er_types::*;
pub use sequence_types::*;
pub use state_machine_types::*;
pub use relationships::*;
pub use workspace::C4Workspace;
