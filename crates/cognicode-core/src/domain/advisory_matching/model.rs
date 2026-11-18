//! What advisory correlation observes about one resolved package.
//!
//! The values here are observations. They say what a pinned feed contains
//! about a package version; they are not an exploitability verdict, they do
//! not replace the verification of an artifact's signature or provenance, and
//! the decision of what a match means for a release belongs to policy.

use serde::{Deserialize, Serialize};

/// A resolved package, reduced to what correlation reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject {
    /// The package id inside its inventory (`name@version`).
    pub id: String,
    /// The crate name.
    pub name: String,
    /// The locked version.
    pub version: String,
    /// Its Package URL identity, when it has one.
    pub purl: Option<String>,
    /// Whether the package comes from crates.io, the ecosystem the feed's
    /// advisories are about. A path, Git or alternative-registry package is
    /// not the crate of the same name on crates.io.
    pub from_crates_io: bool,
}

/// What is known about a package after correlation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageState {
    /// At least one advisory of the feed covers this exact version.
    Affected,
    /// The feed was evaluated completely and no advisory covers this
    /// version. Valid for that feed, at that digest, and nothing more.
    NotFoundInScope,
    /// Matching could not conclude: see the reasons.
    Unknown,
    /// The package is outside what the feed can speak about.
    NotEvaluated,
}

/// Why a package is not `affected` or `not_found_in_scope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// The package is not from crates.io, so crates.io advisories do not
    /// apply to it.
    SourceNotInEcosystem,
    /// An advisory names this crate but its ranges or the locked version
    /// could not be decided.
    UndecidableRange,
    /// Some records of the feed could not be read, so a clean result would
    /// not be a statement about the whole feed.
    FeedIncomplete,
    /// The feed is older than the window the operator accepted.
    FeedStale,
    /// The feed has no advisory about crates.io, so it cannot say anything
    /// about Cargo packages.
    EcosystemNotCovered,
    /// There is no usable feed: it is absent, unreadable, does not match the
    /// pinned digest or cannot be parsed.
    FeedUnavailable,
    /// The unit has no lockfile, so there are no resolved versions to
    /// correlate.
    NoLockfile,
}

/// How firmly an advisory covers a package version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Certainty {
    /// A version list or a range evaluated to this version.
    Affected,
    /// The advisory names the crate but could not be decided for this
    /// version.
    Undecided,
}

/// One advisory, as it applies to one package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvisoryHit {
    /// The advisory identifier chosen as the name of the cluster (the
    /// smallest identifier among the records that describe it).
    pub id: String,
    /// Every other identifier of the same advisory: aliases and the ids of
    /// the other feed records that describe it.
    pub aliases: Vec<String>,
    /// How firmly it covers the version.
    pub certainty: Certainty,
    /// The versions the advisory says fix it, as the feed writes them.
    pub fixed_in: Vec<String>,
    /// A short summary the feed gives, sanitised and length-limited.
    pub summary: Option<String>,
    /// The latest `modified` among the records of the cluster.
    pub modified: String,
}

/// The correlation of one package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageResult {
    /// The package id inside its inventory.
    pub package: String,
    /// The crate name.
    pub name: String,
    /// The locked version.
    pub version: String,
    /// The Package URL identity the match was keyed on, when it has one.
    pub purl: Option<String>,
    /// What is known.
    pub state: PackageState,
    /// Why the state is not conclusive, sorted.
    pub reasons: Vec<Reason>,
    /// The advisories that cover or may cover this version, by id.
    pub advisories: Vec<AdvisoryHit>,
}

/// Whether finding no advisory may be reported as `not_found_in_scope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Absence {
    /// The feed was read completely, is fresh enough and covers crates.io.
    Conclusive,
    /// It may not: absence of a match says nothing, for this reason.
    Inconclusive(Reason),
}
