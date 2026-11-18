//! The goal `advisories`: the resolved Cargo packages of a revision
//! correlated with a pinned OSV feed.
//!
//! The operator supplies the feed as a file inside the workspace and may pin
//! it to a SHA-256 digest; nothing is fetched. The answer is an observation
//! anchored to the digest of the feed bytes and to the revision and lockfile
//! digest the inventory came from. It says which packages the feed covers
//! and why a package could not be judged; it never reports "no
//! vulnerabilities", it does not say whether a vulnerable package is
//! reachable or exploitable, and it does not verify the signature or the
//! provenance of any artifact. Deciding what a match means for a release is
//! the business of a policy engine, not of this goal. It is a thin shell
//! over [`crate::application::advisory_correlation`], which the terminal and
//! the wire both reach through this one function.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::application::advisory_correlation::{
    AdvisoryAnswer, AdvisoryEvaluation, AdvisoryQuery, FeedSupply, answer as correlate,
};
use crate::application::dependency_inventory::InventoryLimits;
use crate::domain::advisory_matching::FeedLimits;
use crate::domain::scm::SourceRef;
use crate::infrastructure::git::GitScmAdapter;
use crate::interface::goals::GoalEnvironment;
use crate::interface::goals::common::{parse_sha256_digest, read_workspace_file};

fn default_revision() -> String {
    "HEAD".to_string()
}

/// Arguments for the `advisories` goal.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AdvisoriesInput {
    /// The Git revision whose resolved packages are correlated. Defaults to
    /// `HEAD`.
    #[serde(default = "default_revision")]
    pub revision: String,
    /// The OSV feed: a JSON file inside the workspace holding an array of
    /// OSV records or an object with a `vulns` array. It is read, never
    /// fetched. Without it the answer is `not_evaluated`.
    #[serde(default)]
    pub feed: Option<String>,
    /// The SHA-256 digest the feed is pinned to, as 64 lowercase hex digits
    /// or `sha256:<hex>`. A feed whose bytes differ is not used.
    #[serde(default)]
    pub expected_digest: Option<String>,
    /// The most days the newest advisory of the feed may be older than now
    /// before the feed counts as stale. Without it freshness is not checked
    /// and silence only means "not in this feed".
    #[serde(default)]
    pub max_age_days: Option<u32>,
    /// Restricts the answer to the workspace units that contain or lie under
    /// this repository-relative path.
    #[serde(default)]
    pub path: Option<String>,
}

/// The answer of the goal `advisories`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalAdvisories {
    /// One line that states the result.
    pub summary: String,
    /// Version of this response contract.
    pub schema_version: String,
    /// The goal that answers.
    pub goal: String,
    /// What this answer does not establish.
    pub limits: Vec<String>,
    /// The correlation, anchored to the feed and to the inventory.
    #[serde(flatten)]
    pub advisories: AdvisoryAnswer,
}

/// What an offline correlation with a pinned feed cannot say. Published with
/// every answer so no consumer reads silence as safety.
const LIMITS: [&str; 6] = [
    "An advisory match is an observation about a locked version, not exploitability: whether the vulnerable code is reachable or used is not established.",
    "It is not artifact verification: signatures, provenance and the integrity of built artifacts are not checked by this goal.",
    "The feed is an operator-supplied input read from a file and never fetched; its content is untrusted, parsed under size and record limits, and nothing in it is executed.",
    "A stale, partial, unparseable or crates.io-free feed never yields a clean result: packages are `unknown` or `not_evaluated`, and `not_found_in_scope` means only that this feed, at this digest, has no advisory covering the version.",
    "Only crates.io packages of Cargo lockfiles are correlated; other ecosystems and sources are `not_evaluated`. A yanked release is not known: neither the lockfile nor an OSV record says so.",
    "What a match means for a release (risk, policy, verdict) is decided elsewhere; this goal only observes.",
];

/// Reads the feed file under the byte limit. Any failure becomes the
/// reason the feed is unusable, so the answer says `not_evaluated`.
fn read_feed(workspace: &Path, raw: &str, limits: &FeedLimits) -> FeedSupply {
    match read_workspace_file(workspace, raw, limits.max_bytes, "feed") {
        Ok(bytes) => FeedSupply::Bytes {
            origin: raw.to_string(),
            bytes,
        },
        Err(reason) => FeedSupply::Unreadable { reason },
    }
}

fn summarise(answer: &AdvisoryAnswer) -> String {
    let totals = &answer.totals;
    if answer.evaluation == AdvisoryEvaluation::NotEvaluated {
        return format!(
            "Revision {}: not evaluated, the feed is {:?}; {} resolved package(s) have no advisory verdict.",
            answer.revision.revision, answer.feed.status, totals.total
        );
    }
    format!(
        "Revision {}: {} package(s): {} affected, {} not found in this feed, {} unknown, {} not evaluated.",
        answer.revision.revision,
        totals.total,
        totals.affected,
        totals.not_found_in_scope,
        totals.unknown,
        totals.not_evaluated
    )
}

