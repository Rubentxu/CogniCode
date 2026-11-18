//! The goal `dependencies`: the resolved Cargo dependency inventory of a
//! revision.
//!
//! The goal reads the tracked `Cargo.toml` and `Cargo.lock` files of a Git
//! revision, offline, and answers with the packages the lockfile pins
//! (version, source, checksum, Package URL identity), the declarations that
//! asked for them, whether the lockfile agrees with the manifests, and what
//! was not collected. It is a thin shell over
//! [`crate::application::dependency_inventory`], which the terminal and the
//! wire both reach through this one function.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::application::dependency_inventory::{
    DependencyInventoryAnswer, InventoryLimits, InventoryQuery, UnitChange, answer as inventory,
};
use crate::domain::dependency_inventory::{CoverageStatus, LockFreshness};
use crate::domain::scm::SourceRef;
use crate::infrastructure::git::GitScmAdapter;
use crate::interface::goals::GoalEnvironment;

fn default_revision() -> String {
    "HEAD".to_string()
}

/// Arguments for the `dependencies` goal.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct DependenciesInput {
    /// The Git revision to inventory. Defaults to `HEAD`.
    #[serde(default = "default_revision")]
    pub revision: String,
    /// A Git revision to compare against: adds, per workspace unit, the
    /// packages added, removed and updated in the lockfile.
    #[serde(default)]
    pub base_ref: Option<String>,
    /// A package, as `name` or `name@version`, whose transitive dependents
    /// are listed.
    #[serde(default)]
    pub dependents_of: Option<String>,
    /// Restricts the answer to the workspace units that contain or lie under
    /// this repository-relative path.
    #[serde(default)]
    pub path: Option<String>,
}

/// The answer of the goal `dependencies`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalDependencies {
    /// One line that states the result.
    pub summary: String,
    /// Version of this response contract.
    pub schema_version: String,
    /// The goal that answers.
    pub goal: String,
    /// What this answer does not establish.
    pub limits: Vec<String>,
    /// The inventory, the dependents and the comparison.
    #[serde(flatten)]
    pub inventory: DependencyInventoryAnswer,
}

/// What an offline reading of manifests and lockfiles cannot say. Published
/// with every answer so no consumer reads silence as completeness.
const LIMITS: [&str; 5] = [
    "Read offline from tracked Cargo.toml and Cargo.lock bytes: Cargo, build scripts and the network are never run.",
    "A lockfile does not record resolved features, nor the dev, build or target kind of an edge between two non-member packages: `reachable_via` is an upper bound.",
    "A Package URL is an identity, not a digest and not provenance; integrity comes from the lockfile checksum or the pinned Git commit.",
    "The observed layer (source imports) and the distributed layer (what a built artifact ships) are not collected.",
    "Only the Cargo ecosystem is read; other ecosystems need an adapter with validated native resolution.",
];

fn count_packages(answer: &DependencyInventoryAnswer) -> usize {
    answer.units.iter().map(|unit| unit.packages.len()).sum()
}

fn summarise(answer: &DependencyInventoryAnswer) -> String {
    let stale = answer
        .units
        .iter()
        .filter(|unit| unit.freshness == LockFreshness::Stale)
        .count();
    let coverage = if answer.coverage.complete {
        "every tracked Cargo file read".to_string()
    } else {
        format!(
            "{} Cargo file(s) not read: coverage is partial",
            answer.coverage.skipped.len()
        )
    };
    format!(
        "Revision {}: {} Cargo unit(s), {} resolved package(s), {} with a stale lockfile; {coverage}.",
        answer.revision.revision,
        answer.units.len(),
        count_packages(answer),
        stale
    )
}

