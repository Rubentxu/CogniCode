//! Layout module for the Sugiyama layout engine
//!
//! Computes node positions, port assignments, compound node hierarchies,
//! and edge routing for C4 diagram rendering.

pub mod cache;
pub mod compound;
pub mod port_assigner;
pub mod sugiyama;
pub mod types;

pub use cache::LayoutCache;
pub use compound::layout_compound;
pub use port_assigner::assign_ports;
pub use sugiyama::compute_layout;
pub use types::*;
