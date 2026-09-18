//! Source patch candidate + validation (e80b — M11 task 12.5).
//!
//! ## One type, one question
//!
//! ```text
//! SourcePatchCandidate   an AI-produced SUGGESTION (untrusted, non-authoritative)
//! ValidatedSourcePatch   a structurally safe candidate (paths proven relative)
//! PatchRef               a content address for an inert artifact
//! ```
//!
//! None of these is a `ChangeProposal`, a source mutation, evidence, or
//! authority. A patch candidate is a suggestion the platform may or may not
//! turn into a proposal; it can never authorise anything.
//!
//! ## Why full-file replacement
//!
//! A [`SourceEdit`] replaces a whole file. That is deliberate: it removes
//! line/byte-offset ambiguity entirely, so "overlapping or ambiguous edits"
//! reduces to "duplicate path", which is a crisp typed rejection rather than a
//! fuzzy overlap computation. One file, at most one edit.
//!
//! ## Malformed output is rejected, never repaired
//!
//! [`SourcePatchCandidate`] carries **raw** `String` paths, so malformed model
//! output is representable and can be rejected with a typed
//! [`PatchValidationError`]. Only [`ValidatedSourcePatch`] carries
//! [`RelativePath`] values, and those can only be built by
//! [`validate_source_patch`].

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::domain::ai::frame::{InvestigationFrame, InvestigationFrameId};
use crate::domain::ai::request::InvestigationRequest;
use crate::domain::ai::response::LlmResponse;
use crate::domain::kernel_ids::{FactId, SnapshotId};
use crate::domain::value_objects::WorkspaceId;

/// The workspace + snapshot a patch candidate was authored against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchBaseScope {
    /// Workspace the patch targets.
    pub workspace: WorkspaceId,
    /// Snapshot the patch was authored against.
    pub snapshot: SnapshotId,
}

impl PatchBaseScope {
    /// Construct from its parts.
    pub fn new(workspace: WorkspaceId, snapshot: SnapshotId) -> Self {
        Self {
            workspace,
            snapshot,
        }
    }
}

/// One file edit proposed by an AI agent.
///
/// `path` is a **raw, untrusted** string. Use
/// [`validate_source_patch`] to obtain validated relative paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceEdit {
    /// Workspace-relative path, as authored by the model. Untrusted.
    pub path: String,
    /// The complete new file content.
    pub new_content: String,
}

/// An AI-produced source patch suggestion. Untrusted, non-authoritative.
///
/// This is what a model may emit. It carries no `ChangeProposalId`, no
/// `base_world`, no `RequestedBy`, no approval, and no authority: those
/// envelope fields belong to trusted orchestration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePatchCandidate {
    /// The scope the patch claims to be authored against.
    pub base_scope: PatchBaseScope,
    /// The proposed file edits.
    pub edits: Vec<SourceEdit>,
    /// Optional bounded rationale for audit.
    pub rationale: Option<String>,
}

/// Limits enforced by [`validate_source_patch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatchBudget {
    /// Maximum number of file edits in one patch.
    pub max_edits: usize,
    /// Maximum total payload (paths + contents) in bytes.
    pub max_payload_bytes: usize,
}

impl Default for PatchBudget {
    fn default() -> Self {
        Self {
            max_edits: 32,
            max_payload_bytes: 256 * 1024,
        }
    }
}

/// A validated, workspace-relative path.
///
/// Constructible only by [`validate_source_patch`]: no public constructor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RelativePath(String);

impl RelativePath {
    /// Borrow the path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RelativePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One validated file edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSourceEdit {
    path: RelativePath,
    new_content: String,
}

impl ValidatedSourceEdit {
    /// The validated workspace-relative path.
    pub fn path(&self) -> &RelativePath {
        &self.path
    }

    /// The complete new file content.
    pub fn new_content(&self) -> &str {
        &self.new_content
    }
}

/// A structurally safe patch candidate. Still not authority.
///
/// Edits are stored in deterministic (path-sorted) order so the canonical
/// bytes, and therefore the [`PatchRef`] derived from them, are stable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSourcePatch {
    base_scope: PatchBaseScope,
    edits: Vec<ValidatedSourceEdit>,
    rationale: Option<String>,
}

impl ValidatedSourcePatch {
    /// The scope the patch was validated against.
    pub fn base_scope(&self) -> &PatchBaseScope {
        &self.base_scope
    }

    /// The validated edits, sorted by path.
    pub fn edits(&self) -> &[ValidatedSourceEdit] {
        &self.edits
    }

    /// The optional rationale.
    pub fn rationale(&self) -> Option<&str> {
        self.rationale.as_deref()
    }

    /// Deterministic canonical encoding used for content addressing.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"cognicode.source-patch.v1\n");
        out.extend_from_slice(self.base_scope.workspace.as_str().as_bytes());
        out.push(b'\n');
        out.extend_from_slice(&self.base_scope.snapshot.get().to_le_bytes());
        for e in &self.edits {
            out.extend_from_slice(&(e.path.as_str().len() as u64).to_le_bytes());
            out.extend_from_slice(e.path.as_str().as_bytes());
            out.extend_from_slice(&(e.new_content.len() as u64).to_le_bytes());
            out.extend_from_slice(e.new_content.as_bytes());
        }
        out
    }
}

