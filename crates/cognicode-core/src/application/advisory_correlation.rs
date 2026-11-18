//! The use case behind advisory correlation: read a pinned OSV feed, take
//! the resolved Cargo inventory of a revision, and say which resolved
//! packages the feed covers.
//!
//! The answer is an observation anchored twice: to the bytes of the feed (by
//! digest) and to the lockfile bytes and revision the inventory came from.
//! Re-running with the same two inputs yields the same answer. It never
//! reports "no vulnerabilities": a package the feed does not cover is
//! `not_found_in_scope` only when the whole feed was read, is not stale and
//! covers crates.io; in every other case the state is `unknown` or
//! `not_evaluated` and says why.
//!
//! The use case does no I/O of its own beyond the revision reader it is
//! handed. The feed bytes, the clock and the pinned digest arrive as values,
//! which keeps the correlation deterministic and testable; no network call
//! exists in this path.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::application::dependency_inventory::{
    DependencyInventoryAnswer, InventoryLimits, InventoryQuery, RevisionAnchor, SourceCoverage,
    answer as inventory,
};
use crate::domain::advisory_matching::{
    Absence, AdvisoryReason, CRATES_IO, FeedError, FeedLimits, PackageResult, PackageState,
    ParsedFeed, RejectedRecord, Subject, correlate, parse_feed,
};
use crate::domain::dependency_inventory::{FileAnchor, LockFreshness, WorkspaceInventory};
use crate::domain::scm::{ContentDigest, RevisionReader, ScmError, SourceRef};

/// The most rejected records listed individually; the total is always given.
const MAX_LISTED_REJECTIONS: usize = 100;
/// Seconds in a day.
const DAY: i64 = 86_400;

/// What the caller managed to obtain of the feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedSupply {
    /// No feed was given.
    Absent,
    /// A feed was named but could not be read.
    Unreadable {
        /// Why, in the caller's words.
        reason: String,
    },
    /// The bytes of the feed.
    Bytes {
        /// Where the feed came from, for the report.
        origin: String,
        /// The bytes.
        bytes: Vec<u8>,
    },
}

/// What a consumer asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvisoryQuery {
    /// The revision whose inventory is correlated.
    pub revision: String,
    /// Restricts the answer to the units that contain or lie under this
    /// repository-relative path.
    pub path: Option<String>,
    /// The feed.
    pub feed: FeedSupply,
    /// The digest the feed is pinned to. A feed whose bytes differ is not
    /// used.
    pub expected_digest: Option<ContentDigest>,
    /// The most days the newest advisory of the feed may be older than
    /// `now_unix` before the feed counts as stale.
    pub max_age_days: Option<u32>,
    /// The current time, in seconds since the Unix epoch.
    pub now_unix: i64,
}

/// How far a feed could be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedStatus {
    /// The feed was read; individual records may still have been rejected.
    Usable,
    /// No feed was given.
    Absent,
    /// The feed could not be read from where it was named.
    Unreadable,
    /// The feed bytes do not match the pinned digest.
    DigestMismatch,
    /// The feed is above the size limit.
    TooLarge,
    /// The feed is not valid JSON.
    Malformed,
    /// The feed is JSON but not an OSV feed shape.
    UnrecognisedShape,
    /// The feed has more records than the limit.
    TooManyRecords,
}

/// Whether the feed is recent enough to conclude from its silence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedFreshness {
    /// The newest advisory is inside the accepted window.
    Fresh,
    /// The newest advisory is older than the accepted window.
    Stale,
    /// No window was asked for: the age of the feed was not checked.
    Unchecked,
    /// A window was asked for but the feed has no dated advisory.
    Undated,
}

