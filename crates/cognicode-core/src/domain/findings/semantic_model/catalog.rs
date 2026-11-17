//! The API model catalog and its per-language adapters (data only).

use serde::{Deserialize, Serialize};

use super::rule::{
    COMMAND_EXECUTION_SUBJECT, COMMAND_INJECTION_KIND, FILE_ACCESS_SUBJECT, PATH_TRAVERSAL_KIND,
    SQL_EXECUTION_SUBJECT, SQL_INJECTION_KIND,
};

/// A source language the catalog may carry an adapter for.
///
/// Listing a language here does **not** mean it is modeled: coverage is
/// decided by [`SemanticModelCatalog::adapter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceLanguage {
    /// Rust.
    Rust,
    /// Java.
    Java,
    /// Kotlin.
    Kotlin,
    /// Go.
    Go,
    /// Python: recognised as a language but carrying no adapter, so every
    /// analysis of it is reported as unsupported.
    Python,
    /// Terraform (HCL): analysed statically from the configuration text by
    /// the infrastructure families; it has no API adapter because it has no
    /// source-to-sink flow.
    Terraform,
    /// Helm charts: analysed through the Kubernetes objects a real render
    /// produced, never through template text. Without a render nothing about
    /// the chart is decided.
    Helm,
}

impl SourceLanguage {
    /// Every language the catalog names, modeled or not, in a stable order.
    pub const ALL: [Self; 7] = [
        Self::Rust,
        Self::Java,
        Self::Kotlin,
        Self::Go,
        Self::Python,
        Self::Terraform,
        Self::Helm,
    ];

    /// Stable lowercase name for reports.
    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::Go => "go",
            Self::Python => "python",
            Self::Terraform => "terraform",
            Self::Helm => "helm",
        }
    }

    /// Whether the language runs on the JVM and resolves types through a
    /// classpath.
    pub fn is_jvm(self) -> bool {
        matches!(self, Self::Java | Self::Kotlin)
    }
}

/// A taint weakness class the catalog can model sinks and sanitizers for.
///
/// Structural families (hardcoded secrets, unsafe code) have no source-to-sink
/// flow and live in [`StructuralFamily`](super::StructuralFamily).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Weakness {
    /// CWE-89: SQL injection.
    SqlInjection,
    /// CWE-78: OS command injection.
    CommandInjection,
    /// CWE-22: path traversal.
    PathTraversal,
}

impl Weakness {
    /// Every taint weakness, in a stable order.
    pub const ALL: [Self; 3] = [
        Self::SqlInjection,
        Self::CommandInjection,
        Self::PathTraversal,
    ];

    /// The CWE identifier.
    pub fn cwe(self) -> &'static str {
        match self {
            Self::SqlInjection => "CWE-89",
            Self::CommandInjection => "CWE-78",
            Self::PathTraversal => "CWE-22",
        }
    }

    /// The dataflow subject carried by statements that reach a sink of this
    /// weakness.
    pub fn sink_subject(self) -> &'static str {
        match self {
            Self::SqlInjection => SQL_EXECUTION_SUBJECT,
            Self::CommandInjection => COMMAND_EXECUTION_SUBJECT,
            Self::PathTraversal => FILE_ACCESS_SUBJECT,
        }
    }

    /// The finding kind the weakness' detector produces.
    pub fn finding_kind(self) -> &'static str {
        match self {
            Self::SqlInjection => SQL_INJECTION_KIND,
            Self::CommandInjection => COMMAND_INJECTION_KIND,
            Self::PathTraversal => PATH_TRAVERSAL_KIND,
        }
    }
}

/// What an API does for taint analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelRole {
    /// Returns data controlled by an external actor.
    Source,
    /// Consumes attacker-influenced data as code, a command or a location.
    /// Only the argument at `text_arg` carries it; every other argument is a
    /// bound value and never reaches the sink.
    Sink {
        /// The weakness this sink belongs to.
        weakness: Weakness,
        /// Zero-based index of the argument carrying the dangerous value.
        text_arg: usize,
    },
    /// Neutralises tainted data for every weakness in `neutralises`.
    Sanitizer {
        /// The weaknesses this sanitizer neutralises.
        neutralises: &'static [Weakness],
    },
    /// Recognised and harmless: it neither introduces nor neutralises taint
    /// (taint still flows through it). Listing it keeps the call from being
    /// reported as unmodeled.
    Inert,
}

/// One modeled API, identified by its fully qualified callee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiModel {
    callee: &'static str,
    role: ModelRole,
}

impl ApiModel {
    /// A source of external data.
    pub const fn source(callee: &'static str) -> Self {
        Self {
            callee,
            role: ModelRole::Source,
        }
    }

