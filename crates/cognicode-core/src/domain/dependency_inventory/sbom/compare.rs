//! Comparison of the resolved inventory with what an SBOM lists.
//!
//! The SBOM is a claim by its producer about what an artifact contains. The
//! comparison says where that claim and the lockfile agree, where they
//! disagree and where the SBOM is silent or ambiguous; it never decides
//! which of the two is right and it never turns an SBOM into evidence about
//! the built bytes. Matching is by name and version, then origin and hash
//! are checked against what the lockfile pins. When a decision would need a
//! guess (two components for one package, a PURL with no source) the answer
//! is `ambiguous` or `unresolved`, with the reason.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::model::{
    ComponentFacts, ComponentScope, ParsedSbom, SbomClaim, SbomComponent, SbomStage,
};
use crate::domain::dependency_inventory::declared::DependencyKind;
use crate::domain::dependency_inventory::model::{
    CoverageStatus, DependencyLayer, LayerStatus, ResolvedPackage, WorkspaceInventory,
};
use crate::domain::dependency_inventory::purl::{ParsedPurl, PurlOrigin, parse_purl, purl_origin};
use crate::domain::dependency_inventory::source::PackageSource;

/// The most unmatched components listed individually; the total is always
/// given.
const MAX_LISTED_UNMATCHED: usize = 200;

/// What the caller says about the document.
#[derive(Debug, Clone, Copy, Default)]
pub struct ComparisonRequest<'a> {
    /// The workspace member the SBOM describes, when the caller knows it.
    /// Without it the member is read from the SBOM subject.
    pub member: Option<&'a str>,
    /// The SHA-256 (lowercase hex) of the artifact the caller says the SBOM
    /// describes.
    pub artifact_digest: Option<&'a str>,
}

/// The result of one comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    /// The SBOM and the lockfile say the same.
    Agrees,
    /// They say different things.
    Differs,
    /// One side does not say enough to decide.
    Unresolved,
}

/// One check of a listed package, with the reason unless it agrees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    /// The result.
    pub state: CheckState,
    /// Why it differs or could not be decided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Check {
    fn agrees() -> Self {
        Self {
            state: CheckState::Agrees,
            detail: None,
        }
    }

    fn differs(detail: impl Into<String>) -> Self {
        Self {
            state: CheckState::Differs,
            detail: Some(detail.into()),
        }
    }

    fn unresolved(detail: impl Into<String>) -> Self {
        Self {
            state: CheckState::Unresolved,
            detail: Some(detail.into()),
        }
    }
}

/// Whether the SBOM lists a resolved package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    /// Exactly one component stands for the package.
    Listed,
    /// No component stands for it.
    NotListed,
    /// More than one component could stand for it and nothing tells which.
    Ambiguous,
}

/// Whether the subject's declarations say the package is part of what it
/// ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Expectation {
    /// Reachable from the subject through a normal dependency.
    Shipping,
    /// Reachable from the subject only through dev or build dependencies.
    NonShipping,
    /// No declaration of the subject reaches it.
    NotReachable,
    /// The subject is unknown: nothing can be said.
    Unassessed,
}

/// The component of the SBOM that stands for a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedComponent {
    /// The `bom-ref`.
    pub bom_ref: Option<String>,
    /// The PURL as the SBOM writes it.
    pub purl: Option<String>,
    /// The stated scope.
    pub scope: ComponentScope,
    /// The licenses it states.
    pub licenses: Vec<String>,
}

/// The comparison of one resolved package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageDistribution {
    /// The package id of the inventory.
    pub package: String,
    /// Whether the SBOM lists it.
    pub presence: Presence,
    /// Whether the subject's declarations say it ships.
    pub expectation: Expectation,
    /// The kinds of declaration through which the subject reaches it.
    pub reached_through: BTreeSet<DependencyKind>,
    /// The component, when exactly one stands for the package.
    pub component: Option<ListedComponent>,
    /// The origin the PURL names against the source the lockfile pins.
    pub origin: Check,
    /// The SBOM hash against the lockfile checksum.
    pub digest: Check,
}

