//! Pure algorithm functions — same `.rs` compiles to native + wasm32.

pub mod god_nodes;
pub mod page_rank;

pub use god_nodes::god_nodes;
pub use page_rank::page_rank;
