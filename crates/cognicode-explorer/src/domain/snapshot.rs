//! Snapshot rendering service using `mmdc` (mermaid-cli).
//!
//! Renders Mermaid diagram text to PNG or SVG by spawning `mmdc` as a
//! subprocess. Uses temp files for I/O since `mmdc stdout` mode is unreliable
//! with C4 macros.
//!
//! ## Concurrency
//!
//! A `Semaphore` with configurable permits (default: 4) bounds concurrent
//! renders to prevent Puppeteer/Chromium from exhausting RAM.

use std::io::Write as IoWrite;
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::Semaphore;

use thiserror::Error;

// ============================================================================
// Public types
// ============================================================================

/// Output format for snapshot rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotFormat {
    Png,
    Svg,
}

impl SnapshotFormat {
    /// Parse a format string (`"png"` or `"svg"`).
    pub fn parse(s: &str) -> Result<Self, SnapshotParseError> {
        match s.to_ascii_lowercase().as_str() {
            "png" => Ok(Self::Png),
            "svg" => Ok(Self::Svg),
            other => Err(SnapshotParseError(other.to_string())),
        }
    }

    /// MIME content type for this format.
    pub fn content_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Svg => "image/svg+xml",
        }
    }

    /// File extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
        }
    }
}

#[derive(Debug, Error)]
#[error("unknown snapshot format: {0} (expected: png, svg)")]
pub struct SnapshotParseError(String);

/// Errors that can occur during snapshot rendering.
#[derive(Debug, Error)]
pub enum SnapshotError {
    /// Mermaid text is empty.
    #[error("mermaid text is empty")]
    MermaidEmpty,

    /// Mermaid text exceeds the 1 MB size limit.
    #[error("mermaid text exceeds 1 MB size limit ({size} bytes)")]
    SizeLimitExceeded { size: usize },

    /// The `mmdc` binary was not found on the system.
    #[error("mmdc not found — install mermaid-cli: npm install -g @mermaid-js/mermaid-cli")]
    MmdcNotFound,

    /// Rendering failed with an error message from mmdc.
    #[error("mmdc render failed: {0}")]
    RenderFailed(String),

    /// Rendering timed out after the configured duration.
    #[error("render timed out after {0:?}")]
    Timeout(Duration),

    /// Graph service is not wired in the current context.
    #[error("graph service not wired")]
    GraphServiceNotWired,

    /// Workspace service is not wired in the current context.
    #[error("workspace service not wired")]
    WorkspaceNotWired,

    /// Target symbol is required for trace-based view kinds.
    #[error("target is required for trace view kinds")]
    TargetRequiredForTrace,

    /// Mermaid text emission failed with a message from the underlying emitter.
    #[error("mermaid emission failed: {0}")]
    EmissionFailed(String),
}

/// Snapshot rendering service.
///
/// Spawns `mmdc` as a subprocess to render Mermaid text to PNG or SVG.
/// Concurrency is bounded by a `Semaphore` (default 4 permits).
#[derive(Clone)]
pub struct SnapshotService {
    permits: Arc<Semaphore>,
    timeout: Duration,
}

impl SnapshotService {
    /// Maximum Mermaid text size: 1 MB.
    pub const MAX_MERMAID_SIZE: usize = 1 * 1024 * 1024;

    /// Default number of concurrent renders.
    pub const DEFAULT_PERMITS: usize = 4;

    /// Default render timeout.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

    /// Create a new `SnapshotService` with the default permit count (4).
    pub fn new() -> Self {
        Self::with_permits(Self::DEFAULT_PERMITS)
    }