/// Why a component stands for no resolved package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnmatchedComponent {
    /// The component name.
    pub name: String,
    /// The component version.
    pub version: Option<String>,
    /// The PURL.
    pub purl: Option<String>,
    /// The `bom-ref`.
    pub bom_ref: Option<String>,
    /// Why nothing matched.
    pub reason: String,
}

/// The kinds of discrepancy between the SBOM and the inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    /// The document states a source stage: it describes what manifests ask
    /// for, not what was shipped.
    SourceStage,
    /// The document states both source and artifact stages.
    StageAmbiguous,
    /// The SBOM subject is not a workspace member of the unit: shipping
    /// cannot be assessed.
    SubjectUnresolved,
    /// The subject version differs from the member version in the revision.
    SubjectVersionDiffers,
    /// The artifact digest the caller gave differs from the one the SBOM
    /// states for its subject.
    ArtifactDigestMismatch,
    /// A direct, unconditional normal dependency of the subject that the
    /// SBOM does not list.
    RequiredNotListed,
    /// A package reachable from the subject (transitively, or through an
    /// optional or target-specific declaration) that the SBOM does not
    /// list. The lockfile spans every target and feature, so this can be
    /// legitimate.
    ReachableNotListed,
    /// Listed, but reachable only through dev or build declarations.
    NonShippingListed,
    /// Listed, but no declaration of the subject reaches it.
    ListedNotReachable,
    /// Listed with `scope: excluded`, yet a normal dependency reaches it.
    ScopeConflict,
    /// The PURL names a different origin than the lockfile pins.
    OriginMismatch,
    /// The SBOM hash differs from the lockfile checksum.
    DigestMismatch,
    /// More than one component could stand for the package.
    AmbiguousMatch,
    /// A component no resolved package matches.
    ListedNotResolved,
}

/// One discrepancy, with the subject it is about.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DistributionFinding {
    /// What kind of discrepancy.
    pub kind: FindingKind,
    /// The package, component or document it is about.
    pub subject: String,
    /// What exactly differs.
    pub detail: String,
}

/// How the SBOM is bound to an artifact digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingState {
    /// The caller gave no artifact digest.
    NotRequested,
    /// The SBOM states a SHA-256 for its subject and it is the one given.
    Agrees,
    /// The SBOM states SHA-256 hashes for its subject and none is the one
    /// given.
    Differs,
    /// The SBOM states no SHA-256 for its subject: it cannot be bound to an
    /// artifact digest.
    NotStatedBySbom,
}

/// The link between the SBOM and an artifact digest, as far as the document
/// itself goes. Not a verification of the artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactBinding {
    /// The digest the caller gave.
    pub requested: Option<String>,
    /// How the SBOM relates to it.
    pub state: BindingState,
}

/// Which workspace member the SBOM was compared as.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SubjectResolution {
    /// The member whose declarations decide what ships.
    Member {
        /// The member name.
        name: String,
        /// The version in the revision.
        version: Option<String>,
    },
    /// No single member could be named.
    Unresolved {
        /// Why.
        reason: String,
    },
}

/// Counts of the comparison, for a summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DistributionCounts {
    /// Resolved packages compared (the subject itself is not).
    pub packages: usize,
    /// Packages exactly one component stands for.
    pub listed: usize,
    /// Packages no component stands for.
    pub not_listed: usize,
    /// Packages more than one component could stand for.
    pub ambiguous: usize,
    /// Listed packages whose origin agrees.
    pub origin_agrees: usize,
    /// Listed packages whose origin differs.
    pub origin_differs: usize,
    /// Listed packages whose origin could not be decided.
    pub origin_unresolved: usize,
    /// Listed packages whose hash agrees.
    pub digest_agrees: usize,
    /// Listed packages whose hash differs.
    pub digest_differs: usize,
    /// Listed packages whose hash could not be decided.
    pub digest_unresolved: usize,
    /// Components no resolved package matches.
    pub unmatched_components: usize,
    /// Components that are not Cargo packages (other PURL types, files,
    /// operating systems): not comparable with a Cargo inventory.
    pub not_comparable_components: usize,
    /// Identical repeated components that were merged.
    pub duplicates_merged: usize,
}

