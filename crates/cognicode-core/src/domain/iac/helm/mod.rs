//! Helm semantics without reinterpreting templates as manifests.
//!
//! Two models, kept apart on purpose:
//!
//! - [`ChartModel`] is what a chart *declares*: templates reduced to the
//!   values and constructs they use, default values, schema, dependencies. It
//!   never contains a Kubernetes object, because a template is Go-template
//!   text whose output depends on values and functions.
//! - [`ManifestModel`] is what a *render produced*: concrete objects, each
//!   with its origin and the digest of the values that fixed it.
//!
//! The official engine is an optional adapter behind [`ChartRenderer`]. The
//! analysis core works with none installed ([`NoRenderer`]); it then reports
//! [`RenderStatus::NotRendered`], [`RenderCoverage::Unknown`] and every check as
//! unknown rather than a clean result.
//!
//! ## Honesty contract
//!
//! - No object exists without a real render. An unrendered chart yields no
//!   objects, no findings and no negative verdicts.
//! - A render is [`RenderCoverage::Complete`] only when the chart and values leave
//!   nothing the analysis knows to be undecided (unset values outside
//!   conditions, schema requirements, missing dependencies, cluster lookups,
//!   randomness). Otherwise it is partial and every "nothing found" becomes
//!   [`CheckVerdict::Unknown`].
//! - Object kinds without a security model are listed, never ignored silently.
//!
//! Pure domain: no I/O.

mod analysis;
mod chart;
mod checks;
mod manifest;
mod observe;
mod render;
mod template;
mod values;

pub use analysis::{
    ChartAnalysis, ChartCheckResult, ChartDelta, CheckVerdict, ComparedDelta, ManifestFactChange,
    ObjectChange, RenderCoverage, analyze_chart, compare,
};
pub use chart::{ChartDependency, ChartModel, PartialReason, ValueGap, ValuesSchema};
pub use checks::{HelmCheck, HelmFinding, check_manifest};
pub use manifest::{
    ContainerFacts, ExposureFacts, ImageRef, ManifestModel, ManifestObject, ManifestObjectId,
    ObjectFacts, ObjectOrigin, PodFacts, PublishedPort, RbacFacts, RbacRule, RbacSubject,
    RenderedDocument, SkippedDocument, WorkloadFacts,
};
pub use observe::{observed_units, structural_family};
pub use render::{
    ChartRenderer, NoRenderer, NotRenderedCause, RenderRequest, RenderResponse, RenderStatus,
};
pub use template::{Construct, TemplateModel, ValueRef, is_helper_path, is_manifest_template};
pub use values::{effective_values, lookup, merge_values, values_digest};
