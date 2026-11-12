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
///
/// This is the one digest of content the product computes: the manifest of
/// the snapshot, the session that writes it, the cursor, the plan and the
/// token that leave the process all derive theirs from here, so two of them
/// that look at the same bytes cannot disagree about what they are.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut stream = DigestStream::new();
    stream.update(bytes);
    stream.finish_hex()
}

/// The 32 bytes of the SHA-256 of `bytes`, for a caller that keeps the digest
/// as a fixed-size value instead of as text.
pub fn sha256_raw(bytes: &[u8]) -> [u8; 32] {
    let mut stream = DigestStream::new();
    stream.update(bytes);
    stream.finish_raw()
}

/// How many hex characters of the SHA-256 travel in a short digest.
pub const SHORT_DIGEST_HEX_LEN: usize = 16;

/// The first [`SHORT_DIGEST_HEX_LEN`] hex characters of the SHA-256 of
/// `bytes`: the form that travels in a cursor, a plan digest or a token.
///
/// Unlike `std::hash::DefaultHasher`, whose algorithm the standard library
/// reserves the right to change between releases, this value is the same in
/// every build: a digest that leaves the process and is compared by another
/// one must not depend on which compiler built each of them.
pub fn sha256_short(bytes: &[u8]) -> String {
    let mut stream = DigestStream::new();
    stream.update(bytes);
    stream.finish_short()
}

/// A SHA-256 over several pieces, for the digest that should not copy them
/// into one buffer first.
///
/// The pieces are joined with nothing in between: a caller that needs the
/// boundary between two of them to matter writes its own separator as a
/// piece of its own.
#[derive(Default)]
pub struct DigestStream {
    hasher: sha2::Sha256,
}

impl DigestStream {
    /// An empty stream: its digest is the digest of no bytes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append `bytes` to what is being digested.
    pub fn update(&mut self, bytes: impl AsRef<[u8]>) -> &mut Self {
        use sha2::Digest;
        self.hasher.update(bytes.as_ref());
        self
    }

    /// The 32 bytes of the whole SHA-256.
    pub fn finish_raw(self) -> [u8; 32] {
        use sha2::Digest;
        self.hasher.finalize().into()
    }

    /// The lowercase hex of the whole SHA-256 (64 characters).
    pub fn finish_hex(self) -> String {
        use sha2::Digest;
        use std::fmt::Write;
        let digest = self.hasher.finalize();
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest {
            let _ = write!(hex, "{byte:02x}");
        }
        hex
    }

    /// The first [`SHORT_DIGEST_HEX_LEN`] hex characters of the digest.
    pub fn finish_short(self) -> String {
        let mut hex = self.finish_hex();
        hex.truncate(SHORT_DIGEST_HEX_LEN);
        hex
    }
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
    fn sha256_abc_vector_is_lowercase_64_hex() {
        let hex = sha256_hex(b"abc");
        assert_eq!(
            hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(hex.len(), 64);
        assert!(
            hex.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
    }

    #[test]
    fn the_pieces_of_a_stream_hash_as_their_concatenation() {
        let mut stream = DigestStream::new();
        stream.update("a").update(b"b").update(String::from("c"));
        assert_eq!(stream.finish_hex(), sha256_hex(b"abc"));
        assert_eq!(
            DigestStream::new().finish_hex(),
            sha256_hex(b""),
            "no pieces is the empty input, not an error"
        );
    }

    #[test]
    fn the_raw_digest_is_the_bytes_of_the_hex_digest() {
        let raw = sha256_raw(b"abc");
        assert_eq!(raw.len(), 32);
        assert_eq!(&raw[..4], &[0xba, 0x78, 0x16, 0xbf]);
        let hex: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, sha256_hex(b"abc"));
        let mut stream = DigestStream::new();
        stream.update("a").update("bc");
        assert_eq!(stream.finish_raw(), raw);
    }

    #[test]
    fn the_short_digest_is_the_fixed_prefix_of_the_sha256() {
        // The vector that pins the short form for every build: the first
        // 16 hex characters of SHA-256("abc").
        assert_eq!(sha256_short(b"abc"), "ba7816bf8f01cfea");
        let mut stream = DigestStream::new();
        stream.update("a").update("bc");
        assert_eq!(stream.finish_short(), "ba7816bf8f01cfea");
        assert_eq!(sha256_short(b"abc").len(), SHORT_DIGEST_HEX_LEN);
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
