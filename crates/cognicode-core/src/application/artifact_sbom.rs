//! The distributed layer of the dependency inventory, read from an SBOM.
//!
//! An SBOM an operator supplies is a claim by its producer about what an
//! artifact contains. This use case binds the claim to the digest of its
//! bytes, reads it within bounds, compares it with the resolved inventory of
//! the one unit it can describe and says how much that lets the distributed
//! layer claim. It never inspects the artifact, never verifies a signature
//! and never fetches anything the document points at.

use serde::{Deserialize, Serialize};

use crate::domain::dependency_inventory::sbom::{
    ComparisonRequest, DistributionReport, SbomError, SbomLimits, compare_distribution,
    parse_cyclonedx,
};
use crate::domain::dependency_inventory::{DependencyLayer, WorkspaceInventory};
use crate::domain::scm::ContentDigest;

/// What the caller managed to obtain of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SbomSupply {
    /// The document was named but could not be read.
    Unreadable {
        /// Why, in the caller's words.
        reason: String,
    },
    /// The bytes of the document.
    Bytes {
        /// Where the document came from, for the report.
        origin: String,
        /// The bytes.
        bytes: Vec<u8>,
    },
}

/// What a consumer asks about the distributed layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributionQuery {
    /// The document.
    pub sbom: SbomSupply,
    /// The digest the document is pinned to. A document whose bytes differ
    /// is not used.
    pub expected_digest: Option<ContentDigest>,
    /// The SHA-256 of the artifact the document is said to describe.
    pub artifact_digest: Option<ContentDigest>,
    /// The workspace member the document describes, when it cannot be read
    /// from the document.
    pub member: Option<String>,
}

/// Whether the document could be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SbomStatus {
    /// The document was read.
    Usable,
    /// The document could not be read from where it was named.
    Unreadable,
    /// The bytes are not the pinned ones.
    DigestMismatch,
    /// The bytes are not a document this reader accepts.
    Refused,
}

/// The document as the answer anchors itself to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SbomDocument {
    /// Where it came from.
    pub origin: String,
    /// The SHA-256 of its bytes.
    pub digest: String,
    /// Its size in bytes.
    pub bytes: usize,
    /// Whether the caller pinned the digest and the bytes matched it.
    pub pinned: bool,
}

/// Whether a unit was compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributionEvaluation {
    /// The unit was compared with the document.
    Evaluated,
    /// The unit was not compared; the reason says why.
    NotEvaluated,
}

/// The comparison of one unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitDistribution {
    /// The unit root.
    pub root: String,
    /// Whether it was compared.
    pub evaluation: DistributionEvaluation,
    /// Why it was not compared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The comparison.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<DistributionReport>,
}

/// What the document and the comparison say, with what could not be said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributionAnswer {
    /// Whether the document could be used.
    pub status: SbomStatus,
    /// Why not, when it could not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
    /// The document, when its bytes were obtained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<SbomDocument>,
    /// One entry per unit in scope.
    pub units: Vec<UnitDistribution>,
}

fn unusable(
    status: SbomStatus,
    problem: String,
    document: Option<SbomDocument>,
    units: &[WorkspaceInventory],
) -> DistributionAnswer {
    DistributionAnswer {
        status,
        problem: Some(problem),
        document,
        units: units
            .iter()
            .map(|unit| not_evaluated(unit, "the SBOM could not be used".to_string()))
            .collect(),
    }
}

fn not_evaluated(unit: &WorkspaceInventory, reason: String) -> UnitDistribution {
    UnitDistribution {
        root: unit.root.clone(),
        evaluation: DistributionEvaluation::NotEvaluated,
        reason: Some(reason),
        report: None,
    }
}

fn describe(error: &SbomError) -> String {
    error.to_string()
}

/// Compares the one unit an SBOM can describe with it and updates the
/// distributed layer of that unit. Several units in scope are not compared:
/// an SBOM describes one artifact and guessing which unit it is would be a
/// claim nobody made.
pub fn distribute(
    units: &mut [WorkspaceInventory],
    query: &DistributionQuery,
    limits: &SbomLimits,
) -> DistributionAnswer {
    let (origin, bytes) = match &query.sbom {
        SbomSupply::Unreadable { reason } => {
            return unusable(SbomStatus::Unreadable, reason.clone(), None, units);
        }
        SbomSupply::Bytes { origin, bytes } => (origin, bytes),
    };
    let digest = ContentDigest::from_bytes(bytes);
    let mut document = SbomDocument {
        origin: origin.clone(),
        digest: digest.as_hex().to_string(),
        bytes: bytes.len(),
        pinned: false,
    };
    if let Some(expected) = &query.expected_digest {
        if *expected != digest {
            let problem = format!(
                "the SBOM digest {} is not the pinned digest {}",
                digest.as_hex(),
                expected.as_hex()
            );
            return unusable(SbomStatus::DigestMismatch, problem, Some(document), units);
        }
        document.pinned = true;
    }
    let parsed = match parse_cyclonedx(bytes, limits) {
        Ok(parsed) => parsed,
        Err(error) => {
            return unusable(SbomStatus::Refused, describe(&error), Some(document), units);
        }
    };
    let request = ComparisonRequest {
        member: query.member.as_deref(),
        artifact_digest: query.artifact_digest.as_ref().map(ContentDigest::as_hex),
    };
    let several = units.len() > 1;
    let count = units.len();
    let entries = units
        .iter_mut()
        .map(|unit| {
            if several {
                return not_evaluated(
                    unit,
                    format!(
                        "an SBOM describes one artifact and {count} workspace units are in scope: narrow the answer with `path`"
                    ),
                );
            }
            let report = compare_distribution(unit, &parsed, &request);
            let layer = report.layer();
            if let Some(slot) = unit
                .layers
                .iter_mut()
                .find(|slot| slot.layer == DependencyLayer::Distributed)
            {
                *slot = layer;
            }
            UnitDistribution {
                root: unit.root.clone(),
                evaluation: DistributionEvaluation::Evaluated,
                reason: None,
                report: Some(report),
            }
        })
        .collect();
    DistributionAnswer {
        status: SbomStatus::Usable,
        problem: None,
        document: Some(document),
        units: entries,
    }
}

#[cfg(test)]
mod tests;