/// The comparison of one unit's inventory with one SBOM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributionReport {
    /// What the SBOM claims about itself and how completely it was read.
    pub claim: SbomClaim,
    /// The member the comparison was made for.
    pub subject: SubjectResolution,
    /// How the SBOM relates to the artifact digest the caller gave.
    pub artifact_binding: ArtifactBinding,
    /// One entry per resolved package, by id.
    pub packages: Vec<PackageDistribution>,
    /// The first components that no resolved package matches.
    pub unmatched: Vec<UnmatchedComponent>,
    /// Every discrepancy.
    pub findings: Vec<DistributionFinding>,
    /// The counts.
    pub counts: DistributionCounts,
}

type Key = (String, String);

/// A component and the identity it claims.
struct Identified<'a> {
    component: &'a SbomComponent,
    purl: Option<ParsedPurl>,
}

enum Identity {
    Cargo(Key),
    /// A Cargo component whose version is unknown.
    Unversioned,
    NotCargo,
}

fn identify(item: &Identified<'_>) -> Identity {
    let component = item.component;
    match &item.purl {
        Some(purl) if purl.purl_type != "cargo" => Identity::NotCargo,
        Some(purl) => match purl.version.clone().or_else(|| component.version.clone()) {
            Some(version) => Identity::Cargo((purl.name.clone(), version)),
            None => Identity::Unversioned,
        },
        None if matches!(
            component.component_type.as_str(),
            "library" | "framework" | "application" | "unstated"
        ) =>
        {
            match &component.version {
                Some(version) => Identity::Cargo((component.name.clone(), version.clone())),
                None => Identity::Unversioned,
            }
        }
        None => Identity::NotCargo,
    }
}

fn normalise_url(url: &str) -> String {
    let url = url.strip_prefix("git+").unwrap_or(url);
    url.trim_end_matches('/')
        .trim_end_matches(".git")
        .to_ascii_lowercase()
}

fn commits_agree(sbom: &str, lock: &str) -> bool {
    let (sbom, lock) = (sbom.to_ascii_lowercase(), lock.to_ascii_lowercase());
    lock.starts_with(&sbom) || sbom.starts_with(&lock)
}

/// The origin the PURL names against the source the lockfile pins.
fn origin_check(source: &PackageSource, purl: Option<&ParsedPurl>) -> Check {
    let Some(purl) = purl else {
        return Check::unresolved("the component has no PURL, so it names no origin");
    };
    let origin = purl_origin(purl);
    match (source, &origin) {
        (PackageSource::Unknown { raw }, _) => Check::unresolved(format!(
            "the lockfile source `{raw}` is in a grammar this reader does not know"
        )),
        (PackageSource::Registry { .. }, PurlOrigin::Unspecified)
            if source.is_default_registry() =>
        {
            Check::agrees()
        }
        (PackageSource::Registry { url }, PurlOrigin::Unspecified) => Check::unresolved(format!(
            "the PURL names no source, which reads as crates.io; the lockfile pins `{url}`"
        )),
        (PackageSource::Registry { url }, PurlOrigin::Registry { url: named }) => {
            if source.is_default_registry() && PackageSource::is_default_index(named)
                || normalise_url(
                    url.trim_start_matches("registry+")
                        .trim_start_matches("sparse+"),
                ) == normalise_url(named)
            {
                Check::agrees()
            } else {
                Check::differs(format!(
                    "the PURL names the registry `{named}`, the lockfile `{url}`"
                ))
            }
        }
        (
            PackageSource::Git { url, commit, .. },
            PurlOrigin::Git {
                url: named,
                commit: named_commit,
            },
        ) => {
            if normalise_url(url) != normalise_url(named) {
                Check::differs(format!(
                    "the PURL names the repository `{named}`, the lockfile `{url}`"
                ))
            } else {
                match named_commit {
                    Some(named_commit) if commits_agree(named_commit, commit) => Check::agrees(),
                    Some(named_commit) => Check::differs(format!(
                        "the PURL pins commit {named_commit}, the lockfile {commit}"
                    )),
                    None => Check::unresolved("the PURL names the repository but no commit"),
                }
            }
        }
        (PackageSource::Git { .. }, PurlOrigin::Unspecified) => Check::unresolved(
            "the PURL names no source, which reads as crates.io; the lockfile pins a Git commit",
        ),
        (PackageSource::Path, PurlOrigin::Local) => Check::agrees(),
        (PackageSource::Path, PurlOrigin::Unspecified) => Check::unresolved(
            "the PURL names no source; the lockfile has a path package with no registry identity",
        ),
        (source, origin) => Check::differs(format!(
            "the PURL names a {} origin, the lockfile a {} source",
            origin_label(origin),
            source.kind()
        )),
    }
}

