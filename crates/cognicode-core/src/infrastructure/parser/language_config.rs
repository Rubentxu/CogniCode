//! `LanguageConfig` — how a language is bound to a compiled tree-sitter
//! grammar.
//!
//! Each language is described by a `const LanguageConfig` consumed by the
//! generic extractor. Adding a language = adding a config, not writing
//! extraction code.
//!
//! What a language *declares* —the node kinds it uses for functions, classes,
//! variables, calls and imports, and the field that names a callee— lives in
//! the domain, in `Language::symbols`. That split is deliberate: the node
//! kinds are the vocabulary of the language, and a vocabulary belongs to the
//! problem, not to the adapter. This file says which grammar answers for a
//! language and which extra reading it needs; it does not restate what the
//! language declares, because a second copy is a copy that drifts.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(clippy::empty_line_after_doc_comments, clippy::items_after_test_module)]

use crate::domain::value_objects::Language;

/// Data-driven configuration describing how to extract structural information
/// from a specific programming language using tree-sitter.
///
/// One `const` config per language. The generic extractor walks the AST using
/// the node kinds the language declares and the config that binds it to a
/// grammar.
pub const NO_TYPE_REFS: Option<TypeRefWalkerFn> = None;
pub const NO_SEMANTIC_HANDLER: Option<SemanticHandlerFn> = None;

#[derive(Debug, Clone, Copy)]
pub struct LanguageConfig {
    /// The language enum variant this config describes.
    pub language: Language,

    /// Function that returns the tree-sitter `Language` for this config.
    pub ts_language: fn() -> tree_sitter::Language,

    pub type_ref_walker: Option<TypeRefWalkerFn>,

    /// Optional post-parse pass for domain-specific extraction.
    /// Receives the generic `ExtractionResult` and enriches it with
    /// domain-specific nodes (e.g., Ansible plays/tasks, Terraform resources).
    /// `None` for code languages (the generic walker is sufficient).
    pub semantic_handler: Option<SemanticHandlerFn>,
}

/// Function signature for a semantic handler.
/// Post-processes a generic `ExtractionResult` to add domain-specific
/// nodes and edges. Used by IaC languages (Ansible, Terraform).
///
/// `source` is the text of the file, not an ornament. A generic extraction
/// leaves a language layer with the labels it chose to keep, and that is
/// enough for the *blocks* of a configuration but not for its *relations*:
/// a reference lives in the body of a block, and the body is not part of a
/// label. A handler that can only see what the other pass decided to keep
/// is not the authority of the language — it is a reader of the other's
/// judgement.
pub type SemanticHandlerFn = fn(
    source_path: &str,
    source_hash: &str,
    source: &str,
    result: &crate::application::ingest::types::ExtractionResult,
) -> crate::application::ingest::types::ExtractionResult;

/// Function signature for a type-reference walker.
/// Receives the tree-sitter node for a function/class definition and the
/// source bytes. Returns type references extracted from the node's
/// type annotations.
pub type TypeRefWalkerFn =
    fn(node: &tree_sitter::Node, source: &[u8]) -> Vec<crate::application::ingest::types::TypeRef>;

/// Sentinel value: no type-ref walker configured for this language.

impl LanguageConfig {
    /// Look up the `LanguageConfig` for a file extension (without the dot).
    ///
    /// Returns `None` for unsupported extensions. The declaration of which
    /// extension belongs to which language lives in the domain, so this
    /// cannot answer for a language the domain does not know.
    pub fn from_extension(ext: &str) -> Option<&'static Self> {
        let language = Language::from_extension(Some(std::ffi::OsStr::new(ext)))?;
        Some(Self::for_language(language))
    }

    /// The file extensions that select this language.
    pub fn extensions(&self) -> &'static [&'static str] {
        self.language.extensions()
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
    C_CONFIG,
    CPP_CONFIG,
    CSHARP_CONFIG,
    HCL_CONFIG,
    YAML_CONFIG,
    RUBY_CONFIG,
    PHP_CONFIG,
    SWIFT_CONFIG,
    SCALA_CONFIG,
    LUA_CONFIG,
    ZIG_CONFIG,
    DART_CONFIG,
    GROOVY_CONFIG,
    ELIXIR_CONFIG,
    ERLANG_CONFIG,
    HASKELL_CONFIG,
    JULIA_CONFIG,
    BASH_CONFIG,
    R_CONFIG,
    POWERSHELL_CONFIG,
    JSON_CONFIG,
    FORTRAN_CONFIG,
    VERILOG_CONFIG,
    SYSTEMVERILOG_CONFIG,
];

