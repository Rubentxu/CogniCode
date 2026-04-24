//! Adapter configurations per language

use serde::{Deserialize, Serialize};

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    Go,
    Java,
    #[serde(other)]
    Unknown,
}

impl Language {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "rust" | "rs" => Language::Rust,
            "python" | "py" => Language::Python,
            "typescript" | "ts" => Language::TypeScript,
            "javascript" | "js" | "jsx" | "tsx" => Language::JavaScript,
            "go" | "golang" => Language::Go,
            "java" | "kotlin" => Language::Java,
            _ => Language::Unknown,
        }
    }
}

/// Debug capabilities for a language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// Crash analysis - launch and wait for crash
    pub crash_analysis: bool,
    /// Snapshot capture - breakpoints that capture state without stopping
    pub snapshot_capture: bool,
    /// Step debugging - step over, into, out
    pub step_debugging: bool,
    /// Expression evaluation
    pub evaluation: bool,
}

impl Default for Capability {
    fn default() -> Self {
        Self {
            crash_analysis: true,
            snapshot_capture: true,
            step_debugging: true,
            evaluation: true,
        }
    }
}

/// Configuration for a debug adapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterConfig {
    /// Adapter name (e.g., "codelldb", "debugpy")
    pub name: String,

    /// Language this adapter supports
    pub language: Language,

    /// Default command/args to launch the adapter
    pub command: String,
    pub args: Vec<String>,

    /// Minimum required version
    pub min_version: Option<String>,

    /// Download URL (if auto-installable)
    pub download_url: Option<DownloadConfig>,

    /// Capabilities of this adapter
    pub capabilities: Capability,
}

/// Download configuration for auto-install
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub url: String,
    pub sha256: Option<String>,
    pub extract_to: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_str() {
        assert_eq!(Language::from_str("rust"), Language::Rust);
        assert_eq!(Language::from_str("python"), Language::Python);
        assert_eq!(Language::from_str("ts"), Language::TypeScript);
        assert_eq!(Language::from_str("unknown_lang"), Language::Unknown);
    }
}
