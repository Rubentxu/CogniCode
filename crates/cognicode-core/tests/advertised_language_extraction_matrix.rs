//! Characterisation of the symbol-extraction gap behind the advertised
//! `support_level` claims.
//!
//! `product/languages.json` advertises 18 languages as
//! `support_level: "supported"`. Measuring what the extractor actually returns
//! for a trivial top-level definition in each of them shows four of them
//! extract **nothing at all**:
//!
//! ```text
//! language     parsed  found
//! Rust         yes     ok        line 0
//! Python       yes     ok        line 0
//! Go           yes     ok        line 2
//! Java         yes     ok        line 1
//! TypeScript   yes     ok        line 0
//! C            yes     ok        line 0
//! Cpp          yes     ok        line 0
//! CSharp       yes     ok        line 1
//! Php          yes     ok        line 1
//! Ruby         yes     ok        line 0
//! Swift        yes     ok        line 0
//! Lua          yes     ok        line 0
//! Scala        yes     MISSING   (extracted [])
//! Bash         yes     MISSING   (extracted [])
//! Yaml         yes     MISSING   (extracted [])
//! Haskell      yes     MISSING   (extracted [])
//! ```
//!
//! So this is not a Haskell defect. Haskell is the fourth language with the
//! same failure, and the causes are distinct:
//!
//! * **Bash** — `function_definition` is found by the grammar, but its name
//!   child is a `word` node. `find_identifier_name` accepts only
//!   `identifier`, `type_identifier`, `name` and `simple_identifier`, so the node
//!   matches and the name is then discarded.
//! * **Haskell** — same shape: `bind` and `signature` match by text but name
//!   the definition through a `variable` child, which is likewise not accepted.
//! * **Yaml** — `block_mapping` matches, but YAML has no identifier node kind at
//!   all; the key is a `flow_node`.
//! * **Scala** — the declared strings `function_declaration` and
//!   `val_declaration` do not appear in a real parse tree at all, so the node
//!   filter never fires.
//!
//! **Why these tests assert the broken behaviour.** A fix will have to change
//! what the extractor accepts, which is a per-language product decision about
//! which kinds count as symbols. Until that decision is written down, pinning
//! today's behaviour is the only honest option, and these tests are written so
//! the fix commit must invert them: every failure message names the language and
//! what is missing.
//!
//! The 12 passing languages are asserted positively above, so a fix that
//! over-corrects and starts matching the wrong node in Rust or Python fails here
//! rather than silently changing behaviour nobody reviewed.

use cognicode_core::domain::value_objects::Language;
use cognicode_core::infrastructure::parser::TreeSitterParser;

fn extract(lang: Language, src: &str) -> Vec<(String, u32)> {
    let parser = TreeSitterParser::new(lang).expect("grammar must load");
    parser
        .find_all_symbols_with_path(src, "probe")
        .expect("extraction must not error")
        .iter()
        .map(|s| (s.name().to_string(), s.location().line()))
        .collect()
}

// --- The four that extract nothing -----------------------------------------

#[test]
fn bash_function_definition_is_not_extracted() {
    let src = "probeBashSymbol() { echo 1; }\n";
    let got = extract(Language::Bash, src);
    assert!(
        got.is_empty(),
        "Bash is advertised support_level='supported' but extracts nothing from \
         its own function definition. Today the behaviour under test is that it \
         returns zero symbols, which is a product claim the code does not back. \
         When Bash support is fixed this test must be inverted to assert that \
         probeBashSymbol is found on line 0."
    );
}

#[test]
fn scala_function_definition_is_not_extracted() {
    let src = "object A {\n  def probeScalaSymbol: Int = 1\n}\n";
    let got = extract(Language::Scala, src);
    assert!(
        got.is_empty(),
        "Scala is advertised support_level='supported' but extracts nothing from \
         its own function definition. Today the behaviour under test is that it \
         returns zero symbols, which is a product claim the code does not back. \
         When Scala support is fixed this test must be inverted to assert that \
         probeScalaSymbol is found on line 1."
    );
}

