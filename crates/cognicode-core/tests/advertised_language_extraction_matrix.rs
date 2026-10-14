//! Characterisation of the symbol-extraction gap behind the advertised
//! `support_level` claims.
//!
//! `product/languages.json` advertises 18 languages as
//! `support_level: "supported"`. Measuring what the extractor actually returns
//! for a trivial top-level definition in each of them showed **four of them
//! extracting nothing at all**:
//!
//! ```text
//! language     parsed  found                    now
//! Rust         yes     ok        line 0        ok
//! Python       yes     ok        line 0        ok
//! Go           yes     ok        line 2        ok
//! Java         yes     ok        line 1        ok
//! TypeScript   yes     ok        line 0        ok
//! C            yes     ok        line 0        ok
//! Cpp          yes     ok        line 0        ok
//! CSharp       yes     ok        line 1        ok
//! Php          yes     ok        line 1        ok
//! Ruby         yes     ok        line 0        ok
//! Swift        yes     ok        line 0        ok
//! Lua          yes     ok        line 0        ok
//! Scala        yes     MISSING   (extracted []) MISSING
//! Bash         yes     MISSING   (extracted []) FIXED
//! Yaml         yes     MISSING   (extracted []) MISSING
//! Haskell      yes     MISSING   (extracted []) PARTIAL
//! ```
//!
//! So this is not a Haskell defect. Haskell was the fourth language with the
//! same failure, and the causes are distinct:
//!
//! * **Bash** — `function_definition` was found by the grammar, but its name
//!   child is a `word` node and `find_identifier_name` only accepted
//!   `identifier`, `type_identifier`, `name` and `simple_identifier`. The node
//!   matched the filter and the name was then discarded. **Fixed**: the lookup
//!   now reads the grammar's `name` FIELD first, which Bash does expose.
//! * **Haskell** — `function @row11` (`use c = ...`) carries a `name` field, so
//!   the same fix recovered it, and the file went from 1 symbol to 2. `mkAll` is
//!   modelled as a `bind`, which nothing in the filter matches, and the module
//!   nodes expose no `name` field at all. **Still partial.**
//! * **Yaml** — `block_mapping` matches, but YAML has no identifier node kind at
//!   all and no `name` field either; the key is a `flow_node`.
//! * **Scala** — the declared strings `function_declaration` and
//!   `val_declaration` do not appear in a real parse tree at all, so the node
//!   filter never fires.
//!
//! **Why the remaining three still assert broken behaviour.** Fixing them needs
//! a per-language node-mapping decision — which node kinds count as symbols —
//! which is a product call, not a lookup change. Until that decision is written
//! down, pinning today's behaviour is the only honest option, and these tests
//! are written so the fix commit must invert them: every failure message names
//! the language and what is missing.
//!
//! Bash was the exception. Its fix turned out not to need a product decision at
//! all, because the name was present in the tree and lost one step later. That
//! test now asserts the correct behaviour positively.
//!
//! The 12 languages that worked are asserted positively below, so a change that
//! starts matching the wrong node in Rust, Python or C fails here rather than
//! silently altering behaviour nobody reviewed.

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

// --- Bash: fixed, asserted positively --------------------------------------
//
// RED before the fix (commit 518e01c2): this asserted the broken behaviour
// and returned zero symbols. The measurement that showed it was fixable
// without a product decision: `function_definition` carries the name as the
// grammar FIELD `name` (kids are `[word, compound_statement]`), and
// `find_identifier_name` only ever looked at child NODE KINDS, where `word`
// is not on the list. So the name was present in the parse tree and thrown
// away one step later.

#[test]
fn bash_function_definition_is_extracted() {
    let src = "probeBashSymbol() { echo 1; }\n";
    let got = extract(Language::Bash, src);
    assert_eq!(
        got,
        vec![("probeBashSymbol".to_string(), 0u32)],
        "Bash is advertised support_level='supported'. A Bash function \
         definition must extract as its own name on the line where it is \
         declared. Measured: {got:?}. If this regresses, the name lookup \
         stopped honouring the grammar's `name` field."
    );
}

// --- Scala, Yaml, Haskell: still not extracted, still characterized ---------
//
// These three remain broken and are NOT fixed here. Unlike Bash they need a
// per-language node-mapping decision, which is a product call:
//   Scala   the declared `function_declaration` / `val_declaration` do not
//           exist in a real tree, so the filter never fires
//   Yaml     the key is a `flow_node` and there is no identifier node kind;
//           YAML has no `name` field to read either
//   Haskell  `bind` / `signature` do carry a `name` field, but the node
//           types the filter accepts do not include them, so fixing the
//           lookup is not enough
//
// The support-level question for these three is for the operator.

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
fn haskell_bind_form_is_not_extracted() {
    // The fixture is deliberately a `bind` (`name = expr`), not a `function`
    // definition. The name-field fix does not reach this shape: the grammar
    // models `probeHsSymbol = 1` as a `bind` node, and nothing in the filter
    // matches `bind` at all. A `function` definition like `use c = ...` IS
    // recovered; see haskell_symbol_extraction_gap.rs for the measured split.
    let src = "probeHsSymbol :: Int\nprobeHsSymbol = 1\n";
    let got = extract(Language::Haskell, src);
    assert!(
        got.is_empty(),
        "Haskell is advertised support_level='supported' but extracts nothing \
         from its own top-level binding in `bind` form. Today the behaviour \
         under test is that it returns zero symbols, which is a product claim \
         the code does not back. When Haskell support is fixed this test must be \
         inverted to assert that probeHsSymbol is found on line 1. See \
         haskell_symbol_extraction_gap.rs for the finer-grained version of this \
         defect."
    );
}

// --- The twelve that do work, pinned so an over-broad fix cannot pass ------

/// Guards the languages that extract correctly today.
///
/// The name lookup changed under these rows: `find_identifier_name` now reads
/// the grammar's `name` field before falling back to the node-kind whitelist.
/// That fallback has to stay, because C and Cpp nest the name inside a
/// declarator and expose no `name` field. Each row asserts the exact expected
/// output, so a change that starts matching the wrong node in Rust, Python or
/// C fails here rather than silently altering behaviour nobody reviewed.
/// (language, source, expected symbols as (name, line))
type HealthyCase = (Language, &'static str, Vec<(&'static str, u32)>);
#[test]
fn supported_languages_that_extract_correctly_still_do() {
    let cases: Vec<HealthyCase> = vec![
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
