//! PlanVersion and PlanHash — versioned, deterministic plan identifiers.
//!
//! Part of e28-1-moldplan-graphplan-contracts: PR1 Foundation Phase 1.
//!
//! ## Design
//!
//! - `PlanVersion` wraps a semver string (`"1.0.0"`, `"0.5.1+custom"`) and
//!   provides strict parsing/validation (only valid semver strings are accepted).
//! - `PlanHash` is the SHA-256 digest (as a hex `String`, 64 chars) of the
//!   **canonicalized** plan bytes — i.e. the output of `serde_json::to_vec`
//!   with a deterministic key order (maps are sorted by key).
//! - Two plans with byte-identical canonical forms produce the same `PlanHash`
//!   regardless of in-memory layout differences.
//! - A plan with `max_hops=3` has a different hash from `max_hops=4` (sensitivity).

use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// Sealed trait — implemented by all plan types to certify backend-neutrality.
use super::neutrality::Sealed;

// ============================================================================
// PlanVersion
// ============================================================================

/// A semver version string for a MoldPlan or GraphPlan.
///
/// `PlanVersion` pins the wire format so that an old executor can reject
/// plans from a future version that has incompatible semantics.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlanVersion(String);

impl Sealed for PlanVersion {}

impl PlanVersion {
    /// Current version of the MoldPlan/GraphPlan wire format.
    pub const CURRENT: &'static str = "1.0.0";

    /// Constructs a `PlanVersion` after validating the string is a valid
    /// semver (major.minor.patch with optional pre-release and build metadata).
    pub fn new(v: impl Into<String>) -> Result<Self, ParsePlanVersionError> {
        let s = v.into();
        // Validate semver format: major.minor.patch with optional pre-release and build
        // Format: major.minor.patch (-prerelease)? (+build)?
        // Per semver 2.0 §2: prerelease identifiers are ASCII alphanumerics and hyphens,
        // with no leading zeros on numeric identifiers.

        // First split off build metadata (+build)
        let (version_and_prerelease, _build) = match s.split_once('+') {
            Some((vp, _b)) => (vp, Some(_b)), // build metadata not yet supported; keep for future
            None => (&s[..], None),
        };

        // Split version from prerelease
        let (version_part, prerelease) = match version_and_prerelease.split_once('-') {
            Some((v, p)) => (v, Some(p)),
            None => (version_and_prerelease, None),
        };

        let mut vparts = version_part.splitn(3, '.');
        let major = vparts.next().ok_or_else(|| ParsePlanVersionError::InvalidSemver(s.clone()))?;
        let minor = vparts.next().ok_or_else(|| ParsePlanVersionError::InvalidSemver(s.clone()))?;
        let patch = vparts.next().ok_or_else(|| ParsePlanVersionError::InvalidSemver(s.clone()))?;

        if major.parse::<u64>().is_err()
            || minor.parse::<u64>().is_err()
            || patch.parse::<u64>().is_err()
        {
            return Err(ParsePlanVersionError::InvalidSemver(s));
        }

        // Validate prerelease format per semver 2.0 §2
        if let Some(pr) = prerelease {
            for segment in pr.split('.') {
                if segment.is_empty() {
                    return Err(ParsePlanVersionError::InvalidSemver(s.clone()));
                }
                // Each segment: ASCII alphanumerics and hyphens only
                if !segment.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                    return Err(ParsePlanVersionError::InvalidSemver(s.clone()));
                }
                // No leading zeros on numeric-only segments
                let chars: Vec<char> = segment.chars().collect();
                if chars.first().map(|c| *c == '0').unwrap_or(false)
                    && segment.len() > 1
                    && chars.get(1).map(|c| c.is_ascii_digit()).unwrap_or(false)
                {
                    return Err(ParsePlanVersionError::InvalidSemver(s.clone()));
                }
            }
        }

        Ok(Self(s))
    }

    /// Returns the version string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PlanVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for PlanVersion {
    fn default() -> Self {
        Self(Self::CURRENT.to_string())
    }
}

/// Error type for [`PlanVersion::new`] failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParsePlanVersionError {
    #[error("invalid semver string: '{0}'")]
    InvalidSemver(String),
}

// ============================================================================
// PlanHash
// ============================================================================

/// SHA-256 digest (hex) of the canonicalized plan bytes.
///
/// The canonical form is `serde_json::to_vec` with a serializer that sorts
/// map keys, so two plans with identical logical content produce the same hash
/// regardless of in-memory key ordering.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlanHash(String);

impl Sealed for PlanHash {}

impl PlanHash {
    /// Compute the `PlanHash` from a serializable value.
    ///
    /// Uses `serde_json::to_vec` internally, which guarantees a deterministic
    /// byte order for maps (key-sorted) when using `serde_json::Serializer`.
    pub fn compute<T: ?Sized + serde::Serialize>(value: &T) -> Self {
        let bytes = serde_json::to_vec(value).expect("value must serialize");
        Self::from_bytes(&bytes)
    }

    /// Compute the `PlanHash` directly from a byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let digest = hasher.finalize();
        Self(hex::encode(digest))
    }

    /// Returns the hex digest as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the raw byte length of the digest (32 bytes for SHA-256).
    pub const fn byte_len() -> usize {
        32
    }

    /// Returns `true` if the digest is the expected length (64 hex chars).
    pub fn is_valid(&self) -> bool {
        self.0.len() == 64 && self.0.chars().all(|c| c.is_ascii_hexdigit())
    }
}

impl fmt::Display for PlanHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sha256:{}", self.0)
    }
}

/// A tiny struct that carries both version and hash metadata for a plan.
/// Used as part of `PlanMetadata` and the top-level `MoldPlan` / `GraphPlan`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlanMetadata {
    pub version: PlanVersion,
    pub hash: PlanHash,
}

