//! Event payloads: bounded inline data, or a digest reference (M7, cycle e63).
//!
//! ## The rule this module exists to enforce
//!
//! > A million facts do not produce a million events.
//!
//! The log records *that* something happened, not the thing itself. A fact
//! batch commit is **one** event carrying the batch size and a content digest;
//! the facts stay in the evidence kernel. Anything larger than the inline
//! budget must go through [`EventPayloadRef::Artifact`], which references the
//! bytes by digest instead of embedding them.
//!
//! This is what keeps the log from quietly becoming a second database, and it
//! is why the bound is enforced at construction: an oversized inline payload is
//! a build-time-visible modelling mistake, not a runtime surprise.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Maximum size, in bytes, of an inline payload.
pub const MAX_INLINE_PAYLOAD_BYTES: usize = 4096;

/// Why a payload was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadError {
    /// The payload exceeds [`MAX_INLINE_PAYLOAD_BYTES`].
    TooLarge {
        /// Attempted size in bytes.
        attempted: usize,
        /// The budget.
        limit: usize,
    },
    /// The payload's summary was empty.
    EmptySummary,
    /// An artifact reference carried an empty media type.
    EmptyMediaType,
    /// A digest was not a well-formed `sha256:<64 hex>`.
    MalformedDigest,
}

impl std::fmt::Display for PayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { attempted, limit } => write!(
                f,
                "inline payload is {attempted} bytes, which exceeds the {limit}-byte budget; reference it as an artifact instead"
            ),
            Self::EmptySummary => f.write_str("a payload summary must not be empty"),
            Self::EmptyMediaType => f.write_str("an artifact media type must not be empty"),
            Self::MalformedDigest => {
                f.write_str("a content digest must be `sha256:` followed by 64 hex digits")
            }
        }
    }
}

impl std::error::Error for PayloadError {}

/// A SHA-256 content digest rendered `sha256:<64 hex>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentDigest(String);

impl ContentDigest {
    /// The digest of `bytes`.
    pub fn of(bytes: &[u8]) -> Self {
        Self(format!(
            "sha256:{}",
            crate::domain::findings::sha256_hex(bytes)
        ))
    }

    /// Parse a `sha256:<64 hex>` digest.
    pub fn try_new(value: impl Into<String>) -> Result<Self, PayloadError> {
        let value = value.into();
        let Some(hex) = value.strip_prefix("sha256:") else {
            return Err(PayloadError::MalformedDigest);
        };
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(PayloadError::MalformedDigest);
        }
        Ok(Self(value))
    }

    /// Borrow the raw `sha256:<hex>` value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The hex part.
    pub fn hex(&self) -> &str {
        self.0.strip_prefix("sha256:").unwrap_or(&self.0)
    }
}

impl std::fmt::Display for ContentDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for ContentDigest {
    type Error = PayloadError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<ContentDigest> for String {
    fn from(value: ContentDigest) -> Self {
        value.0
    }
}

/// A small, bounded, structured inline payload.
///
/// The byte budget is enforced on every mutation, so a payload can never grow
/// past it. `fields` is a `BTreeMap`, so serialization order is stable and two
/// equal payloads always render identically — which is what makes replay
/// comparisons meaningful.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedEventPayload {
    summary: String,
    fields: BTreeMap<String, String>,
}

impl BoundedEventPayload {
    /// A payload with just a summary.
    pub fn new(summary: impl Into<String>) -> Result<Self, PayloadError> {
        Self::from_parts(summary.into(), BTreeMap::new())
    }

    /// Add or replace a field, enforcing the budget.
    pub fn with_field(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, PayloadError> {
        let mut fields = self.fields;
        fields.insert(key.into(), value.into());
        Self::from_parts(self.summary, fields)
    }

    fn from_parts(summary: String, fields: BTreeMap<String, String>) -> Result<Self, PayloadError> {
        if summary.trim().is_empty() {
            return Err(PayloadError::EmptySummary);
        }
        let candidate = Self { summary, fields };
        let size = candidate.byte_len();
        if size > MAX_INLINE_PAYLOAD_BYTES {
            return Err(PayloadError::TooLarge {
                attempted: size,
                limit: MAX_INLINE_PAYLOAD_BYTES,
            });
        }
        Ok(candidate)
    }

    /// The human-readable summary.
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// A field, if present.
    pub fn field(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(|v| v.as_str())
    }

    /// All fields, in stable key order.
    pub fn fields(&self) -> &BTreeMap<String, String> {
        &self.fields
    }

    /// The serialized size this payload would occupy inline.
    pub fn byte_len(&self) -> usize {
        self.summary.len()
            + self
                .fields
                .iter()
                .map(|(k, v)| k.len() + v.len())
                .sum::<usize>()
    }
}

/// Where an event's payload lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "storage", rename_all = "snake_case")]
pub enum EventPayloadRef {
    /// Small structured data carried with the event.
    Inline(BoundedEventPayload),
    /// A reference to bytes held elsewhere, identified by content digest.
    ///
    /// This is how "a million facts" stays one event.
    Artifact {
        /// Digest of the referenced bytes.
        digest: ContentDigest,
        /// Media type of the referenced bytes.
        media_type: String,
        /// Size of the referenced bytes.
        byte_len: u64,
    },
}

impl EventPayloadRef {
    /// An inline payload.
    pub fn inline(payload: BoundedEventPayload) -> Self {
        Self::Inline(payload)
    }