/// Answers the goal over the repository at `workspace`, at the instant
/// `now_unix`.
///
/// The clock is a value so the correlation stays reproducible; [`run`]
/// supplies the wall clock.
pub fn advisories_at(
    workspace: &Path,
    input: &AdvisoriesInput,
    now_unix: i64,
) -> Result<GoalAdvisories, String> {
    let feed_limits = FeedLimits::default();
    let expected_digest = input
        .expected_digest
        .as_deref()
        .map(|raw| parse_sha256_digest("expected_digest", raw))
        .transpose()?;
    let root = workspace
        .canonicalize()
        .map_err(|error| format!("cannot resolve repository root: {error}"))?;
    let source =
        SourceRef::new("git", &root.to_string_lossy()).map_err(|error| error.to_string())?;
    let feed = match input.feed.as_deref() {
        None => FeedSupply::Absent,
        Some(raw) => read_feed(&root, raw, &feed_limits),
    };
    let scm = GitScmAdapter::new(root);
    let query = AdvisoryQuery {
        revision: input.revision.clone(),
        path: input.path.clone(),
        feed,
        expected_digest,
        max_age_days: input.max_age_days,
        now_unix,
    };
    let answer = correlate(
        &scm,
        &source,
        &query,
        &InventoryLimits::default(),
        &feed_limits,
    )
    .map_err(|error| {
        format!(
            "cannot correlate the advisories of `{}`: {error}",
            input.revision
        )
    })?;
    Ok(GoalAdvisories {
        summary: summarise(&answer),
        schema_version: "cognicode.goal.advisories/v1".to_string(),
        goal: "advisories".to_string(),
        limits: LIMITS.iter().map(|limit| limit.to_string()).collect(),
        advisories: answer,
    })
}

/// Answers the goal with the wall clock.
pub fn advisories(workspace: &Path, input: &AdvisoriesInput) -> Result<GoalAdvisories, String> {
    advisories_at(workspace, input, chrono::Utc::now().timestamp())
}

/// Runs the goal from a shared CLI/MCP context.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let input: AdvisoriesInput = serde_json::from_value(arguments.clone()).map_err(|error| {
        format!("the arguments of the goal `advisories` are not valid: {error}")
    })?;
    let answer = advisories(ctx.workspace, &input)?;
    serde_json::to_value(answer).map_err(|error| {
        format!("the answer of the goal `advisories` is not serialisable: {error}")
    })
}

/// The answer as text for a terminal.
pub fn render(answer: &Value) -> String {
    let goal: GoalAdvisories = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `advisories` is the answer its own type describes");
    let feed = &goal.advisories.feed;
    let mut text = format!("{}\n", goal.summary);
    text.push_str(&format!(
        "feed {} digest {} ({:?}, freshness {:?}, pinned: {})\n",
        feed.origin.as_deref().unwrap_or("(none)"),
        feed.digest.as_deref().unwrap_or("(none)"),
        feed.status,
        feed.freshness,
        feed.pinned
    ));
    if let Some(problem) = &feed.problem {
        text.push_str(&format!("  problem: {problem}\n"));
    }
    if feed.records_rejected > 0 {
        text.push_str(&format!(
            "  {} record(s) rejected out of {}\n",
            feed.records_rejected, feed.records_seen
        ));
    }
    text.push_str(&format!(
        "considered: {}; not considered: {}\n",
        goal.advisories.coverage.ecosystems_considered.join(", "),
        goal.advisories
            .coverage
            .ecosystems_not_considered
            .join(", ")
    ));
    for unit in &goal.advisories.units {
        text.push_str(&format!(
            "\nunit {} ({:?}), resolved digest {}\n",
            unit.root, unit.evaluation, unit.resolved_digest
        ));
        for package in &unit.packages {
            let reasons = if package.reasons.is_empty() {
                String::new()
            } else {
                format!(" {:?}", package.reasons)
            };
            text.push_str(&format!(
                "  {} {} {:?}{reasons}\n",
                package.name, package.version, package.state
            ));
            for hit in &package.advisories {
                text.push_str(&format!(
                    "    {} {:?}, fixed in {}\n",
                    hit.id,
                    hit.certainty,
                    if hit.fixed_in.is_empty() {
                        "(unknown)".to_string()
                    } else {
                        hit.fixed_in.join(", ")
                    }
                ));
            }
        }
    }
    text
}

#[cfg(test)]
mod tests;