// ── Rust ─────────────────────────────────────────────────────────────────────

pub const RUST_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Rust,
    ts_language: || tree_sitter_rust::LANGUAGE.into(),
    type_ref_walker: Some(crate::infrastructure::parser::type_ref_walkers::walk_rust_type_refs),
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── Python ──────────────────────────────────────────────────────────────────

pub const PYTHON_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Python,
    ts_language: || tree_sitter_python::LANGUAGE.into(),
    type_ref_walker: Some(crate::infrastructure::parser::type_ref_walkers::walk_python_type_refs),
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── TypeScript ───────────────────────────────────────────────────────────────

pub const TYPESCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::TypeScript,
    ts_language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    type_ref_walker: Some(
        crate::infrastructure::parser::type_ref_walkers::walk_typescript_type_refs,
    ),
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── JavaScript ───────────────────────────────────────────────────────────────

pub const JAVASCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::JavaScript,
    ts_language: || tree_sitter_javascript::LANGUAGE.into(),
    type_ref_walker: Some(
        crate::infrastructure::parser::type_ref_walkers::walk_typescript_type_refs,
    ),
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── Go ───────────────────────────────────────────────────────────────────────

pub const GO_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Go,
    ts_language: || tree_sitter_go::LANGUAGE.into(),
    type_ref_walker: Some(crate::infrastructure::parser::type_ref_walkers::walk_go_type_refs),
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── Java ─────────────────────────────────────────────────────────────────────

pub const JAVA_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Java,
    ts_language: || tree_sitter_java::LANGUAGE.into(),
    type_ref_walker: Some(crate::infrastructure::parser::type_ref_walkers::walk_java_type_refs),
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── C ────────────────────────────────────────────────────────────────────────

pub const C_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::C,
    ts_language: tree_sitter_c::language,
    type_ref_walker: Some(crate::infrastructure::parser::type_ref_walkers::walk_c_type_refs),
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── C++ ──────────────────────────────────────────────────────────────────────

pub const CPP_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Cpp,
    ts_language: || tree_sitter_cpp::LANGUAGE.into(),
    type_ref_walker: Some(crate::infrastructure::parser::type_ref_walkers::walk_cpp_type_refs),
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── C# ───────────────────────────────────────────────────────────────────────

pub const CSHARP_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::CSharp,
    ts_language: || tree_sitter_c_sharp::LANGUAGE.into(),
    type_ref_walker: Some(crate::infrastructure::parser::type_ref_walkers::walk_csharp_type_refs),
    semantic_handler: NO_SEMANTIC_HANDLER,
};

pub const HCL_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Hcl,
    ts_language: || tree_sitter_hcl::LANGUAGE.into(),
    // An HCL file is a configuration, not a program. The generic reader
    // declares nothing here: a block is not a function, a literal is not
    // something anybody calls, and a key is not a dependency. The Terraform
    // reader is the authority for this language, and it publishes the
    // identities the configuration actually declares.
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: Some(crate::infrastructure::parser::terraform_handler::interpret_terraform),
};

// ── YAML / Ansible ──────────────────────────────────────────────────────────