    /// A sink of `weakness` whose dangerous value is argument `text_arg`.
    pub const fn sink(callee: &'static str, weakness: Weakness, text_arg: usize) -> Self {
        Self {
            callee,
            role: ModelRole::Sink { weakness, text_arg },
        }
    }

    /// A SQL execution sink whose query text is argument `text_arg`.
    pub const fn sql_sink(callee: &'static str, text_arg: usize) -> Self {
        Self::sink(callee, Weakness::SqlInjection, text_arg)
    }

    /// A sanitizer for every weakness in `neutralises`.
    pub const fn sanitizer(callee: &'static str, neutralises: &'static [Weakness]) -> Self {
        Self {
            callee,
            role: ModelRole::Sanitizer { neutralises },
        }
    }

    /// A recognised API with no taint effect.
    pub const fn inert(callee: &'static str) -> Self {
        Self {
            callee,
            role: ModelRole::Inert,
        }
    }

    /// The fully qualified callee.
    pub fn callee(&self) -> &'static str {
        self.callee
    }

    /// The role of this API.
    pub fn role(&self) -> ModelRole {
        self.role
    }

    /// Whether this API is a source.
    pub fn is_source(&self) -> bool {
        matches!(self.role, ModelRole::Source)
    }

    /// Whether this API is a sink of any weakness.
    pub fn is_sink(&self) -> bool {
        matches!(self.role, ModelRole::Sink { .. })
    }

    /// Whether this API is a sink of `weakness`.
    pub fn sinks(&self, weakness: Weakness) -> bool {
        matches!(self.role, ModelRole::Sink { weakness: w, .. } if w == weakness)
    }

    /// Whether this API neutralises `weakness`.
    pub fn sanitizes(&self, weakness: Weakness) -> bool {
        matches!(self.role, ModelRole::Sanitizer { neutralises } if neutralises.contains(&weakness))
    }
}

/// The API models of one language: one or more tables searched in order, so a
/// family of languages can share a table (the JDK is one library for Java and
/// Kotlin) without copying rows.
#[derive(Debug, Clone, Copy)]
pub struct LanguageAdapter {
    language: SourceLanguage,
    tables: &'static [&'static [ApiModel]],
}

impl LanguageAdapter {
    /// The language this adapter covers.
    pub fn language(&self) -> SourceLanguage {
        self.language
    }

    /// Every model of this adapter.
    pub fn models(&self) -> impl Iterator<Item = &'static ApiModel> + '_ {
        self.tables.iter().flat_map(|table| table.iter())
    }

    /// The model of `callee`, when the adapter knows it.
    pub fn lookup(&self, callee: &str) -> Option<&'static ApiModel> {
        self.models().find(|model| model.callee == callee)
    }
}

/// Numeric coercion: a value that parsed as a number cannot carry SQL text, a
/// command line or a path.
const NUMERIC: &[Weakness] = &Weakness::ALL;
/// Reducing a path to its last component removes every directory step.
const BASENAME: &[Weakness] = &[Weakness::PathTraversal];

/// The JDK and common libraries, seen from any JVM language.
const JVM: &[ApiModel] = &[
    ApiModel::source("javax.servlet.http.HttpServletRequest.getParameter"),
    ApiModel::source("javax.servlet.http.HttpServletRequest.getHeader"),
    ApiModel::source("javax.servlet.http.HttpServletRequest.getQueryString"),
    ApiModel::source("java.lang.System.getenv"),
    ApiModel::sql_sink("java.sql.Statement.executeQuery", 0),
    ApiModel::sql_sink("java.sql.Statement.executeUpdate", 0),
    ApiModel::sql_sink("java.sql.Statement.execute", 0),
    ApiModel::sql_sink("java.sql.Connection.prepareStatement", 0),
    ApiModel::sink("java.lang.Runtime.exec", Weakness::CommandInjection, 0),
    ApiModel::sink(
        "java.lang.ProcessBuilder.<init>",
        Weakness::CommandInjection,
        0,
    ),
    ApiModel::sink(
        "java.lang.ProcessBuilder.command",
        Weakness::CommandInjection,
        0,
    ),
    ApiModel::sink("java.nio.file.Files.readString", Weakness::PathTraversal, 0),
    ApiModel::sink(
        "java.nio.file.Files.readAllBytes",
        Weakness::PathTraversal,
        0,
    ),
    ApiModel::sink(
        "java.nio.file.Files.writeString",
        Weakness::PathTraversal,
        0,
    ),
    ApiModel::sink("java.io.FileInputStream.<init>", Weakness::PathTraversal, 0),
    ApiModel::sink(
        "java.io.FileOutputStream.<init>",
        Weakness::PathTraversal,
        0,
    ),
    ApiModel::sanitizer("java.lang.Integer.parseInt", NUMERIC),
    ApiModel::sanitizer("java.lang.Long.parseLong", NUMERIC),
    ApiModel::sanitizer("java.nio.file.Path.getFileName", BASENAME),
    ApiModel::sanitizer("org.apache.commons.io.FilenameUtils.getName", BASENAME),
    ApiModel::inert("java.sql.Connection.createStatement"),
    ApiModel::inert("java.sql.PreparedStatement.setString"),
    ApiModel::inert("java.sql.PreparedStatement.setInt"),
    ApiModel::inert("java.sql.PreparedStatement.setLong"),
    ApiModel::inert("java.sql.PreparedStatement.executeQuery"),
    ApiModel::inert("java.sql.PreparedStatement.executeUpdate"),
    ApiModel::inert("java.lang.Runtime.getRuntime"),
    ApiModel::inert("java.nio.file.Paths.get"),
    ApiModel::inert("java.nio.file.Path.of"),
    ApiModel::inert("java.nio.file.Path.resolve"),
    ApiModel::inert("java.io.File.<init>"),
];

