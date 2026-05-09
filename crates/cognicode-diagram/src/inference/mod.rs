//! Inference engine — extracts C4 model elements from code analysis

pub mod engine;
pub mod uml_rules;
pub mod code_inference;

pub use engine::InferenceEngine;
