//! RustVerifier — rustc-based Rust code verification
//!
//! This verifier checks Rust source files by compiling them with `rustc` in a
//! sandboxed temporary directory.

use crate::domain::traits::code_verifier::{CodeVerifier, CodeVerifierError, CompilationResult};
use async_trait::async_trait;
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::TempDir;

/// Maximum length for error message snippets
const MAX_ERROR_LENGTH: usize = 200;

/// rustc invocation arguments
const RUSTC_ARGS: &[&str] = &["--edition", "2021", "--crate-type", "lib"];

/// Verifies Rust source files by compiling them with rustc.
#[derive(Debug, Default)]
pub struct RustVerifier;

impl RustVerifier {
    /// Creates a new RustVerifier instance.
    pub fn new() -> Self {
        Self
    }

    /// Sets up a temporary file for rustc verification.
    ///
    /// Creates a TempDir with prefix `cognicode_rust_verify_`, writes content to a temp file,
    /// and returns both the TempDir (for lifetime management) and the temp file path.
    fn setup_temp_file(
        content: &str,
        file_name: &str,
    ) -> Result<(TempDir, std::path::PathBuf), CodeVerifierError> {
        let temp_dir = TempDir::with_prefix("cognicode_rust_verify_").map_err(|e| {
            CodeVerifierError::SubprocessFailed(format!("Failed to create temp dir: {}", e))
        })?;

        let temp_file_path = temp_dir.path().join(file_name);

        {
            let mut file = std::fs::File::create(&temp_file_path).map_err(|e| {
                CodeVerifierError::SubprocessFailed(format!("Failed to create temp file: {}", e))
            })?;
            file.write_all(content.as_bytes()).map_err(|e| {
                CodeVerifierError::SubprocessFailed(format!("Failed to write to temp file: {}", e))
            })?;
        }

        Ok((temp_dir, temp_file_path))
    }

    /// Runs rustc asynchronously with kill_on_drop(true).
    ///
    /// When the returned future is dropped (e.g., on timeout), the child process is killed.
    ///
    /// `current_dir` is set to the parent dir of the temp file so output artifacts
    /// (`*.rlib`) are written into the per-call temp dir and never collide between
    /// parallel tests. See `verify_impl` for the full rationale and M0.5 reference.
    async fn run_rustc_async(
        temp_file: std::path::PathBuf,
    ) -> Result<std::process::Output, CodeVerifierError> {
        let work_dir = temp_file
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let mut cmd = tokio::process::Command::new("rustc");
        cmd.args(RUSTC_ARGS)
            .arg(&temp_file)
            .current_dir(&work_dir)
            .kill_on_drop(true);

        cmd.output().await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CodeVerifierError::ToolchainUnavailable("rustc".to_string())
            } else {
                CodeVerifierError::SubprocessFailed(format!("rustc execution failed: {}", e))
            }
        })
    }

    /// Internal synchronous verification logic.
    fn verify_impl(
        &self,
        path: &str,
        _timeout_secs: Option<u64>,
    ) -> Result<CompilationResult, CodeVerifierError> {
        // Check if file extension is .rs
        let file_path = Path::new(path);
        if file_path.extension().and_then(|e| e.to_str()) != Some("rs") {
            return Ok(CompilationResult::Skipped {
                reason: "not-rust".to_string(),
            });
        }

        // Read the file content
        let content = fs::read_to_string(path)
            .map_err(|e| CodeVerifierError::FileUnreadable(e.to_string()))?;

        // Get the file name for temp file
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("lib.rs");

        // Set up temp file (file I/O)
        let (temp_dir, temp_file_path) = Self::setup_temp_file(&content, file_name)?;

        // Run rustc --edition 2021 --crate-type lib.
        //
        // IMPORTANT: We set `current_dir` to the temp_dir so that the output
        // artifacts (e.g. `libvalid.rlib` for an input file `valid.rs`) land in
        // the per-test temp dir rather than in the workspace target directory.
        // Without this, two tests running in parallel that both compile a file
        // named `valid.rs` race on the same `libvalid.rlib` output path in CWD
        // and one of them fails with "failed to open object file: No such file
        // or directory (os error 2)" — see M0.5 in MAINTENANCE.md.
        let output = std::process::Command::new("rustc")
            .args(RUSTC_ARGS)
            .arg(&temp_file_path)
            .current_dir(temp_dir.path())
            .output();

        // Keep temp_dir alive until after rustc completes so the .rlib output
        // has somewhere to live before cleanup. The function returns the result
        // first, then `temp_dir` is dropped at the end of this scope.
        drop(temp_dir);

        match output {
            Ok(output) if output.status.success() => {
                // Compilation succeeded
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                Ok(CompilationResult::Verified { stdout })
            }
            Ok(output) => {
                // Compilation failed - truncate stderr to MAX_ERROR_LENGTH chars
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let error_snippet = if stderr.len() > MAX_ERROR_LENGTH {
                    stderr[..MAX_ERROR_LENGTH].to_string()
                } else {
                    stderr
                };
                Ok(CompilationResult::Rejected {
                    error: error_snippet,
                })
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Err(CodeVerifierError::ToolchainUnavailable("rustc".to_string()))
                } else {
                    Err(CodeVerifierError::SubprocessFailed(format!(
                        "rustc execution failed: {}",
                        e
                    )))
                }
            }
        }
    }
}

