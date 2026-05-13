//! # Inference Module
//!
//! Extracts C4 model elements from code analysis, project configurations, and schemas.
//!
//! ## Inference Pipeline
//!
//! The inference engine processes source code and configuration to produce model elements:
//!
//! 1. **Context Inference (L1)** — External systems and people from project metadata
//! 2. **Container Inference (L2)** — Services, databases, and containers from project structure
//! 3. **Component Inference (L3)** — Components within containers from code analysis
//! 4. **Code Inference (L4)** — Classes, interfaces, and functions from AST analysis
//!
//! ## Specialized Inference
//!
//! - **Sequence Inference** — Generates sequence diagrams from call graph traversal
//! - **Deployment Inference** — Parses Dockerfile and docker-compose.yml for infrastructure diagrams
//! - **ER Inference** — Parses SQL schemas, Diesel schema.rs, and Prisma schema.prisma for ER diagrams
//! - **Config Parsers** — Language-specific configuration file parsers (Python, Rust, etc.)
//!
//! ## Usage
//!
//! ```ignore
//! use cognicode_diagram::inference::InferenceEngine;
//! use cognicode_core::domain::aggregates::call_graph::CallGraph;
//!
//! let engine = InferenceEngine::new(&call_graph);
//! let workspace = engine.infer_workspace("MyProject");
//! ```

pub mod activity_inference;
pub mod engine;
pub mod multi_lang_engine;
pub mod uml_rules;
pub mod code_inference;
pub mod component_inference;
pub mod container_inference;
pub mod context_inference;
pub mod config_parsers;
pub mod deployment_inference;
pub mod er_inference;
pub mod sequence_inference;
pub mod state_machine_inference;
pub mod ts_inference;

pub use activity_inference::{infer_activity_from_function, find_activities, ActivityInferenceOptions};
pub use engine::InferenceEngine;
pub use component_inference::ComponentInference;
pub use container_inference::ContainerInference;
pub use context_inference::ContextInference;
pub use config_parsers::detect_and_parse;
pub use deployment_inference::infer_deployment;
pub use er_inference::infer_er_diagram;
pub use sequence_inference::{infer_sequence, infer_sequence_default, find_entry_points, SequenceInferenceOptions};
pub use state_machine_inference::{
    infer_state_machine_from_enum, infer_state_machine_from_struct, find_state_machines,
    StateMachineInferenceOptions,
};
pub use ts_inference::TsInference;
pub use multi_lang_engine::{MultiLangEngine, Language};