    /// A digest reference.
    pub fn artifact(
        digest: ContentDigest,
        media_type: impl Into<String>,
        byte_len: u64,
    ) -> Result<Self, PayloadError> {
        let media_type = media_type.into();
        if media_type.trim().is_empty() {
            return Err(PayloadError::EmptyMediaType);
        }
        Ok(Self::Artifact {
            digest,
            media_type,
            byte_len,
        })
    }

    /// Whether this payload is carried inline.
    pub fn is_inline(&self) -> bool {
        matches!(self, Self::Inline(_))
    }

    /// The inline payload, if any.
    pub fn inline_payload(&self) -> Option<&BoundedEventPayload> {
        match self {
            Self::Inline(payload) => Some(payload),
            Self::Artifact { .. } => None,
        }
    }

    /// The referenced digest, if any.
    pub fn digest(&self) -> Option<&ContentDigest> {
        match self {
            Self::Inline(_) => None,
            Self::Artifact { digest, .. } => Some(digest),
        }
    }

    /// The summary, whichever form the payload takes.
    pub fn summary(&self) -> &str {
        match self {
            Self::Inline(payload) => payload.summary(),
            Self::Artifact { media_type, .. } => media_type.as_str(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digests_are_content_addressed_and_validated() {
        let a = ContentDigest::of(b"hello");
        let b = ContentDigest::of(b"hello");
        let c = ContentDigest::of(b"world");
        assert_eq!(a, b, "the same bytes yield the same digest");
        assert_ne!(a, c);
        assert!(a.as_str().starts_with("sha256:"));
        assert_eq!(a.hex().len(), 64);

        assert_eq!(
            ContentDigest::try_new("md5:abc").unwrap_err(),
            PayloadError::MalformedDigest
        );
        assert_eq!(
            ContentDigest::try_new("sha256:nothex").unwrap_err(),
            PayloadError::MalformedDigest
        );
        assert_eq!(
            ContentDigest::try_new(format!("sha256:{}", "a".repeat(63))).unwrap_err(),
            PayloadError::MalformedDigest
        );
    }

    #[test]
    fn inline_payloads_are_bounded() {
        let small = BoundedEventPayload::new("ok").unwrap();
        assert_eq!(small.summary(), "ok");

        // Grow past the budget one field at a time.
        let mut payload = BoundedEventPayload::new("big").unwrap();
        let mut rejected = None;
        for i in 0..1000 {
            match payload.with_field(format!("k{i}"), "x".repeat(64)) {
                Ok(next) => payload = next,
                Err(err) => {
                    rejected = Some(err);
                    break;
                }
            }
        }
        let err = rejected.expect("the budget must be enforced");
        match err {
            PayloadError::TooLarge { attempted, limit } => {
                assert!(attempted > limit);
                assert_eq!(limit, MAX_INLINE_PAYLOAD_BYTES);
            }
            other => panic!("expected TooLarge, got {other:?}"),
        }
    }

    #[test]
    fn an_empty_summary_is_rejected() {
        assert_eq!(
            BoundedEventPayload::new("   ").unwrap_err(),
            PayloadError::EmptySummary
        );
    }

    #[test]
    fn fields_are_stably_ordered() {
        let a = BoundedEventPayload::new("s")
            .unwrap()
            .with_field("z", "1")
            .unwrap()
            .with_field("a", "2")
            .unwrap();
        let b = BoundedEventPayload::new("s")
            .unwrap()
            .with_field("a", "2")
            .unwrap()
            .with_field("z", "1")
            .unwrap();
        assert_eq!(a, b, "insertion order must not matter");
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap(),
            "and the rendering must be identical, so replay comparisons work"
        );
        assert_eq!(a.field("a"), Some("2"));
        assert_eq!(a.field("missing"), None);
    }

    #[test]
    fn artifacts_reference_bytes_instead_of_embedding_them() {
        let digest = ContentDigest::of(&[0u8; 1024]);
        let payload = EventPayloadRef::artifact(digest.clone(), "application/json", 1024).unwrap();
        assert!(!payload.is_inline());
        assert_eq!(payload.digest(), Some(&digest));
        assert!(payload.inline_payload().is_none());

        assert_eq!(
            EventPayloadRef::artifact(digest, "  ", 1).unwrap_err(),
            PayloadError::EmptyMediaType
        );
    }

    #[test]
    fn payloads_round_trip() {
        let inline = EventPayloadRef::inline(
            BoundedEventPayload::new("batch committed")
                .unwrap()
                .with_field("count", "1000")
                .unwrap(),
        );
        let json = serde_json::to_string(&inline).unwrap();
        assert_eq!(
            serde_json::from_str::<EventPayloadRef>(&json).unwrap(),
            inline
        );

        let artifact =
            EventPayloadRef::artifact(ContentDigest::of(b"x"), "application/json", 1).unwrap();
        let json = serde_json::to_string(&artifact).unwrap();
        assert_eq!(
            serde_json::from_str::<EventPayloadRef>(&json).unwrap(),
            artifact
        );
    }
}