/// What was done with the feed, anchored to its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedReport {
    /// Where the feed came from.
    pub origin: Option<String>,
    /// The SHA-256 of the feed bytes, when there were bytes.
    pub digest: Option<String>,
    /// Whether the caller pinned the feed to a digest.
    pub pinned: bool,
    /// The size of the feed.
    pub bytes: Option<usize>,
    /// How far the feed could be used.
    pub status: FeedStatus,
    /// What went wrong, when the feed was not usable.
    pub problem: Option<String>,
    /// Whether the feed is recent enough.
    pub freshness: FeedFreshness,
    /// The `modified` of the newest accepted advisory.
    pub newest_modified: Option<String>,
    /// The records the feed carries.
    pub records_seen: usize,
    /// The records accepted as current advisories.
    pub records_accepted: usize,
    /// The accepted records that cover crates.io.
    pub records_crates_io: usize,
    /// The withdrawn records.
    pub records_withdrawn: usize,
    /// The records only about other ecosystems.
    pub records_other_ecosystem: usize,
    /// Copies of a record that were merged.
    pub duplicates_collapsed: usize,
    /// How many records were rejected.
    pub records_rejected: usize,
    /// The first rejected records, by position.
    pub rejected: Vec<RejectedRecord>,
    /// How many records name each ecosystem, as the feed spells it.
    pub ecosystems: BTreeMap<String, usize>,
}

/// Whether a result was computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdvisoryEvaluation {
    /// Packages were correlated with the feed.
    Evaluated,
    /// They were not: absence of matches says nothing.
    NotEvaluated,
}

/// Counts of the package states of a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StateCounts {
    /// Packages in the unit.
    pub total: usize,
    /// Packages with at least one advisory covering their version.
    pub affected: usize,
    /// Packages the whole feed was checked against with no advisory found.
    pub not_found_in_scope: usize,
    /// Packages matching could not conclude about.
    pub unknown: usize,
    /// Packages outside what the feed can speak about.
    pub not_evaluated: usize,
}

impl StateCounts {
    fn add(&mut self, state: PackageState) {
        self.total += 1;
        match state {
            PackageState::Affected => self.affected += 1,
            PackageState::NotFoundInScope => self.not_found_in_scope += 1,
            PackageState::Unknown => self.unknown += 1,
            PackageState::NotEvaluated => self.not_evaluated += 1,
        }
    }

    fn merge(&mut self, other: &Self) {
        self.total += other.total;
        self.affected += other.affected;
        self.not_found_in_scope += other.not_found_in_scope;
        self.unknown += other.unknown;
        self.not_evaluated += other.not_evaluated;
    }
}

/// The correlation of one workspace unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitAdvisories {
    /// The unit root.
    pub root: String,
    /// The lockfile the packages were read from, with its digest.
    pub lockfile: Option<FileAnchor>,
    /// Whether the lockfile agrees with the manifests.
    pub lock_freshness: LockFreshness,
    /// The digest of the resolved package graph.
    pub resolved_digest: String,
    /// The digest of the feed this result was computed against.
    pub feed_digest: Option<String>,
    /// Whether the unit was correlated.
    pub evaluation: AdvisoryEvaluation,
    /// Why the unit was not, or why its absences are not conclusive.
    pub reasons: Vec<AdvisoryReason>,
    /// How many packages are in each state.
    pub counts: StateCounts,
    /// The result for each resolved package, in inventory order.
    pub packages: Vec<PackageResult>,
}

/// What of the advisory space was considered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvisoryCoverage {
    /// The ecosystems packages were correlated in.
    pub ecosystems_considered: Vec<String>,
    /// The ecosystems the feed names that were not considered.
    pub ecosystems_not_considered: Vec<String>,
    /// The advisories of the feed that were matched against packages.
    pub advisories_considered: usize,
    /// What of the revision's Cargo files was read.
    pub inventory: SourceCoverage,
}

/// The answer to an [`AdvisoryQuery`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvisoryAnswer {
    /// The revision the inventory describes.
    pub revision: RevisionAnchor,
    /// Whether the revision was correlated with the feed.
    pub evaluation: AdvisoryEvaluation,
    /// What was done with the feed.
    pub feed: FeedReport,
    /// What was considered.
    pub coverage: AdvisoryCoverage,
    /// The package states over every unit.
    pub totals: StateCounts,
    /// One result per workspace unit.
    pub units: Vec<UnitAdvisories>,
}

/// The feed after it was read, with what correlation needs of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedEvaluation {
    /// What is reported about the feed.
    pub report: FeedReport,
    /// The records, when the feed was usable.
    pub parsed: Option<ParsedFeed>,
    /// Whether silence may be reported as `not_found_in_scope`.
    pub absence: Absence,
}

