//! The API model catalog and its per-language adapters (data only).

use serde::{Deserialize, Serialize};

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
}

/// A weakness class the catalog can model sinks and sanitizers for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Weakness {
    /// CWE-89: SQL injection.
    SqlInjection,
}

impl Weakness {
    /// The CWE identifier.
    pub fn cwe(self) -> &'static str {
        match self {
            Self::SqlInjection => "CWE-89",
        }
    }
}

/// What an API does for taint analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelRole {
    /// Returns data controlled by an external actor.
    Source,
    /// Executes attacker-influenced text. Only the argument at `text_arg`
    /// carries the executable text; every other argument is a bound value and
    /// never reaches the sink.
    Sink {
        /// The weakness this sink belongs to.
        weakness: Weakness,
        /// Zero-based index of the argument carrying the text.
        text_arg: usize,
    },
    /// Neutralises tainted data for `weakness`.
    Sanitizer {
        /// The weakness this sanitizer neutralises.
        weakness: Weakness,
    },
    /// Recognised and harmless: it neither introduces nor neutralises taint.
    /// Listing it keeps the call from being reported as unmodeled.
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

    /// A SQL execution sink whose query text is argument `text_arg`.
    pub const fn sql_sink(callee: &'static str, text_arg: usize) -> Self {
        Self {
            callee,
            role: ModelRole::Sink {
                weakness: Weakness::SqlInjection,
                text_arg,
            },
        }
    }

    /// A sanitizer for SQL injection.
    pub const fn sql_sanitizer(callee: &'static str) -> Self {
        Self {
            callee,
            role: ModelRole::Sanitizer {
                weakness: Weakness::SqlInjection,
            },
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

    /// Whether this API is a sink.
    pub fn is_sink(&self) -> bool {
        matches!(self.role, ModelRole::Sink { .. })
    }

    /// Whether this API is a sanitizer.
    pub fn is_sanitizer(&self) -> bool {
        matches!(self.role, ModelRole::Sanitizer { .. })
    }
}

/// The API models of one language.
#[derive(Debug, Clone, Copy)]
pub struct LanguageAdapter {
    language: SourceLanguage,
    models: &'static [ApiModel],
}

impl LanguageAdapter {
    /// The language this adapter covers.
    pub fn language(&self) -> SourceLanguage {
        self.language
    }

    /// Every model of this adapter.
    pub fn models(&self) -> &'static [ApiModel] {
        self.models
    }

    /// The model of `callee`, when the adapter knows it.
    pub fn lookup(&self, callee: &str) -> Option<&'static ApiModel> {
        self.models.iter().find(|model| model.callee == callee)
    }
}

const JAVA: &[ApiModel] = &[
    ApiModel::source("javax.servlet.http.HttpServletRequest.getParameter"),
    ApiModel::source("javax.servlet.http.HttpServletRequest.getHeader"),
    ApiModel::source("javax.servlet.http.HttpServletRequest.getQueryString"),
    ApiModel::sql_sink("java.sql.Statement.executeQuery", 0),
    ApiModel::sql_sink("java.sql.Statement.executeUpdate", 0),
    ApiModel::sql_sink("java.sql.Statement.execute", 0),
    ApiModel::sql_sink("java.sql.Connection.prepareStatement", 0),
    ApiModel::sql_sanitizer("java.lang.Integer.parseInt"),
    ApiModel::sql_sanitizer("java.lang.Long.parseLong"),
    ApiModel::inert("java.sql.PreparedStatement.setString"),
    ApiModel::inert("java.sql.PreparedStatement.setInt"),
    ApiModel::inert("java.sql.PreparedStatement.setLong"),
    ApiModel::inert("java.sql.PreparedStatement.executeQuery"),
    ApiModel::inert("java.sql.PreparedStatement.executeUpdate"),
];

const GO: &[ApiModel] = &[
    ApiModel::source("net/http.Request.FormValue"),
    ApiModel::source("net/http.Header.Get"),
    ApiModel::sql_sink("database/sql.DB.Query", 0),
    ApiModel::sql_sink("database/sql.DB.QueryRow", 0),
    ApiModel::sql_sink("database/sql.DB.Exec", 0),
    ApiModel::sql_sanitizer("strconv.Atoi"),
    ApiModel::sql_sanitizer("strconv.ParseInt"),
];

const RUST: &[ApiModel] = &[
    ApiModel::source("actix_web::HttpRequest::query_string"),
    ApiModel::source("std::env::var"),
    ApiModel::sql_sink("sqlx::query", 0),
    ApiModel::sql_sink("rusqlite::Connection::execute", 0),
    ApiModel::sql_sink("rusqlite::Connection::prepare", 0),
    ApiModel::sql_sanitizer("i64::from_str"),
    ApiModel::sql_sanitizer("u64::from_str"),
    ApiModel::inert("sqlx::query::Query::bind"),
];

/// The built-in catalog: one adapter per modeled language.
///
/// Kotlin is deliberately absent: until an adapter exists the catalog reports
/// it as unsupported rather than analysing it against nothing.
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
                    models: RUST,
                },
                LanguageAdapter {
                    language: SourceLanguage::Java,
                    models: JAVA,
                },
                LanguageAdapter {
                    language: SourceLanguage::Go,
                    models: GO,
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

    #[test]
    fn every_modeled_language_has_source_sink_and_sanitizer() {
        let catalog = SemanticModelCatalog::builtin();
        for language in [
            SourceLanguage::Rust,
            SourceLanguage::Java,
            SourceLanguage::Go,
        ] {
            let adapter = catalog.adapter(language).expect("modeled language");
            assert!(
                adapter.models().iter().any(ApiModel::is_source),
                "{language:?}"
            );
            assert!(
                adapter.models().iter().any(ApiModel::is_sink),
                "{language:?}"
            );
            assert!(
                adapter.models().iter().any(ApiModel::is_sanitizer),
                "{language:?}"
            );
        }
    }

    #[test]
    fn callees_are_unique_within_an_adapter() {
        let catalog = SemanticModelCatalog::builtin();
        for language in [
            SourceLanguage::Rust,
            SourceLanguage::Java,
            SourceLanguage::Go,
        ] {
            let models = catalog.adapter(language).expect("modeled").models();
            let mut seen = std::collections::BTreeSet::new();
            for model in models {
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
    fn kotlin_has_no_adapter() {
        assert!(
            SemanticModelCatalog::builtin()
                .adapter(SourceLanguage::Kotlin)
                .is_none()
        );
    }

    #[test]
    fn weakness_names_its_cwe() {
        assert_eq!(Weakness::SqlInjection.cwe(), "CWE-89");
    }
}
