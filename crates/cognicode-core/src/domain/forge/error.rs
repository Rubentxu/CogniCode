//! Named failures of the forge ports. A forge read or publication that
//! cannot be completed is reported as one of these; it is never
//! flattened into an empty answer or a green check.

/// A forge port could not complete its reading or publication.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ForgeError {
    /// A pull-request number must be a positive integer.
    #[error("pull-request number must be positive")]
    InvalidPullRequestNumber,

    /// An identity coming from the forge (a sha, a repository name)
    /// is not well formed, so no request is built from it.
    #[error("malformed {field}: {value:?}")]
    MalformedIdentity { field: &'static str, value: String },

    /// The source names a provider this adapter does not speak for.
    #[error("adapter for {expected:?} cannot serve provider {found:?}")]
    ForeignProvider {
        expected: &'static str,
        found: String,
    },

    /// The forge does not know the pull request.
    #[error("pull request {pull_request} not found")]
    PullRequestNotFound { pull_request: String },

    /// The forge refused the credentials or the permission for the
    /// operation. Never retried with other credentials.
    #[error("forge refused the operation: {detail}")]
    PermissionDenied { detail: String },

    /// The forge answered with a payload that does not carry what the
    /// contract needs; half an answer is never completed by guessing.
    #[error("malformed forge response: {detail}")]
    MalformedResponse { detail: String },

    /// The adapter was asked for something its forge has not declared.
    #[error("forge capability {capability:?} is not supported")]
    CapabilityUnsupported { capability: &'static str },

    /// Transport-level failure (program absent, unreadable output, a
    /// read over the agreed cap, a non-success exit).
    #[error("forge failure: {detail}")]
    ProviderFailure { detail: String },
}
