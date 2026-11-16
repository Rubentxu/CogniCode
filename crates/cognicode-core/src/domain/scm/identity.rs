//! Source identity: who provides the source, which revision of it,
//! and what its content is. Identities are validated at construction;
//! an invalid identity never exists as a value.

use sha2::{Digest as _, Sha256};

use super::error::ScmError;

/// Opaque provider identity. The module never interprets it beyond
/// non-emptiness: `"git"`, `"fake-provider"`, a forge URL — it is the
/// adapter's business what the string means, and two providers with
/// different ids never share a namespace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(value: &str) -> Result<Self, ScmError> {
        if value.trim().is_empty() {
            return Err(ScmError::EmptyField { field: "provider" });
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A versioned source: one provider plus one project namespace. It
/// says WHERE content comes from, never WHICH revision — that is
/// [`RevisionHandle`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceRef {
    provider: ProviderId,
    project: String,
}

impl SourceRef {
    pub fn new(provider: &str, project: &str) -> Result<Self, ScmError> {
        let provider = ProviderId::new(provider)?;
        if project.trim().is_empty() {
            return Err(ScmError::EmptyField { field: "project" });
        }
        Ok(Self {
            provider,
            project: project.to_string(),
        })
    }

    pub fn provider(&self) -> &ProviderId {
        &self.provider
    }

    pub fn project(&self) -> &str {
        &self.project
    }
}

/// An OPAQUE revision handle as the provider names it (`"rev-a"`, a
/// commit SHA, a change id — anything). This is deliberately NOT the
/// canonical numeric `RevisionId` of the analysis kernel nor a
/// `SnapshotId`: adapters translate opaque handles into those when
/// they persist results (CAA-03/CAA-04), so the kernel keeps its
/// internal identity law untouched.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RevisionHandle(String);

impl RevisionHandle {
    pub fn new(value: &str) -> Result<Self, ScmError> {
        if value.trim().is_empty() {
            return Err(ScmError::EmptyField { field: "handle" });
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// SHA-256 content identity in lowercase hex. Two revisions with the
/// same content digest across their tracked trees may reuse parsed
/// artifacts even when their SCM handles differ; that reuse key is
/// the whole promise of this type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentDigest(String);

impl ContentDigest {
    /// Digests raw bytes canonically. Infallible by construction.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(hex::encode(Sha256::digest(bytes)))
    }

    /// Accepts only a 64-character lowercase hex string.
    pub fn from_hex(value: &str) -> Result<Self, ScmError> {
        let well_formed = value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !well_formed {
            return Err(ScmError::DigestInvalid {
                value: value.to_string(),
            });
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

/// A normalized repository-relative path key. The validation is
/// fail-closed and lexical (no filesystem truth): absolute paths,
/// backslash separators, and `.`/`..`/empty segments can never become
/// keys, so downstream joins cannot escape the repository root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathKey(String);

impl PathKey {
    pub fn new(value: &str) -> Result<Self, ScmError> {
        let reject = |reason_path: &str| ScmError::UnsafePath {
            path: reason_path.to_string(),
        };
        if value.is_empty() || value.starts_with('/') || value.contains('\\') {
            return Err(reject(value));
        }
        if value
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        {
            return Err(reject(value));
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One revision of one source, known by its opaque handle and by the
/// content digest of its tracked tree. The pair is the whole value:
/// SCM identity (handle) and content identity (digest) stay separate
/// on purpose — that is what lets identical content under different
/// handles reuse parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRevision {
    source: SourceRef,
    handle: RevisionHandle,
    content: ContentDigest,
}

impl SourceRevision {
    pub fn new(source: SourceRef, handle: RevisionHandle, content: ContentDigest) -> Self {
        Self {
            source,
            handle,
            content,
        }
    }

    pub fn source(&self) -> &SourceRef {
        &self.source
    }

    pub fn handle(&self) -> &RevisionHandle {
        &self.handle
    }

    pub fn content(&self) -> &ContentDigest {
        &self.content
    }
}