#[test]
fn yaml_key_is_not_extracted() {
    let src = "probe_yaml_symbol: value\n";
    let got = extract(Language::Yaml, src);
    assert!(
        got.is_empty(),
        "Yaml is advertised support_level='supported' but extracts nothing from \
         its own top-level key. Today the behaviour under test is that it \
         returns {{}} symbols, which is a product claim the code does not back. \
         When YAML support is fixed this test must be inverted to assert that \
         probe_yaml_symbol is found on line 0."
    );
}

#[test]
fn haskell_bind_is_not_extracted() {
    let src = "probeHsSymbol :: Int\nprobeHsSymbol = 1\n";
    let got = extract(Language::Haskell, src);
    assert!(
        got.is_empty(),
        "Haskell is advertised support_level='supported' but extracts nothing \
         from its own binding. Today the behaviour under test is that it returns \
         zero symbols, which is a product claim the code does not back. When \
         Haskell support is fixed this test must be inverted to assert that \
         probeHsSymbol is found on line 1. See haskell_symbol_extraction_gap.rs \
         for the finer-grained version of this defect."
    );
}

// --- The twelve that do work, pinned so an over-broad fix cannot pass ------

/// Guards the languages that extract correctly today.
///
/// A fix that widens `find_identifier_name` to accept `word` and `variable`
/// (needed for Bash and Haskell) must not start matching arbitrary nodes in the
/// languages that already work. Each row asserts the exact expected output.
#[test]
fn supported_languages_that_extract_correctly_still_do() {
    let cases: Vec<(Language, &str, Vec<(&str, u32)>)> = vec![
        (
            Language::Rust,
            "fn probe() -> u32 { 1 }\n",
            vec![("probe", 0)],
        ),
        (
            Language::Python,
            "def probe():\n    return 1\n",
            vec![("probe", 0)],
        ),
        (
            Language::Go,
            "package main\n\nfunc probe() int { return 1 }\n",
            vec![("probe", 2)],
        ),
        (
            Language::Java,
            "class A {\n    void probe() {}\n}\n",
            vec![("A", 0), ("probe", 1)],
        ),
        (
            Language::TypeScript,
            "function probe() { return 1; }\n",
            vec![("probe", 0)],
        ),
        (
            Language::C,
            "int probe(void) { return 1; }\n",
            vec![("probe", 0)],
        ),
        (
            Language::Cpp,
            "int probe() { return 1; }\n",
            vec![("probe", 0)],
        ),
        (Language::Ruby, "def probe\n  1\nend\n", vec![("probe", 0)]),
        (
            Language::Swift,
            "func probe() -> Int { return 1 }\n",
            vec![("probe", 0)],
        ),
        (
            Language::Lua,
            "function probe() return 1 end\n",
            vec![("probe", 0)],
        ),
        (
            Language::Php,
            "<?php\nfunction probe() { return 1; }\n",
            vec![("probe", 1)],
        ),
        (
            Language::CSharp,
            "class A {\n    void Probe() {}\n}\n",
            vec![("A", 0), ("Probe", 1)],
        ),
    ];

    let mut report = String::new();
    let mut failures = Vec::new();
    for (lang, src, expected) in cases {
        let got = extract(lang, src);
        let expected: Vec<(String, u32)> = expected
            .into_iter()
            .map(|(n, l)| (n.to_string(), l))
            .collect();
        if got != expected {
            failures.push(format!("{:?}: expected {expected:?}, got {got:?}", lang));
        }
        report.push_str(&format!("{:?}: {:?}\n", lang, got));
    }

    assert!(
        failures.is_empty(),
        "A fix for the Bash/Haskell/Scala/YAML extraction gap must not change \
         behaviour for the languages that work today.\n\nMeasured:\n{report}\n\
         Divergences:\n{}",
        failures.join("\n")
    );
}
