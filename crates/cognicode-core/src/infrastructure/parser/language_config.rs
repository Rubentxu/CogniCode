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
pub const NO_TYPE_REFS: Option<TypeRefWalkerFn> = None;
pub const NO_SEMANTIC_HANDLER: Option<SemanticHandlerFn> = None;

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
    pub type_ref_walker: Option<TypeRefWalkerFn>,

    /// Optional post-parse pass for domain-specific extraction (ADR-024).
    /// Receives the generic `ExtractionResult` and enriches it with
    /// domain-specific nodes (e.g., Ansible plays/tasks, Terraform resources).
    /// `None` for code languages (the generic walker is sufficient).
    pub semantic_handler: Option<SemanticHandlerFn>,
}

/// Function signature for a semantic handler (ADR-024).
/// Post-processes a generic `ExtractionResult` to add domain-specific
/// nodes and edges. Used by IaC languages (Ansible, Terraform).
pub type SemanticHandlerFn = fn(
    source_path: &str,
    source_hash: &str,
    result: &crate::application::ingest::types::ExtractionResult,
) -> crate::application::ingest::types::ExtractionResult;

/// Function signature for a type-reference walker.
/// Receives the tree-sitter node for a function/class definition and the
/// source bytes. Returns type references extracted from the node's
/// type annotations.
pub type TypeRefWalkerFn = fn(
    node: &tree_sitter::Node,
    source: &[u8],
) -> Vec<crate::application::ingest::types::TypeRef>;

/// Sentinel value: no type-ref walker configured for this language.

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
    extensions: &["rs"],
    ts_language: || tree_sitter_rust::LANGUAGE.into(),
    function_types: &["function_item"],
    class_types: &["struct_item", "enum_item", "trait_item", "impl_item", "union_item"],
    variable_types: &["let_declaration", "const_item", "static_item"],
    call_types: &["call_expression", "macro_invocation"],
    call_has_function_field: false,
    import_types: &["use_declaration"],
    type_ref_walker: Some(crate::infrastructure::parser::type_ref_walkers::walk_rust_type_refs),
    semantic_handler: NO_SEMANTIC_HANDLER,
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
    type_ref_walker: Some(crate::infrastructure::parser::type_ref_walkers::walk_python_type_refs),
    semantic_handler: NO_SEMANTIC_HANDLER,
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
    type_ref_walker: Some(crate::infrastructure::parser::type_ref_walkers::walk_typescript_type_refs),
    semantic_handler: NO_SEMANTIC_HANDLER,
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
    type_ref_walker: Some(crate::infrastructure::parser::type_ref_walkers::walk_typescript_type_refs),
    semantic_handler: NO_SEMANTIC_HANDLER,
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
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
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
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── C ────────────────────────────────────────────────────────────────────────

pub const C_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::C,
    extensions: &["c", "h"],
    ts_language: tree_sitter_c::language,
    function_types: &["function_definition"],
    class_types: &["struct_specifier", "union_specifier", "enum_specifier"],
    variable_types: &["declaration"],
    call_types: &["call_expression"],
    call_has_function_field: true,
    import_types: &["preproc_include"],
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── C++ ──────────────────────────────────────────────────────────────────────

pub const CPP_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Cpp,
    extensions: &["cpp", "cc", "cxx", "hpp", "hxx"],
    ts_language: || tree_sitter_cpp::LANGUAGE.into(),
    function_types: &["function_definition"],
    class_types: &["class_specifier", "struct_specifier", "union_specifier"],
    variable_types: &["declaration"],
    call_types: &["call_expression"],
    call_has_function_field: true,
    import_types: &["preproc_include", "using_declaration"],
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── C# ───────────────────────────────────────────────────────────────────────

pub const CSHARP_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::CSharp,
    extensions: &["cs"],
    ts_language: || tree_sitter_c_sharp::LANGUAGE.into(),
    function_types: &["method_declaration"],
    class_types: &["class_declaration", "struct_declaration", "interface_declaration", "enum_declaration"],
    variable_types: &["local_declaration_statement", "field_declaration"],
    call_types: &["invocation_expression"],
    call_has_function_field: true,
    import_types: &["using_directive"],
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── HCL / Terraform (ADR-024) ───────────────────────────────────────────────

pub const HCL_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Hcl,
    extensions: &["tf", "tfvars", "hcl"],
    ts_language: || tree_sitter_hcl::LANGUAGE.into(),
    function_types: &["block"],
    class_types: &["block"],
    variable_types: &["attribute"],
    call_types: &["expression"],
    call_has_function_field: false,
    import_types: &["block"],
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: Some(crate::infrastructure::parser::terraform_handler::interpret_terraform),
};

