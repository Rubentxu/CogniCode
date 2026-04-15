//! LSP client implementation

use lsp_types::{ClientCapabilities, InitializeParams, ShowMessageRequestParams};
use parking_lot::RwLock;
use std::collections::HashMap;

/// LSP client for communicating with editors and external LSP servers
pub struct LspClient {
    capabilities: RwLock<Option<ClientCapabilities>>,
    #[allow(dead_code)]
    server_info: RwLock<Option<ServerInfo>>,
    /// Language to LSP server mapping
    lsp_servers: RwLock<HashMap<String, LspServerConfig>>,
}

/// Configuration for an external LSP server
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    /// Path to the LSP server executable
    pub command: String,
    /// Command line arguments
    pub args: Vec<String>,
    /// Working directory
    pub cwd: Option<String>,
    /// Supported languages
    pub languages: Vec<String>,
}

/// Server info received from LSP
#[derive(Debug, Clone)]
pub struct ServerInfo {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub version: String,
}

impl LspClient {
    /// Creates a new LSP client
    pub fn new() -> Self {
        Self {
            capabilities: RwLock::new(None),
            server_info: RwLock::new(None),
            lsp_servers: RwLock::new(HashMap::new()),
        }
    }

    /// Creates a new LSP client with predefined server configurations
    pub fn with_servers(servers: HashMap<String, LspServerConfig>) -> Self {
        Self {
            capabilities: RwLock::new(None),
            server_info: RwLock::new(None),
            lsp_servers: RwLock::new(servers),
        }
    }

    /// Initializes the client with server capabilities
    pub fn initialize(&self, params: &InitializeParams) {
        *self.capabilities.write() = params.capabilities.clone().into();
    }

    /// Gets the client capabilities
    pub fn capabilities(&self) -> Option<ClientCapabilities> {
        self.capabilities.read().clone()
    }

    /// Registers an external LSP server configuration
    pub fn register_lsp_server(&self, language: &str, config: LspServerConfig) {
        self.lsp_servers
            .write()
            .insert(language.to_string(), config);
    }

    /// Gets the LSP server configuration for a language
    #[allow(dead_code)]
    pub fn get_lsp_server(&self, language: &str) -> Option<LspServerConfig> {
        self.lsp_servers.read().get(language).cloned()
    }

    /// Gets all registered LSP servers
    #[allow(dead_code)]
    pub fn get_all_servers(&self) -> HashMap<String, LspServerConfig> {
        self.lsp_servers.read().clone()
    }

    /// Shows a message to the user
    pub fn show_message(&self, message: &str, _message_type: u32) {
        tracing::info!("[LSP] {}", message);
    }

    /// Logs a message
    pub fn log_message(&self, message: &str) {
        tracing::debug!("[LSP] {}", message);
    }

    /// Handles a showMessageRequest from the server
    #[allow(dead_code)]
    pub fn handle_show_message_request(&self, _params: ShowMessageRequestParams) -> Option<String> {
        // In a real implementation, this would show a dialog to the user
        // For now, return None (no action taken)
        None
    }
}

impl Default for LspClient {
    fn default() -> Self {
        Self::new()
    }
}

/// LSP-related errors
#[derive(Debug, thiserror::Error)]
pub enum LspError {
    #[error("Server not found for language: {0}")]
    ServerNotFound(String),

    #[error("Failed to spawn LSP process: {0}")]
    SpawnFailed(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Process error: {0}")]
    ProcessError(String),

    #[error("Communication error: {0}")]
    CommunicationError(String),

    #[error("Timeout waiting for response")]
    Timeout,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_client_creation() {
        let client = LspClient::new();
        assert!(client.capabilities().is_none());
    }

    #[test]
    fn test_register_lsp_server() {
        let client = LspClient::new();
        let config = LspServerConfig {
            command: "rust-analyzer".to_string(),
            args: vec![],
            cwd: None,
            languages: vec!["rust".to_string()],
        };
        client.register_lsp_server("rust", config);

        let servers = client.lsp_servers.read();
        assert!(servers.contains_key("rust"));
    }

    #[test]
    fn test_lsp_server_config() {
        let config = LspServerConfig {
            command: "pyright".to_string(),
            args: vec!["--stdio".to_string()],
            cwd: Some("/project".to_string()),
            languages: vec!["python".to_string()],
        };

        assert_eq!(config.command, "pyright");
        assert_eq!(config.args, vec!["--stdio"]);
        assert_eq!(config.cwd, Some("/project".to_string()));
    }
}
