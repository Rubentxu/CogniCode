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
    /// Schema version of this JSON document shape. Format
    /// `cognicode.doctor/vMAJOR`. Independent from the runtime
    /// `version` field (which carries the bin semver) so that schema
    /// and runtime can evolve separately. Bumping this major signals
    /// a breaking change for downstream consumers — add fields with
    /// `#[serde(default)]` instead.
    ///
    /// PRF-CLI-07: contract for machine-readable, schema-versioned
    /// output.
    pub schema_version: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorSummary {
    pub core: String,
    pub lsp: String,
    pub parsers: String,
}

impl DoctorReport {
    pub fn overall_status(&self) -> DoctorStatus {
        let statuses = [&self.summary.core, &self.summary.lsp, &self.summary.parsers];

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
        Command::new(name)
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
            })
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
///
/// MEDIDO 2026-10-04 sobre el binario de v0.101.9 publicado. Esto iteraba por
/// **lenguaje**, y varios lenguajes comparten binario a proposito
/// (`language.rs:328` y `:331`: `TypeScript | JavaScript` a
/// `typescript-language-server`, `C | Cpp` a `clangd`). El reparto es
/// correcto; informar de lo mismo dos veces no lo es, y el doctor mostraba:
///
/// ```
///   ❌ typescript-language-server
///   ❌ typescript-language-server
///   ✅ clangd  Homebrew clangd version 23.1.0
///   Features: linux
///   Platform: x86_64-unknown-linux-gnu
///   ✅ clangd  Homebrew clangd version 23.1.0
/// ```
///
/// Dos arreglos, y el segundo es el que no se ve mirando el codigo:
///
/// 1. **Un binario, una linea.** Se recorre por binario unico, no por
///    lenguaje. El nombre de la fila es el del SERVIDOR (`lsp_server_name`),
///    que es lo que el usuario tiene que instalar; el lenguaje ya aparece en
///    la fila de "not yet supported" de cada uno que no tiene servidor.
///    `lsp_server_name` y `lsp_install_command` coinciden para un mismo
///    binario —el primer lenguaje elige el mismo par para los que comparten—,
///    asi que la fila no pierde informacion al deduplicar.
///
/// 2. **Una version, una linea.** `clangd --version` responde en tres lineas
///    y se imprimian las tres, con `Features:` y `Platform:` sin sangrar, de
///    modo que se leian como si fueran datos del doctor. Se toma la primera
///    linea no vacia, que es la que trae la version; el resto es la salida
///    del binario, no del doctor.
///
/// Lo que NO cambia: los lenguajes sin servidor siguen saliendo uno a uno
/// como `not yet supported`. Eso es informacion distinta por lenguaje —cada
/// uno es un lenguaje que el producto reconoce y no tiene servidor— y
/// deduplicarlo dejaria a veinte filas iguales.
fn check_lsp_section(_detected_languages: &[Language]) -> DoctorSection {
    let mut section = DoctorSection::new("Language Servers (LSP)");

    // Los servidores van PRIMERO, y los lenguajes sin servidor despues. El
    // orden es el de antes: lo que el usuario puede usar ahora esta arriba, y
    // lo que el producto reconoce pero no tiene servidor es informacion de
    // fondo, veinte filas identicas. Invertirlo hacia los "not yet supported"
    // hacia lo que el lector busca es una mejora de forma que es una
    // regresion de uso.
    //
    // Un binario unico, un `check_binary`. Se recorre `all_languages` en orden
    // para que cada fila salga en la posicion que le corresponde, y el primer
    // lenguaje que declara un binario aporta su nombre y su comando de
    // instalacion —que para los que comparten binario son los mismos.
    let mut seen: Vec<&str> = Vec::new();
    for lang in Language::all_languages() {
        let binary = lang.lsp_server_binary();
        if binary.is_empty() || seen.contains(&binary) {
            continue;
        }
        seen.push(binary);

        let install_cmd = lang.lsp_install_command();
        let (found, version, _path) = check_binary(binary, &["--version"]);

        if found {
            section.add_check(DoctorCheck::ok(
                lang.lsp_server_name(),
                &first_line(&version),
            ));
        } else {
            section.add_check(DoctorCheck::missing(lang.lsp_server_name(), install_cmd));
        }
    }

    // Y despues, los lenguajes que el producto reconoce y para los que no hay
    // servidor. Una fila por lenguaje, porque la fila dice que lenguaje
    // reconoce el producto: deduplicarlos por binario vacio dejaria veinte
    // filas identicas y el usuario no veria que el producto los soporta.
    for lang in Language::all_languages() {
        if lang.lsp_server_binary().is_empty() {
            section.add_check(DoctorCheck::info(lang.name(), "not yet supported"));
        }
    }

    section
}