impl Sealed for PlanMetadata {}

impl PlanMetadata {
    pub fn new(version: PlanVersion, hash: PlanHash) -> Self {
        Self { version, hash }
    }

    /// Returns the plan version string.
    pub fn version_str(&self) -> &str {
        self.version.as_str()
    }

    /// Returns the plan hash hex string.
    pub fn hash_str(&self) -> &str {
        self.hash.as_str()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Task 1.1a RED — PlanVersion semver + PlanHash determinism
    // Scenario: `moldplan-graphplan::PlanVersion and Hash` (Hash stability +
    //           Hash sensitivity)
    // Assert: hash(plan_a) == hash(plan_b) for byte-identical inputs;
    //         hash(max_hops=3) != hash(max_hops=4)
    // -------------------------------------------------------------------------

    /// `PlanVersion::new("1.0.0")` must succeed and round-trip through Display.
    #[test]
    fn plan_version_valid() {
        let v = PlanVersion::new("1.0.0").expect("valid semver");
        assert_eq!(v.as_str(), "1.0.0");
        assert_eq!(v.to_string(), "1.0.0");
    }

    /// `PlanVersion::new("0.5.1+custom")` with pre-release must succeed.
    #[test]
    fn plan_version_with_prerelease() {
        let v = PlanVersion::new("0.5.1-alpha.1").expect("valid prerelease");
        assert_eq!(v.as_str(), "0.5.1-alpha.1");
    }

    /// `PlanVersion::new("not-semver")` must fail.
    #[test]
    fn plan_version_invalid_rejected() {
        assert!(PlanVersion::new("not-semver").is_err());
        assert!(PlanVersion::new("").is_err());
        assert!(PlanVersion::new("1.0").is_err()); // missing patch
    }

    /// `PlanVersion::default()` equals `PlanVersion::CURRENT`.
    #[test]
    fn plan_version_default() {
        assert_eq!(PlanVersion::default().as_str(), PlanVersion::CURRENT);
    }

    /// `PlanVersion` must be serializable and deserializable via serde.
    #[test]
    fn plan_version_serde_roundtrip() {
        let v = PlanVersion::new("2.1.0").unwrap();
        let json = serde_json::to_string(&v).expect("serialize");
        let parsed: PlanVersion = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, v);
    }

    /// `PlanHash::compute` produces a 64-char hex string for any serializable value.
    #[test]
    fn plan_hash_hex_length() {
        #[derive(serde::Serialize)]
        struct FakePlan {
            max_hops: u32,
        }
        let plan = FakePlan { max_hops: 3 };
        let hash = PlanHash::compute(&plan);
        assert_eq!(hash.as_str().len(), 64, "SHA-256 hex is always 64 chars");
        assert!(hash.is_valid());
    }

    /// Two `FakePlan` with identical fields produce the same hash.
    #[test]
    fn plan_hash_deterministic() {
        #[derive(serde::Serialize, PartialEq, Eq)]
        struct FakePlan {
            max_hops: u32,
            depth: u32,
        }
        let a = FakePlan { max_hops: 3, depth: 5 };
        let b = FakePlan { max_hops: 3, depth: 5 };
        let hash_a = PlanHash::compute(&a);
        let hash_b = PlanHash::compute(&b);
        assert_eq!(hash_a, hash_b, "identical content → identical hash");
    }

    /// Two `FakePlan` with different `max_hops` produce different hashes (sensitivity).
    #[test]
    fn plan_hash_sensitive_to_data() {
        #[derive(serde::Serialize, PartialEq, Eq)]
        struct FakePlan {
            max_hops: u32,
        }
        let a = FakePlan { max_hops: 3 };
        let b = FakePlan { max_hops: 4 };
        let hash_a = PlanHash::compute(&a);
        let hash_b = PlanHash::compute(&b);
        assert_ne!(hash_a, hash_b, "different data → different hash");
    }

    /// `PlanHash` must be serializable and deserializable via serde.
    #[test]
    fn plan_hash_serde_roundtrip() {
        #[derive(serde::Serialize)]
        struct FakePlan { value: i32 }
        let plan = FakePlan { value: 42 };
        let hash = PlanHash::compute(&plan);
        let json = serde_json::to_string(&hash).expect("serialize");
        let parsed: PlanHash = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, hash);
    }

    /// `PlanMetadata` carries both version and hash.
    #[test]
    fn plan_metadata_carries_both() {
        let version = PlanVersion::new("1.0.0").unwrap();
        #[derive(serde::Serialize)]
        struct P { x: i32 }
        let hash = PlanHash::compute(&P { x: 1 });
        let meta = PlanMetadata::new(version.clone(), hash.clone());
        assert_eq!(meta.version_str(), "1.0.0");
        assert_eq!(meta.hash_str(), hash.as_str());
    }

    /// `PlanMetadata` is serializable.
    #[test]
    fn plan_metadata_serde_roundtrip() {
        let version = PlanVersion::new("1.0.0").unwrap();
        #[derive(serde::Serialize)]
        struct P { x: i32 }
        let hash = PlanHash::compute(&P { x: 1 });
        let meta = PlanMetadata::new(version, hash);
        let json = serde_json::to_string(&meta).expect("serialize");
        let parsed: PlanMetadata = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, meta);
    }

    /// `PlanHash::display` shows the `sha256:` prefix.
    #[test]
    fn plan_hash_display_prefix() {
        #[derive(serde::Serialize)]
        struct P { x: i32 }
        let hash = PlanHash::compute(&P { x: 0 });
        let display = hash.to_string();
        assert!(
            display.starts_with("sha256:"),
            "Display must show 'sha256:' prefix: {display}"
        );
    }
}