/// Why a patch candidate was rejected.
///
/// Failures are typed and disjoint; malformed model output is never silently
/// repaired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchValidationError {
    /// The response does not belong to the requested frame.
    FrameMismatch {
        /// The frame the caller validated against.
        expected: InvestigationFrameId,
        /// The frame the response claims.
        actual: InvestigationFrameId,
    },
    /// The response's echoed provenance names a different frame.
    ProvenanceFrameMismatch {
        /// The frame the caller validated against.
        expected: InvestigationFrameId,
        /// The frame echoed by the response provenance.
        actual: InvestigationFrameId,
    },
    /// The response answers a different request.
    RequestDigestMismatch {
        /// The request's content digest.
        expected: u64,
        /// The digest the response recorded.
        actual: u64,
    },
    /// The response observed a canonical fact outside the frame's declared scope.
    ReadSetOutOfScope {
        /// The offending fact.
        fact: FactId,
    },
    /// The candidate claims a different workspace/snapshot than the frame.
    BaseScopeMismatch {
        /// The frame's scope.
        expected: PatchBaseScope,
        /// The candidate's claimed scope.
        actual: PatchBaseScope,
    },
    /// No edits.
    EmptyPatch,
    /// Too many edits.
    TooManyEdits {
        /// Observed edit count.
        observed: usize,
        /// The limit.
        limit: usize,
    },
    /// Payload exceeds the budget.
    PayloadTooLarge {
        /// Observed payload bytes.
        observed: usize,
        /// The limit.
        limit: usize,
    },
    /// An empty path.
    EmptyPath,
    /// An absolute path (POSIX root or Windows drive).
    AbsolutePath {
        /// The offending path.
        path: String,
    },
    /// A `..` traversal component.
    PathTraversal {
        /// The offending path.
        path: String,
    },
    /// A non-canonical path (empty / `.` component, backslash, NUL).
    NonCanonicalPath {
        /// The offending path.
        path: String,
    },
    /// Two edits target the same path.
    DuplicatePath {
        /// The duplicated path.
        path: String,
    },
}

impl std::fmt::Display for PatchValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FrameMismatch { expected, actual } => {
                write!(
                    f,
                    "response frame {actual} does not match request frame {expected}"
                )
            }
            Self::ProvenanceFrameMismatch { expected, actual } => write!(
                f,
                "response provenance frame {actual} does not match request frame {expected}"
            ),
            Self::RequestDigestMismatch { expected, actual } => {
                write!(
                    f,
                    "response request digest {actual} does not match request {expected}"
                )
            }
            Self::ReadSetOutOfScope { fact } => {
                write!(
                    f,
                    "observed read set contains fact {fact:?} outside the declared scope"
                )
            }
            Self::BaseScopeMismatch { expected, actual } => write!(
                f,
                "candidate base scope {actual:?} does not match frame scope {expected:?}"
            ),
            Self::EmptyPatch => f.write_str("patch contains no edits"),
            Self::TooManyEdits { observed, limit } => {
                write!(f, "patch has {observed} edits, limit is {limit}")
            }
            Self::PayloadTooLarge { observed, limit } => {
                write!(f, "patch payload is {observed} bytes, limit is {limit}")
            }
            Self::EmptyPath => f.write_str("patch contains an empty path"),
            Self::AbsolutePath { path } => write!(f, "absolute path is not allowed: {path}"),
            Self::PathTraversal { path } => write!(f, "path traversal is not allowed: {path}"),
            Self::NonCanonicalPath { path } => write!(f, "non-canonical path: {path}"),
            Self::DuplicatePath { path } => write!(f, "duplicate path in patch: {path}"),
        }
    }
}

impl std::error::Error for PatchValidationError {}

