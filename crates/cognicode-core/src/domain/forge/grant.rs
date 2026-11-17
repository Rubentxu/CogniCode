//! How much an evaluation may do with the code under review.
//!
//! The rule is structural, not advisory: the only value that permits
//! executing repository code or touching secrets is built from a head
//! that lives in the base repository. A fork, or a head whose origin
//! the forge did not state, can only ever receive [`ExecutionGrant::Sandboxed`].

use super::identity::HeadOrigin;

/// Proof that the head comes from the base repository. It has no public
/// constructor: [`ExecutionGrant::for_origin`] is the only source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustedExecution {
    _private: (),
}

/// What the evaluator of a pull request is allowed to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionGrant {
    /// Analyze the revisions as data. Do not build, run tests, run
    /// scripts or load plugins from the pull request, and do not use any
    /// credential: none is handed to the evaluator.
    Sandboxed,
    /// The head is authored inside the base repository, so executing it
    /// with the repository's permissions adds no authority the author
    /// did not already hold.
    Trusted(TrustedExecution),
}

impl ExecutionGrant {
    /// Fork and unknown origins are sandboxed; only the base
    /// repository's own branches are trusted.
    pub fn for_origin(origin: HeadOrigin) -> Self {
        match origin {
            HeadOrigin::SameRepository => Self::Trusted(TrustedExecution { _private: () }),
            HeadOrigin::Fork | HeadOrigin::Unknown => Self::Sandboxed,
        }
    }

    /// Whether code of the pull request may be executed.
    pub fn permits_code_execution(&self) -> bool {
        matches!(self, Self::Trusted(_))
    }

    /// Whether the evaluator may be given privileged secrets.
    pub fn permits_secrets(&self) -> bool {
        matches!(self, Self::Trusted(_))
    }
}
