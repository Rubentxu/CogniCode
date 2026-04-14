//! CogniCode Doctor - Multi-section diagnostics for tool availability
//!
//! Provides comprehensive checking of external tooling needed to run CogniCode.

use crate::infrastructure::parser::Language;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Status levels for doctor checks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorStatus {
    Ok,
    Warn,
    Missing,
    Info,
}

impl DoctorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DoctorStatus::Ok => "ok",
            DoctorStatus::Warn => "warning",
            DoctorStatus::Missing => "missing",
            DoctorStatus::Info => "info",
        }
    }

    pub fn from_bool(present: bool) -> Self {
        if present {
            DoctorStatus::Ok
        } else {
            DoctorStatus::Missing
        }
    }
}

/// A single doctor check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub name: String,
    pub status: DoctorStatus,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_hint: Option<String>,
}

impl DoctorCheck {
    pub fn ok(name: &str, detail: &str) -> Self {
        Self {
            name: name.to_string(),
            status: DoctorStatus::Ok,
            detail: detail.to_string(),
            install_hint: None,
        }
    }

    pub fn warn(name: &str, detail: &str, hint: &str) -> Self {
        Self {
            name: name.to_string(),
            status: DoctorStatus::Warn,
            detail: detail.to_string(),
            install_hint: Some(hint.to_string()),
        }
    }

    pub fn missing(name: &str, install_hint: &str) -> Self {
        Self {
            name: name.to_string(),
            status: DoctorStatus::Missing,
            detail: "not found".to_string(),
            install_hint: Some(install_hint.to_string()),
        }
    }

    pub fn info(name: &str, detail: &str) -> Self {
        Self {
            name: name.to_string(),
            status: DoctorStatus::Info,
            detail: detail.to_string(),
            install_hint: None,
        }
    }
}

/// A section of doctor checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorSection {
    pub title: String,
    pub checks: Vec<DoctorCheck>,
    pub status: DoctorStatus,
}

impl DoctorSection {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            checks: Vec::new(),
            status: DoctorStatus::Ok,
        }
    }

    pub fn add_check(&mut self, check: DoctorCheck) {
        // Update section status based on check
        let new_status = match (&self.status, &check.status) {
            // If any check is missing, section is missing
            (_, DoctorStatus::Missing) => DoctorStatus::Missing,
            // If any check is warn, section is warn (unless already missing)
            (DoctorStatus::Ok, DoctorStatus::Warn) => DoctorStatus::Warn,
            (DoctorStatus::Warn, DoctorStatus::Warn) => DoctorStatus::Warn,
            // Info doesn't affect status
            (_, DoctorStatus::Info) => self.status,
            // Keep current status
            _ => self.status,
        };
        self.status = new_status;
        self.checks.push(check);
    }

    pub fn count_found(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status != DoctorStatus::Missing)
            .count()
    }

    pub fn count_total(&self) -> usize {
        self.checks.len()
    }
}

/// Detected workspace languages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub languages: Vec<String>,
    pub path: String,
}

