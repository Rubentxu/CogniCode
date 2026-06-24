//! Pure algorithm functions — same `.rs` compiles to native + wasm32.

pub mod all_simple_paths;
pub mod cluster_components;
pub mod communities;
pub mod community_god_nodes;
pub mod condensation;
pub mod feedback_arc_set;
pub mod god_nodes;
pub mod page_rank;
pub mod surprising_connections;
pub mod transitive_reduction;

pub use all_simple_paths::all_simple_paths;
pub use cluster_components::cluster_components;
pub use communities::communities;
pub use community_god_nodes::community_god_nodes;
pub use condensation::condensation;
pub use feedback_arc_set::feedback_arc_set;
pub use god_nodes::god_nodes;
pub use page_rank::page_rank;
pub use surprising_connections::surprising_connections;
pub use transitive_reduction::transitive_reduction;