/// Validate a patch candidate against the request/response lineage and the
/// patch budget.
///
/// Rules, in order (fail-closed on the first violation):
///
/// 1. The response must belong to `frame` (both its `frame_id` and the echoed
///    `RequestProvenance.frame_id`).
/// 2. The response must answer `request` (its `request_digest` must equal
///    `request.content_digest()`).
/// 3. Every fact in `response.observed_read_set()` must be inside
///    `frame.scope().declared_read_set`.
/// 4. The candidate's `base_scope` must equal the frame's workspace + snapshot.
/// 5. The patch must be non-empty, within the edit and payload budgets.
/// 6. Every path must be relative, canonical, and free of `..` traversal.
/// 7. No two edits may target the same path.
///
/// Edits are returned sorted by path for determinism.
pub fn validate_source_patch(
    frame: &InvestigationFrame,
    request: &InvestigationRequest,
    response: &LlmResponse,
    candidate: &SourcePatchCandidate,
    budget: PatchBudget,
) -> Result<ValidatedSourcePatch, PatchValidationError> {
    // 1. Frame identity (response id and echoed provenance).
    if response.frame_id() != frame.id() {
        return Err(PatchValidationError::FrameMismatch {
            expected: frame.id(),
            actual: response.frame_id(),
        });
    }
    if response.request_provenance().frame_id != frame.id() {
        return Err(PatchValidationError::ProvenanceFrameMismatch {
            expected: frame.id(),
            actual: response.request_provenance().frame_id,
        });
    }

    // 2. Request pairing.
    let expected_digest = request.content_digest();
    if response.request_digest() != expected_digest {
        return Err(PatchValidationError::RequestDigestMismatch {
            expected: expected_digest,
            actual: response.request_digest(),
        });
    }

    // 3. Observed reads must be inside the declared scope.
    let declared = &frame.scope().declared_read_set;
    for fact in response.observed_read_set().iter() {
        if !declared.contains(fact) {
            return Err(PatchValidationError::ReadSetOutOfScope { fact: *fact });
        }
    }

    // 4. Base scope must match the frame.
    let expected_scope =
        PatchBaseScope::new(frame.scope().workspace.clone(), frame.scope().snapshot);
    if candidate.base_scope != expected_scope {
        return Err(PatchValidationError::BaseScopeMismatch {
            expected: expected_scope,
            actual: candidate.base_scope.clone(),
        });
    }

    // 5. Non-empty + budgets.
    if candidate.edits.is_empty() {
        return Err(PatchValidationError::EmptyPatch);
    }
    if candidate.edits.len() > budget.max_edits {
        return Err(PatchValidationError::TooManyEdits {
            observed: candidate.edits.len(),
            limit: budget.max_edits,
        });
    }
    let payload: usize = candidate
        .edits
        .iter()
        .map(|e| e.path.len() + e.new_content.len())
        .sum();
    if payload > budget.max_payload_bytes {
        return Err(PatchValidationError::PayloadTooLarge {
            observed: payload,
            limit: budget.max_payload_bytes,
        });
    }

    // 6 + 7. Path validation and duplicate detection.
    let mut validated: Vec<ValidatedSourceEdit> = Vec::with_capacity(candidate.edits.len());
    for edit in &candidate.edits {
        let path = validate_relative_path(&edit.path)?;
        if validated.iter().any(|v| v.path == path) {
            return Err(PatchValidationError::DuplicatePath {
                path: path.as_str().to_string(),
            });
        }
        validated.push(ValidatedSourceEdit {
            path,
            new_content: edit.new_content.clone(),
        });
    }

    // Deterministic order for stable content addressing.
    validated.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(ValidatedSourcePatch {
        base_scope: candidate.base_scope.clone(),
        edits: validated,
        rationale: candidate.rationale.clone(),
    })
}

/// Validate one candidate path as a canonical workspace-relative path.
pub fn validate_relative_path(raw: &str) -> Result<RelativePath, PatchValidationError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(PatchValidationError::EmptyPath);
    }
    if trimmed.contains('\0') || trimmed.contains('\\') {
        return Err(PatchValidationError::NonCanonicalPath {
            path: raw.to_string(),
        });
    }
    if trimmed.starts_with('/') || looks_like_windows_drive(trimmed) {
        return Err(PatchValidationError::AbsolutePath {
            path: raw.to_string(),
        });
    }
    for component in trimmed.split('/') {
        if component.is_empty() || component == "." {
            return Err(PatchValidationError::NonCanonicalPath {
                path: raw.to_string(),
            });
        }
        if component == ".." {
            return Err(PatchValidationError::PathTraversal {
                path: raw.to_string(),
            });
        }
    }
    Ok(RelativePath(trimmed.to_string()))
}

fn looks_like_windows_drive(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

// ---------------------------------------------------------------------------
// WU4 — patch artifact seam
// ---------------------------------------------------------------------------

/// A content address for a stored patch artifact.
///
/// Content-addressed (`sha256:<hex>` over [`ValidatedSourcePatch::canonical_bytes`]):
/// identical patches collapse to the same ref, so proposal ids derived from a
/// deterministic fake are reproducible.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PatchRef(String);

impl PatchRef {
    /// The content address of a validated patch.
    pub fn from_content(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let digest = hasher.finalize();
        let mut hex = String::with_capacity(64);
        for b in digest {
            hex.push_str(&format!("{b:02x}"));
        }
        Self(format!("sha256:{hex}"))
    }

    /// Borrow the reference string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PatchRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why a patch artifact could not be stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchArtifactError {
    /// The sink refused the patch.
    Rejected(String),
}

impl std::fmt::Display for PatchArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(msg) => write!(f, "patch artifact rejected: {msg}"),
        }
    }
}

impl std::error::Error for PatchArtifactError {}

/// Port: store an inert patch artifact and return its reference.
///
/// This is **not** a canonical store: a stored patch is not truth and grants no
/// authority. It exists so a `ChangeProposal::SourcePatch { patch_ref }` can
/// reference the candidate that justified it.
pub trait PatchArtifactSink {
    /// Store the patch, returning its content address.
    fn store(&self, patch: ValidatedSourcePatch) -> Result<PatchRef, PatchArtifactError>;
}
