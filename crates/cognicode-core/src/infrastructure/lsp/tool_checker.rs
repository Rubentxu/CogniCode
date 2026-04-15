use crate::infrastructure::parser::Language;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ToolStatus {
    pub available: bool,
    pub binary_path: Option<PathBuf>,
    pub version: Option<String>,
    pub install_command: &'static str,
}

impl std::fmt::Display for ToolStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.available {
            "Available"
        } else {
            "Missing"
        };
        write!(f, "{}", status)
    }
}

pub struct ToolAvailabilityChecker;

impl ToolAvailabilityChecker {
    pub fn check(language: Language) -> ToolStatus {
        let binary = language.lsp_server_binary();
        let install_command = language.lsp_install_command();

        match Self::which(binary) {
            Some(path) => {
                let version = Self::get_version(binary);
                ToolStatus {
                    available: true,
                    binary_path: Some(path),
                    version,
                    install_command,
                }
            }
            None => ToolStatus {
                available: false,
                binary_path: None,
                version: None,
                install_command,
            },
        }
    }

    pub fn doctor_report() -> Vec<(Language, ToolStatus)> {
        vec![
            (Language::Rust, Self::check(Language::Rust)),
            (Language::Python, Self::check(Language::Python)),
            (Language::TypeScript, Self::check(Language::TypeScript)),
            (Language::JavaScript, Self::check(Language::JavaScript)),
            (Language::Go, Self::check(Language::Go)),
            (Language::Java, Self::check(Language::Java)),
        ]
    }

    fn which(binary: &str) -> Option<PathBuf> {
        let output = Command::new("which").arg(binary).output().ok()?;
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
        None
    }

    fn get_version(binary: &str) -> Option<String> {
        let output = Command::new(binary).arg("--version").output().ok()?;
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !version.is_empty() {
                return Some(version);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_sh() {
        let status = ToolAvailabilityChecker::check(Language::Rust);
        assert_eq!(status.install_command, "rustup component add rust-analyzer");
        assert_eq!(status.install_command, Language::Rust.lsp_install_command());
    }

    #[test]
    fn test_doctor_report_structure() {
        let report = ToolAvailabilityChecker::doctor_report();
        assert!(!report.is_empty());
        for (lang, status) in &report {
            assert_eq!(status.install_command, lang.lsp_install_command());
        }
    }

    #[test]
    fn test_check_nonexistent_binary() {
        let status = ToolAvailabilityChecker::check(Language::Rust);
        if !status.available {
            assert!(status.binary_path.is_none());
            assert!(status.version.is_none());
        }
    }

    #[test]
    fn test_tool_status_display() {
        let available = ToolStatus {
            available: true,
            binary_path: Some(PathBuf::from("/usr/bin/rust-analyzer")),
            version: Some("1.0".to_string()),
            install_command: "rustup component add rust-analyzer",
        };
        assert_eq!(format!("{}", available), "Available");

        let missing = ToolStatus {
            available: false,
            binary_path: None,
            version: None,
            install_command: "npm install -g pyright",
        };
        assert_eq!(format!("{}", missing), "Missing");
    }

    #[test]
    fn test_language_lsp_methods() {
        assert_eq!(Language::Rust.lsp_server_binary(), "rust-analyzer");
        assert_eq!(Language::Python.lsp_server_binary(), "pyright-langserver");
        assert_eq!(
            Language::TypeScript.lsp_server_binary(),
            "typescript-language-server"
        );
        assert_eq!(
            Language::JavaScript.lsp_server_binary(),
            "typescript-language-server"
        );
    }

    // Task 4.1: Language enum LSP method unit tests

    #[test]
    fn test_language_lsp_server_binary() {
        assert_eq!(Language::Rust.lsp_server_binary(), "rust-analyzer");
        assert_eq!(Language::Python.lsp_server_binary(), "pyright-langserver");
        assert_eq!(
            Language::TypeScript.lsp_server_binary(),
            "typescript-language-server"
        );
        assert_eq!(
            Language::JavaScript.lsp_server_binary(),
            "typescript-language-server"
        );
    }

    #[test]
    fn test_language_lsp_install_command() {
        assert_eq!(
            Language::Rust.lsp_install_command(),
            "rustup component add rust-analyzer"
        );
        assert!(Language::Python.lsp_install_command().contains("npm"));
        assert!(Language::TypeScript.lsp_install_command().contains("npm"));
        assert!(Language::JavaScript.lsp_install_command().contains("npm"));
    }

    #[test]
    fn test_language_lsp_args() {
        assert!(Language::Rust.lsp_args().is_empty());
        assert!(Language::Python.lsp_args().contains(&"--stdio"));
        assert!(Language::TypeScript.lsp_args().contains(&"--stdio"));
        assert!(Language::JavaScript.lsp_args().contains(&"--stdio"));
    }

    #[test]
    fn test_language_lsp_server_name() {
        assert_eq!(Language::Rust.lsp_server_name(), "rust-analyzer");
        assert_eq!(Language::Python.lsp_server_name(), "pyright");
        assert_eq!(
            Language::TypeScript.lsp_server_name(),
            "typescript-language-server"
        );
        assert_eq!(
            Language::JavaScript.lsp_server_name(),
            "typescript-language-server"
        );
    }
}
