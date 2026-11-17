//! A static model of Terraform configuration.
//!
//! The model never assumes HCL is evaluated: a value is known
//! only when the configuration text itself fixes it, and everything else is
//! [`TfValue::Unknown`] with the reason recorded. No `terraform plan`, no
//! provider, no credentials and no module code is run to build it.

pub mod analysis;
pub mod eval;
pub mod graph;
pub mod ir;
pub mod observe;
pub mod rules;
pub mod value;

pub use analysis::{Coverage, Explanation, RebindReport, TfAnalysis};
pub use eval::{Bindings, Evaluation, constant};
pub use graph::{FlowEdge, Node, NodeAddr, NodeKind, ResourceInstance, TfGraph, UnresolvedModule};
pub use ir::{
    Reference, ResourceMode, Step, TfAttribute, TfBlock, TfBody, TfExpr, TfLocal, TfModule,
    TfModuleCall, TfOutput, TfProject, TfProvider, TfResource, TfVariable, UnreadSource,
    resolve_local_source,
};
pub use observe::{observed_units, structural_family};
pub use rules::{CheckResult, MODELED_TYPES, Outcome, RiskFamily, assess, is_absent, is_modeled};
pub use value::{TfValue, UnknownCause};