/// Kotlin spellings the JDK table does not cover: stdlib extensions compile to
/// `kotlin.*` facade functions with the receiver as first argument.
const KOTLIN: &[ApiModel] = &[
    ApiModel::sanitizer("kotlin.text.toInt", NUMERIC),
    ApiModel::sanitizer("kotlin.text.toLong", NUMERIC),
    ApiModel::sink("kotlin.io.readText", Weakness::PathTraversal, 0),
    ApiModel::sink("kotlin.io.writeText", Weakness::PathTraversal, 0),
];

const GO: &[ApiModel] = &[
    ApiModel::source("net/http.Request.FormValue"),
    ApiModel::source("net/http.Header.Get"),
    ApiModel::source("os.Getenv"),
    ApiModel::sql_sink("database/sql.DB.Query", 0),
    ApiModel::sql_sink("database/sql.DB.QueryRow", 0),
    ApiModel::sql_sink("database/sql.DB.Exec", 0),
    ApiModel::sink("os/exec.Command", Weakness::CommandInjection, 0),
    ApiModel::sink("os/exec.CommandContext", Weakness::CommandInjection, 1),
    ApiModel::sink("os.Open", Weakness::PathTraversal, 0),
    ApiModel::sink("os.ReadFile", Weakness::PathTraversal, 0),
    ApiModel::sink("os.Create", Weakness::PathTraversal, 0),
    ApiModel::sink("os.WriteFile", Weakness::PathTraversal, 0),
    ApiModel::sanitizer("strconv.Atoi", NUMERIC),
    ApiModel::sanitizer("strconv.ParseInt", NUMERIC),
    ApiModel::sanitizer("path/filepath.Base", BASENAME),
    ApiModel::inert("fmt.Sprintf"),
    ApiModel::inert("path/filepath.Join"),
];

const RUST: &[ApiModel] = &[
    ApiModel::source("actix_web::HttpRequest::query_string"),
    ApiModel::source("std::env::var"),
    ApiModel::sql_sink("sqlx::query", 0),
    ApiModel::sql_sink("rusqlite::Connection::execute", 0),
    ApiModel::sql_sink("rusqlite::Connection::prepare", 0),
    ApiModel::sink("std::process::Command::new", Weakness::CommandInjection, 0),
    ApiModel::sink("std::fs::read_to_string", Weakness::PathTraversal, 0),
    ApiModel::sink("std::fs::read", Weakness::PathTraversal, 0),
    ApiModel::sink("std::fs::write", Weakness::PathTraversal, 0),
    ApiModel::sink("std::fs::File::open", Weakness::PathTraversal, 0),
    ApiModel::sink("std::fs::File::create", Weakness::PathTraversal, 0),
    ApiModel::sanitizer("i64::from_str", NUMERIC),
    ApiModel::sanitizer("u64::from_str", NUMERIC),
    ApiModel::sanitizer("u32::from_str", NUMERIC),
    ApiModel::sanitizer("std::path::Path::file_name", BASENAME),
    ApiModel::inert("sqlx::query::Query::bind"),
    ApiModel::inert("std::path::Path::join"),
    ApiModel::inert("std::path::PathBuf::from"),
];

/// The built-in catalog: one adapter per modeled language.
///
/// A language absent from the catalog is reported as unsupported rather than
/// analysed against an empty model.
#[derive(Debug, Clone)]
pub struct SemanticModelCatalog {
    adapters: Vec<LanguageAdapter>,
}

impl SemanticModelCatalog {
    /// The shipped catalog.
    pub fn builtin() -> Self {
        Self {
            adapters: vec![
                LanguageAdapter {
                    language: SourceLanguage::Rust,
                    tables: &[RUST],
                },
                LanguageAdapter {
                    language: SourceLanguage::Java,
                    tables: &[JVM],
                },
                LanguageAdapter {
                    language: SourceLanguage::Kotlin,
                    tables: &[JVM, KOTLIN],
                },
                LanguageAdapter {
                    language: SourceLanguage::Go,
                    tables: &[GO],
                },
            ],
        }
    }

