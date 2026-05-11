//! # Layout Module
//!
//! Sugiyama hierarchical layout algorithm for C4 diagrams.
//!
//! ## Layout Process
//!
//! The layout engine computes node positions using the Sugiyama algorithm:
//!
//! 1. **Vertex Assignment** — Assigns nodes to ranks (layers) based on graph topology
//! 2. **Edge Crossing Minimization** — Minimizes crossings within each rank using barycenter method
//! 3. **Position Computation** — Computes x/y positions for each node within its rank
//! 4. **Port Assignment** — Assigns connection ports based on direction (TB, LR, BT, RL)
//! 5. **Compound Layout** — Handles nested/compound nodes with children
//!
//! ## Layout Configuration
//!
//! ```ignore
//! use cognicode_diagram::layout::sugiyama::compute_layout;
//! use cognicode_diagram::layout::types::{LayoutConfig, LayoutDirection};
//! use cognicode_diagram::model::workspace::C4Workspace;
//!
//! let workspace = C4Workspace::new("MySystem");
//! let config = LayoutConfig {
//!     direction: LayoutDirection::TB,
//!     ..Default::default()
//! };
//! let layout = compute_layout(&workspace, &config);
//! ```
//!
//! ## Layout Directions
//!
//! - **TB** (Top-to-Bottom) — Default, hierarchical vertical layout
//! - **LR** (Left-to-Right) — Horizontal layout
//! - **BT** (Bottom-to-Top) — Inverted vertical layout
//! - **RL** (Right-to-Left) — Inverted horizontal layout

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
