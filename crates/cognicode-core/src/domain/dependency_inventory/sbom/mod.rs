//! Artifact SBOMs read against the resolved inventory.
//!
//! An SBOM is a claim by its producer. Reading one fills the distributed
//! layer of an inventory at best partially: the artifact itself is never
//! inspected, a signature is never verified and nothing in a document is
//! executed or fetched. Ambiguity (repeated components, a PURL with no
//! source, a subject that is not a workspace member) is reported, never
//! guessed.
//!
//! * [`parse_cyclonedx`] reads a CycloneDX JSON document within bounds.
//! * [`compare_distribution`] compares one unit's resolved inventory with it.

pub mod compare;
pub mod cyclonedx;
pub mod model;

pub use compare::{
    ArtifactBinding, BindingState, Check, CheckState, ComparisonRequest, DistributionCounts,
    DistributionFinding, DistributionFindingKind, DistributionReport, Expectation, ListedComponent,
    PackageDistribution, Presence, SubjectResolution, UnmatchedComponent, compare_distribution,
};
pub use cyclonedx::parse_cyclonedx;
pub use model::{
    ComponentFacts, ComponentScope, ParsedSbom, SbomClaim, SbomComponent, SbomDiagnostic,
    SbomDiagnosticKind, SbomError, SbomLimits, SbomStage, SbomSubject,
};

#[cfg(test)]
mod tests;
