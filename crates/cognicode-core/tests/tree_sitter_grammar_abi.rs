//! Test: which tree-sitter grammars fail `set_language` on this core?
//!
//! `TreeSitterParser::new(language)` fails when the grammar's ABI version is
//! outside tree-sitter 0.27's supported range, or when the grammar is not
//! parseable. `LightweightIndex::build_index` handles that failure with:
//!
//! ```ignore
//! Err(_) => TreeSitterParser::new(Language::Rust).unwrap()
//! ```
//!
//! i.e. it silently substitutes the Rust grammar for whatever language failed.
//! This test reports which languages actually take that branch, so the
//! substitution is a measured fact rather than a hypothesis.

use cognicode_core::domain::value_objects::Language;
use cognicode_core::infrastructure::parser::TreeSitterParser;

#[test]
fn report_which_grammars_fail_to_load() {
    let languages = [
        Language::Python,
        Language::Rust,
        Language::JavaScript,
        Language::TypeScript,
        Language::Go,
        Language::Java,
        Language::C,
        Language::Cpp,
        Language::CSharp,
        Language::Hcl,
        Language::Yaml,
        Language::Ruby,
        Language::Php,
        Language::Swift,
        Language::Scala,
        Language::Lua,
        Language::Zig,
        Language::Dart,
        Language::Groovy,
        Language::Elixir,
        Language::Erlang,
        Language::Haskell,
        Language::Julia,
        Language::Bash,
        Language::R,
        Language::PowerShell,
        Language::Json,
        Language::Fortran,
        Language::Verilog,
        Language::SystemVerilog,
    ];

    let mut ok = Vec::new();
    let mut failed = Vec::new();

    for lang in languages {
        match TreeSitterParser::new(lang) {
            Ok(p) => {
                // Prove the parser actually produces a tree, not just that the
                // pointer was accepted. A grammar that loads but cannot parse
                // is the same silent-wrong-language class of defect.
                let tree = p.parse_tree("fn main() {}");
                if tree.is_err() {
                    failed.push((format!("{lang:?}"), "parses to Err".to_string()));
                } else {
                    ok.push(format!("{lang:?}"));
                }
            }
            Err(e) => failed.push((format!("{lang:?}"), format!("{e}"))),
        }
    }

    println!("\n=== grammars that load and parse: {} ===", ok.len());
    for l in &ok {
        println!("  ok      {l}");
    }
    println!("\n=== grammars that FAIL (these get Rust substituted) ===");
    for (l, e) in &failed {
        println!("  FAILED  {l:<14} {e}");
    }
    println!("\nfailures: {}", failed.len());

    assert!(
        failed.is_empty(),
        "{} grammar(s) fail to load on tree-sitter 0.27, so \
         LightweightIndex::build_index silently indexes those files with the \
         Rust grammar instead: {failed:?}",
        failed.len()
    );
}
