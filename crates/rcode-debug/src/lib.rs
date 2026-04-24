//! rcode-debug: DAP-based debugging for agents
//!
//! Provides autonomous debugging via Debug Adapter Protocol (DAP).
//! The main entry points are:
//! - [`debug_analyze()`] - Analyze a crash and return root cause + recommendation
//! - [`debug_doctor()`] - Check debugging capabilities and available tools

pub mod client;
pub mod doctor;
pub mod error;
pub mod adapter;
pub mod orchestrator;
pub mod analysis;

/// Testing utilities
#[cfg(test)]
pub mod testing;

pub use client::DapClient;
pub use doctor::{Doctor, DoctorReport};
pub use error::{DebugError, Result};
pub use adapter::{AdapterConfig, AdapterRegistry};
pub use orchestrator::{DebugOrchestrator, DebugAnalyzeRequest, DebugAnalysisResult};
pub use analysis::{AnalysisEngine, RootCause, Recommendation};

/// Main entry point: analyze a crash and return root cause + recommendation
pub async fn debug_analyze(request: DebugAnalyzeRequest) -> Result<DebugAnalysisResult> {
    let orchestrator = DebugOrchestrator::new();
    orchestrator.analyze(request).await
}

/// Main entry point: check debugging capabilities
pub async fn debug_doctor() -> Result<DoctorReport> {
    let doctor = Doctor::new();
    doctor.check().await
}