/// La primera linea no vacia de la salida de `--version`.
///
/// MEDIDO 2026-10-04: `clangd --version` responde
/// `Homebrew clangd version 23.1.0 / Features: linux / Platform: x86_64...`.
/// Imprimir las tres en una fila de tabla mete lineas que el usuario lee como
/// informacion del doctor. `None` devuelve "found", que es lo que se
/// mostraba antes y sigue siendo cierto: el binario esta.
fn first_line(version: &Option<String>) -> String {
    version
        .as_deref()
        .and_then(|raw| raw.lines().map(str::trim).find(|l| !l.is_empty()))
        .unwrap_or("found")
        .to_string()
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

/// Detect languages in a workspace by scanning for markers
fn detect_workspace_languages(workspace_path: &Path) -> Vec<Language> {
    let mut languages = Vec::new();
    let mut seen = HashSet::new();

    // Check for Cargo.toml -> Rust
    if workspace_path.join("Cargo.toml").exists() && seen.insert(Language::Rust) {
        languages.push(Language::Rust);
    }

    // Check for go.mod -> Go
    if workspace_path.join("go.mod").exists() && seen.insert(Language::Go) {
        languages.push(Language::Go);
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
    if (workspace_path.join("pom.xml").exists()
        || glob::glob(&workspace_path.join("**/*.java").to_string_lossy())
            .ok()
            .map(|g| g.count())
            .unwrap_or(0)
            > 0)
        && seen.insert(Language::Java)
    {
        languages.push(Language::Java);
    }

    // Check for *.py files -> Python
    if glob::glob(&workspace_path.join("**/*.py").to_string_lossy())
        .ok()
        .map(|g| g.count())
        .unwrap_or(0)
        > 0
        && seen.insert(Language::Python)
    {
        languages.push(Language::Python);
    }

    languages
}

/// Run all doctor checks and generate a report
pub fn run_doctor_checks(workspace_path: Option<&Path>) -> DoctorReport {
    let version = env!("CARGO_PKG_VERSION").to_string();

    // Detect workspace languages if path provided
    let detected_languages = workspace_path
        .map(detect_workspace_languages)
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

    let summary = DoctorSummary {
        core: core.status.as_str().to_string(),
        lsp: lsp.status.as_str().to_string(),
        parsers: parsers.status.as_str().to_string(),
    };

    DoctorReport {
        schema_version: "cognicode.doctor/v1".to_string(),
        version,
        sections: DoctorSections { core, lsp, parsers },
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

    // Workspace info
    if let Some(ref ws) = report.workspace
        && !ws.languages.is_empty()
    {
        output.push_str("\nℹ️  Workspace languages detected: ");
        output.push_str(&ws.languages.join(", "));
        output.push('\n');
    }

    // Summary
    output.push_str("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    output.push_str(&format!(
        "  Core:   {} {}\n",
        status_icon(report.sections.core.status),
        report.summary.core
    ));
    output.push_str(&format!(
        "  LSP:    {} {}{}\n",
        status_icon(report.sections.lsp.status),
        report.summary.lsp,
        if report.sections.lsp.count_found() < report.sections.lsp.count_total() {
            format!(
                " ({}/{})",
                report.sections.lsp.count_found(),
                report.sections.lsp.count_total()
            )
        } else {
            String::new()
        }
    ));
    output.push_str(&format!(
        "  Parse:  {} {}\n",
        status_icon(report.sections.parsers.status),
        report.summary.parsers
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

    /// Un servidor de lenguaje, una linea.
    ///
    /// MEDIDO 2026-10-04 sobre el binario de v0.101.9 publicado, no sobre el
    /// fuente:
    ///
    /// ```
    /// ❌ Language Servers (LSP)
    ///   ✅ rust-analyzer  rust-analyzer 1.96.0 (ac68faa 2026-05-25)
    ///   ❌ pyright
    ///   ❌ typescript-language-server
    ///   ❌ typescript-language-server     <-- la misma linea, dos veces
    ///   ✅ gopls  found
    ///   ❌ eclipse-jdtls
    ///   ✅ clangd  Homebrew clangd version 23.1.0
    ///   Features: linux                    <-- el bloque de clangd, dos veces
    ///   Platform: x86_64-unknown-linux-gnu
    ///   ✅ clangd  Homebrew clangd version 23.1.0
    ///   ❌ omnisharp
    ///   ℹ️  HCL  not yet supported
    /// ```
    ///
    /// La causa: `check_lsp_section` itera por **lenguaje**, y varios
    /// lenguajes comparten binario a proposito — `TypeScript | JavaScript` a
    /// `typescript-language-server`, `C | Cpp` a `clangd` (verificado en
    /// `language.rs:328` y `:331`). El reparto es correcto; lo que no lo es es
    /// informar de lo mismo dos veces.
    ///
    /// Y la segunda mitad de la salida es peor que la duplicacion: cuando
    /// `clangd` responde, su `--version` multi-linea se imprime entero, y el
    /// usuario lee `Features:` y `Platform:` como si fueran informacion del
    /// doctor y no de la salida de `clangd --version`.
    ///
    /// MEDIDO el alcance: 7 lenguajes con servidor, 6 binarios unicos. Las
    /// dos colisiones son TypeScript/JavaScript y C/Cpp. El resto de los
    /// lenguajes caen en `_ => ""` y salen como `not yet supported`, que es
    /// correcto y se queda.
    #[test]
    fn the_lsp_section_reports_each_server_once() {
        let section = check_lsp_section(&[]);
        let rendered = format!("{:?}", section);

        let mut servers: Vec<String> = section
            .checks
            .iter()
            .filter(|c| !matches!(c.status, DoctorStatus::Info))
            .map(|c| c.name.clone())
            .collect();
        let total = servers.len();
        servers.sort();
        servers.dedup();

        assert_eq!(
            servers.len(),
            total,
            "la seccion LSP repite un servidor: {rendered}\n\
             `check_lsp_section` recorre lenguajes, y TypeScript/JavaScript y \
             C/Cpp comparten binario. Hay que informar por binario unico, no \
             por lenguaje."
        );
    }

    /// Un `--version` que devuelve varias lineas se muestra en una.
    ///
    /// MEDIDO 2026-10-04 sobre el binario publicado: `clangd --version`
    /// responde
    ///
    /// ```
    /// Homebrew clangd version 23.1.0
    /// Features: linux
    /// Platform: x86_64-unknown-linux-gnu
    /// ```
    ///
    /// y el doctor imprimia las tres, con `Features:` y `Platform:` fuera de
    /// indentacion, de modo que se leian como si fueran suyas. Se toma la
    /// primera linea no vacia, que es la que trae la version.
    #[test]
    fn a_multi_line_version_is_reported_on_one_line() {
        let section = check_lsp_section(&[]);
        for check in &section.checks {
            let detail = &check.detail;
            assert!(
                !detail.contains('\n'),
                "el detalle de `{}` tiene saltos de linea y se imprime entero, \
                 y sus lineas siguientes se leen como informacion del doctor: {detail:?}",
                check.name
            );
        }
    }

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

    // PRF-CLI-07: `cognicode doctor --format json` MUST emit a
    // machine-readable, schema-versioned document on stdout so downstream
    // consumers can pin a contract. Today the JSON contains the runtime
    // version (the bin's semver) but NOT a schema version (the shape of
    // the document). Without `schema_version`, breaking changes to the
    // JSON shape are silently shipped to consumers.
    //
    // Contract pinned by this test (will turn green once the schema
    // version field is added):
    //   1. The serialized document contains a top-level `schema_version`
    //      field of the form `cognicode.doctor/vMAJOR` (semver in the
    //      major position is enough for now; minor/patch are reserved).
    //   2. The existing `version` field (runtime semver) is preserved.
    //   3. `schema_version` and `version` are independent: changing the
    //      bin's semver does NOT bump the schema's major.
    #[test]
    fn test_doctor_json_includes_schema_version_prf_cli_07() {
        let report = run_doctor_checks(None);
        let json = format_doctor_json(&report);
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("doctor --format json must be valid JSON");

        // 1. Top-level `schema_version` present and well-formed.
        let schema_version = parsed
            .get("schema_version")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("doctor JSON missing schema_version: {json}"));
        assert!(
            schema_version.starts_with("cognicode.doctor/v"),
            "doctor JSON schema_version must start with 'cognicode.doctor/v', got: {schema_version}"
        );
        let major = schema_version
            .strip_prefix("cognicode.doctor/v")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or_else(|| panic!("schema_version major must parse as u32: {schema_version}"));
        assert_eq!(
            major, 1,
            "PRF-CLI-07 pins schema_version to v1 for the doctor shape"
        );

        // 2. The runtime version is preserved (separate concern).
        assert!(
            parsed.get("version").is_some(),
            "doctor JSON must keep the runtime 'version' field alongside schema_version"
        );

        // 3. Schema and runtime are independent: the runtime semver
        //    MUST NOT be reused as the schema version.
        let runtime_version = parsed.get("version").and_then(|v| v.as_str()).unwrap();
        assert_ne!(
            schema_version, runtime_version,
            "schema_version and version must be independent (got both = {runtime_version:?})"
        );
    }
}
