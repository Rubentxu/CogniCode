//! End-to-end integration tests for CogniCode
//!
//! These tests verify the complete flow from MCP request to code analysis,
//! exercising the full stack including tree-sitter parsing, graph building,
//! and refactoring operations.

use cognicode::application::services::analysis_service::AnalysisService;
use cognicode::application::services::refactor_service::RefactorService;
use cognicode::infrastructure::parser::{Language, TreeSitterParser};
use std::io::Write;
use tempfile::{NamedTempFile, TempDir};

/// Tests parsing a Python file and extracting symbols
#[test]
fn test_e2e_python_parse_and_symbol_extraction() {
    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(file, "def hello():").unwrap();
    writeln!(file, "    print('Hello, world!')").unwrap();
    writeln!(file).unwrap();
    writeln!(file, "class MyClass:").unwrap();
    writeln!(file, "    def __init__(self):").unwrap();
    writeln!(file, "        self.value = 42").unwrap();
    writeln!(file, "    def get_value(self):").unwrap();
    writeln!(file, "        return self.value").unwrap();

    let service = AnalysisService::new();
    let symbols = service.get_file_symbols(file.path()).unwrap();

    // Should find hello function and MyClass
    let names: Vec<_> = symbols.iter().map(|s| s.name.clone()).collect();
    assert!(
        names.contains(&"hello".to_string()),
        "Should find hello function"
    );
    assert!(
        names.contains(&"MyClass".to_string()),
        "Should find MyClass"
    );
    assert!(
        names.contains(&"__init__".to_string()),
        "Should find __init__ method"
    );
    assert!(
        names.contains(&"get_value".to_string()),
        "Should find get_value method"
    );
}

/// Tests parsing a Rust file and extracting symbols
#[test]
fn test_e2e_rust_parse_and_symbol_extraction() {
    let mut file = NamedTempFile::with_suffix(".rs").unwrap();
    writeln!(file, "struct Person {{").unwrap();
    writeln!(file, "    name: String,").unwrap();
    writeln!(file, "    age: u32,").unwrap();
    writeln!(file, "}}").unwrap();
    writeln!(file).unwrap();
    writeln!(file, "impl Person {{").unwrap();
    writeln!(file, "    fn new(name: String, age: u32) -> Self {{").unwrap();
    writeln!(file, "        Person {{ name, age }}").unwrap();
    writeln!(file, "    }}").unwrap();
    writeln!(file, "}}").unwrap();

    let service = AnalysisService::new();
    let symbols = service.get_file_symbols(file.path()).unwrap();

    let names: Vec<_> = symbols.iter().map(|s| s.name.clone()).collect();
    assert!(
        names.contains(&"Person".to_string()),
        "Should find Person struct"
    );
    assert!(
        names.contains(&"new".to_string()),
        "Should find new function"
    );
}

/// Tests parsing JavaScript and extracting symbols
#[test]
fn test_e2e_javascript_parse_and_symbol_extraction() {
    let mut file = NamedTempFile::with_suffix(".js").unwrap();
    writeln!(file, "function greet(name) {{").unwrap();
    writeln!(file, "    console.log('Hello, ' + name);").unwrap();
    writeln!(file, "}}").unwrap();
    writeln!(file).unwrap();
    writeln!(file, "class User {{").unwrap();
    writeln!(file, "    constructor(name) {{").unwrap();
    writeln!(file, "        this.name = name;").unwrap();
    writeln!(file, "}}").unwrap();

    let service = AnalysisService::new();
    let symbols = service.get_file_symbols(file.path()).unwrap();

    let names: Vec<_> = symbols.iter().map(|s| s.name.clone()).collect();
    assert!(
        names.contains(&"greet".to_string()),
        "Should find greet function"
    );
    assert!(
        names.contains(&"User".to_string()),
        "Should find User class"
    );
}

/// Tests building a project graph with call relationships
#[test]
fn test_e2e_build_project_graph_with_calls() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create first Python file with calls
    let file1_path = temp_path.join("module1.py");
    std::fs::write(
        &file1_path,
        r#"
def a():
    b()
    c()

def b():
    c()

def c():
    pass
"#,
    )
    .unwrap();

    // Create second Python file with cross-module calls
    let file2_path = temp_path.join("module2.py");
    std::fs::write(
        &file2_path,
        r#"
def d():
    a()
    c()
"#,
    )
    .unwrap();

    let service = AnalysisService::new();
    service.build_project_graph(temp_path).unwrap();

    let graph = service.get_project_graph();

    // The graph should have symbols from both files
    assert!(
        graph.symbol_count() >= 4,
        "Should have at least 4 symbols (a, b, c, d)"
    );
}

