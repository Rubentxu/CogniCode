//! Debug Orchestrator - coordinates autonomous debugging

use std::path::PathBuf;
use std::time::Instant;

use crate::adapter::configs::Language;
use crate::client::{DapClient, LaunchConfig};
use crate::doctor::Doctor;
use crate::error::{DebugError, Result};
use crate::analysis::{AnalysisEngine, RootCause, Recommendation};

/// Main orchestrator for autonomous debugging
pub struct DebugOrchestrator {
    doctor: Doctor,
    analysis: AnalysisEngine,
}

impl DebugOrchestrator {
    /// Create a new orchestrator
    pub fn new() -> Self {
        Self {
            doctor: Doctor::new(),
            analysis: AnalysisEngine::new(),
        }
    }

    /// Analyze a crash and return root cause + recommendation
    pub async fn analyze(&self, request: DebugAnalyzeRequest) -> Result<DebugAnalysisResult> {
        let start = Instant::now();
        let language = Language::from_str(&request.target.language);

        // 1. Check capabilities
        let caps = self.doctor.check_language(&language).await;

        if !caps.toolchain_ok {
            return Err(DebugError::ToolchainNotFound(
                request.target.language.clone()
            ));
        }

        // 2. Find or install adapter
        let adapter_path = self.find_adapter(&language).await?;

        // 3. Connect to adapter
        let mut client = DapClient::connect(&adapter_path).await?;

        // 4. Initialize
        let _caps = client.initialize().await?;

        // 5. Build launch config
        let launch_config = LaunchConfig {
            program: request.target.program.clone(),
            args: request.target.args.clone().unwrap_or_default(),
            env: request.target.env.clone().unwrap_or_default(),
            cwd: request.target.cwd.clone(),
            no_debug: false,
        };

        // 6. Launch
        client.launch(&launch_config).await?;

        // 7. Configuration done
        client.configuration_done().await?;

        // 8. Run until crash
        let stopped = match client.continue_(None).await {
            Ok(e) => e,
            Err(e) => {
                client.disconnect().await.ok();
                return Err(e);
            }
        };

        // 9. Capture state
        let stack = client.stack_trace(None, None).await?;
        let vars = client.variables(0).await?;

        // 10. Cleanup
        client.disconnect().await?;

        // 11. Generate analysis
        let root_cause = self.analysis.analyze_crash(&stopped, &stack, &vars);
        let recommendation = self.analysis.suggest_fix(&root_cause);

        let elapsed = start.elapsed();

        Ok(DebugAnalysisResult {
            success: true,
            root_cause,
            recommendation,
            stack_trace: stack,
            variables: vars,
            capabilities_used: CapabilitiesUsed {
                mode: "crash_analysis".to_string(),
                adapter: caps.adapter_version.unwrap_or_default(),
                execution_time_ms: elapsed.as_millis() as u64,
            },
        })
    }

    /// Find adapter path for a language
    async fn find_adapter(&self, language: &Language) -> Result<PathBuf> {
        // First check if it's in PATH
        let config = self.doctor.check_language(language).await;

        if let Some(_version) = config.adapter_version {
            // Adapter is available, find its path
            // For now, assume it's in PATH
            let cmd = match language {
                Language::Rust => "codelldb",
                Language::Python => "debugpy",
                Language::TypeScript | Language::JavaScript => "node",
                Language::Go => "dlv",
                Language::Java => "java",
                _ => return Err(DebugError::UnsupportedLanguage(format!("{:?}", language))),
            };
            Ok(PathBuf::from(cmd))
        } else {
            Err(DebugError::AdapterNotFound {
                language: format!("{:?}", language),
                instructions: config.install_instructions.unwrap_or_default(),
            })
        }
    }
}

impl Default for DebugOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Request to analyze a crash
#[derive(Debug, Clone)]
pub struct DebugAnalyzeRequest {
    pub target: Target,
    pub error: ErrorInfo,
    pub context: Option<AnalysisContext>,
}

/// Target to debug
#[derive(Debug, Clone)]
pub struct Target {
    pub language: String,
    pub name: String,
    pub program: String,
    pub args: Option<Vec<String>>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub cwd: Option<String>,
}

/// Error information
#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub kind: ErrorKind,
    pub message: String,
    pub output: Option<String>,
}

/// Error kind
#[derive(Debug, Clone)]
pub enum ErrorKind {
    Panic,
    Assertion,
    ExitCode,
    Timeout,
}

impl ErrorKind {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "panic" | "panicked" => ErrorKind::Panic,
            "assertion" | "assert" | "failed" => ErrorKind::Assertion,
            "exit_code" | "exit" => ErrorKind::ExitCode,
            "timeout" | "timed_out" => ErrorKind::Timeout,
            _ => ErrorKind::ExitCode,
        }
    }
}

/// Analysis context (optional hints)
#[derive(Debug, Clone)]
pub struct AnalysisContext {
    pub suspect_functions: Option<Vec<String>>,
}

/// Result of analysis
#[derive(Debug, Clone)]
pub struct DebugAnalysisResult {
    pub success: bool,
    pub root_cause: RootCause,
    pub recommendation: Recommendation,
    pub stack_trace: Vec<crate::client::StackFrame>,
    pub variables: Vec<crate::client::Variable>,
    pub capabilities_used: CapabilitiesUsed,
}

impl DebugAnalysisResult {
    /// Convert stack frames to a simple representation for storage/transmission
    pub fn stack_trace_summary(&self) -> Vec<StackTraceEntry> {
        self.stack_trace.iter().map(|sf| StackTraceEntry {
            function: sf.name.clone(),
            file: sf.source.as_ref().and_then(|s| s.path.clone()).unwrap_or_default(),
            line: sf.line,
            column: sf.column,
        }).collect()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StackTraceEntry {
    pub function: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// Capabilities used during analysis
#[derive(Debug, Clone)]
pub struct CapabilitiesUsed {
    pub mode: String,
    pub adapter: String,
    pub execution_time_ms: u64,
}
