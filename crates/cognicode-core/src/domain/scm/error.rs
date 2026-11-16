//! The vocabulary of this module speaks with a handful of named
//! failures. Every one of them carries what the caller needs to
//! attribute the failure (provider, reference, path) — an unknown
//! cause is never flattened into "empty result".

/// A neutral source read failed for one of the named reasons.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScmError {
    /// An identity field arrived blank; identity strings are
    /// never trimmed-into-existence.
    #[error("field `{field}` of a source identity must not be empty")]
    EmptyField { field: &'static str },

    /// A path is not a safe repository-relative key (absolute,
    /// backslash-separated, or containing `.`/`..`/empty segments).
    #[error("unsafe path {path:?}: must be a normalized repository-relative path")]
    UnsafePath { path: String },

    /// The provider does not know the requested reference.
    #[error("reference {reference:?} not found at provider {provider:?}")]
    ReferenceNotFound { provider: String, reference: String },

    /// The provider cannot establish that `base` is an ancestor of
    /// `head`; without ancestry there is no diff to report.
    #[error("no recorded ancestry from {base:?} to {head:?}")]
    MissingAncestor { base: String, head: String },

    /// The bytes for one tracked path are not available in the
    /// revision; absence of content is never reported as emptiness.
    #[error("content unavailable for {path:?}")]
    ContentUnavailable { path: String },

    /// The provider was asked for something it has not declared.
    #[error("provider capability {capability:?} is not supported")]
    CapabilityUnsupported { capability: &'static str },

    /// A digest string is not 64 lowercase hex characters.
    #[error("invalid content digest {value:?}: expected 64 lowercase hex chars")]
    DigestInvalid { value: String },

    /// Two inventory entries claimed the same path.
    #[error("duplicate inventory path {path:?}")]
    DuplicatePath { path: String },
}
