use crate::infrastructure::parser::Language;
use crate::infrastructure::lsp::error::{LspProcessError, ServerStatus};
use crate::infrastructure::lsp::json_rpc::JsonRpcTransport;
use lsp_types::{InitializeParams, ServerCapabilities};
use serde_json::Value;
use std::path::Path;
use std::time::Instant;
use tokio::process::{Child, Command};
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, warn};

const REQUEST_TIMEOUT_SECS: u64 = 30;

pub struct LspProcess {
    child: Child,
    transport: JsonRpcTransport,
    capabilities: Option<ServerCapabilities>,
    language: Language,
    initialized: bool,
    last_activity: Instant,
    status: ServerStatus,
}

impl LspProcess {
    pub async fn spawn(
        language: Language,
        workspace_root: &Path,
    ) -> Result<Self, LspProcessError> {
        let binary = language.lsp_server_binary();
        let args = language.lsp_args();

        info!("Spawning {} for {:?}", binary, language);

        let mut child = Command::new(binary)
            .args(args)
            .current_dir(workspace_root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| LspProcessError::SpawnFailed {
                binary: binary.to_string(),
                reason: e.to_string(),
            })?;

        let stdin = child.stdin.take().ok_or_else(|| LspProcessError::SpawnFailed {
            binary: binary.to_string(),
            reason: "Failed to capture stdin".to_string(),
        })?;

        let stdout = child.stdout.take().ok_or_else(|| LspProcessError::SpawnFailed {
            binary: binary.to_string(),
            reason: "Failed to capture stdout".to_string(),
        })?;

        let transport = JsonRpcTransport::new(stdin, stdout);

        Ok(Self {
            child,
            transport,
            capabilities: None,
            language,
            initialized: false,
            last_activity: Instant::now(),
            status: ServerStatus::Starting,
        })
    }

    pub async fn initialize(
        &mut self,
        workspace_root: &Path,
    ) -> Result<ServerCapabilities, LspProcessError> {
        if self.initialized {
            return Ok(self
                .capabilities
                .clone()
                .unwrap_or_else(|| ServerCapabilities::default()));
        }

        let root_uri = format!("file://{}", workspace_root.display());

        let mut params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri: Some(lsp_types::Url::parse(&root_uri).unwrap()),
            #[allow(deprecated)]
            root_path: None,
            initialization_options: None,
            capabilities: lsp_types::ClientCapabilities::default(),
            trace: None,
            workspace_folders: None,
            client_info: None,
            locale: None,
        };

        // jdtls requires workspaceFolders in initializationOptions
        if self.language == Language::Java {
            params.initialization_options = Some(serde_json::json!({
                "workspaceFolders": [
                    {
                        "uri": root_uri,
                        "name": workspace_root.display().to_string()
                    }
                ]
            }));
        }

        let result = timeout(
            Duration::from_secs(REQUEST_TIMEOUT_SECS),
            self.transport.send_request(
                "initialize",
                Some(serde_json::to_value(params).unwrap()),
            ),
        )
        .await
        .map_err(|_| LspProcessError::Timeout {
            operation: "initialize".to_string(),
            language: self.language.name().to_string(),
        })?
        .map_err(LspProcessError::Transport)?;

        let caps: ServerCapabilities = serde_json::from_value(result.result.unwrap_or_default())
            .unwrap_or_default();

        self.transport
            .send_notification("initialized", Some(Value::Null))
            .await
            .map_err(LspProcessError::Transport)?;

        self.capabilities = Some(caps.clone());
        self.initialized = true;
        self.status = ServerStatus::Ready;
        self.last_activity = Instant::now();

        debug!(
            "Initialized {} with capabilities: {:?}",
            self.language.name(),
            caps
        );

