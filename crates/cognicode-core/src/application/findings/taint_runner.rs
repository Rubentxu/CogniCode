//! The taint-flow seam (M6, cycle e60).
//!
//! A tiny **application-local** seam so the dataflow adapter can be composed
//! with (and tested against) a counting spy without turning
//! `ProgramAnalysisService` into infrastructure, and without inventing a
//! domain port. `DetectorBackend` already *is* the domain port; this trait only
//! abstracts the engine call.

use crate::application::program_analysis::{
    ProgramAnalysisService, TaintFlowRequest, TaintFlowResult,
};
use crate::domain::analytics::descriptor::AnalyticsError;
use crate::domain::plan::limits::PlanLimits;

/// Runs the M5 forward-taint algorithm from a typed request.
pub trait TaintFlowRunner {
    /// Execute forward taint over the request.
    fn taint_flow(
        &self,
        request: &TaintFlowRequest,
        limits: &PlanLimits,
    ) -> Result<TaintFlowResult, AnalyticsError>;
}

impl TaintFlowRunner for ProgramAnalysisService {
    fn taint_flow(
        &self,
        request: &TaintFlowRequest,
        limits: &PlanLimits,
    ) -> Result<TaintFlowResult, AnalyticsError> {
        // Inherent method wins; there is exactly one implementation.
        ProgramAnalysisService::taint_flow(self, request, limits)
    }
}
