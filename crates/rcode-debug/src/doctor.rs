//! Doctor - health checks for debugging capabilities


use crate::adapter::configs::{AdapterConfig, Capability, Language};
use crate::adapter::installer::AdapterInstaller;
use crate::adapter::registry::AdapterRegistry;
use serde::{Deserialize, Serialize};

/// Doctor report - health status of all debugging capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub timestamp: String,
    pub debug_enabled: bool,
    pub languages: Vec<LanguageDebugStatus>,
}

impl DoctorReport {
    /// Convert to XML format for agent consumption
    pub fn to_xml(&self) -> String {
        let mut xml = String::from("<debug-health>\n");
        xml.push_str(&format!("  <status enabled=\"{}\" />\n", self.debug_enabled));

        for lang in &self.languages {
            xml.push_str(&format!(
                "  <{} available=\"{}\"{}>\n",
                lang.language,
                lang.available,
                lang.toolchain_version
                    .as_ref()
                    .map(|v| format!(" toolchain=\"{}\"", v))
                    .unwrap_or_default()
            ));
            xml.push_str(&format!(
                "    <capabilities crash_analysis=\"{}\" snapshot_capture=\"{}\" step_debugging=\"{}\" evaluation=\"{}\" />\n",
                lang.capabilities.crash_analysis,
                lang.capabilities.snapshot_capture,
                lang.capabilities.step_debugging,
                lang.capabilities.evaluation,
            ));
            if let Some(ref instructions) = lang.install_instructions {
                xml.push_str(&format!("    <install>{}</install>\n", instructions));
            }
            if let Some(ref note) = lang.note {
                xml.push_str(&format!("    <note>{}</note>\n", note));
            }
            xml.push_str(&format!("  </{}>\n", lang.language));
        }

        xml.push_str("</debug-health>");
        xml
    }
}

/// Debug status for a single language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageDebugStatus {
    pub language: String,
    pub available: bool,
    pub toolchain_ok: bool,
    pub toolchain_version: Option<String>,
    pub adapter_installed: bool,
    pub adapter_version: Option<String>,
    pub capabilities: Capability,
    pub install_instructions: Option<String>,
    pub note: Option<String>,
}

/// Doctor - checks debugging health
pub struct Doctor {
    registry: AdapterRegistry,
    installer: AdapterInstaller,
}

impl Doctor {
    /// Create a new doctor
    pub fn new() -> Self {
        Self {
            registry: AdapterRegistry::new(),
            installer: AdapterInstaller::new(),
        }
    }

    /// Perform a full health check
    pub async fn check(&self) -> crate::error::Result<DoctorReport> {
        let mut languages = Vec::new();

        for lang in self.registry.supported_languages() {
            let status = self.check_language(&lang).await;
            languages.push(status);
        }

        Ok(DoctorReport {
            timestamp: format!("{:?}", time::OffsetDateTime::now_utc()),
            debug_enabled: true,
            languages,
        })
    }

    /// Check health for a specific language
    pub async fn check_language(&self, language: &Language) -> LanguageDebugStatus {
        let lang_name = match language {
            Language::Rust => "rust",
            Language::Python => "python",
            Language::TypeScript => "typescript",
            Language::JavaScript => "javascript",
            Language::Go => "go",
            Language::Java => "java",
            Language::Unknown => "unknown",
        };

        let config = self.registry.get(language);

        // Check toolchain
        let (toolchain_ok, toolchain_version) = self.check_toolchain(language).await;

        // Check adapter
        let (adapter_installed, adapter_version, install_instructions) =
            self.check_adapter(language, config).await;

        // Determine capabilities based on what's available
        let capabilities = if let Some(cfg) = config {
            if adapter_installed {
                cfg.capabilities.clone()
            } else {
                // No adapter = limited capabilities
                Capability {
                    crash_analysis: true, // Can still do crash analysis with basic tools
                    snapshot_capture: false,
                    step_debugging: false,
                    evaluation: false,
                }
            }
        } else {
            Capability::default()
        };

        let available = toolchain_ok && adapter_installed;
        let note = if !toolchain_ok {
            Some(format!("{} not found in PATH", self.toolchain_command(language)))
        } else if !adapter_installed {
            Some(format!("{} not installed", config.map(|c| c.name.as_str()).unwrap_or("adapter")))
        } else {
            None
        };

        LanguageDebugStatus {
            language: lang_name.to_string(),
            available,
            toolchain_ok,
            toolchain_version,
            adapter_installed,
            adapter_version,
            capabilities,
            install_instructions,
            note,
        }
    }

    /// Check if toolchain is available
    async fn check_toolchain(&self, language: &Language) -> (bool, Option<String>) {
        let cmd = self.toolchain_command(language);
        match tokio::process::Command::new(&cmd).arg("--version").output().await {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                (true, Some(version))
            }
            _ => (false, None),
        }
    }

    /// Check if adapter is installed
    async fn check_adapter(
        &self,
        language: &Language,
        config: Option<&AdapterConfig>,
    ) -> (bool, Option<String>, Option<String>) {
        if let Some(cfg) = config {
            // Check if it's in PATH
            if tokio::process::Command::new(&cfg.command).arg("--help").output().await.is_ok() {
                return (true, Some(cfg.name.clone()), None);
            }

            // Check if it's in our install dir
            let install_path = self.installer.install_dir().join(&cfg.command);
            if install_path.exists() {
                return (true, Some(cfg.name.clone()), None);
            }

            // Not installed
            let install_instructions = if cfg.download_url.is_some() {
                Some(format!("Install with: cargo install {} or download from release page", cfg.name))
            } else {
                let lang_name = match language {
                    Language::Rust => "Rust",
                    Language::Python => "Python",
                    Language::TypeScript => "TypeScript",
                    Language::JavaScript => "JavaScript",
                    Language::Go => "Go",
                    Language::Java => "Java",
                    Language::Unknown => "Unknown",
                };
                Some(format!("Install the {} package for {}", cfg.name, lang_name))
            };
            (false, None, install_instructions)
        } else {
            (false, None, Some(format!("No adapter configuration for {:?}", language)))
        }
    }

    /// Get the toolchain command for a language
    fn toolchain_command(&self, language: &Language) -> &str {
        match language {
            Language::Rust => "cargo",
            Language::Python => "python3",
            Language::TypeScript | Language::JavaScript => "node",
            Language::Go => "go",
            Language::Java => "java",
            Language::Unknown => "echo",
        }
    }

    /// Check if debugging is available for a language
    pub async fn is_available(&self, language: &Language) -> bool {
        let status = self.check_language(language).await;
        status.available
    }

    /// Get install instructions for a language
    pub async fn get_install_instructions(&self, language: &Language) -> Option<String> {
        let status = self.check_language(language).await;
        status.install_instructions
    }
}

impl Default for Doctor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_doctor_check() {
        let doctor = Doctor::new();
        let report = doctor.check().await.unwrap();

        assert!(report.debug_enabled);
        assert!(!report.languages.is_empty());
    }

    #[tokio::test]
    async fn test_rust_status() {
        let doctor = Doctor::new();
        let status = doctor.check_language(&Language::Rust).await;

        assert_eq!(status.language, "rust");
        // May or may not be available depending on environment
    }
}
