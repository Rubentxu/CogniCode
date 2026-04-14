//! LSP Proxy Service - Routes operations between external LSP and CogniCode
//!
//! This service delegates basic LSP operations (hover, completion) to external
//! LSPs like rust-analyzer and pyright, while handling premium CogniCode
//! operations (impact analysis, cycle detection, complexity) internally.

use crate::application::services::analysis_service::AnalysisService;
use crate::domain::traits::code_intelligence::CodeIntelligenceProvider;
use crate::domain::value_objects::Location;
use crate::infrastructure::lsp::providers::composite::CompositeProvider;
use crate::infrastructure::lsp::{LspClient, LspError, LspServerConfig};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Operations that can be handled by an external LSP
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalLspOperation {
    Hover,
    Completion,
    GoToDefinition,
    FindReferences,
}

/// Operations that are CogniCode premium features
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PremiumOperation {
    AnalyzeImpact,
    DetectCycles,
    GetComplexity,
    SafeRefactor,
    GetCallHierarchy,
}

/// Result from an external LSP operation
#[derive(Debug, Clone)]
pub struct ExternalLspResult {
    /// The raw LSP response (JSON)
    pub response: serde_json::Value,
    /// Whether the operation succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Service that proxies LSP operations
pub struct LspProxyService {
    /// The underlying LSP client
    lsp_client: Arc<LspClient>,
    /// Analysis service for premium operations
    analysis_service: Arc<AnalysisService>,
    /// Server configurations by language
    servers: Arc<RwLock<HashMap<String, LspServerConfig>>>,
    /// Whether proxy mode is enabled
    proxy_enabled: bool,
    /// Composite provider for LSP-based code intelligence
    composite_provider: Option<Arc<CompositeProvider>>,
    /// Workspace root for LSP operations
    workspace_root: Option<PathBuf>,
}

impl LspProxyService {
    /// Creates a new LspProxyService with a workspace root
    pub fn new(analysis_service: Arc<AnalysisService>, workspace_root: PathBuf) -> Self {
        Self {
            lsp_client: Arc::new(LspClient::new()),
            analysis_service,
            servers: Arc::new(RwLock::new(HashMap::new())),
            proxy_enabled: false,
            composite_provider: None,
            workspace_root: Some(workspace_root),
        }
    }

    /// Creates a new LspProxyService without a workspace root
    pub fn new_without_workspace(analysis_service: Arc<AnalysisService>) -> Self {
        Self {
            lsp_client: Arc::new(LspClient::new()),
            analysis_service,
            servers: Arc::new(RwLock::new(HashMap::new())),
            proxy_enabled: false,
            composite_provider: None,
            workspace_root: None,
        }
    }

    /// Enables proxy mode with default server configurations
    pub fn enable_proxy_mode(&mut self) {
        self.proxy_enabled = true;
        self.setup_default_servers();
    }

    /// Enables proxy mode with a CompositeProvider for full LSP support
    pub fn enable_proxy_mode_with_provider(&mut self) {
        self.proxy_enabled = true;
        self.setup_default_servers();

        if let Some(ref workspace_root) = self.workspace_root {
            self.composite_provider = Some(Arc::new(CompositeProvider::new(workspace_root)));
        }
    }

    /// Sets up default LSP server configurations
    fn setup_default_servers(&mut self) {
        let mut servers = self.servers.write().unwrap();
        servers.insert(
            "rust".to_string(),
            LspServerConfig {
                command: "rust-analyzer".to_string(),
                args: vec![],
                cwd: None,
                languages: vec!["rust".to_string()],
            },
        );
        servers.insert(
            "python".to_string(),
            LspServerConfig {
                command: "pyright".to_string(),
                args: vec!["--stdio".to_string()],
                cwd: None,
                languages: vec!["python".to_string()],
            },
        );
        servers.insert(
            "typescript".to_string(),
            LspServerConfig {
                command: "typescript-language-server".to_string(),
                args: vec!["--stdio".to_string()],
                cwd: None,
                languages: vec!["typescript".to_string(), "javascript".to_string()],
            },
        );
    }