fn empty_report(status: FeedStatus, problem: Option<String>, pinned: bool) -> FeedReport {
    FeedReport {
        origin: None,
        digest: None,
        pinned,
        bytes: None,
        status,
        problem,
        freshness: FeedFreshness::Unchecked,
        newest_modified: None,
        records_seen: 0,
        records_accepted: 0,
        records_crates_io: 0,
        records_withdrawn: 0,
        records_other_ecosystem: 0,
        duplicates_collapsed: 0,
        records_rejected: 0,
        rejected: Vec::new(),
        ecosystems: BTreeMap::new(),
    }
}

fn unusable(report: FeedReport) -> FeedEvaluation {
    FeedEvaluation {
        report,
        parsed: None,
        absence: Absence::Inconclusive(AdvisoryReason::FeedUnavailable),
    }
}

fn status_of(error: &FeedError) -> FeedStatus {
    match error {
        FeedError::TooLarge { .. } => FeedStatus::TooLarge,
        FeedError::Malformed { .. } => FeedStatus::Malformed,
        FeedError::TooManyRecords { .. } => FeedStatus::TooManyRecords,
        FeedError::UnrecognisedShape => FeedStatus::UnrecognisedShape,
    }
}

fn freshness_of(parsed: &ParsedFeed, max_age_days: Option<u32>, now_unix: i64) -> FeedFreshness {
    let Some(days) = max_age_days else {
        return FeedFreshness::Unchecked;
    };
    match parsed.newest_modified_unix() {
        None => FeedFreshness::Undated,
        Some(newest) if now_unix.saturating_sub(newest) > i64::from(days) * DAY => {
            FeedFreshness::Stale
        }
        Some(_) => FeedFreshness::Fresh,
    }
}

fn absence_of(parsed: &ParsedFeed, freshness: FeedFreshness) -> Absence {
    if parsed.crates_io_records() == 0 {
        return Absence::Inconclusive(AdvisoryReason::EcosystemNotCovered);
    }
    if !parsed.rejected.is_empty() {
        return Absence::Inconclusive(AdvisoryReason::FeedIncomplete);
    }
    match freshness {
        FeedFreshness::Stale | FeedFreshness::Undated => {
            Absence::Inconclusive(AdvisoryReason::FeedStale)
        }
        FeedFreshness::Fresh | FeedFreshness::Unchecked => Absence::Conclusive,
    }
}

/// Reads and judges the feed: pinned digest, parse under limits, freshness,
/// and whether its silence may be trusted.
pub fn evaluate_feed(query: &AdvisoryQuery, limits: &FeedLimits) -> FeedEvaluation {
    let pinned = query.expected_digest.is_some();
    let (origin, bytes) = match &query.feed {
        FeedSupply::Absent => {
            return unusable(empty_report(
                FeedStatus::Absent,
                Some("no advisory feed was given".to_string()),
                pinned,
            ));
        }
        FeedSupply::Unreadable { reason } => {
            return unusable(empty_report(
                FeedStatus::Unreadable,
                Some(reason.clone()),
                pinned,
            ));
        }
        FeedSupply::Bytes { origin, bytes } => (origin, bytes),
    };
    let digest = ContentDigest::from_bytes(bytes);
    let mut report = empty_report(FeedStatus::Usable, None, pinned);
    report.origin = Some(origin.clone());
    report.digest = Some(digest.as_hex().to_string());
    report.bytes = Some(bytes.len());
    if let Some(expected) = &query.expected_digest
        && *expected != digest
    {
        report.status = FeedStatus::DigestMismatch;
        report.problem = Some(format!(
            "the feed digest {} is not the pinned digest {}",
            digest.as_hex(),
            expected.as_hex()
        ));
        return unusable(report);
    }
    let parsed = match parse_feed(bytes, limits) {
        Ok(parsed) => parsed,
        Err(error) => {
            report.status = status_of(&error);
            report.problem = Some(error.to_string());
            return unusable(report);
        }
    };
    let freshness = freshness_of(&parsed, query.max_age_days, query.now_unix);
    report.freshness = freshness;
    report.newest_modified = parsed.newest_modified().map(str::to_string);
    report.records_seen = parsed.records_seen;
    report.records_accepted = parsed.records.len();
    report.records_crates_io = parsed.crates_io_records();
    report.records_withdrawn = parsed.withdrawn.len();
    report.records_other_ecosystem = parsed.other_ecosystem_records;
    report.duplicates_collapsed = parsed.duplicates_collapsed;
    report.records_rejected = parsed.rejected.len();
    report.rejected = parsed
        .rejected
        .iter()
        .take(MAX_LISTED_REJECTIONS)
        .cloned()
        .collect();
    report.ecosystems = parsed.ecosystems.clone();
    let absence = absence_of(&parsed, freshness);
    FeedEvaluation {
        report,
        parsed: Some(parsed),
        absence,
    }
}