fn origin_label(origin: &PurlOrigin) -> &'static str {
    match origin {
        PurlOrigin::Unspecified => "unspecified",
        PurlOrigin::Registry { .. } => "registry",
        PurlOrigin::Git { .. } => "git",
        PurlOrigin::Local => "local",
    }
}

fn digest_check(package: &ResolvedPackage, component: &SbomComponent) -> Check {
    match (&package.checksum, component.sha256.is_empty()) {
        (None, _) => Check::unresolved(format!(
            "the lockfile records no checksum for this {} package",
            package.source.kind()
        )),
        (Some(_), true) => Check::unresolved(if component.other_hash_algorithms.is_empty() {
            "the SBOM states no hash for the component".to_string()
        } else {
            format!(
                "the SBOM states no SHA-256 for the component, only {}",
                component.other_hash_algorithms.join(", ")
            )
        }),
        (Some(checksum), false) if component.sha256.iter().any(|hash| hash == checksum) => {
            Check::agrees()
        }
        (Some(checksum), false) => Check::differs(format!(
            "the SBOM states SHA-256 {}, the lockfile records {checksum}",
            component.sha256.join(", ")
        )),
    }
}

/// The kinds of declaration through which `member` reaches each package. A
/// lockfile does not record the kind of an edge between two non-members, so
/// the kinds are an upper bound: a build-only dependency of a registry
/// crate carries the kind of the member edge that reached the crate.
fn reach_from(
    unit: &WorkspaceInventory,
    member: &str,
) -> BTreeMap<String, BTreeSet<DependencyKind>> {
    let by_id: BTreeMap<&str, &ResolvedPackage> = unit
        .packages
        .iter()
        .map(|package| (package.id.as_str(), package))
        .collect();
    let mut declared: BTreeMap<&str, Vec<(&str, DependencyKind)>> = BTreeMap::new();
    for package in &unit.packages {
        for usage in &package.declared_by {
            declared
                .entry(usage.member.as_str())
                .or_default()
                .push((package.id.as_str(), usage.declaration.kind));
        }
    }
    let mut reach: BTreeMap<String, BTreeSet<DependencyKind>> = BTreeMap::new();
    let mut stack: Vec<(&str, DependencyKind)> = declared
        .get(member)
        .into_iter()
        .flatten()
        .map(|&(id, kind)| (id, kind))
        .collect();
    while let Some((id, carried)) = stack.pop() {
        if !reach.entry(id.to_string()).or_default().insert(carried) {
            continue;
        }
        let Some(package) = by_id.get(id) else {
            continue;
        };
        if package.workspace_member {
            for &(next, kind) in declared.get(package.name.as_str()).into_iter().flatten() {
                match kind {
                    DependencyKind::Development => {}
                    DependencyKind::Build => stack.push((next, DependencyKind::Build)),
                    DependencyKind::Normal => stack.push((next, carried)),
                }
            }
        } else {
            stack.extend(
                package
                    .dependencies
                    .iter()
                    .map(|next| (next.as_str(), carried)),
            );
        }
    }
    reach
}

fn resolve_subject(
    unit: &WorkspaceInventory,
    sbom: &ParsedSbom,
    request: &ComparisonRequest<'_>,
) -> SubjectResolution {
    let names: Vec<String> = match request.member {
        Some(member) => vec![member.to_string()],
        None => sbom
            .subject
            .iter()
            .flat_map(|subject| {
                let purl_name = subject
                    .purl
                    .as_deref()
                    .and_then(|raw| parse_purl(raw).ok())
                    .map(|purl| purl.name);
                purl_name.into_iter().chain([subject.name.clone()])
            })
            .collect(),
    };
    if names.is_empty() {
        return SubjectResolution::Unresolved {
            reason: "the SBOM does not say what it describes".to_string(),
        };
    }
    for name in &names {
        let matching: Vec<_> = unit
            .members
            .iter()
            .filter(|member| member.name == *name)
            .collect();
        if let [member] = matching.as_slice() {
            return SubjectResolution::Member {
                name: member.name.clone(),
                version: member.version.clone(),
            };
        }
    }
    SubjectResolution::Unresolved {
        reason: format!(
            "`{}` is not a workspace member of this unit; name the member the SBOM describes",
            names.join("` or `")
        ),
    }
}

