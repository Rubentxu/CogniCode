//! The registry may only claim what each grammar can produce.
//!
//! A registry that names a node kind its grammar never emits is a promise
//! the product cannot keep. The reader looks for a node that is not there,
//! finds nothing, and the language is published as if it had been read. The
//! failure is invisible: the graph is simply empty, and the only place
//! where the claim shows is the support matrix, which says the language is
//! supported.
//!
//! So the declaration is checked against the grammar itself, at runtime,
//! for every language the product supports. The grammar is the authority
//! and the registry is measured against it: nothing about a language is
//! written down twice, and nothing has to be kept in sync by hand.
//!
//! The published matrix is checked here too. It says, per language, what
//! the generic reader cannot assert, and that statement is only worth
//! something if the code behind it agrees.

use cognicode_core::domain::value_objects::Language;
use cognicode_core::infrastructure::parser::language_config::LanguageConfig;

/// Every node kind the grammar can produce.
fn node_kinds(grammar: &tree_sitter::Language) -> Vec<&str> {
    (0..grammar.node_kind_count() as u16)
        .filter_map(|id| grammar.node_kind_for_id(id))
        .collect()
}

/// The categories a reader can publish for a language.
const CATEGORIES: [&str; 5] = ["function", "class", "variable", "call", "import"];

/// What the generic reader cannot assert for a language.
///
/// A category the language declares nothing for is a category the generic
/// reader cannot assert: it has no node kind to look for. An import has two
/// ways to be declared, because a language may have an import statement or
/// a call that loads a dependency, and either one is enough.
///
/// The rule is stated here and in the generator that publishes the matrix.
/// That is not a copy to keep in sync: the two are written on purpose, so
/// that a change to one of them is a disagreement the matrix check can see.
fn generic_reader_limits(language: Language) -> Vec<&'static str> {
    let symbols = language.symbols();
    let mut limits: Vec<&'static str> = Vec::new();
    for category in CATEGORIES {
        let declared = match category {
            "function" => !symbols.function_types.is_empty(),
            "class" => !symbols.class_types.is_empty(),
            "variable" => !symbols.variable_types.is_empty(),
            "call" => !symbols.call_types.is_empty(),
            "import" => !symbols.import_types.is_empty() || !symbols.import_callees.is_empty(),
            _ => unreachable!("the categories are the ones above"),
        };
        if !declared {
            limits.push(category);
        }
    }
    limits
}

/// The published support matrix.
fn published_matrix() -> serde_json::Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../product/languages.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the support matrix must exist: it is published with the product and \
             this test runs in the repository. Path: {} ({e})",
            path.display()
        )
    });
    serde_json::from_str(&text).expect("the support matrix is json")
}

#[test]
fn every_declared_node_kind_is_produced_by_its_grammar() {
    let mut offenders: Vec<String> = Vec::new();

    for config in LanguageConfig::all() {
        let grammar = (config.ts_language)();
        let kinds = node_kinds(&grammar);
        let symbols = config.language.symbols();

        for (category, declared) in [
            ("function", symbols.function_types),
            ("class", symbols.class_types),
            ("variable", symbols.variable_types),
            ("call", symbols.call_types),
            ("import", symbols.import_types),
        ] {
            for kind in declared {
                if !kinds.contains(kind) {
                    offenders.push(format!(
                        "{:?} declares `{kind}` as {category}, and its grammar produces no such node",
                        config.language
                    ));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "the registry claims node kinds the grammar does not produce:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn every_declared_callee_field_exists_in_its_grammar() {
    let mut offenders: Vec<String> = Vec::new();

    for config in LanguageConfig::all() {
        let grammar = (config.ts_language)();
        for field in config.language.symbols().callee_fields {
            if grammar.field_id_for_name(field).is_none() {
                offenders.push(format!(
                    "{:?} names a callee by `{field}`, and its grammar has no such field",
                    config.language
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "the registry names callees by fields the grammar does not have:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_published_matrix_declares_what_the_registry_allows() {
    let matrix = published_matrix();
    let languages = matrix["languages"]
        .as_array()
        .expect("the matrix lists its languages");

    let mut offenders: Vec<String> = Vec::new();
    for language in Language::all_languages() {
        let entry = languages
            .iter()
            .find(|entry| entry["id"] == language.id())
            .unwrap_or_else(|| panic!("the support matrix does not mention {:?}", language));

        let published: Vec<String> = entry["not_asserted_by_generic_reader"]
            .as_array()
            .unwrap_or_else(|| {
                panic!(
                    "the matrix does not say what the generic reader cannot assert for {:?}",
                    language
                )
            })
            .iter()
            .map(|value| value.as_str().unwrap_or_default().to_string())
            .collect();

        let expected: Vec<String> = generic_reader_limits(*language)
            .iter()
            .map(|category| category.to_string())
            .collect();

        if published != expected {
            offenders.push(format!(
                "{:?} is published as {published:?}, and the registry says {expected:?}",
                language
            ));
        }
    }

    assert!(
        offenders.is_empty(),
        "the published matrix says something the registry does not:\n  {}",
        offenders.join("\n  ")
    );
}