fn subjects_of(unit: &WorkspaceInventory) -> Vec<Subject> {
    unit.packages
        .iter()
        .map(|package| Subject {
            id: package.id.clone(),
            name: package.name.clone(),
            version: package.version.clone(),
            purl: package.purl.clone(),
            from_crates_io: package.source.is_default_registry(),
        })
        .collect()
}

fn unit_advisories(unit: &WorkspaceInventory, feed: &FeedEvaluation) -> UnitAdvisories {
    let records = feed
        .parsed
        .as_ref()
        .map_or(&[][..], |parsed| parsed.records.as_slice());
    let packages = correlate(records, &subjects_of(unit), feed.absence);
    let mut counts = StateCounts::default();
    for package in &packages {
        counts.add(package.state);
    }
    let mut reasons: Vec<AdvisoryReason> = packages
        .iter()
        .flat_map(|package| package.reasons.iter().copied())
        .collect();
    if unit.lockfile.is_none() {
        reasons.push(AdvisoryReason::NoLockfile);
    }
    reasons.sort_unstable();
    reasons.dedup();
    let evaluation = if feed.parsed.is_some() && unit.lockfile.is_some() {
        AdvisoryEvaluation::Evaluated
    } else {
        AdvisoryEvaluation::NotEvaluated
    };
    UnitAdvisories {
        root: unit.root.clone(),
        lockfile: unit.lockfile.clone(),
        lock_freshness: unit.freshness,
        resolved_digest: unit.resolved_digest.clone(),
        feed_digest: feed.report.digest.clone(),
        evaluation,
        reasons,
        counts,
        packages,
    }
}

/// Correlates an inventory with an evaluated feed. Pure.
pub fn correlate_inventory(
    inventory: &DependencyInventoryAnswer,
    feed: FeedEvaluation,
) -> AdvisoryAnswer {
    let units: Vec<UnitAdvisories> = inventory
        .units
        .iter()
        .map(|unit| unit_advisories(unit, &feed))
        .collect();
    let mut totals = StateCounts::default();
    for unit in &units {
        totals.merge(&unit.counts);
    }
    let evaluation = if feed.parsed.is_some() && !units.is_empty() {
        AdvisoryEvaluation::Evaluated
    } else {
        AdvisoryEvaluation::NotEvaluated
    };
    let coverage = AdvisoryCoverage {
        ecosystems_considered: vec![CRATES_IO.to_string()],
        ecosystems_not_considered: feed
            .report
            .ecosystems
            .keys()
            .filter(|name| name.as_str() != CRATES_IO)
            .cloned()
            .collect(),
        advisories_considered: feed.report.records_crates_io,
        inventory: inventory.coverage.clone(),
    };
    AdvisoryAnswer {
        revision: inventory.revision.clone(),
        evaluation,
        feed: feed.report,
        coverage,
        totals,
        units,
    }
}

/// Answers `query` over the revisions the reader serves.
pub fn answer(
    reader: &dyn RevisionReader,
    source: &SourceRef,
    query: &AdvisoryQuery,
    inventory_limits: &InventoryLimits,
    feed_limits: &FeedLimits,
) -> Result<AdvisoryAnswer, ScmError> {
    let inventory_query = InventoryQuery {
        revision: query.revision.clone(),
        base: None,
        dependents: None,
        path: query.path.clone(),
    };
    let packages = inventory(reader, source, &inventory_query, inventory_limits)?;
    Ok(correlate_inventory(
        &packages,
        evaluate_feed(query, feed_limits),
    ))
}

#[cfg(test)]
mod tests;