fn artifact_binding(sbom: &ParsedSbom, request: &ComparisonRequest<'_>) -> ArtifactBinding {
    let requested = request.artifact_digest.map(str::to_string);
    let state = match (request.artifact_digest, sbom.subject.as_ref()) {
        (None, _) => BindingState::NotRequested,
        (Some(_), None) => BindingState::NotStatedBySbom,
        (Some(_), Some(subject)) if subject.sha256.is_empty() => BindingState::NotStatedBySbom,
        (Some(digest), Some(subject)) if subject.sha256.iter().any(|hash| hash == digest) => {
            BindingState::Agrees
        }
        (Some(_), Some(_)) => BindingState::Differs,
    };
    ArtifactBinding { requested, state }
}

/// Whether two components repeat each other exactly.
fn identical(a: &SbomComponent, b: &SbomComponent) -> bool {
    a.purl == b.purl && a.sha256 == b.sha256 && a.scope == b.scope
}

struct Matching<'a> {
    /// Per key, the distinct components after merging exact repeats.
    by_key: BTreeMap<Key, Vec<&'a Identified<'a>>>,
    duplicates_merged: usize,
    unversioned: Vec<&'a Identified<'a>>,
    not_comparable: usize,
}

fn group<'a>(items: &'a [Identified<'a>]) -> Matching<'a> {
    let mut matching = Matching {
        by_key: BTreeMap::new(),
        duplicates_merged: 0,
        unversioned: Vec::new(),
        not_comparable: 0,
    };
    for item in items {
        match identify(item) {
            Identity::NotCargo => matching.not_comparable += 1,
            Identity::Unversioned => matching.unversioned.push(item),
            Identity::Cargo(key) => {
                let slot = matching.by_key.entry(key).or_default();
                if slot
                    .iter()
                    .any(|seen| identical(seen.component, item.component))
                {
                    matching.duplicates_merged += 1;
                } else {
                    slot.push(item);
                }
            }
        }
    }
    matching
}

/// What the subject's declarations say about one package.
struct Reach<'a> {
    kinds: &'a BTreeSet<DependencyKind>,
    unconditional_direct: bool,
}

fn expectation(kinds: &BTreeSet<DependencyKind>, assessed: bool) -> Expectation {
    if !assessed {
        Expectation::Unassessed
    } else if kinds.contains(&DependencyKind::Normal) {
        Expectation::Shipping
    } else if kinds.is_empty() {
        Expectation::NotReachable
    } else {
        Expectation::NonShipping
    }
}

fn is_unconditional_direct(package: &ResolvedPackage, member: &str) -> bool {
    package.declared_by.iter().any(|usage| {
        usage.member == member
            && usage.declaration.kind == DependencyKind::Normal
            && usage.declaration.target.is_none()
            && !usage.declaration.optional
    })
}

struct Judged {
    row: PackageDistribution,
    findings: Vec<DistributionFinding>,
}

fn finding(kind: FindingKind, subject: &str, detail: impl Into<String>) -> DistributionFinding {
    DistributionFinding {
        kind,
        subject: subject.to_string(),
        detail: detail.into(),
    }
}