/// Answers the goal over the repository at `workspace`.
pub fn dependencies(
    workspace: &Path,
    input: &DependenciesInput,
) -> Result<GoalDependencies, String> {
    let root = workspace
        .canonicalize()
        .map_err(|error| format!("cannot resolve repository root: {error}"))?;
    let source =
        SourceRef::new("git", &root.to_string_lossy()).map_err(|error| error.to_string())?;
    let scm = GitScmAdapter::new(root);
    let query = InventoryQuery {
        revision: input.revision.clone(),
        base: input.base_ref.clone(),
        dependents: input.dependents_of.clone(),
        path: input.path.clone(),
    };
    let answer =
        inventory(&scm, &source, &query, &InventoryLimits::default()).map_err(|error| {
            format!(
                "cannot inventory the dependencies of `{}`: {error}",
                input.revision
            )
        })?;
    Ok(GoalDependencies {
        summary: summarise(&answer),
        schema_version: "cognicode.goal.dependencies/v1".to_string(),
        goal: "dependencies".to_string(),
        limits: LIMITS.iter().map(|limit| limit.to_string()).collect(),
        inventory: answer,
    })
}

/// Runs the goal from a shared CLI/MCP context.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let input: DependenciesInput = serde_json::from_value(arguments.clone()).map_err(|error| {
        format!("the arguments of the goal `dependencies` are not valid: {error}")
    })?;
    let answer = dependencies(ctx.workspace, &input)?;
    serde_json::to_value(answer).map_err(|error| {
        format!("the answer of the goal `dependencies` is not serialisable: {error}")
    })
}

fn layer_line(unit: &crate::domain::dependency_inventory::WorkspaceInventory) -> String {
    let uncollected: Vec<String> = unit
        .layers
        .iter()
        .filter(|layer| layer.status != CoverageStatus::Collected)
        .map(|layer| format!("{:?}={:?}", layer.layer, layer.status).to_lowercase())
        .collect();
    uncollected.join(", ")
}

/// The answer as text for a terminal.
pub fn render(answer: &Value) -> String {
    let goal: GoalDependencies = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `dependencies` is the answer its own type describes");
    let mut text = format!("{}\n", goal.summary);
    for skipped in &goal.inventory.coverage.skipped {
        text.push_str(&format!(
            "  not read: {} ({})\n",
            skipped.path, skipped.reason
        ));
    }
    for unit in &goal.inventory.units {
        text.push_str(&format!(
            "\nunit {} ({} member(s), {} package(s)), lockfile {:?}\n",
            unit.root,
            unit.members.len(),
            unit.packages.len(),
            unit.freshness
        ));
        if let Some(lock) = &unit.lockfile {
            text.push_str(&format!("  lock digest {}\n", lock.digest));
        }
        for found in &unit.discrepancies {
            text.push_str(&format!(
                "  {:?}: {} ({})\n",
                found.kind, found.subject, found.detail
            ));
        }
        for versions in &unit.multiple_versions {
            text.push_str(&format!(
                "  several versions of {}: {}\n",
                versions.name,
                versions.versions.join(", ")
            ));
        }
        for package in &unit.packages {
            let identity = package.purl.as_deref().unwrap_or("(no PURL)");
            text.push_str(&format!(
                "  {} {} [{:?}] {identity}\n",
                package.id,
                package.source.kind(),
                package.integrity
            ));
        }
        text.push_str(&format!("  not collected: {}\n", layer_line(unit)));
    }
    if let Some(dependents) = &goal.inventory.dependents {
        for unit in dependents {
            match &unit.report {
                None => text.push_str(&format!(
                    "\n{}: the package is not resolved here\n",
                    unit.root
                )),
                Some(report) => {
                    text.push_str(&format!(
                        "\n{}: dependents of {} ({} affected member(s))\n",
                        unit.root,
                        report.targets.join(", "),
                        report.affected_members.len()
                    ));
                    for dependent in &report.dependents {
                        text.push_str(&format!("  {} (depth {})\n", dependent.id, dependent.depth));
                    }
                }
            }
        }
    }
    if let Some(comparison) = &goal.inventory.comparison {
        text.push_str(&format!("\ncompared with {}\n", comparison.base.revision));
        for unit in &comparison.units {
            if unit.change == UnitChange::Unchanged {
                continue;
            }
            text.push_str(&format!(
                "  unit {}: {:?}, +{} -{} ~{}\n",
                unit.root,
                unit.change,
                unit.delta.added.len(),
                unit.delta.removed.len(),
                unit.delta.updated.len()
            ));
        }
    }
    text
}

#[cfg(test)]
mod tests;
