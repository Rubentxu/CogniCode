//! The `security` goal: what the security pack says about the Terraform and
//! Helm in the project.
//!
//! The goal runs the [`SecurityPack`](crate::application::findings::SecurityPack)
//! over every Terraform root and every Helm chart it finds and publishes, per
//! unit and per family, one of four verdicts that are never merged:
//!
//! | Verdict | Meaning |
//! |---|---|
//! | `found` | the family produced findings in the unit |
//! | `not_observed` | nothing found, within the stated scope only; it is not a clean bill |
//! | `unknown` | nothing found, but checks that could not decide remain (an unbound variable, a chart that was not rendered) |
//! | `unsupported` | the family has no model for the language: nothing was analysed |
//!
//! ## Honesty
//!
//! There is no "clean" in this report. A chart is judged only through a real
//! render: when the `helm` engine is missing or fails, the chart is reported
//! as not rendered, its coverage as unknown, and every check that depends on
//! objects as `unknown`. The Terraform coverage lists what was not read, what
//! no check models and which variables nothing binds. The capability matrix of
//! the pack is published next to the verdicts with its limits, so a reader can
//! tell a family the pack does not model from a family that found nothing.
//!
//! The run is a report, not a gate: findings are produced under the candidate
//! authority and are not grounded in the project graph, so none of them is
//! counted as blocking.
//!
//! One function, [`run`], serves the command line and the MCP tool.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::domain::iac::helm::ChartRenderer;
use crate::infrastructure::helm::HelmCli;
use crate::interface::goals::GoalEnvironment;

mod scan;
mod text;

/// Arguments of the `security` goal.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct SecurityInput {
    /// Publish the capability matrix of every language the pack names. By
    /// default only the Terraform and Helm rows are published.
    #[serde(default)]
    pub all_languages: bool,
}

/// One of the four things a pack run may claim. There is no clean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerdictKind {
    /// The family produced findings.
    Found,
    /// Nothing was found, within the stated scope only.
    NotObserved,
    /// Nothing was found, but checks that could not decide remain.
    Unknown,
    /// The family has no model for the language: nothing was analysed.
    Unsupported,
}

/// A finding of a family in a unit.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FindingReport {
    /// The finding kind.
    pub kind: String,
    /// The risk the pack assigned.
    pub risk: String,
    /// The strength of the evidence.
    pub evidence_class: String,
    /// What the finding says.
    pub message: String,
}

/// The verdict of one family over one unit.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FamilyVerdict {
    /// The family.
    pub family: String,
    /// What the run may claim.
    pub verdict: VerdictKind,
    /// The scope of a negative, the reason of an unsupported pair or the
    /// number of findings, in one line.
    pub detail: String,
    /// The checks that could not decide; empty unless the verdict is `unknown`
    /// or a finding came with undecided checks.
    pub unresolved: Vec<String>,
    /// The findings; empty unless the verdict is `found`.
    pub findings: Vec<FindingReport>,
    /// Notes the pack attached to the run.
    pub diagnostics: Vec<String>,
}

/// One unit (a file of a Terraform root, a template or the chart descriptor of
/// a Helm chart) with the verdict of every structural family over it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnitReport {
    /// The unit, as the pack names it.
    pub path: String,
    /// One verdict per family.
    pub families: Vec<FamilyVerdict>,
}

/// What a Terraform analysis could not look at. Anything listed here is not
/// evaluated, never clean.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerraformCoverage {
    /// Whether every resource was assessed or known not to exist, and every
    /// module opened and every file read.
    pub complete: bool,
    /// Resource and data source instances in the graph.
    pub resources: usize,
    /// Those a check looked at.
    pub assessed: usize,
    /// Managed resource types no check models.
    pub not_evaluated_types: Vec<String>,
    /// Module calls that could not be opened, with their source.
    pub unresolved_modules: Vec<String>,
    /// Sources that exist in the project and were not read, with the reason.
    pub unread: Vec<String>,
    /// Root variables that nothing binds: values that depend on them are unknown.
    pub unbound_variables: Vec<String>,
}

/// A Terraform root and what the pack said about its files.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerraformReport {
    /// The root directory, relative to the workspace (`.` for the workspace).
    pub root: String,
    /// What the analysis covers.
    pub coverage: TerraformCoverage,
    /// One entry per file the analysis looked at.
    pub units: Vec<UnitReport>,
}