    /// Registers an LSP server configuration
    pub fn register_server(&self, language: &str, config: LspServerConfig) {
        self.servers
            .write()
            .unwrap()
            .insert(language.to_string(), config);
    }

    /// Determines if an operation should be routed to external LSP
    pub fn should_delegate_to_external(operation: &str) -> bool {
        matches!(
            operation,
            "hover"
                | "completion"
                | "textDocument/hover"
                | "textDocument/completion"
                | "textDocument/definition"
                | "textDocument/references"
                | "goto_definition"
                | "find_references"
        )
    }

    /// Determines if an operation is a premium CogniCode operation
    pub fn is_premium_operation(operation: &str) -> bool {
        matches!(
            operation,
            "analyze_impact"
                | "check_architecture"
                | "get_complexity"
                | "safe_refactor"
                | "get_call_hierarchy"
                | "get_file_symbols"
        )
    }

    /// Routes an operation to the appropriate handler
    ///
    /// Returns Ok(Some(result)) for handled operations, Ok(None) if the
    /// operation should be handled by the caller.
    pub async fn route_operation(
        &self,
        operation: &str,
        params: &serde_json::Value,
    ) -> Result<Option<serde_json::Value>, LspProxyError> {
        if !self.proxy_enabled {
            return Ok(None);
        }

        let provider = match self.composite_provider.as_ref() {
            Some(p) => p,
            None => {
                tracing::debug!(
                    "Proxy mode enabled but no composite provider, skipping operation: {}",
                    operation
                );
                return Ok(None);
            }
        };

        let location = match self.extract_location(params) {
            Ok(loc) => loc,
            Err(e) => {
                tracing::warn!("Failed to extract location from params: {}", e);
                return Err(e);
            }
        };

        match operation {
            "hover" | "textDocument/hover" => {
                let result = provider.hover(&location).await?;
                Ok(result.map(|h| serde_json::to_value(h).ok()).flatten())
            }
            "textDocument/definition" | "goto_definition" => {
                let result = provider.get_definition(&location).await?;
                Ok(result.map(|l| serde_json::to_value(l).ok()).flatten())
            }
            "textDocument/references" | "find_references" => {
                let result = provider.find_references(&location, true).await?;
                Ok(Some(serde_json::to_value(result).ok()).flatten())
            }
            _ => {
                tracing::debug!("Unsupported operation: {}", operation);
                Ok(None)
            }
        }
    }

    /// Extracts a Location from LSP-style params
    ///
    /// Expected format: `{ "textDocument": { "uri": "file:///path/to/file.rs" }, "position": { "line": N, "character": M } }`
    /// Note: LSP uses 0-indexed lines/characters, but Location uses 1-indexed.
    fn extract_location(&self, params: &serde_json::Value) -> Result<Location, LspProxyError> {
        let text_document = params
            .get("textDocument")
            .ok_or(LspProxyError::InvalidParams)?;
        let uri = text_document
            .get("uri")
            .ok_or(LspProxyError::InvalidParams)?
            .as_str()
            .ok_or(LspProxyError::InvalidParams)?;

        // Convert file:// URI to file path
        let file_path = if uri.starts_with("file://") {
            uri.strip_prefix("file://").unwrap_or(uri)
        } else {
            uri
        };

        let position = params.get("position").ok_or(LspProxyError::InvalidParams)?;
        let line = position
            .get("line")
            .ok_or(LspProxyError::InvalidParams)?
            .as_u64()
            .ok_or(LspProxyError::InvalidParams)? as u32;
        let character = position
            .get("character")
            .ok_or(LspProxyError::InvalidParams)?
            .as_u64()
            .ok_or(LspProxyError::InvalidParams)? as u32;

        // LSP uses 0-indexed, Location uses 1-indexed
        Ok(Location::new(
            file_path.to_string(),
            line + 1,
            character + 1,
        ))
    }

    /// Gets the analysis service for premium operations
    pub fn analysis_service(&self) -> Arc<AnalysisService> {
        self.analysis_service.clone()
    }

    /// Gets the underlying LSP client
    pub fn lsp_client(&self) -> Arc<LspClient> {
        self.lsp_client.clone()
    }