    /// Create a new `SnapshotService` with a custom permit count.
    pub fn with_permits(permits: usize) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(permits)),
            timeout: Self::DEFAULT_TIMEOUT,
        }
    }

    /// Set the render timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Render Mermaid text to the requested format.
    ///
    /// # Errors
    ///
    /// Returns `SnapshotError` if:
    /// - `mermaid_text` is empty
    /// - `mermaid_text` exceeds [`MAX_MERMAID_SIZE`]
    /// - `mmdc` is not installed
    /// - rendering fails or times out
    pub async fn render(
        &self,
        mermaid_text: &str,
        format: SnapshotFormat,
    ) -> Result<Vec<u8>, SnapshotError> {
        // 1. Validate empty
        if mermaid_text.trim().is_empty() {
            return Err(SnapshotError::MermaidEmpty);
        }

        // 2. Validate size
        let size = mermaid_text.len();
        if size > Self::MAX_MERMAID_SIZE {
            return Err(SnapshotError::SizeLimitExceeded { size });
        }

        // 3. Check mmdc exists
        which::which("mmdc").map_err(|_| SnapshotError::MmdcNotFound)?;

        // 4. Acquire concurrency permit
        let _permit = self.permits.acquire().await.expect("semaphore closed");

        // 5. Render with timeout
        tokio::time::timeout(self.timeout, self.render_inner(mermaid_text, format))
            .await
            .map_err(|_| SnapshotError::Timeout(self.timeout))?
    }

    async fn render_inner(
        &self,
        mermaid_text: &str,
        format: SnapshotFormat,
    ) -> Result<Vec<u8>, SnapshotError> {
        let output_ext = format.extension();

        // Create temp files using sync I/O (tempfile is sync-only)
        let mut input_file = tempfile::NamedTempFile::with_suffix(".mmd")
            .map_err(|e| SnapshotError::RenderFailed(format!("failed to create temp file: {e}")))?;

        // Write Mermaid text to input file (sync)
        input_file
            .write_all(mermaid_text.as_bytes())
            .map_err(|e| SnapshotError::RenderFailed(format!("failed to write temp file: {e}")))?;
        let input_path = input_file.path().to_owned();

        let output_file =
            tempfile::NamedTempFile::with_suffix(&format!(".{output_ext}")).map_err(|e| {
                SnapshotError::RenderFailed(format!("failed to create output temp file: {e}"))
            })?;
        let output_path = output_file.path().to_owned();

        // Build mmdc command
        // mmdc -i <input.mmd> -o <output.png|svg> -b transparent
        let mut cmd = Command::new("mmdc");
        cmd.arg("-i").arg(&input_path);
        cmd.arg("-o").arg(&output_path);
        cmd.arg("-b").arg("transparent");
        // Suppress puppeteer sandbox warnings on Linux
        #[cfg(target_os = "linux")]
        {
            cmd.arg("--puppeteerArgs").arg("--no-sandbox");
        }

        // Run and capture stderr
        let output = cmd
            .output()
            .await
            .map_err(|e| SnapshotError::RenderFailed(format!("failed to spawn mmdc: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let msg = if stderr.is_empty() {
                format!("mmdc exited with status {}", output.status)
            } else {
                format!("mmdc error: {stderr}")
            };
            return Err(SnapshotError::RenderFailed(msg));
        }

        // Read output file using sync I/O
        let output_data = std::fs::read(&output_path)
            .map_err(|e| SnapshotError::RenderFailed(format!("failed to read output file: {e}")))?;

        // Temp files are dropped here, triggering cleanup
        Ok(output_data)
    }
}

impl Default for SnapshotService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // SnapshotFormat::parse
    // ------------------------------------------------------------------------

    #[test]
    fn snapshot_format_parse_png() {
        assert_eq!(SnapshotFormat::parse("png").unwrap(), SnapshotFormat::Png);
        assert_eq!(SnapshotFormat::parse("PNG").unwrap(), SnapshotFormat::Png);
    }

    #[test]
    fn snapshot_format_parse_svg() {
        assert_eq!(SnapshotFormat::parse("svg").unwrap(), SnapshotFormat::Svg);
        assert_eq!(SnapshotFormat::parse("SVG").unwrap(), SnapshotFormat::Svg);
    }

    #[test]
    fn snapshot_format_parse_unknown() {
        let result = SnapshotFormat::parse("jpg");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unknown snapshot format")
        );
    }

    #[test]
    fn snapshot_format_content_type() {
        assert_eq!(SnapshotFormat::Png.content_type(), "image/png");
        assert_eq!(SnapshotFormat::Svg.content_type(), "image/svg+xml");
    }

    #[test]
    fn snapshot_format_extension() {
        assert_eq!(SnapshotFormat::Png.extension(), "png");
        assert_eq!(SnapshotFormat::Svg.extension(), "svg");
    }

    // ------------------------------------------------------------------------
    // SnapshotService construction
    // ------------------------------------------------------------------------

    #[test]
    fn snapshot_service_new() {
        let svc = SnapshotService::new();
        // Just verify it doesn't panic
    }

    #[test]
    fn snapshot_service_with_permits() {
        let svc = SnapshotService::with_permits(8);
        let svc2 = SnapshotService::with_permits(2);
        // Both should construct without panicking
        let _ = svc;
        let _ = svc2;
    }

    #[test]
    fn snapshot_service_with_timeout() {
        let svc = SnapshotService::new().with_timeout(Duration::from_secs(60));
        let _ = svc;
    }

    // ------------------------------------------------------------------------
    // Size validation (without actually calling mmdc)
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn render_empty_mermaid() {
        let svc = SnapshotService::new();
        let result = svc.render("", SnapshotFormat::Png).await;
        assert!(matches!(result, Err(SnapshotError::MermaidEmpty)));
    }

    #[tokio::test]
    async fn render_whitespace_only_mermaid() {
        let svc = SnapshotService::new();
        let result = svc.render("   \n\t  ", SnapshotFormat::Png).await;
        assert!(matches!(result, Err(SnapshotError::MermaidEmpty)));
    }

    #[tokio::test]
    async fn render_size_limit_exceeded() {
        let svc = SnapshotService::new();
        // Create text larger than 1 MB
        let large_text = "x".repeat(1024 * 1024 + 1);
        let result = svc.render(&large_text, SnapshotFormat::Png).await;
        assert!(
            matches!(result, Err(SnapshotError::SizeLimitExceeded { size }) if size > 1024 * 1024)
        );
    }
}