pub const YAML_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Yaml,
    ts_language: || tree_sitter_yaml::LANGUAGE.into(),
    // A YAML document is data. The Ansible reader gives a playbook its plays,
    // its tasks and its imports; for any other dialect there is nothing to
    // declare, and the generic reader leaves the file empty rather than
    // calling a mapping a class or a key an import.
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: Some(crate::infrastructure::parser::ansible_handler::interpret_ansible),
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
        assert!(config.language.symbols().function_types.contains(&"function_item"));
    }

    #[test]
    fn test_from_extension_python() {
        let config = LanguageConfig::from_extension("py").unwrap();
        assert_eq!(config.language, Language::Python);
        assert!(
            config
                .language
                .symbols()
                .function_types
                .contains(&"function_definition")
        );
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

    /// One node kind means one thing.
    ///
    /// A kind that sits in two categories is a promise the reader cannot keep:
    /// the node is published twice, once as each thing it was said to be, and
    /// the graph shows a symbol that does not exist. A language that has no
    /// such kind says so with an empty list.
    #[test]
    fn test_no_node_kind_sits_in_two_categories() {
        for lang in Language::all_languages() {
            let symbols = lang.symbols();
            let categories = [
                ("function", symbols.function_types),
                ("class", symbols.class_types),
                ("variable", symbols.variable_types),
                ("call", symbols.call_types),
                ("import", symbols.import_types),
            ];
            for (i, (name_i, kinds_i)) in categories.iter().enumerate() {
                for (name_j, kinds_j) in categories.iter().skip(i + 1) {
                    for kind in *kinds_i {
                        assert!(
                            !kinds_j.contains(kind),
                            "{lang:?} declares `{kind}` as {name_i} and as {name_j}; a node kind means one thing"
                        );
                    }
                }
            }
        }
    }

    /// Every extension of a language selects that language.
    ///
    /// The registry and the domain each know which extensions belong to a
    /// language, and a file that lands on one language in one place and
    /// another in the other place is read two different ways.
    #[test]
    fn test_extensions_select_the_same_language_as_the_domain() {
        for config in LanguageConfig::all() {
            for ext in config.extensions() {
                let from_domain = Language::from_extension(Some(std::ffi::OsStr::new(ext)));
                assert_eq!(
                    from_domain,
                    Some(config.language),
                    "the extension `{ext}` selects {from_domain:?} in the domain and \
                     {:?} in the registry",
                    config.language
                );
            }
        }
    }
}

// ── Ruby ─────────────────────────────────────────────────────────────────────

pub const RUBY_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Ruby,
    ts_language: || tree_sitter_ruby::LANGUAGE.into(),
    type_ref_walker: Some(crate::infrastructure::parser::type_ref_walkers::walk_ruby_type_refs),
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── PHP ──────────────────────────────────────────────────────────────────────

pub const PHP_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Php,
    ts_language: || tree_sitter_php::LANGUAGE_PHP.into(),
    type_ref_walker: Some(crate::infrastructure::parser::type_ref_walkers::walk_php_type_refs),
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── Swift ────────────────────────────────────────────────────────────────────

pub const SWIFT_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Swift,
    ts_language: || tree_sitter_swift::LANGUAGE.into(),
    type_ref_walker: Some(crate::infrastructure::parser::type_ref_walkers::walk_swift_type_refs),
    semantic_handler: NO_SEMANTIC_HANDLER,
};

pub const SCALA_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Scala,
    ts_language: || tree_sitter_scala::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

pub const LUA_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Lua,
    ts_language: || tree_sitter_lua::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

pub const ZIG_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Zig,
    ts_language: || tree_sitter_zig::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

pub const DART_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Dart,
    ts_language: || tree_sitter_dart::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

pub const GROOVY_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Groovy,
    ts_language: || tree_sitter_groovy::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

pub const ELIXIR_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Elixir,
    ts_language: || tree_sitter_elixir::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};
pub const ERLANG_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Erlang,
    ts_language: || tree_sitter_erlang::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};
pub const HASKELL_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Haskell,
    ts_language: || tree_sitter_haskell::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};
pub const JULIA_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Julia,
    ts_language: || tree_sitter_julia::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};
pub const BASH_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Bash,
    ts_language: || tree_sitter_bash::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};
pub const R_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::R,
    ts_language: || tree_sitter_r::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};
pub const POWERSHELL_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::PowerShell,
    ts_language: || tree_sitter_powershell::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};
pub const JSON_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Json,
    ts_language: || tree_sitter_json::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

pub const FORTRAN_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Fortran,
    ts_language: || tree_sitter_fortran::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};
pub const VERILOG_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Verilog,
    ts_language: || tree_sitter_verilog::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};
pub const SYSTEMVERILOG_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::SystemVerilog,
    ts_language: || tree_sitter_systemverilog::LANGUAGE.into(),
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};