fn judge_listed(
    package: &ResolvedPackage,
    item: &Identified<'_>,
    reach: &Reach<'_>,
    expectation: Expectation,
) -> Judged {
    let component = item.component;
    let origin = origin_check(&package.source, item.purl.as_ref());
    let digest = digest_check(package, component);
    let mut findings = Vec::new();
    if let Some(detail) = origin
        .detail
        .as_ref()
        .filter(|_| origin.state == CheckState::Differs)
    {
        findings.push(finding(FindingKind::OriginMismatch, &package.id, detail));
    }
    if let Some(detail) = digest
        .detail
        .as_ref()
        .filter(|_| digest.state == CheckState::Differs)
    {
        findings.push(finding(FindingKind::DigestMismatch, &package.id, detail));
    }
    let kinds: Vec<&str> = reach.kinds.iter().map(|kind| kind.table()).collect();
    match expectation {
        Expectation::NonShipping if component.scope != ComponentScope::Excluded => {
            findings.push(finding(
                FindingKind::NonShippingListed,
                &package.id,
                format!(
                    "listed in the SBOM but reachable from the subject only through {}",
                    kinds.join(", ")
                ),
            ));
        }
        Expectation::NotReachable => findings.push(finding(
            FindingKind::ListedNotReachable,
            &package.id,
            "listed in the SBOM but no declaration of the subject reaches it",
        )),
        Expectation::Shipping if component.scope == ComponentScope::Excluded => {
            findings.push(finding(
                FindingKind::ScopeConflict,
                &package.id,
                "the SBOM marks it excluded, yet the subject reaches it through a normal dependency",
            ));
        }
        _ => {}
    }
    Judged {
        row: PackageDistribution {
            package: package.id.clone(),
            presence: Presence::Listed,
            expectation,
            reached_through: reach.kinds.clone(),
            component: Some(ListedComponent {
                bom_ref: component.bom_ref.clone(),
                purl: component.purl.clone(),
                scope: component.scope,
                licenses: component.licenses.clone(),
            }),
            origin,
            digest,
        },
        findings,
    }
}

fn judge_absent(
    package: &ResolvedPackage,
    presence: Presence,
    candidates: &[&Identified<'_>],
    reach: &Reach<'_>,
    expectation: Expectation,
) -> Judged {
    let mut findings = Vec::new();
    match presence {
        Presence::Ambiguous => {
            let refs: Vec<String> = candidates
                .iter()
                .map(|item| {
                    item.component
                        .bom_ref
                        .clone()
                        .or_else(|| item.component.purl.clone())
                        .unwrap_or_else(|| item.component.name.clone())
                })
                .collect();
            findings.push(finding(
                FindingKind::AmbiguousMatch,
                &package.id,
                format!(
                    "{} components could stand for it and nothing tells which: {}",
                    candidates.len(),
                    refs.join(", ")
                ),
            ));
        }
        _ if expectation == Expectation::Shipping && reach.unconditional_direct => {
            findings.push(finding(
                FindingKind::RequiredNotListed,
                &package.id,
                "a direct dependency of the subject that the SBOM does not list",
            ));
        }
        _ if expectation == Expectation::Shipping => findings.push(finding(
            FindingKind::ReachableNotListed,
            &package.id,
            "reachable from the subject but not listed; the lockfile spans every target and feature, so the artifact may legitimately omit it",
        )),
        _ => {}
    }
    let unresolved = |reason: &str| Check::unresolved(reason);
    Judged {
        row: PackageDistribution {
            package: package.id.clone(),
            presence,
            expectation,
            reached_through: reach.kinds.clone(),
            component: None,
            origin: unresolved("the package is not listed by exactly one component"),
            digest: unresolved("the package is not listed by exactly one component"),
        },
        findings,
    }
}

/// The component that stands for `package`, or why none does.
fn pick<'a>(
    package: &ResolvedPackage,
    candidates: &[&'a Identified<'a>],
    peers: usize,
) -> Result<&'a Identified<'a>, (Presence, Vec<&'a Identified<'a>>)> {
    let origin_of = |item: &Identified<'_>| origin_check(&package.source, item.purl.as_ref()).state;
    match candidates {
        [] => Err((Presence::NotListed, Vec::new())),
        [only] if peers <= 1 => Ok(only),
        [only] => match origin_of(only) {
            CheckState::Agrees => Ok(only),
            CheckState::Differs => Err((Presence::NotListed, Vec::new())),
            CheckState::Unresolved => Err((Presence::Ambiguous, vec![*only])),
        },
        many => {
            let agreeing: Vec<&Identified<'_>> = many
                .iter()
                .copied()
                .filter(|item| origin_of(item) == CheckState::Agrees)
                .collect();
            match agreeing.as_slice() {
                [one] => Ok(one),
                _ => Err((Presence::Ambiguous, many.to_vec())),
            }
        }
    }
}