// ── YAML / Ansible (ADR-024) ────────────────────────────────────────────────

pub const YAML_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Yaml,
    extensions: &["yml", "yaml"],
    ts_language: || tree_sitter_yaml::LANGUAGE.into(),
    function_types: &["block_mapping"],
    class_types: &["block_mapping"],
    variable_types: &["block_mapping_pair"],
    call_types: &["flow_node"],
    call_has_function_field: false,
    import_types: &["block_mapping_pair"],
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

// ── Ruby ─────────────────────────────────────────────────────────────────────

pub const RUBY_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Ruby,
    extensions: &["rb"],
    ts_language: || tree_sitter_ruby::LANGUAGE.into(),
    function_types: &["method", "singleton_method"],
    class_types: &["class", "module"],
    variable_types: &["assignment"],
    call_types: &["call"],
    call_has_function_field: true,
    import_types: &["call"],
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── PHP ──────────────────────────────────────────────────────────────────────

pub const PHP_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Php,
    extensions: &["php"],
    ts_language: || tree_sitter_php::LANGUAGE_PHP.into(),
    function_types: &["function_definition", "method_declaration"],
    class_types: &["class_declaration", "interface_declaration", "trait_declaration"],
    variable_types: &["expression_statement"],
    call_types: &["function_call_expression", "method_call_expression"],
    call_has_function_field: true,
    import_types: &["namespace_use_declaration"],
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

// ── Swift ────────────────────────────────────────────────────────────────────

pub const SWIFT_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Swift,
    extensions: &["swift"],
    ts_language: || tree_sitter_swift::LANGUAGE.into(),
    function_types: &["function_declaration", "method_declaration"],
    class_types: &["class_declaration", "struct_declaration", "enum_declaration", "protocol_declaration"],
    variable_types: &["variable_declaration"],
    call_types: &["call_expression"],
    call_has_function_field: true,
    import_types: &["import_declaration"],
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};


pub const SCALA_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Scala,
    extensions: &["scala"],
    ts_language: || tree_sitter_scala::LANGUAGE.into(),
    function_types: &["function_declaration", "method_declaration"],
    class_types: &["class_declaration", "object_declaration", "trait_declaration"],
    variable_types: &["val_declaration", "var_declaration"],
    call_types: &["call_expression"],
    call_has_function_field: true,
    import_types: &["import_declaration"],
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

pub const LUA_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Lua,
    extensions: &["lua", "luau"],
    ts_language: || tree_sitter_lua::LANGUAGE.into(),
    function_types: &["function_declaration"],
    class_types: &["function_declaration"],
    variable_types: &["variable_declaration"],
    call_types: &["function_call"],
    call_has_function_field: true,
    import_types: &["function_call"],
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

pub const ZIG_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Zig,
    extensions: &["zig"],
    ts_language: || tree_sitter_zig::LANGUAGE.into(),
    function_types: &["function_declaration"],
    class_types: &["struct_declaration"],
    variable_types: &["variable_declaration"],
    call_types: &["call_expression"],
    call_has_function_field: true,
    import_types: &["variable_declaration"],
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

pub const DART_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Dart,
    extensions: &["dart"],
    ts_language: || tree_sitter_dart::LANGUAGE.into(),
    function_types: &["method_declaration", "function_expression"],
    class_types: &["class_declaration", "mixin_declaration"],
    variable_types: &["variable_declaration"],
    call_types: &["function_expression_invocation"],
    call_has_function_field: true,
    import_types: &["import_specification"],
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

pub const GROOVY_CONFIG: LanguageConfig = LanguageConfig {
    language: Language::Groovy,
    extensions: &["groovy", "gradle"],
    ts_language: || tree_sitter_groovy::LANGUAGE.into(),
    function_types: &["method_declaration"],
    class_types: &["class_declaration"],
    variable_types: &["variable_declaration"],
    call_types: &["method_call_expression"],
    call_has_function_field: true,
    import_types: &["import_declaration"],
    type_ref_walker: NO_TYPE_REFS,
    semantic_handler: NO_SEMANTIC_HANDLER,
};