    /// The adapter of `language`, or `None` when the language is not modeled.
    pub fn adapter(&self, language: SourceLanguage) -> Option<&LanguageAdapter> {
        self.adapters.iter().find(|a| a.language == language)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODELED: [SourceLanguage; 4] = [
        SourceLanguage::Rust,
        SourceLanguage::Java,
        SourceLanguage::Kotlin,
        SourceLanguage::Go,
    ];

    #[test]
    fn every_modeled_language_has_a_source_and_a_sink_and_sanitizer_for_each_weakness() {
        let catalog = SemanticModelCatalog::builtin();
        for language in MODELED {
            let adapter = catalog.adapter(language).expect("modeled language");
            assert!(
                adapter.models().any(ApiModel::is_source),
                "{language:?}: no source"
            );
            for weakness in Weakness::ALL {
                assert!(
                    adapter.models().any(|m| m.sinks(weakness)),
                    "{language:?}: no {weakness:?} sink"
                );
                assert!(
                    adapter.models().any(|m| m.sanitizes(weakness)),
                    "{language:?}: no {weakness:?} sanitizer"
                );
            }
        }
    }

    #[test]
    fn callees_are_unique_within_an_adapter() {
        let catalog = SemanticModelCatalog::builtin();
        for language in MODELED {
            let adapter = catalog.adapter(language).expect("modeled");
            let mut seen = std::collections::BTreeSet::new();
            for model in adapter.models() {
                assert!(
                    seen.insert(model.callee()),
                    "{language:?}: duplicate model for {}",
                    model.callee()
                );
            }
        }
    }

    #[test]
    fn lookup_is_exact_not_a_prefix_match() {
        let catalog = SemanticModelCatalog::builtin();
        let java = catalog.adapter(SourceLanguage::Java).expect("java");
        assert!(java.lookup("java.sql.Statement.executeQuery").is_some());
        assert!(
            java.lookup("java.sql.Statement.executeQueryAsync")
                .is_none()
        );
        assert!(java.lookup("executeQuery").is_none());
    }

    #[test]
    fn kotlin_reaches_the_jdk_through_the_shared_jvm_table_and_adds_its_own_apis() {
        let catalog = SemanticModelCatalog::builtin();
        let kotlin = catalog.adapter(SourceLanguage::Kotlin).expect("kotlin");
        let java = catalog.adapter(SourceLanguage::Java).expect("java");
        // The JDK is the same library seen from either language.
        assert_eq!(
            kotlin.lookup("java.lang.Runtime.exec"),
            java.lookup("java.lang.Runtime.exec")
        );
        // Kotlin-only spellings are not Java's.
        assert!(kotlin.lookup("kotlin.text.toInt").is_some());
        assert!(java.lookup("kotlin.text.toInt").is_none());
    }

    #[test]
    fn a_sanitizer_neutralises_only_the_weaknesses_it_names() {
        let catalog = SemanticModelCatalog::builtin();
        let go = catalog.adapter(SourceLanguage::Go).expect("go");
        let atoi = go.lookup("strconv.Atoi").expect("modeled");
        assert!(Weakness::ALL.into_iter().all(|w| atoi.sanitizes(w)));
        let base = go.lookup("path/filepath.Base").expect("modeled");
        assert!(base.sanitizes(Weakness::PathTraversal));
        assert!(!base.sanitizes(Weakness::SqlInjection));
        assert!(!base.sanitizes(Weakness::CommandInjection));
    }

    #[test]
    fn a_sink_belongs_to_one_weakness() {
        let catalog = SemanticModelCatalog::builtin();
        let rust = catalog.adapter(SourceLanguage::Rust).expect("rust");
        let command = rust.lookup("std::process::Command::new").expect("modeled");
        assert!(command.sinks(Weakness::CommandInjection));
        assert!(!command.sinks(Weakness::SqlInjection));
        assert!(!command.sinks(Weakness::PathTraversal));
    }

    #[test]
    fn weaknesses_name_their_cwe_subject_and_finding_kind() {
        assert_eq!(Weakness::SqlInjection.cwe(), "CWE-89");
        assert_eq!(Weakness::CommandInjection.cwe(), "CWE-78");
        assert_eq!(Weakness::PathTraversal.cwe(), "CWE-22");
        let subjects: std::collections::BTreeSet<_> = Weakness::ALL
            .into_iter()
            .map(Weakness::sink_subject)
            .collect();
        let kinds: std::collections::BTreeSet<_> = Weakness::ALL
            .into_iter()
            .map(Weakness::finding_kind)
            .collect();
        assert_eq!(subjects.len(), Weakness::ALL.len());
        assert_eq!(kinds.len(), Weakness::ALL.len());
    }
}
