//! Registry of debug adapters per language

use std::collections::HashMap;

use which::which;

use crate::adapter::configs::{AdapterConfig, Capability, DownloadConfig, Language};

/// Registry of known debug adapters
pub struct AdapterRegistry {
    configs: HashMap<Language, AdapterConfig>,
}

impl AdapterRegistry {
    /// Create a new registry with default adapters
    pub fn new() -> Self {
        let mut configs = HashMap::new();

        // Rust: codelldb (LLDB-based)
        configs.insert(
            Language::Rust,
            AdapterConfig {
                name: "codelldb".to_string(),
                language: Language::Rust,
                command: "codelldb".to_string(),
                args: vec!["--port".to_string(), "{port}".to_string()],
                min_version: Some("1.10.0".to_string()),
                download_url: Some(DownloadConfig {
                    url: "https://github.com/vadimcn/codelldb/releases/download/v1.10.0/codelldb-x86_64-linux.vsix".to_string(),
                    sha256: None,
                    extract_to: ".local/share/rcode-debug/adapters".to_string(),
                }),
                capabilities: Capability {
                    crash_analysis: true,
                    snapshot_capture: true,
                    step_debugging: true,
                    evaluation: false, // Limited in Rust
                },
            },
        );

        // Python: debugpy
        configs.insert(
            Language::Python,
            AdapterConfig {
                name: "debugpy".to_string(),
                language: Language::Python,
                command: "python3".to_string(),
                args: vec![
                    "-m".to_string(),
                    "debugpy".to_string(),
                    "listen".to_string(),
                    "--port".to_string(),
                    "{port}".to_string(),
                ],
                min_version: Some("1.6.0".to_string()),
                download_url: Some(DownloadConfig {
                    url: "https://files.pythonhosted.org/packages/source/d/debugpy/debugpy-1.8.0.tar.gz".to_string(),
                    sha256: None,
                    extract_to: ".local/share/rcode-debug/adapters".to_string(),
                }),
                capabilities: Capability {
                    crash_analysis: true,
                    snapshot_capture: true,
                    step_debugging: true,
                    evaluation: true,
                },
            },
        );

        // TypeScript/JavaScript: js-debug (built into Node.js)
        configs.insert(
            Language::TypeScript,
            AdapterConfig {
                name: "js-debug".to_string(),
                language: Language::TypeScript,
                command: "node".to_string(),
                args: vec![
                    "--inspect".to_string(),
                    "-p".to_string(),
                    "{port}".to_string(),
                ],
                min_version: Some("18.0.0".to_string()),
                download_url: None, // Built into Node.js
                capabilities: Capability {
                    crash_analysis: true,
                    snapshot_capture: true,
                    step_debugging: true,
                    evaluation: true,
                },
            },
        );

        configs.insert(
            Language::JavaScript,
            AdapterConfig {
                name: "js-debug".to_string(),
                language: Language::JavaScript,
                command: "node".to_string(),
                args: vec![
                    "--inspect".to_string(),
                    "-p".to_string(),
                    "{port}".to_string(),
                ],
                min_version: Some("18.0.0".to_string()),
                download_url: None,
                capabilities: Capability {
                    crash_analysis: true,
                    snapshot_capture: true,
                    step_debugging: true,
                    evaluation: true,
                },
            },
        );

        // Go: delve
        configs.insert(
            Language::Go,
            AdapterConfig {
                name: "dlv".to_string(),
                language: Language::Go,
                command: "dlv".to_string(),
                args: vec!["dap".to_string(), "--listen".to_string(), "127.0.0.1:{port}".to_string()],
                min_version: Some("1.21.0".to_string()),
                download_url: Some(DownloadConfig {
                    url: "https://github.com/go-delve/delve/releases/download/v1.22.0/dlv-1.22.0-linux-amd64.tar.gz".to_string(),
                    sha256: None,
                    extract_to: ".local/share/rcode-debug/adapters".to_string(),
                }),
                capabilities: Capability {
                    crash_analysis: true,
                    snapshot_capture: true,
                    step_debugging: true,
                    evaluation: true,
                },
            },
        );

        // Java: java-debug
        configs.insert(
            Language::Java,
            AdapterConfig {
                name: "java-debug".to_string(),
                language: Language::Java,
                command: "java".to_string(),
                args: vec![
                    "-agentlib:jdwp=transport=dt_socket,server=y,suspend=y,address={port}".to_string(),
                ],
                min_version: Some("11".to_string()),
                download_url: Some(DownloadConfig {
                    url: "https://repo1.maven.org/maven2/com/microsoft/java/debugger/com.microsoft.java.debug.plugin/0.53.0/com.microsoft.java.debug.plugin-0.53.0.jar".to_string(),
                    sha256: None,
                    extract_to: ".local/share/rcode-debug/adapters".to_string(),
                }),
                capabilities: Capability {
                    crash_analysis: true,
                    snapshot_capture: true,
                    step_debugging: true,
                    evaluation: true,
                },
            },
        );

        Self { configs }
    }

    /// Get adapter config for a language
    pub fn get(&self, language: &Language) -> Option<&AdapterConfig> {
        self.configs.get(language)
    }

    /// Get adapter config by language name
    pub fn get_by_name(&self, name: &str) -> Option<&AdapterConfig> {
        let lang = Language::from_str(name);
        self.get(&lang)
    }

    /// Check if an adapter is installed and available
    pub fn is_available(&self, language: &Language) -> bool {
        if let Some(config) = self.get(language) {
            which(&config.command).is_ok()
        } else {
            false
        }
    }

    /// Get all available languages
    pub fn available_languages(&self) -> Vec<Language> {
        self.configs
            .keys()
            .filter(|lang| self.is_available(lang))
            .cloned()
            .collect()
    }

    /// Get all configured languages
    pub fn supported_languages(&self) -> Vec<Language> {
        self.configs.keys().cloned().collect()
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}
