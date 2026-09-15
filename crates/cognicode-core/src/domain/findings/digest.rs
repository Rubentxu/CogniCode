//! Detector content digests (M6, cycle e57).
//!
//! Two digests are maintained deliberately:
//!
//! - **semantic digest** — `sha256(id, requires, steps)`. Stable across a
//!   detector's *authority* or *name* changing, so historical replay and
//!   detector comparison can tell "same algorithm" from "same instance".
//! - **instance digest** — `sha256(whole definition)`, including `authority`
//!   and `name`, for audit of the exact admitted instance.
//!
//! FNV-1a (used in e55) was replaced: a 64-bit non-cryptographic checksum is
//! not an adequate historical/audit identity. SHA-256 matches the hash used
//! elsewhere in the analysis surface.
//!
//! Pure domain: no I/O.

use serde::{Deserialize, Serialize};
use std::fmt;

/// SHA-256 of `bytes`, lowercase hex (no prefix).
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// A SHA-256 content digest rendered `sha256:<64 hex>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DetectorDigest(String);

impl DetectorDigest {
    /// Validate a `sha256:<64 hex>` string.
    pub fn new(value: impl Into<String>) -> Result<Self, DigestError> {
        let value = value.into();
        let Some(hex) = value.strip_prefix("sha256:") else {
            return Err(DigestError::BadPrefix);
        };
        if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(DigestError::BadLengthOrChars);
        }
        Ok(Self(value))
    }

    /// Digest arbitrary content.
    pub fn from_content(content: &str) -> Self {
        Self(format!("sha256:{}", sha256_hex(content.as_bytes())))
    }

    /// Digest raw bytes.
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self(format!("sha256:{}", sha256_hex(bytes)))
    }

    /// Borrow the raw `sha256:<hex>` string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for DetectorDigest {
    type Error = DigestError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<DetectorDigest> for String {
    fn from(value: DetectorDigest) -> Self {
        value.0
    }
}

impl fmt::Display for DetectorDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a digest string was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigestError {
    /// Missing the `sha256:` prefix.
    BadPrefix,
    /// The hex payload is not 64 hex characters.
    BadLengthOrChars,
}

impl fmt::Display for DigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BadPrefix => "digest must start with `sha256:`",
            Self::BadLengthOrChars => "digest payload must be 64 hex characters",
        })
    }
}

impl std::error::Error for DigestError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vector() {
        // SHA-256 of the empty string.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn digest_is_stable_and_prefixed() {
        let a = DetectorDigest::from_content("hello");
        let b = DetectorDigest::from_content("hello");
        assert_eq!(a, b);
        assert!(a.as_str().starts_with("sha256:"));
        assert_eq!(a.as_str().len(), "sha256:".len() + 64);
    }

    #[test]
    fn digest_changes_with_content() {
        assert_ne!(
            DetectorDigest::from_content("a"),
            DetectorDigest::from_content("b")
        );
    }

    #[test]
    fn new_rejects_malformed() {
        assert_eq!(
            DetectorDigest::new("fnv1a64:abcd").unwrap_err(),
            DigestError::BadPrefix
        );
        assert_eq!(
            DetectorDigest::new("sha256:short").unwrap_err(),
            DigestError::BadLengthOrChars
        );
        assert_eq!(
            DetectorDigest::new(format!("sha256:{}", "z".repeat(64))).unwrap_err(),
            DigestError::BadLengthOrChars
        );
        assert!(DetectorDigest::new(format!("sha256:{}", "a".repeat(64))).is_ok());
    }

    #[test]
    fn serde_round_trip() {
        let d = DetectorDigest::from_content("x");
        let json = serde_json::to_string(&d).unwrap();
        let parsed: DetectorDigest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, d);
    }
}