    /// Gets the proxy enabled status
    pub fn is_proxy_enabled(&self) -> bool {
        self.proxy_enabled
    }
}

/// Errors that can occur in LSP proxy operations
#[derive(Debug, thiserror::Error)]
pub enum LspProxyError {
    #[error("External LSP not running")]
    LspNotRunning,

    #[error("Operation not supported: {0}")]
    UnsupportedOperation(String),

    #[error("Language not supported: {0}")]
    LanguageNotSupported(String),

    #[error("Invalid parameters")]
    InvalidParams,

    #[error("LSP error: {0}")]
    LspError(#[from] LspError),

    #[error("Analysis error: {0}")]
    AnalysisError(#[from] crate::application::error::AppError),

    #[error("Code intelligence error: {0}")]
    CodeIntelligenceError(#[from] crate::domain::traits::code_intelligence::CodeIntelligenceError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_delegate_to_external() {
        assert!(LspProxyService::should_delegate_to_external("hover"));
        assert!(LspProxyService::should_delegate_to_external(
            "textDocument/hover"
        ));
        assert!(LspProxyService::should_delegate_to_external("completion"));
        assert!(LspProxyService::should_delegate_to_external(
            "textDocument/definition"
        ));

        assert!(!LspProxyService::should_delegate_to_external(
            "analyze_impact"
        ));
        assert!(!LspProxyService::should_delegate_to_external(
            "get_complexity"
        ));
        assert!(!LspProxyService::should_delegate_to_external(
            "check_architecture"
        ));
    }

    #[test]
    fn test_is_premium_operation() {
        assert!(LspProxyService::is_premium_operation("analyze_impact"));
        assert!(LspProxyService::is_premium_operation("get_complexity"));
        assert!(LspProxyService::is_premium_operation("check_architecture"));
        assert!(LspProxyService::is_premium_operation("safe_refactor"));

        assert!(!LspProxyService::is_premium_operation("hover"));
        assert!(!LspProxyService::is_premium_operation("completion"));
    }

    #[test]
    fn test_lsp_proxy_service_creation() {
        let service = LspProxyService::new_without_workspace(Arc::new(AnalysisService::new()));
        assert!(!service.is_proxy_enabled());
    }

    #[test]
    fn test_register_and_get_server() {
        let service = LspProxyService::new_without_workspace(Arc::new(AnalysisService::new()));

        let config = LspServerConfig {
            command: "custom-lsp".to_string(),
            args: vec!["--stdio".to_string()],
            cwd: Some("/custom/path".to_string()),
            languages: vec!["custom".to_string()],
        };

        service.register_server("custom", config);

        // Can't easily verify internal state, but we can test routing logic
        // The server config is stored and can be used when proxy mode is enabled
        assert!(!service.is_proxy_enabled());
    }

    #[test]
    fn test_enable_proxy_mode_sets_servers() {
        let mut service = LspProxyService::new_without_workspace(Arc::new(AnalysisService::new()));
        assert!(!service.is_proxy_enabled());

        service.enable_proxy_mode();
        assert!(service.is_proxy_enabled());

        // After enabling, the service should have default servers configured
        // This is tested indirectly via the fact that enable_proxy_mode doesn't panic
    }

    #[tokio::test]
    async fn test_route_operation_when_disabled() {
        let service = LspProxyService::new_without_workspace(Arc::new(AnalysisService::new()));

        let result = service.route_operation("hover", &serde_json::json!({})).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_route_operation_when_enabled_no_lsp() {
        let mut service = LspProxyService::new_without_workspace(Arc::new(AnalysisService::new()));
        service.enable_proxy_mode();

        // Even with proxy enabled, if no LSP is running, it returns None
        let result = service.route_operation("hover", &serde_json::json!({})).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_analysis_service_accessor() {
        let analysis_service = Arc::new(AnalysisService::new());
        let service = LspProxyService::new_without_workspace(analysis_service.clone());

        let retrieved = service.analysis_service();
        assert!(Arc::ptr_eq(&analysis_service, &retrieved));
    }

    #[test]
    fn test_lsp_client_accessor() {
        let service = LspProxyService::new_without_workspace(Arc::new(AnalysisService::new()));
        let _client = service.lsp_client();
        // Just verify it returns a valid Arc<LspClient>
        // The client can be used for further operations
    }

    #[test]
    fn test_should_delegate_comprehensive() {
        // External operations
        assert!(LspProxyService::should_delegate_to_external("hover"));
        assert!(LspProxyService::should_delegate_to_external(
            "textDocument/hover"
        ));
        assert!(LspProxyService::should_delegate_to_external("completion"));
        assert!(LspProxyService::should_delegate_to_external(
            "textDocument/completion"
        ));
        assert!(LspProxyService::should_delegate_to_external(
            "textDocument/definition"
        ));
        assert!(LspProxyService::should_delegate_to_external(
            "textDocument/references"
        ));
        assert!(LspProxyService::should_delegate_to_external(
            "goto_definition"
        ));
        assert!(LspProxyService::should_delegate_to_external(
            "find_references"
        ));

        // Premium operations - should NOT delegate
        assert!(!LspProxyService::should_delegate_to_external(
            "analyze_impact"
        ));
        assert!(!LspProxyService::should_delegate_to_external(
            "get_complexity"
        ));
        assert!(!LspProxyService::should_delegate_to_external(
            "check_architecture"
        ));
        assert!(!LspProxyService::should_delegate_to_external(
            "safe_refactor"
        ));
        assert!(!LspProxyService::should_delegate_to_external(
            "get_call_hierarchy"
        ));
        assert!(!LspProxyService::should_delegate_to_external(
            "get_file_symbols"
        ));
    }

    #[test]
    fn test_is_premium_operation_comprehensive() {
        // Premium operations
        assert!(LspProxyService::is_premium_operation("analyze_impact"));
        assert!(LspProxyService::is_premium_operation("check_architecture"));
        assert!(LspProxyService::is_premium_operation("get_complexity"));
        assert!(LspProxyService::is_premium_operation("safe_refactor"));
        assert!(LspProxyService::is_premium_operation("get_call_hierarchy"));
        assert!(LspProxyService::is_premium_operation("get_file_symbols"));

        // External operations - should NOT be premium
        assert!(!LspProxyService::is_premium_operation("hover"));
        assert!(!LspProxyService::is_premium_operation("completion"));
        assert!(!LspProxyService::is_premium_operation("textDocument/hover"));
        assert!(!LspProxyService::is_premium_operation(
            "textDocument/definition"
        ));
        assert!(!LspProxyService::is_premium_operation("find_references"));
    }

    #[test]
    fn test_external_lsp_result() {
        let success_result = ExternalLspResult {
            response: serde_json::json!({"contents": "hello"}),
            success: true,
            error: None,
        };
        assert!(success_result.success);
        assert!(success_result.error.is_none());

        let error_result = ExternalLspResult {
            response: serde_json::json!({}),
            success: false,
            error: Some("LSP not available".to_string()),
        };
        assert!(!error_result.success);
        assert!(error_result.error.is_some());
    }

    #[test]
    fn test_lsp_proxy_error_display() {
        let error = LspProxyError::LspNotRunning;
        assert_eq!(error.to_string(), "External LSP not running");

        let error = LspProxyError::UnsupportedOperation("test".to_string());
        assert_eq!(error.to_string(), "Operation not supported: test");

        let error = LspProxyError::LanguageNotSupported("python".to_string());
        assert_eq!(error.to_string(), "Language not supported: python");
    }

    // Note: Testing with real rust-analyzer requires the binary to be installed.
    // To test with real LSP:
    // 1. Install rust-analyzer: rustup component add rust-analyzer
    // 2. Create a test Rust project
    // 3. Start LspProxyService with proxy mode enabled
    // 4. Send actual LSP requests to the running rust-analyzer
    //
    // Example:
    // ```rust,ignore
    // #[test]
    // fn test_real_rust_analyzer() {
    //     let mut service = LspProxyService::new(Arc::new(AnalysisService::new()));
    //     service.enable_proxy_mode();
    //
    //     // Would need to actually start rust-analyzer and send requests
    //     // This is an integration test that requires the LSP binary
    // }
    // ```
}
