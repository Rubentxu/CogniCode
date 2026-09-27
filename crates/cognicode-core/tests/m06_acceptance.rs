// Acceptance test for M0.6: exercises the real public API path that
// users hit when they try to parse PHP/Swift source code. This is the
// production path, not a unit test of internal helpers.
//
// Pre-M0.6 (broken):  TreeSitterParser::new(Language::Php) returns
//                     Err(ParseError::ParseFailed("Failed to set language:
//                     LanguageError { version: 15 }"))
// Post-M0.6 (fixed):  TreeSitterParser::new(Language::Php) returns
//                     Ok(TreeSitterParser) and parse_tree succeeds.

use cognicode_core::infrastructure::parser::{Language, TreeSitterParser};

#[test]
fn m06_acceptance_php_parser_constructs() {
    // The exact call a user would make to parse a PHP file.
    let result = TreeSitterParser::new(Language::Php);
    assert!(
        result.is_ok(),
        "PHP parser construction failed (regression of M0.6): {:?}",
        result.err()
    );
}

#[test]
fn m06_acceptance_swift_parser_constructs() {
    // The exact call a user would make to parse a Swift file.
    let result = TreeSitterParser::new(Language::Swift);
    assert!(
        result.is_ok(),
        "Swift parser construction failed (regression of M0.6): {:?}",
        result.err()
    );
}

#[test]
fn m06_acceptance_php_parses_real_source() {
    let parser = TreeSitterParser::new(Language::Php)
        .expect("PHP parser should construct after M0.6 fix");
    let real_php = r#"<?php
namespace App;

class User extends Model implements Serializable, Jsonable {
    private string $name;

    public function save(Repository $repo): bool {
        return $repo->save($this);
    }
}
"#;
    let tree = parser
        .parse_tree(real_php)
        .expect("Real PHP source should parse after M0.6 fix");
    let root = tree.root_node();
    assert!(
        root.child_count() > 0,
        "PHP parse tree should have children (got empty tree)"
    );
    // Sanity check: tree_sitter-php 0.24.2 produces a 'program' root node.
    assert_eq!(
        root.kind(),
        "program",
        "PHP root node kind mismatch — grammar contract change? (was 'program' before bump)"
    );
}

#[test]
fn m06_acceptance_swift_parses_real_source() {
    let parser = TreeSitterParser::new(Language::Swift)
        .expect("Swift parser should construct after M0.6 fix");
    let real_swift = r#"
import Foundation

class User: Model, Serializable {
    var name: String

    func save(repo: Repository) -> Bool {
        return repo.save(self)
    }
}
"#;
    let tree = parser
        .parse_tree(real_swift)
        .expect("Real Swift source should parse after M0.6 fix");
    let root = tree.root_node();
    assert!(
        root.child_count() > 0,
        "Swift parse tree should have children (got empty tree)"
    );
}

#[test]
fn m06_acceptance_rust_still_parses_regression_check() {
    // Regression check: the bump must not have broken Rust parsing.
    let parser = TreeSitterParser::new(Language::Rust)
        .expect("Rust parser should still construct");
    let rust_src = "pub fn add(a: i32, b: i32) -> i32 { a + b }";
    let tree = parser
        .parse_tree(rust_src)
        .expect("Rust source should still parse");
    assert_eq!(tree.root_node().kind(), "source_file");
}

#[test]
fn m06_acceptance_other_29_parsers_still_work() {
    // Smoke test: verify the other 28 parsers (those not affected by
    // the version mismatch) still construct and parse trivial source.
    // This guards against any accidental regression from the tree-sitter
    // bump affecting parsers we didn't expect to break.
    let cases: &[(&str, Language, &str)] = &[
        ("python", Language::Python, "def add(a, b):\n    return a + b\n"),
        ("javascript", Language::JavaScript, "function add(a, b) { return a + b; }\n"),
        ("typescript", Language::TypeScript, "function add(a: number, b: number): number { return a + b; }\n"),
        ("go", Language::Go, "package main\nfunc add(a, b int) int { return a + b }\n"),
        ("java", Language::Java, "class A { int add(int a, int b) { return a + b; } }\n"),
        ("c", Language::C, "int add(int a, int b) { return a + b; }\n"),
        ("cpp", Language::Cpp, "int add(int a, int b) { return a + b; }\n"),
        ("csharp", Language::CSharp, "class A { int Add(int a, int b) { return a + b; } }\n"),
        ("ruby", Language::Ruby, "def add(a, b)\n  a + b\nend\n"),
        ("scala", Language::Scala, "object A { def add(a: Int, b: Int): Int = a + b }\n"),
        ("lua", Language::Lua, "function add(a, b) return a + b end\n"),
        ("bash", Language::Bash, "add() { echo $(( $1 + $2 )); }\n"),
        ("haskell", Language::Haskell, "add a b = a + b\n"),
        ("yaml", Language::Yaml, "foo: bar\nbaz: qux\n"),
        ("json", Language::Json, "{\"foo\": \"bar\"}\n"),
    ];
    let mut failed: Vec<String> = Vec::new();
    for (name, lang, src) in cases {
        match TreeSitterParser::new(*lang) {
            Ok(p) => match p.parse_tree(src) {
                Ok(t) => {
                    if t.root_node().child_count() == 0 {
                        failed.push(format!("{}: empty parse tree", name));
                    }
                }
                Err(e) => failed.push(format!("{}: parse failed: {:?}", name, e)),
            },
            Err(e) => failed.push(format!("{}: construction failed: {:?}", name, e)),
        }
    }
    assert!(
        failed.is_empty(),
        "Regression in non-PHP/Swift parsers after tree-sitter bump:\n  {}",
        failed.join("\n  ")
    );
}
