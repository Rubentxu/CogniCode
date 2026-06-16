//! `LanguageConfig` — data-driven per-language tree-sitter extraction
//! configuration (ADR-018).
//!
//! Each language is described by a `const LanguageConfig` consumed by the
//! generic extractor. Adding a language = adding a config, not writing
//! extraction code.

use super::tree_sitter_parser::Language;

/// Data-driven configuration describing how to extract structural information
/// from a specific programming language using tree-sitter.
///
/// One `const` config per language. The generic extractor walks the AST using
/// the config's node-type sets.
#[derive(Debug, Clone, Copy)]
pub struct LanguageConfig {
    /// The language enum variant this config describes.
    pub language: Language,

    /// File extensions (without the dot) that map to this language.
    pub extensions: &'static [&'static str],

    /// Function that returns the tree-sitter `Language` for this config.
    pub ts_language: fn() -> tree_sitter::Language,

    /// Tree-sitter node types that represent function definitions.
    /// The extractor creates a `Symbol(Function)` node for each.
    pub function_types: &'static [&'static str],

    /// Tree-sitter node types that represent class/struct/trait/interface
    /// definitions. The extractor creates a `Symbol(Class)` or
    /// `Symbol(Trait)` node for each.
    pub class_types: &'static [&'static str],

    /// Tree-sitter node types that represent variable/field declarations.
    pub variable_types: &'static [&'static str],

    /// Tree-sitter node types that represent call expressions.
    /// The extractor follows these to build `Calls` edges.
    pub call_types: &'static [&'static str],

    /// Whether call nodes expose the callee via a `"function"` field.
    /// If `false`, the callee is the first named child.
    pub call_has_function_field: bool,

    /// Tree-sitter node types that represent import/include statements.
    /// The extractor creates `Imports` edges from the file node.
    pub import_types: &'static [&'static str],
}

impl LanguageConfig {
    /// Look up the `LanguageConfig` for a file extension (without the dot).
    ///
    /// Returns `None` for unsupported extensions.
    pub fn from_extension(ext: &str) -> Option<&'static Self> {
        let lower = ext.to_lowercase();
        ALL_LANGUAGES
            .iter()
            .find(|config| config.extensions.iter().any(|e| *e == lower))
    }

    /// Look up the `LanguageConfig` for a `Language` enum variant.
    pub fn for_language(lang: Language) -> &'static Self {
        ALL_LANGUAGES
            .iter()
            .find(|config| config.language == lang)
            .expect("every Language variant must have a LanguageConfig")
    }

    /// Returns all registered language configs.
    pub fn all() -> &'static [LanguageConfig] {
        ALL_LANGUAGES
    }
}

// ============================================================================
// Language configs — one const per language
// ============================================================================

/// All registered language configs, ordered by priority (most common first).
pub static ALL_LANGUAGES: &[LanguageConfig] = &[
    RUST_CONFIG,
    PYTHON_CONFIG,
    TYPESCRIPT_CONFIG,
    JAVASCRIPT_CONFIG,
    GO_CONFIG,
    JAVA_CONFIG,
];

// ── Rust ─────────────────────────────────────────────────────────────────────

pub const RUST_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Rust,
    extensions: &["rs"],
    ts_language: || tree_sitter_rust::LANGUAGE.into(),
    function_types: &["function_item"],
    class_types: &["struct_item", "enum_item", "trait_item", "impl_item", "union_item"],
    variable_types: &["let_declaration", "const_item", "static_item"],
    call_types: &["call_expression", "macro_invocation"],
    call_has_function_field: false,
    import_types: &["use_declaration"],
};

// ── Python ──────────────────────────────────────────────────────────────────

pub const PYTHON_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Python,
    extensions: &["py", "pyw"],
    ts_language: || tree_sitter_python::LANGUAGE.into(),
    function_types: &["function_definition"],
    class_types: &["class_definition"],
    variable_types: &["assignment"],
    call_types: &["call"],
    call_has_function_field: true,
    import_types: &["import_statement", "import_from_statement"],
};

// ── TypeScript ───────────────────────────────────────────────────────────────

