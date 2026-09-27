// walker-grammar-drift acceptance test (M0.10 end-to-end).
//
// Pine que el camino completo del parser extrae symbols para PHP y
// Swift — exactamente el gap que N+10 identificó como consecuencia
// del walker-grammar-drift. Estos tests ejercitan la API pública
// real (TreeSitterParser::find_all_symbols_with_path) que es la
// que usa `cognicode analyze` end-to-end.
//
// Pre-M0.10:
//   - PHP `function_node_type()` devolvía `"method_declaration"`,
//     pero el grammar emite `function_definition` para funciones
//     globales. Por eso `find_all_symbols_with_path` retornaba
//     `Vec::new()` para `<?php function f() {}`.
//   - Idem para Swift (`function_declaration` vs `method_declaration`).
//   - Idem para PHP `class_node_type` que sí pineaba correctamente.
//   - Esto causaba que `cognicode analyze` reportara
//     `Languages: {}` y `parsed_files=0` para proyectos PHP/Swift.

use cognicode_core::infrastructure::parser::{Language, TreeSitterParser};

#[test]
fn m10_acceptance_php_global_function_extracted_as_symbol() {
    let parser = TreeSitterParser::new(Language::Php).expect("PHP parser constructs");
    let source = "<?php\nfunction save(User $user, Repository $repo): void { }";
    let symbols = parser
        .find_all_symbols_with_path(source, "/tmp/test.php")
        .expect("should extract symbols from PHP function");
    assert!(
        !symbols.is_empty(),
        "PHP global function should yield ≥1 symbol (got {}); grammar emits `function_definition`, not `method_declaration`",
        symbols.len()
    );
    let names: Vec<_> = symbols.iter().map(|s| s.name().to_string()).collect();
    assert!(
        names.iter().any(|n| n == "save"),
        "should find 'save' function in {:?}",
        names
    );
}

#[test]
fn m10_acceptance_php_class_with_extends_implements_extracted_as_symbol() {
    let parser = TreeSitterParser::new(Language::Php).expect("PHP parser constructs");
    let source = "<?php\nclass User extends Model implements Serializable { }";
    let symbols = parser
        .find_all_symbols_with_path(source, "/tmp/user.php")
        .expect("should extract symbols from PHP class");
    assert!(
        !symbols.is_empty(),
        "PHP class should yield ≥1 symbol (got {})",
        symbols.len()
    );
    let names: Vec<_> = symbols.iter().map(|s| s.name().to_string()).collect();
    assert!(
        names.iter().any(|n| n == "User"),
        "should find 'User' class in {:?}",
        names
    );
}

#[test]
fn m10_acceptance_swift_global_function_extracted_as_symbol() {
    let parser = TreeSitterParser::new(Language::Swift).expect("Swift parser constructs");
    let source = "func save(user: User, repo: Repository) -> Error? { return nil }";
    let symbols = parser
        .find_all_symbols_with_path(source, "/tmp/test.swift")
        .expect("should extract symbols from Swift function");
    assert!(
        !symbols.is_empty(),
        "Swift global function should yield ≥1 symbol (got {}); grammar emits `function_declaration`, not `method_declaration`",
        symbols.len()
    );
    let names: Vec<_> = symbols.iter().map(|s| s.name().to_string()).collect();
    assert!(
        names.iter().any(|n| n == "save"),
        "should find 'save' function in {:?}",
        names
    );
}

#[test]
fn m10_acceptance_swift_class_with_inheritance_extracted_as_symbol() {
    let parser = TreeSitterParser::new(Language::Swift).expect("Swift parser constructs");
    let source = "class User: Model, Serializable { }";
    let symbols = parser
        .find_all_symbols_with_path(source, "/tmp/user.swift")
        .expect("should extract symbols from Swift class");
    assert!(
        !symbols.is_empty(),
        "Swift class should yield ≥1 symbol (got {})",
        symbols.len()
    );
    let names: Vec<_> = symbols.iter().map(|s| s.name().to_string()).collect();
    assert!(
        names.iter().any(|n| n == "User"),
        "should find 'User' class in {:?}",
        names
    );
}

#[test]
fn m10_acceptance_php_method_node_type_corrected() {
    // After the M0.10 fix, PHP's `function_node_type` returns
    // `function_definition` (the kind emitted by tree-sitter-php 0.24.2
    // for free functions). This test pins the corrected value so
    // regressions are caught early.
    assert_eq!(
        Language::Php.function_node_type(),
        "function_definition",
        "PHP global function node kind"
    );
}

#[test]
fn m10_acceptance_swift_function_node_type_corrected() {
    assert_eq!(
        Language::Swift.function_node_type(),
        "function_declaration",
        "Swift global function node kind"
    );
}