impl WorkspaceInfo {
    pub fn empty() -> Self {
        Self {
            languages: Vec::new(),
            path: String::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.languages.is_empty()
    }
}

/// Full doctor report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub version: String,
    pub sections: DoctorSections,
    pub summary: DoctorSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorSections {
    pub core: DoctorSection,
    pub lsp: DoctorSection,
    pub parsers: DoctorSection,
    pub tools: DoctorSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorSummary {
    pub core: String,
    pub lsp: String,
    pub parsers: String,
    pub tools: String,
}

impl DoctorReport {
    pub fn overall_status(&self) -> DoctorStatus {
        let statuses = [
            &self.summary.core,
            &self.summary.lsp,
            &self.summary.parsers,
            &self.summary.tools,
        ];

        if statuses.iter().any(|s| *s == "missing") {
            DoctorStatus::Missing
        } else if statuses.iter().any(|s| *s == "warning") {
            DoctorStatus::Warn
        } else {
            DoctorStatus::Ok
        }
    }
}

/// Check if a binary exists in PATH and optionally get its version
fn check_binary(name: &str, version_args: &[&str]) -> (bool, Option<String>, Option<PathBuf>) {
    // Use `which` to find the binary
    let which_output = Command::new("which").arg(name).output().ok();

    let path = which_output.filter(|o| o.status.success()).and_then(|o| {
        let path_str = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if path_str.is_empty() {
            None
        } else {
            Some(PathBuf::from(path_str))
        }
    });

    let found = path.is_some();

    let version = if found {
        // Try to get version
        let version_output = Command::new(name)
            .args(version_args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                let version_str = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if version_str.is_empty() {
                    None
                } else {
                    Some(version_str)
                }
            });
        version_output
    } else {
        None
    };

    (found, version, path)
}

/// Check the core runtime section
fn check_core_section() -> DoctorSection {
    let mut section = DoctorSection::new("Core Runtime");

    // Check cognicode-mcp binary - just check if it exists since --version starts the server
    let (found, _version, _path) = check_binary("cognicode-mcp", &[]);

    if found {
        // Just show version from Cargo.toml since running the binary starts the server
        section.add_check(DoctorCheck::ok(
            "cognicode-mcp binary",
            env!("CARGO_PKG_VERSION"),
        ));
    } else {
        section.add_check(DoctorCheck::missing(
            "cognicode-mcp binary",
            "cargo install cognicode",
        ));
    }

    // Check OTLP telemetry (optional)
    let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();
    if otlp_endpoint.is_some() {
        section.add_check(DoctorCheck::ok("OTLP telemetry", "configured"));
    } else {
        section.add_check(DoctorCheck::warn(
            "OTLP telemetry",
            "not configured",
            "set OTEL_EXPORTER_OTLP_ENDPOINT to enable telemetry",
        ));
    }

    section
}

/// Check the LSP servers section
fn check_lsp_section(_detected_languages: &[Language]) -> DoctorSection {
    let mut section = DoctorSection::new("Language Servers (LSP)");

    for lang in Language::all_languages() {
        let binary = lang.lsp_server_binary();

        // Go and Java don't have LSP servers yet
        if binary.is_empty() {
            section.add_check(DoctorCheck::info(lang.name(), "not yet supported"));
            continue;
        }

        let install_cmd = lang.lsp_install_command();
        let (found, version, _path) = check_binary(binary, &["--version"]);

        if found {
            let version_str = version.unwrap_or_else(|| "found".to_string());
            section.add_check(DoctorCheck::ok(lang.lsp_server_name(), &version_str));
        } else {
            section.add_check(DoctorCheck::missing(lang.lsp_server_name(), install_cmd));
        }
    }

    section
}

/// Check the built-in parsers section (always green since tree-sitter grammars are bundled)
fn check_parsers_section() -> DoctorSection {
    let mut section = DoctorSection::new("Built-in Parsers (tree-sitter)");

    for lang in Language::all_languages() {
        // All tree-sitter parsers are bundled, so they're always available
        section.add_check(DoctorCheck::ok(lang.name(), "bundled"));
    }

    section
}

/// Sandbox/validation tool definition
struct ToolDef {
    name: &'static str,
    install_hint: &'static str,
    languages: &'static [Language], // Empty means universal
}

/// Check the sandbox/validation tools section
fn check_tools_section(detected_languages: &[Language]) -> DoctorSection {
    let mut section = DoctorSection::new("Sandbox / Validation Tools");

    // Universal tools
    let universal_tools = vec![ToolDef {
        name: "git",
        install_hint: "apt install git / brew install git",
        languages: &[],
    }];

    // Language-specific tools
    let language_tools = vec![
        // Rust
        ToolDef {
            name: "cargo",
            install_hint: "rustup install stable",
            languages: &[Language::Rust],
        },
        ToolDef {
            name: "rustfmt",
            install_hint: "rustup component add rustfmt",
            languages: &[Language::Rust],
        },
        // Python
        ToolDef {
            name: "python3",
            install_hint: "apt install python3 / brew install python3",
            languages: &[Language::Python],
        },
        ToolDef {
            name: "pytest",
            install_hint: "pip install pytest",
            languages: &[Language::Python],
        },
        // JavaScript/TypeScript
        ToolDef {
            name: "node",
            install_hint: "apt install nodejs / brew install node",
            languages: &[Language::JavaScript, Language::TypeScript],
        },
        ToolDef {
            name: "npm",
            install_hint: "apt install npm / brew install npm",
            languages: &[Language::JavaScript, Language::TypeScript],
        },
        ToolDef {
            name: "jest",
            install_hint: "npm install -g jest",
            languages: &[Language::JavaScript, Language::TypeScript],
        },
        // Go
        ToolDef {
            name: "go",
            install_hint: "apt install golang / brew install go",
            languages: &[Language::Go],
        },
        // Java
        ToolDef {
            name: "javac",
            install_hint: "apt install default-jdk",
            languages: &[Language::Java],
        },
    ];

    // Check universal tools first
    for tool in universal_tools {
        let (found, version, _path) = check_binary(tool.name, &["--version"]);
        if found {
            let version_str = version.unwrap_or_else(|| "found".to_string());
            section.add_check(DoctorCheck::ok(tool.name, &version_str));
        } else {
            section.add_check(DoctorCheck::missing(tool.name, tool.install_hint));
        }
    }

    // Check language-specific tools
    let detected_set: HashSet<Language> = detected_languages.iter().cloned().collect();

    for tool in language_tools {
        // Skip if tool is not relevant to detected languages
        if !tool.languages.is_empty() && !tool.languages.iter().any(|l| detected_set.contains(l)) {
            continue;
        }

        let (found, version, _path) = check_binary(tool.name, &["--version"]);
        if found {
            let version_str = version.unwrap_or_else(|| "found".to_string());
            section.add_check(DoctorCheck::ok(tool.name, &version_str));
        } else {
            section.add_check(DoctorCheck::missing(tool.name, tool.install_hint));
        }
    }

    section
}

/// Detect languages in a workspace by scanning for markers
fn detect_workspace_languages(workspace_path: &Path) -> Vec<Language> {
    let mut languages = Vec::new();
    let mut seen = HashSet::new();

    // Check for Cargo.toml -> Rust
    if workspace_path.join("Cargo.toml").exists() {
        if seen.insert(Language::Rust) {
            languages.push(Language::Rust);
        }
    }

    // Check for go.mod -> Go
    if workspace_path.join("go.mod").exists() {
        if seen.insert(Language::Go) {
            languages.push(Language::Go);
        }
    }

    // Check for package.json -> JavaScript/TypeScript
    if workspace_path.join("package.json").exists() {
        if seen.insert(Language::JavaScript) {
            languages.push(Language::JavaScript);
        }
        if seen.insert(Language::TypeScript) {
            languages.push(Language::TypeScript);
        }
    }

    // Check for pom.xml or *.java -> Java
    if workspace_path.join("pom.xml").exists()
        || glob::glob(&workspace_path.join("**/*.java").to_string_lossy())
            .ok()
            .map(|g| g.count())
            .unwrap_or(0)
            > 0
    {
        if seen.insert(Language::Java) {
            languages.push(Language::Java);
        }
    }

    // Check for *.py files -> Python
    if glob::glob(&workspace_path.join("**/*.py").to_string_lossy())
        .ok()
        .map(|g| g.count())
        .unwrap_or(0)
        > 0
    {
        if seen.insert(Language::Python) {
            languages.push(Language::Python);
        }
    }

    languages
}

/// Run all doctor checks and generate a report
pub fn run_doctor_checks(workspace_path: Option<&Path>) -> DoctorReport {
    let version = env!("CARGO_PKG_VERSION").to_string();

    // Detect workspace languages if path provided
    let detected_languages = workspace_path
        .map(|p| detect_workspace_languages(p))
        .unwrap_or_default();

    let workspace_info = workspace_path.map(|p| WorkspaceInfo {
        languages: detected_languages
            .iter()
            .map(|l| l.name().to_string())
            .collect(),
        path: p.to_string_lossy().to_string(),
    });

    let core = check_core_section();
    let lsp = check_lsp_section(&detected_languages);
    let parsers = check_parsers_section();
    let tools = check_tools_section(&detected_languages);

    let summary = DoctorSummary {
        core: core.status.as_str().to_string(),
        lsp: lsp.status.as_str().to_string(),
        parsers: parsers.status.as_str().to_string(),
        tools: tools.status.as_str().to_string(),
    };

    DoctorReport {
        version,
        sections: DoctorSections {
            core,
            lsp,
            parsers,
            tools,
        },
        summary,
        workspace: workspace_info,
    }
}

/// Format status icon for text output
fn status_icon(status: DoctorStatus) -> &'static str {
    match status {
        DoctorStatus::Ok => "✅",
        DoctorStatus::Warn => "⚠️ ",
        DoctorStatus::Missing => "❌",
        DoctorStatus::Info => "ℹ️ ",
    }
}

