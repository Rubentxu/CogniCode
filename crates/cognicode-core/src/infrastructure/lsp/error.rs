use crate::infrastructure::lsp::json_rpc::JsonRpcTransportError;

#[derive(Debug, thiserror::Error)]
pub enum LspProcessError {
    #[error("Failed to spawn {binary}: {reason}")]
    SpawnFailed { binary: String, reason: String },

    #[error("Not initialized. Call initialize() first.")]
    NotInitialized,

    #[error("Timeout during {operation} for {language}")]
    Timeout { operation: String, language: String },

    #[error("Transport error: {0}")]
    Transport(#[from] JsonRpcTransportError),

    #[error("Failed to kill process: {0}")]
    KillFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Server {language} not ready after {waited_secs}s: {status}")]
    ServerNotReady {
        language: String,
        status: ServerStatus,
        waited_secs: u64,
    },

    #[error("Server {language} crashed: {reason} (crash count: {crash_count})")]
    ServerCrashed {
        language: String,
        reason: String,
        crash_count: u32,
    },

    #[error("Request {method} timed out after {waited_secs}s")]
    RequestTimeout { method: String, waited_secs: u64 },

    #[error("Request {method} was cancelled")]
    Cancelled { method: String },

    #[error("Server not found for language: {0}")]
    ServerNotFound(String),

    #[error("Communication error: {0}")]
    CommunicationError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServerStatus {
    Starting,
    Indexing { progress: f32 },
    Ready,
    Busy,
    Crashed { reason: String },
}

impl Default for ServerStatus {
    fn default() -> Self {
        ServerStatus::Starting
    }
}

impl ServerStatus {
    pub fn is_ready(&self) -> bool {
        matches!(self, ServerStatus::Ready)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, ServerStatus::Crashed { .. })
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ServerStatus::Starting => "starting",
            ServerStatus::Indexing { .. } => "indexing",
            ServerStatus::Ready => "ready",
            ServerStatus::Busy => "busy",
            ServerStatus::Crashed { .. } => "crashed",
        }
    }
}

impl std::fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerStatus::Starting => write!(f, "starting"),
            ServerStatus::Indexing { progress } => write!(f, "indexing ({:.0}%)", progress),
            ServerStatus::Ready => write!(f, "ready"),
            ServerStatus::Busy => write!(f, "busy"),
            ServerStatus::Crashed { reason } => write!(f, "crashed: {}", reason),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub message: String,
    pub percentage: Option<f32>,
    pub status: ServerStatus,
}