fn unmatched_reason(item: &Identified<'_>, versions: &BTreeMap<&str, Vec<&str>>) -> String {
    let component = item.component;
    let name = item
        .purl
        .as_ref()
        .map_or(component.name.as_str(), |purl| purl.name.as_str());
    match versions.get(name) {
        Some(locked) => format!(
            "the lockfile resolves `{name}` only at {}",
            locked.join(", ")
        ),
        None => format!("the lockfile resolves no package named `{name}`"),
    }
}

/// Compares the resolved inventory of one unit with an SBOM.
pub fn compare_distribution(
    unit: &WorkspaceInventory,
    sbom: &ParsedSbom,
    request: &ComparisonRequest<'_>,
) -> DistributionReport {
    let subject = resolve_subject(unit, sbom, request);
    let subject_name = match &subject {
        SubjectResolution::Member { name, .. } => Some(name.as_str()),
        SubjectResolution::Unresolved { .. } => None,
    };
    let reach = subject_name
        .map(|name| reach_from(unit, name))
        .unwrap_or_default();
    let empty = BTreeSet::new();
    let items: Vec<Identified<'_>> = sbom
        .components
        .iter()
        .map(|component| Identified {
            component,
            purl: component
                .purl
                .as_deref()
                .and_then(|raw| parse_purl(raw).ok()),
        })
        .collect();
    let matching = group(&items);
    let mut peers: BTreeMap<Key, usize> = BTreeMap::new();
    for package in &unit.packages {
        *peers
            .entry((package.name.clone(), package.version.clone()))
            .or_default() += 1;
    }
    let is_subject = |package: &ResolvedPackage| {
        package.workspace_member && subject_name == Some(package.name.as_str())
    };
    let mut rows = Vec::new();
    let mut findings = Vec::new();
    let mut used: BTreeSet<Key> = BTreeSet::new();
    for package in unit.packages.iter().filter(|package| !is_subject(package)) {
        let key = (package.name.clone(), package.version.clone());
        let candidates: &[&Identified<'_>] = matching.by_key.get(&key).map_or(&[], Vec::as_slice);
        let kinds = reach.get(&package.id).unwrap_or(&empty);
        let package_reach = Reach {
            kinds,
            unconditional_direct: subject_name
                .is_some_and(|name| is_unconditional_direct(package, name)),
        };
        let expected = expectation(kinds, subject_name.is_some());
        let judged = match pick(package, candidates, peers[&key]) {
            Ok(item) => {
                used.insert(key);
                judge_listed(package, item, &package_reach, expected)
            }
            Err((presence, tied)) => {
                if presence == Presence::Ambiguous {
                    used.insert(key);
                }
                judge_absent(package, presence, &tied, &package_reach, expected)
            }
        };
        rows.push(judged.row);
        findings.extend(judged.findings);
    }
    if let SubjectResolution::Member { name, version } = &subject {
        used.insert((name.clone(), version.clone().unwrap_or_default()));
    }

    let mut locked_versions: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for package in &unit.packages {
        locked_versions
            .entry(package.name.as_str())
            .or_default()
            .push(package.version.as_str());
    }
    let mut unmatched: Vec<UnmatchedComponent> = Vec::new();
    let stray = matching
        .by_key
        .iter()
        .filter(|(key, _)| !used.contains(*key))
        .flat_map(|(_, items)| items.iter().copied())
        .map(|item| (item, None))
        .chain(
            matching
                .unversioned
                .iter()
                .map(|item| (*item, Some("the component has no version"))),
        );
    for (item, fixed) in stray {
        let reason = fixed.map_or_else(|| unmatched_reason(item, &locked_versions), str::to_string);
        let component = item.component;
        let label = component
            .bom_ref
            .clone()
            .unwrap_or_else(|| component.name.clone());
        findings.push(finding(
            FindingKind::ListedNotResolved,
            &label,
            reason.clone(),
        ));
        unmatched.push(UnmatchedComponent {
            name: component.name.clone(),
            version: component.version.clone(),
            purl: component.purl.clone(),
            bom_ref: component.bom_ref.clone(),
            reason,
        });
    }
    let unmatched_total = unmatched.len();
    unmatched.truncate(MAX_LISTED_UNMATCHED);

    let binding = artifact_binding(sbom, request);
    stage_findings(sbom, &subject, unit, &binding, &mut findings);
    findings.sort();
    let counts = count(&rows, unmatched_total, &matching);
    DistributionReport {
        claim: sbom.claim(),
        subject,
        artifact_binding: binding,
        packages: rows,
        unmatched,
        findings,
        counts,
    }
}

fn stage_findings(
    sbom: &ParsedSbom,
    subject: &SubjectResolution,
    unit: &WorkspaceInventory,
    binding: &ArtifactBinding,
    findings: &mut Vec<DistributionFinding>,
) {
    let document = sbom
        .serial_number
        .clone()
        .unwrap_or_else(|| "the document".to_string());
    match sbom.stage {
        SbomStage::Source => findings.push(finding(
            FindingKind::SourceStage,
            &document,
            "the SBOM states a pre-build stage: it lists what manifests ask for, not what was shipped",
        )),
        SbomStage::Ambiguous => findings.push(finding(
            FindingKind::StageAmbiguous,
            &document,
            "the SBOM states both source and artifact lifecycle phases",
        )),
        SbomStage::Artifact | SbomStage::Unstated => {}
    }
    match subject {
        SubjectResolution::Unresolved { reason } => {
            findings.push(finding(
                FindingKind::SubjectUnresolved,
                &document,
                reason.clone(),
            ));
        }
        SubjectResolution::Member { name, version } => {
            let claimed = sbom.subject.as_ref().and_then(|s| s.version.as_deref());
            if let (Some(claimed), Some(actual)) = (claimed, version.as_deref())
                && claimed != actual
            {
                findings.push(finding(
                    FindingKind::SubjectVersionDiffers,
                    name,
                    format!(
                        "the SBOM describes version {claimed}, the revision has {actual} in {}",
                        unit.root
                    ),
                ));
            }
        }
    }
    if binding.state == BindingState::Differs {
        findings.push(finding(
            FindingKind::ArtifactDigestMismatch,
            &document,
            "the SBOM states SHA-256 hashes for its subject and none is the artifact digest given",
        ));
    }
}

fn count(
    rows: &[PackageDistribution],
    unmatched: usize,
    matching: &Matching<'_>,
) -> DistributionCounts {
    let mut counts = DistributionCounts {
        packages: rows.len(),
        unmatched_components: unmatched,
        not_comparable_components: matching.not_comparable,
        duplicates_merged: matching.duplicates_merged,
        ..DistributionCounts::default()
    };
    for row in rows {
        match row.presence {
            Presence::Listed => counts.listed += 1,
            Presence::NotListed => counts.not_listed += 1,
            Presence::Ambiguous => counts.ambiguous += 1,
        }
        if row.presence != Presence::Listed {
            continue;
        }
        match row.origin.state {
            CheckState::Agrees => counts.origin_agrees += 1,
            CheckState::Differs => counts.origin_differs += 1,
            CheckState::Unresolved => counts.origin_unresolved += 1,
        }
        match row.digest.state {
            CheckState::Agrees => counts.digest_agrees += 1,
            CheckState::Differs => counts.digest_differs += 1,
            CheckState::Unresolved => counts.digest_unresolved += 1,
        }
    }
    counts
}

impl DistributionReport {
    /// The state of the distributed layer after this comparison.
    ///
    /// An SBOM is a claim of its producer: the layer is at best `partial`,
    /// because the artifact itself was not inspected. A source-stage SBOM
    /// does not describe a distribution at all.
    pub fn layer(&self) -> LayerStatus {
        let (status, note) = match self.claim.stage {
            SbomStage::Source => (
                CoverageStatus::NotCollected,
                "the SBOM states a pre-build stage: it lists what sources ask for, not what was shipped".to_string(),
            ),
            _ => (
                CoverageStatus::Partial,
                format!(
                    "read from an SBOM, a claim by its producer that was not checked against the artifact: {} of {} resolved package(s) listed",
                    self.counts.listed, self.counts.packages
                ),
            ),
        };
        LayerStatus {
            layer: DependencyLayer::Distributed,
            status,
            note,
        }
    }

    /// The facts of the SBOM that a comparison read.
    pub fn facts(&self) -> ComponentFacts {
        self.claim.components
    }
}
