//! Adapters: bridge concrete graph types to the
//! [`GraphBuilder`](crate::graph_builder::GraphBuilder) trait.

pub mod json_graph;

#[cfg(feature = "petgraph-adapter")]
pub mod call_graph_projection;