/// Whether concrete objects exist for a chart.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RenderReport {
    /// `rendered` or `not_rendered`.
    pub status: String,
    /// The documents the engine rendered; `None` when it did not render.
    pub documents: Option<usize>,
    /// Why nothing was rendered; `None` when it rendered.
    pub cause: Option<String>,
}

/// How much of the chart the result covers.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HelmCoverage {
    /// `complete`, `partial` or `unknown`.
    pub state: String,
    /// What is undecided.
    pub reasons: Vec<String>,
}

/// A Helm chart, its render and what the pack said about it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HelmReport {
    /// The chart directory, relative to the workspace.
    pub path: String,
    /// The chart name.
    pub chart: String,
    /// The chart version.
    pub version: String,
    /// Whether the chart was rendered, and if not, why.
    pub render: RenderReport,
    /// How much of the chart the result covers.
    pub coverage: HelmCoverage,
    /// What the analysis does not cover regardless of coverage.
    pub limits: Vec<String>,
    /// One entry per place the analysis speaks about.
    pub units: Vec<UnitReport>,
}

/// A project or chart that was found and could not be analysed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Unanalysed {
    /// The directory, relative to the workspace.
    pub path: String,
    /// `terraform` or `helm`.
    pub kind: String,
    /// Why nothing was analysed.
    pub reason: String,
}

/// How many (unit, family) pairs each verdict has.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VerdictTotals {
    /// Pairs with findings.
    pub found: usize,
    /// Pairs with a scoped negative.
    pub not_observed: usize,
    /// Pairs that could not decide.
    pub unknown: usize,
    /// Pairs the pack has no model for.
    pub unsupported: usize,
}

/// One cell of the capability matrix.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MatrixRow {
    /// The language.
    pub language: String,
    /// The family.
    pub family: String,
    /// `supported` or `unsupported`.
    pub support: String,
    /// The algorithm that produces the family's findings.
    pub algorithm: String,
    /// The backend that runs the family; `None` when unsupported.
    pub backend: Option<String>,
    /// The strongest evidence class the backend can produce.
    pub evidence_ceiling: Option<String>,
    /// Whether a finding of the family can block with that ceiling.
    pub can_block: bool,
    /// What a finding claims: `vulnerability` or `risk`.
    pub nature: String,
    /// When a finding may block.
    pub blocking: String,
    /// Why the pair is unsupported; `None` when supported.
    pub reason: Option<String>,
    /// Where the analysis stops. A negative never claims more.
    pub limits: Vec<String>,
}

/// The answer of the `security` goal.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GoalSecurity {
    /// One line that summarises the answer.
    pub summary: String,
    /// The version of this answer's schema.
    pub schema_version: String,
    /// The goal that answers.
    pub goal: String,
    /// The version of the pack that judged.
    pub pack_version: String,
    /// The Terraform roots found, with their verdicts.
    pub terraform: Vec<TerraformReport>,
    /// The Helm charts found, with their render and verdicts.
    pub helm: Vec<HelmReport>,
    /// What was found and could not be analysed.
    pub unanalysed: Vec<Unanalysed>,
    /// How many (unit, family) pairs each verdict has.
    pub totals: VerdictTotals,
    /// The capability matrix of the pack.
    pub matrix: Vec<MatrixRow>,
    /// What this report does not claim.
    pub limits: Vec<String>,
}

/// Runs the pack over the Terraform and Helm of `workspace`, rendering charts
/// through `renderer`.
///
/// # Errors
/// When `workspace` is not a directory.
pub fn security(
    workspace: &Path,
    renderer: &dyn ChartRenderer,
    input: &SecurityInput,
) -> Result<GoalSecurity, String> {
    scan::security(workspace, renderer, input)
}

/// The implementation of the goal: reads its arguments and returns its answer.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let input = serde_json::from_value::<SecurityInput>(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `security` are not valid: {error}"))?;
    let answer = security(ctx.workspace, &HelmCli::from_path(), &input)?;
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `security` is not serialisable: {error}"))
}

/// The answer of the goal `security` as text.
pub fn render(answer: &Value) -> String {
    text::render_text(
        &serde_json::from_value::<GoalSecurity>(answer.clone())
            .expect("the answer of the goal `security` is the answer its own type describes"),
    )
}

#[cfg(test)]
mod tests;