        Ok(caps)
    }

    pub async fn request(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, LspProcessError> {
        if !self.initialized {
            return Err(LspProcessError::NotInitialized);
        }

        self.last_activity = Instant::now();

        let result = timeout(
            Duration::from_secs(REQUEST_TIMEOUT_SECS),
            self.transport.send_request(method, params),
        )
        .await
        .map_err(|_| LspProcessError::Timeout {
            operation: method.to_string(),
            language: self.language.name().to_string(),
        })?
        .map_err(LspProcessError::Transport)?;

        Ok(result.result.unwrap_or(Value::Null))
    }

    pub async fn notification(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), LspProcessError> {
        self.transport
            .send_notification(method, params)
            .await
            .map_err(LspProcessError::Transport)
    }

    pub async fn open_document(&mut self, file_path: &str, content: &str) -> Result<(), LspProcessError> {
        // LSP spec requires lowercase languageId
        let language_id = match self.language {
            Language::Rust => "rust",
            Language::Python => "python",
            Language::JavaScript => "javascript",
            Language::TypeScript => "typescript",
            Language::Go => "go",
            Language::Java => "java",
        };
        let params = serde_json::json!({
            "textDocument": {
                "uri": format!("file://{}", file_path),
                "languageId": language_id,
                "version": 1,
                "text": content
            }
        });
        self.notification("textDocument/didOpen", Some(params)).await
    }

    pub async fn shutdown(&mut self) -> Result<(), LspProcessError> {
        if !self.initialized {
            self.kill().await?;
            return Ok(());
        }

        let _ = self
            .transport
            .send_request("shutdown", None)
            .await;

        let _ = self
            .transport
            .send_notification("exit", None)
            .await;

        match timeout(Duration::from_secs(5), self.child.wait()).await {
            Ok(Ok(status)) => {
                debug!("{} exited with status: {}", self.language.name(), status);
            }
            Ok(Err(e)) => {
                warn!("Error waiting for {} exit: {}", self.language.name(), e);
                self.kill().await?;
            }
            Err(_) => {
                warn!(
                    "Timeout waiting for {} exit, killing",
                    self.language.name()
                );
                self.kill().await?;
            }
        }

        self.initialized = false;
        Ok(())
    }

    async fn kill(&mut self) -> Result<(), LspProcessError> {
        match self.child.kill().await {
            Ok(()) => {
                let _ = self.child.wait().await;
                debug!("Killed {} process", self.language.name());
                Ok(())
            }
            Err(e) => {
                error!("Failed to kill {} process: {}", self.language.name(), e);
                Err(LspProcessError::KillFailed(e.to_string()))
            }
        }
    }

    pub fn capabilities(&self) -> Option<&ServerCapabilities> {
        self.capabilities.as_ref()
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn is_ready(&self) -> bool {
        self.initialized && self.status.is_ready()
    }

    pub fn status(&self) -> &ServerStatus {
        &self.status
    }

    pub fn set_status(&mut self, status: ServerStatus) {
        self.status = status;
    }

    pub fn last_activity(&self) -> Instant {
        self.last_activity
    }

    pub fn language(&self) -> Language {
        self.language
    }
}

impl Drop for LspProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_status_is_ready() {
        assert!(ServerStatus::Ready.is_ready());
        assert!(!ServerStatus::Starting.is_ready());
        assert!(!ServerStatus::Busy.is_ready());
        assert!(!ServerStatus::Indexing { progress: 50.0 }.is_ready());
        assert!(!ServerStatus::Crashed { reason: "test".to_string() }.is_ready());
    }

    #[test]
    fn test_server_status_is_terminal() {
        assert!(ServerStatus::Crashed { reason: "test".to_string() }.is_terminal());
        assert!(!ServerStatus::Ready.is_terminal());
        assert!(!ServerStatus::Starting.is_terminal());
        assert!(!ServerStatus::Busy.is_terminal());
        assert!(!ServerStatus::Indexing { progress: 50.0 }.is_terminal());
    }

    #[test]
    fn test_server_status_as_str() {
        assert_eq!(ServerStatus::Starting.as_str(), "starting");
        assert_eq!(ServerStatus::Ready.as_str(), "ready");
        assert_eq!(ServerStatus::Busy.as_str(), "busy");
        assert_eq!(ServerStatus::Indexing { progress: 50.0 }.as_str(), "indexing");
        assert_eq!(
            ServerStatus::Crashed { reason: "error".to_string() }.as_str(),
            "crashed"
        );
    }

    #[test]
    fn test_server_status_display() {
        assert_eq!(format!("{}", ServerStatus::Starting), "starting");
        assert_eq!(format!("{}", ServerStatus::Ready), "ready");
        assert_eq!(format!("{}", ServerStatus::Busy), "busy");
        assert_eq!(format!("{}", ServerStatus::Indexing { progress: 75.0 }), "indexing (75%)");
        assert_eq!(
            format!("{}", ServerStatus::Crashed { reason: "segfault".to_string() }),
            "crashed: segfault"
        );
    }

    #[test]
    fn test_server_status_default() {
        let status = ServerStatus::default();
        assert_eq!(status, ServerStatus::Starting);
        assert!(!status.is_ready());
    }

    #[test]
    fn test_request_timeout_constant() {
        assert_eq!(REQUEST_TIMEOUT_SECS, 30);
    }
}
