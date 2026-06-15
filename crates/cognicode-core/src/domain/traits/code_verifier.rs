//! Code Verifier Trait — ISP-segregated interface for code verification
//!
//! This trait abstracts the verification of code files via compilation checks.
//! Implementations include RustVerifier (rustc-based verification).

use crate::application::error::AppResult;
use async_trait::async_trait;

/// Compilation result from a code verification pass.
#[derive(Debug, Clone)]
pub enum CompilationResult {
    /// File compiled successfully
    Verified {
        /// stdout from the compiler
        stdout: String,
    },
    /// File failed to compile
    Rejected {
        /// Error message from the compiler (truncated to 200 chars)
        error: String,
    },
    /// File was skipped (e.g., not a supported language)
    Skipped {
        /// Reason for skipping
        reason: String,
    },
}

/// Trait for verifying code files via compilation checks.
///
/// This follows the ISP (Interface Segregation Principle) by providing
/// a dedicated interface for code verification, separate from file operations.
#[async_trait]
pub trait CodeVerifier: Send + Sync {
    /// Synchronous verification of a single file.
    ///
    /// Returns `Ok(CompilationResult)` on success (including Skipped/Rejected).
    /// Returns `Err` only on infrastructure failures (e.g., rustc not found).
    fn verify(&self, path: &str) -> AppResult<CompilationResult>;

    /// Verifies a file with a timeout limit.
    ///
    /// When the timeout is reached, the subprocess is killed and
    /// `Err(AppError::InvalidParameter("Verification timed out"))` is returned.
    ///
    /// This method is async to allow timeout enforcement without blocking.
    async fn verify_with_timeout(
        &self,
        path: &str,
        timeout_secs: u64,
    ) -> AppResult<CompilationResult>;
}
