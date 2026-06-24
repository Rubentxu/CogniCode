//! Pure algorithm functions — same `.rs` compiles to native + wasm32.

pub mod communities;
pub mod community_god_nodes;
pub mod god_nodes;
pub mod page_rank;
pub mod surprising_connections;

pub use communities::communities;
pub use community_god_nodes::community_god_nodes;
pub use god_nodes::god_nodes;
pub use page_rank::page_rank;
pub use surprising_connections::surprising_connections;