/// Tests rename refactoring generates correct edits
#[test]
fn test_e2e_rename_refactoring() {
    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(file, "def old_function():").unwrap();
    writeln!(file, "    old_function()").unwrap();
    writeln!(file, "    old_function()").unwrap();

    let service = RefactorService::new();
    let edits = service
        .generate_rename_edits(
            file.path().to_str().unwrap(),
            "old_function",
            "new_function",
        )
        .unwrap();

    // Should generate 3 edits (definition + 2 calls)
    assert_eq!(edits.len(), 3, "Should generate 3 edits for 3 occurrences");
}

/// Tests VFS syntax validation accepts valid edits
#[test]
fn test_e2e_vfs_syntax_validation_accepts_valid() {
    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(file, "def foo():").unwrap();
    writeln!(file, "    pass").unwrap();

    let service = RefactorService::new();

    let source = std::fs::read_to_string(file.path()).unwrap();
    let valid_edits = vec![cognicode::domain::aggregates::refactor::TextEdit::new(
        cognicode::domain::value_objects::SourceRange::new(
            cognicode::domain::value_objects::Location::new(file.path().to_str().unwrap(), 0, 4),
            cognicode::domain::value_objects::Location::new(file.path().to_str().unwrap(), 0, 7),
        ),
        "bar", // Rename "foo" to "bar"
    )];

    // The validation should pass because "def bar():" is valid
    let result =
        service.validate_edits_syntax(file.path().to_str().unwrap(), &source, &valid_edits);
    assert!(
        result.is_ok(),
        "Syntax validation should accept valid code: {:?}",
        result
    );
}

/// Tests call relationship detection in Python
#[test]
fn test_e2e_python_call_relationships() {
    let parser = TreeSitterParser::new(Language::Python).unwrap();
    let source = r#"
def caller():
    helper()
    another()
    helper()

def helper():
    pass

def another():
    pass
"#;

    let relationships = parser.find_call_relationships(source, "test.py").unwrap();

    // caller calls helper twice and another once
    let caller_calls: Vec<_> = relationships
        .iter()
        .filter(|(caller, _)| caller.name() == "caller")
        .collect();

    assert_eq!(caller_calls.len(), 3, "caller should have 3 calls");
    let callees: Vec<_> = caller_calls.iter().map(|(_, c)| c.as_str()).collect();
    assert!(callees.contains(&"helper"), "Should call helper");
    assert!(callees.contains(&"another"), "Should call another");
}

/// Tests find_usages with multiple occurrences
#[test]
fn test_e2e_find_usages_multiple_occurrences() {
    let source = r#"
def calculate(x):
    return calculate(x * 2)

def main():
    result = calculate(10)
    print(result)
"#;

    let parser = TreeSitterParser::new(Language::Python).unwrap();
    let occurrences = parser
        .find_all_occurrences_of_identifier(source, "calculate")
        .unwrap();

    // Should find: definition, recursive call, and call in main = 3 total
    assert_eq!(
        occurrences.len(),
        3,
        "Should find 3 occurrences of 'calculate'"
    );
}

/// Tests that unsupported file types are rejected gracefully
#[test]
fn test_e2e_unsupported_file_type() {
    let mut file = NamedTempFile::with_suffix(".xyz").unwrap();
    writeln!(file, "some random content").unwrap();

    let service = AnalysisService::new();
    let result = service.get_file_symbols(file.path());

    assert!(result.is_err(), "Should error on unsupported file type");
}

/// Tests full flow: parse -> build graph -> analyze impact
#[test]
fn test_e2e_full_analysis_flow() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create a small project
    let main_path = temp_path.join("main.py");
    std::fs::write(
        &main_path,
        r#"
def helper():
    pass

def main():
    helper()
    helper()
"#,
    )
    .unwrap();

    // Step 1: Parse symbols
    let service = AnalysisService::new();
    let symbols = service.get_file_symbols(&main_path).unwrap();
    assert!(!symbols.is_empty(), "Should extract symbols");

    // Step 2: Build project graph
    service.build_project_graph(temp_path).unwrap();
    let graph = service.get_project_graph();
    assert!(
        graph.symbol_count() >= 2,
        "Should have at least 2 symbols in graph"
    );
}