pub const TYPESCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::TypeScript,
    extensions: &["ts", "tsx"],
    ts_language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    function_types: &["function_declaration", "method_definition", "arrow_function"],
    class_types: &["class_declaration", "interface_declaration"],
    variable_types: &["variable_declaration", "lexical_declaration"],
    call_types: &["call_expression", "new_expression"],
    call_has_function_field: true,
    import_types: &["import_statement", "export_statement"],
};

// ── JavaScript ───────────────────────────────────────────────────────────────

pub const JAVASCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::JavaScript,
    extensions: &["js", "jsx", "mjs", "cjs"],
    ts_language: || tree_sitter_javascript::LANGUAGE.into(),
    function_types: &["function_declaration", "method_definition", "arrow_function"],
    class_types: &["class_declaration"],
    variable_types: &["variable_declaration", "lexical_declaration"],
    call_types: &["call_expression", "new_expression"],
    call_has_function_field: true,
    import_types: &["import_statement", "export_statement"],
};

// ── Go ───────────────────────────────────────────────────────────────────────

pub const GO_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Go,
    extensions: &["go"],
    ts_language: || tree_sitter_go::LANGUAGE.into(),
    function_types: &["function_declaration", "method_declaration"],
    class_types: &["type_declaration"],
    variable_types: &["short_var_declaration", "var_declaration"],
    call_types: &["call_expression"],
    call_has_function_field: true,
    import_types: &["import_declaration"],
};

// ── Java ─────────────────────────────────────────────────────────────────────

pub const JAVA_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Java,
    extensions: &["java"],
    ts_language: || tree_sitter_java::LANGUAGE.into(),
    function_types: &["method_declaration", "constructor_declaration"],
    class_types: &["class_declaration", "interface_declaration", "enum_declaration"],
    variable_types: &["local_variable_declaration", "field_declaration"],
    call_types: &["method_invocation"],
    call_has_function_field: false,
    import_types: &["import_declaration"],
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_extension_rust() {
        let config = LanguageConfig::from_extension("rs").unwrap();
        assert_eq!(config.language, Language::Rust);
        assert!(config.function_types.contains(&"function_item"));
    }

    #[test]
    fn test_from_extension_python() {
        let config = LanguageConfig::from_extension("py").unwrap();
        assert_eq!(config.language, Language::Python);
        assert!(config.function_types.contains(&"function_definition"));
    }

    #[test]
    fn test_from_extension_typescript() {
        let config = LanguageConfig::from_extension("ts").unwrap();
        assert_eq!(config.language, Language::TypeScript);
        let config2 = LanguageConfig::from_extension("tsx").unwrap();
        assert_eq!(config2.language, Language::TypeScript);
    }

    #[test]
    fn test_from_extension_javascript() {
        let config = LanguageConfig::from_extension("js").unwrap();
        assert_eq!(config.language, Language::JavaScript);
    }

    #[test]
    fn test_from_extension_go() {
        let config = LanguageConfig::from_extension("go").unwrap();
        assert_eq!(config.language, Language::Go);
    }

    #[test]
    fn test_from_extension_java() {
        let config = LanguageConfig::from_extension("java").unwrap();
        assert_eq!(config.language, Language::Java);
    }

    #[test]
    fn test_from_extension_case_insensitive() {
        let config = LanguageConfig::from_extension("RS").unwrap();
        assert_eq!(config.language, Language::Rust);
    }

    #[test]
    fn test_from_extension_unknown() {
        assert!(LanguageConfig::from_extension("brainfuck").is_none());
    }

    #[test]
    fn test_for_language_roundtrip() {
        for lang in Language::all_languages() {
            let config = LanguageConfig::for_language(*lang);
            assert_eq!(config.language, *lang);
        }
    }

    #[test]
    fn test_all_configs_have_imports() {
        for config in LanguageConfig::all() {
            assert!(
                !config.import_types.is_empty(),
                "{:?} should have at least one import type",
                config.language
            );
        }
    }

    #[test]
    fn test_all_configs_have_calls() {
        for config in LanguageConfig::all() {
            assert!(
                !config.call_types.is_empty(),
                "{:?} should have at least one call type",
                config.language
            );
        }
    }
}
