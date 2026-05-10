//! Inference engine — extracts C4 model elements from code analysis

pub mod engine;
pub mod uml_rules;
pub mod code_inference;
pub mod component_inference;
pub mod container_inference;
pub mod context_inference;
pub mod config_parsers;

pub use engine::InferenceEngine;
pub use component_inference::ComponentInference;
pub use container_inference::ContainerInference;
pub use context_inference::ContextInference;
pub use config_parsers::detect_and_parse;