pub const ELIXIR_CONFIG: LanguageConfig = LanguageConfig { language: Language::Elixir, extensions: &["ex","exs"], ts_language: || tree_sitter_elixir::LANGUAGE.into(), function_types: &["function"], class_types: &["module"], variable_types: &["variable_declaration"], call_types: &["call"], call_has_function_field: true, import_types: &["call"], type_ref_walker: NO_TYPE_REFS, semantic_handler: NO_SEMANTIC_HANDLER };
pub const ERLANG_CONFIG: LanguageConfig = LanguageConfig { language: Language::Erlang, extensions: &["erl","hrl"], ts_language: || tree_sitter_erlang::LANGUAGE.into(), function_types: &["function_clause"], class_types: &["module"], variable_types: &["variable_declaration"], call_types: &["function_call"], call_has_function_field: true, import_types: &["function_call"], type_ref_walker: NO_TYPE_REFS, semantic_handler: NO_SEMANTIC_HANDLER };
pub const HASKELL_CONFIG: LanguageConfig = LanguageConfig { language: Language::Haskell, extensions: &["hs"], ts_language: || tree_sitter_haskell::LANGUAGE.into(), function_types: &["function"], class_types: &["module"], variable_types: &["declaration"], call_types: &["application"], call_has_function_field: true, import_types: &["import"], type_ref_walker: NO_TYPE_REFS, semantic_handler: NO_SEMANTIC_HANDLER };
pub const JULIA_CONFIG: LanguageConfig = LanguageConfig { language: Language::Julia, extensions: &["jl"], ts_language: || tree_sitter_julia::LANGUAGE.into(), function_types: &["function_definition"], class_types: &["module_definition"], variable_types: &["assignment"], call_types: &["call_expression"], call_has_function_field: true, import_types: &["import_statement"], type_ref_walker: NO_TYPE_REFS, semantic_handler: NO_SEMANTIC_HANDLER };
pub const BASH_CONFIG: LanguageConfig = LanguageConfig { language: Language::Bash, extensions: &["sh","bash"], ts_language: || tree_sitter_bash::LANGUAGE.into(), function_types: &["function_definition"], class_types: &["function_definition"], variable_types: &["variable_assignment"], call_types: &["command"], call_has_function_field: true, import_types: &["command"], type_ref_walker: NO_TYPE_REFS, semantic_handler: NO_SEMANTIC_HANDLER };

pub const R_CONFIG: LanguageConfig = LanguageConfig { language: Language::R, extensions: &["r","R"], ts_language: || tree_sitter_r::LANGUAGE.into(), function_types: &["function_definition"], class_types: &["function_definition"], variable_types: &["assignment"], call_types: &["call"], call_has_function_field: true, import_types: &["call"], type_ref_walker: NO_TYPE_REFS, semantic_handler: NO_SEMANTIC_HANDLER };
pub const POWERSHELL_CONFIG: LanguageConfig = LanguageConfig { language: Language::PowerShell, extensions: &["ps1","psm1"], ts_language: || tree_sitter_powershell::LANGUAGE.into(), function_types: &["function_definition"], class_types: &["function_definition"], variable_types: &["assignment"], call_types: &["command"], call_has_function_field: true, import_types: &["command"], type_ref_walker: NO_TYPE_REFS, semantic_handler: NO_SEMANTIC_HANDLER };
pub const JSON_CONFIG: LanguageConfig = LanguageConfig { language: Language::Json, extensions: &["json"], ts_language: || tree_sitter_json::LANGUAGE.into(), function_types: &["object"], class_types: &["object"], variable_types: &["pair"], call_types: &["string"], call_has_function_field: false, import_types: &["object"], type_ref_walker: NO_TYPE_REFS, semantic_handler: NO_SEMANTIC_HANDLER };

pub const FORTRAN_CONFIG: LanguageConfig = LanguageConfig { language: Language::Fortran, extensions: &["f","f90","f95","f03","f08"], ts_language: || tree_sitter_fortran::LANGUAGE.into(), function_types: &["function_definition"], class_types: &["module"], variable_types: &["variable_declaration"], call_types: &["call_expression"], call_has_function_field: true, import_types: &["use_statement"], type_ref_walker: NO_TYPE_REFS, semantic_handler: NO_SEMANTIC_HANDLER };
pub const VERILOG_CONFIG: LanguageConfig = LanguageConfig { language: Language::Verilog, extensions: &["v"], ts_language: || tree_sitter_verilog::LANGUAGE.into(), function_types: &["module_declaration"], class_types: &["module_declaration"], variable_types: &["variable_declaration"], call_types: &["module_instantiation"], call_has_function_field: false, import_types: &["include_statement"], type_ref_walker: NO_TYPE_REFS, semantic_handler: NO_SEMANTIC_HANDLER };
pub const SYSTEMVERILOG_CONFIG: LanguageConfig = LanguageConfig { language: Language::SystemVerilog, extensions: &["sv"], ts_language: || tree_sitter_systemverilog::LANGUAGE.into(), function_types: &["module_declaration"], class_types: &["module_declaration"], variable_types: &["variable_declaration"], call_types: &["module_instantiation"], call_has_function_field: false, import_types: &["include_statement"], type_ref_walker: NO_TYPE_REFS, semantic_handler: NO_SEMANTIC_HANDLER };
