//! # Model Module
//!
//! C4 Model types and workspace representation — agnostic to output format.
//!
//! ## Submodules
//!
//! - **c4_types** — Core C4 model types: Person, SoftwareSystem, Container, Component, CodeElement
//! - **deployment** — Deployment diagram model: nodes, relationships, networks, volumes
//! - **er_types** — Entity-Relationship diagram model: entities, columns, relationships
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

pub mod c4_types;
pub mod deployment;
pub mod er_types;
pub mod relationships;
pub mod workspace;
pub mod views;
pub mod styles;

pub use c4_types::*;
pub use deployment::*;
pub use er_types::*;
pub use relationships::*;
pub use workspace::C4Workspace;