pub trait ProgressCallback: Send + Sync + Fn(ProgressUpdate) + 'static {}
impl<F: Send + Sync + Fn(ProgressUpdate) + 'static> ProgressCallback for F {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_failed_error() {
        let err = LspProcessError::SpawnFailed {
            binary: "rust-analyzer".to_string(),
            reason: "not found".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("rust-analyzer"));
        assert!(msg.contains("not found"));
        assert!(msg.contains("Failed to spawn"));
        assert!(matches!(err, LspProcessError::SpawnFailed { .. }));
    }

    #[test]
    fn test_not_initialized_error() {
        let err = LspProcessError::NotInitialized;
        let msg = err.to_string();
        assert!(msg.contains("Not initialized"));
        assert!(msg.contains("initialize()"));
        assert!(matches!(err, LspProcessError::NotInitialized));
    }

    #[test]
    fn test_timeout_error() {
        let err = LspProcessError::Timeout {
            operation: "hover".to_string(),
            language: "rust".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("hover"));
        assert!(msg.contains("rust"));
        assert!(msg.contains("Timeout"));
        assert!(matches!(err, LspProcessError::Timeout { .. }));
    }

    #[test]
    fn test_kill_failed_error() {
        let err = LspProcessError::KillFailed("pid 123".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Failed to kill"));
        assert!(msg.contains("pid 123"));
        assert!(matches!(err, LspProcessError::KillFailed(_)));
    }

    #[test]
    fn test_server_not_ready_error() {
        let status = ServerStatus::Starting;
        let err = LspProcessError::ServerNotReady {
            language: "rust".to_string(),
            status,
            waited_secs: 30,
        };
        let msg = err.to_string();
        assert!(msg.contains("rust"));
        assert!(msg.contains("30"));
        assert!(msg.contains("not ready"));
        assert!(matches!(err, LspProcessError::ServerNotReady { .. }));
    }

    #[test]
    fn test_server_crashed_error() {
        let err = LspProcessError::ServerCrashed {
            language: "rust".to_string(),
            reason: "out of memory".to_string(),
            crash_count: 3,
        };
        let msg = err.to_string();
        assert!(msg.contains("rust"));
        assert!(msg.contains("out of memory"));
        assert!(msg.contains("3"));
        assert!(msg.contains("crashed"));
        assert!(matches!(err, LspProcessError::ServerCrashed { .. }));
    }

    #[test]
    fn test_request_timeout_error() {
        let err = LspProcessError::RequestTimeout {
            method: "textDocument/definition".to_string(),
            waited_secs: 10,
        };
        let msg = err.to_string();
        assert!(msg.contains("textDocument/definition"));
        assert!(msg.contains("10"));
        assert!(msg.contains("timed out"));
        assert!(matches!(err, LspProcessError::RequestTimeout { .. }));
    }

    #[test]
    fn test_cancelled_error() {
        let err = LspProcessError::Cancelled {
            method: "shutdown".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("shutdown"));
        assert!(msg.contains("cancelled"));
        assert!(matches!(err, LspProcessError::Cancelled { .. }));
    }

    #[test]
    fn test_server_not_found_error() {
        let err = LspProcessError::ServerNotFound("python".to_string());
        let msg = err.to_string();
        assert!(msg.contains("python"));
        assert!(msg.contains("Server not found"));
        assert!(matches!(err, LspProcessError::ServerNotFound(_)));
    }

    #[test]
    fn test_communication_error() {
        let err = LspProcessError::CommunicationError("pipe broken".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Communication error"));
        assert!(msg.contains("pipe broken"));
        assert!(matches!(err, LspProcessError::CommunicationError(_)));
    }

    #[test]
    fn test_internal_error() {
        let err = LspProcessError::Internal("unknown".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Internal error"));
        assert!(msg.contains("unknown"));
        assert!(matches!(err, LspProcessError::Internal(_)));
    }

    #[test]
    fn test_io_error_conversion() {
        use std::io;
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err = LspProcessError::Io(io_err);
        let msg = err.to_string();
        assert!(msg.contains("IO error"));
        assert!(msg.contains("file not found"));
        assert!(matches!(err, LspProcessError::Io(_)));
    }

    #[test]
    fn test_server_status_default() {
        let status = ServerStatus::default();
        assert!(matches!(status, ServerStatus::Starting));
    }

    #[test]
    fn test_server_status_is_ready() {
        assert!(!ServerStatus::Starting.is_ready());
        assert!(!ServerStatus::Indexing { progress: 50.0 }.is_ready());
        assert!(ServerStatus::Ready.is_ready());
        assert!(!ServerStatus::Busy.is_ready());
        assert!(!ServerStatus::Crashed {
            reason: "oops".to_string()
        }
        .is_ready());
    }

    #[test]
    fn test_server_status_is_terminal() {
        assert!(!ServerStatus::Starting.is_terminal());
        assert!(!ServerStatus::Indexing { progress: 50.0 }.is_terminal());
        assert!(!ServerStatus::Ready.is_terminal());
        assert!(!ServerStatus::Busy.is_terminal());
        assert!(ServerStatus::Crashed {
            reason: "oops".to_string()
        }
        .is_terminal());
    }

    #[test]
    fn test_server_status_as_str() {
        assert_eq!(ServerStatus::Starting.as_str(), "starting");
        assert_eq!(
            ServerStatus::Indexing { progress: 50.0 }.as_str(),
            "indexing"
        );
        assert_eq!(ServerStatus::Ready.as_str(), "ready");
        assert_eq!(ServerStatus::Busy.as_str(), "busy");
        assert_eq!(
            ServerStatus::Crashed {
                reason: "oops".to_string()
            }
            .as_str(),
            "crashed"
        );
    }

    #[test]
    fn test_server_status_display() {
        assert_eq!(ServerStatus::Starting.to_string(), "starting");
        assert_eq!(ServerStatus::Ready.to_string(), "ready");
        assert_eq!(ServerStatus::Busy.to_string(), "busy");
        assert_eq!(
            ServerStatus::Indexing { progress: 75.0 }.to_string(),
            "indexing (75%)"
        );
        assert_eq!(
            ServerStatus::Crashed {
                reason: "panic".to_string()
            }
            .to_string(),
            "crashed: panic"
        );
    }

    #[test]
    fn test_server_status_clone_eq() {
        let s1 = ServerStatus::Indexing { progress: 42.0 };
        let s2 = s1.clone();
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_progress_update_struct() {
        let update = ProgressUpdate {
            message: "Indexing...".to_string(),
            percentage: Some(50.0),
            status: ServerStatus::Indexing { progress: 50.0 },
        };
        assert_eq!(update.message, "Indexing...");
        assert_eq!(update.percentage, Some(50.0));
        assert!(matches!(
            update.status,
            ServerStatus::Indexing { progress: 50.0 }
        ));
    }
}
