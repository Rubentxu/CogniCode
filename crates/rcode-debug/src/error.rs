//! Error types for rcode-debug

use thiserror::Error;

/// Result type for rcode-debug operations
pub type Result<T> = std::result::Result<T, DebugError>;

/// Main error enum for debugging operations
#[derive(Error, Debug)]
pub enum DebugError {
    /// Adapter not found or not installed
    #[error("Debug adapter not available for {language}. Install instructions: {instructions}")]
    AdapterNotFound {
        language: String,
        instructions: String,
    },

    /// Toolchain not found (e.g., cargo, python3)
    #[error("Toolchain not found: {0}. Please install it first.")]
    ToolchainNotFound(String),

    /// Failed to connect to debug adapter
    #[error("Failed to connect to debug adapter: {0}")]
    ConnectionFailed(String),

    /// DAP protocol error
    #[error("DAP protocol error: {0}")]
    ProtocolError(String),

    /// Protocol parsing error (alias for ProtocolError)
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Adapter returned an error
    #[error("Adapter error: {0}")]
    AdapterError(String),

    /// Debug session error
    #[error("Debug session error: {0}")]
    SessionError(String),

    /// Launch failed
    #[error("Failed to launch target: {0}")]
    LaunchFailed(String),

    /// Target process crashed unexpectedly
    #[error("Target process crashed: {0}")]
    TargetCrashed(String),

    /// Breakpoint could not be set
    #[error("Failed to set breakpoint at {file}:{line}: {reason}")]
    BreakpointFailed {
        file: String,
        line: u32,
        reason: String,
    },

    /// Evaluation failed
    #[error("Failed to evaluate expression: {0}")]
    EvaluationFailed(String),

    /// Timeout waiting for event
    #[error("Timeout waiting for debug event: {0}")]
    Timeout(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Debugging disabled by configuration
    #[error("Debugging is disabled. Enable in .rcode/config.toml or set RCODE_DEBUG=1")]
    Disabled,

    /// Unsupported language
    #[error("Language not supported: {0}")]
    UnsupportedLanguage(String),

    /// Auto-install failed
    #[error("Failed to auto-install {language} adapter: {reason}")]
    AutoInstallFailed {
        language: String,
        reason: String,
    },
}