/// Format a doctor section for text output
fn format_section_text(section: &DoctorSection) -> String {
    let mut output = String::new();

    let status_marker = status_icon(section.status);
    output.push_str(&format!("\n{} {}\n", status_marker, section.title));

    for check in &section.checks {
        let icon = status_icon(check.status);
        output.push_str(&format!("  {} {}", icon, check.name));

        if !check.detail.is_empty() && check.detail != "not found" {
            output.push_str(&format!("  {}", check.detail));
        }

        output.push('\n');

        if let Some(ref hint) = check.install_hint {
            if check.status == DoctorStatus::Missing {
                output.push_str(&format!("      (install: {})\n", hint));
            } else if check.status == DoctorStatus::Warn {
                output.push_str(&format!("      ({})\n", hint));
            }
        }
    }

    output
}

/// Format the full doctor report as text
pub fn format_doctor_text(report: &DoctorReport) -> String {
    let mut output = String::new();

    output.push_str(&format!("CogniCode Doctor v{}", report.version));
    output.push_str("\n========================\n");

    // Core section
    output.push_str(&format_section_text(&report.sections.core));

    // LSP section
    output.push_str(&format_section_text(&report.sections.lsp));

    // Parsers section
    output.push_str(&format_section_text(&report.sections.parsers));

    // Tools section
    output.push_str(&format_section_text(&report.sections.tools));

    // Workspace info
    if let Some(ref ws) = report.workspace {
        if !ws.languages.is_empty() {
            output.push_str("\nℹ️  Workspace languages detected: ");
            output.push_str(&ws.languages.join(", "));
            output.push('\n');
        }
    }

    // Summary
    output.push_str("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    output.push_str(&format!(
        "  Core:   {} {}\n",
        status_icon(report.sections.core.status),
        report.summary.core
    ));
    output.push_str(&format!(
        "  LSP:    {} {} ({}/{})\n",
        status_icon(report.sections.lsp.status),
        report.summary.lsp,
        report.sections.lsp.count_found(),
        report.sections.lsp.count_total()
    ));
    output.push_str(&format!(
        "  Parse:  {} {}\n",
        status_icon(report.sections.parsers.status),
        report.summary.parsers
    ));
    output.push_str(&format!(
        "  Tools:  {} {} ({}/{})\n",
        status_icon(report.sections.tools.status),
        report.summary.tools,
        report.sections.tools.count_found(),
        report.sections.tools.count_total()
    ));
    output.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    output
}

