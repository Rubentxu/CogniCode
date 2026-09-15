//! Findings application layer (M6).
//!
//! Adapters that implement the domain's `DetectorBackend` port on top of
//! application-layer services. The domain stays free of analysis-engine types;
//! the adapter owns the engine and the IR→engine translation.

pub mod m5_dataflow_backend;
pub mod taint_runner;

pub use m5_dataflow_backend::M5DataflowBackend;
pub use taint_runner::TaintFlowRunner;