#[async_trait]
impl CodeVerifier for RustVerifier {
    fn verify(&self, path: &str) -> Result<CompilationResult, CodeVerifierError> {
        self.verify_impl(path, None)
    }

    async fn verify_with_timeout(
        &self,
        path: &str,
        timeout_secs: u64,
    ) -> Result<CompilationResult, CodeVerifierError> {
        let file_path_owned = path.to_string();

        // Read file content and set up temp file (file I/O - can be blocking)
        let file_path = Path::new(&file_path_owned);
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("lib.rs")
            .to_string();

        let content = std::fs::read_to_string(&file_path_owned)
            .map_err(|e| CodeVerifierError::FileUnreadable(e.to_string()))?;

        // Set up temp file (file I/O - can be blocking)
        let (temp_dir, temp_file_path) = Self::setup_temp_file(&content, &file_name)?;

        let timeout_duration = std::time::Duration::from_secs(timeout_secs);

        // Use tokio::time::timeout with tokio::process::Command directly
        // kill_on_drop ensures process dies on timeout
        // We use spawn_blocking for the blocking file operations and temp dir management
        let output_result =
            tokio::time::timeout(timeout_duration, Self::run_rustc_async(temp_file_path)).await;

        // Drop temp_dir explicitly after rustc completes to ensure cleanup happens
        // while temp_file_path reference is still valid
        drop(temp_dir);

        // Map output to compilation result
        match output_result {
            Ok(Ok(output)) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                Ok(CompilationResult::Verified { stdout })
            }
            Ok(Ok(output)) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let error_snippet = if stderr.len() > MAX_ERROR_LENGTH {
                    stderr[..MAX_ERROR_LENGTH].to_string()
                } else {
                    stderr
                };
                Ok(CompilationResult::Rejected {
                    error: error_snippet,
                })
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(CodeVerifierError::Timeout(timeout_secs)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_verify_compilable_rust() {
        let verifier = RustVerifier::new();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("valid.rs");
        std::fs::write(&file_path, "pub fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();

        let result = verifier.verify(file_path.to_str().unwrap());
        assert!(result.is_ok(), "Verification should succeed");

        match result.unwrap() {
            CompilationResult::Verified { stdout } => {
                assert!(
                    stdout.is_empty() || stdout.contains("warning") || stdout.contains("Compiling")
                );
            }
            other => panic!("Expected Verified, got {:?}", other),
        }
    }

    #[test]
    #[serial]
    fn test_verify_broken_rust() {
        let verifier = RustVerifier::new();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("broken.rs");
        // Missing closing brace
        std::fs::write(&file_path, "pub fn add(a: i32, b: i32) -> i32 { a + b").unwrap();

        let result = verifier.verify(file_path.to_str().unwrap());
        assert!(result.is_ok(), "Verification should return result");

        match result.unwrap() {
            CompilationResult::Rejected { error } => {
                assert!(!error.is_empty());
                assert!(error.len() <= MAX_ERROR_LENGTH);
            }
            other => panic!("Expected Rejected, got {:?}", other),
        }
    }

    #[test]
    fn test_verify_non_rust_file() {
        let verifier = RustVerifier::new();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("script.py");
        std::fs::write(&file_path, "def hello():\n    print('world')").unwrap();

        let result = verifier.verify(file_path.to_str().unwrap());
        assert!(result.is_ok(), "Verification should return result");

        match result.unwrap() {
            CompilationResult::Skipped { reason } => {
                assert_eq!(reason, "not-rust");
            }
            other => panic!("Expected Skipped, got {:?}", other),
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_verify_with_timeout_success() {
        let verifier = RustVerifier::new();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("valid.rs");
        std::fs::write(&file_path, "pub fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();

        let result = verifier
            .verify_with_timeout(file_path.to_str().unwrap(), 10)
            .await;
        assert!(result.is_ok(), "Verification should succeed");

        match result.unwrap() {
            CompilationResult::Verified { .. } => {}
            other => panic!("Expected Verified, got {:?}", other),
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_verify_with_timeout_zero() {
        let verifier = RustVerifier::new();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("valid.rs");
        std::fs::write(&file_path, "pub fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();

        // 0 second timeout - should trigger timeout error
        let result = verifier
            .verify_with_timeout(file_path.to_str().unwrap(), 0)
            .await;

        assert!(result.is_err(), "Timeout should cause error");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("timed out") || err_msg.contains("timeout"),
            "Error should mention timeout, got: {}",
            err_msg
        );
    }
}