/// Format the full doctor report as JSON
pub fn format_doctor_json(report: &DoctorReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doctor_status_as_str() {
        assert_eq!(DoctorStatus::Ok.as_str(), "ok");
        assert_eq!(DoctorStatus::Warn.as_str(), "warning");
        assert_eq!(DoctorStatus::Missing.as_str(), "missing");
        assert_eq!(DoctorStatus::Info.as_str(), "info");
    }

    #[test]
    fn test_doctor_check_ok() {
        let check = DoctorCheck::ok("test", "v1.0");
        assert_eq!(check.name, "test");
        assert_eq!(check.status, DoctorStatus::Ok);
        assert_eq!(check.detail, "v1.0");
        assert!(check.install_hint.is_none());
    }

    #[test]
    fn test_doctor_check_missing() {
        let check = DoctorCheck::missing("test", "install test");
        assert_eq!(check.name, "test");
        assert_eq!(check.status, DoctorStatus::Missing);
        assert_eq!(check.detail, "not found");
        assert_eq!(check.install_hint, Some("install test".to_string()));
    }

    #[test]
    fn test_doctor_section_status() {
        let mut section = DoctorSection::new("Test");

        section.add_check(DoctorCheck::ok("tool1", "v1"));
        assert_eq!(section.status, DoctorStatus::Ok);

        section.add_check(DoctorCheck::warn("tool2", "not configured", "configure it"));
        assert_eq!(section.status, DoctorStatus::Warn);

        section.add_check(DoctorCheck::missing("tool3", "install it"));
        assert_eq!(section.status, DoctorStatus::Missing);
    }

    #[test]
    fn test_language_all_languages() {
        let langs = Language::all_languages();
        assert!(langs.contains(&Language::Rust));
        assert!(langs.contains(&Language::Python));
        assert!(langs.contains(&Language::JavaScript));
        assert!(langs.contains(&Language::TypeScript));
        assert!(langs.contains(&Language::Go));
        assert!(langs.contains(&Language::Java));
    }
}
