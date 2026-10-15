//! Integration tests for refactoring operations (Extract, Inline, Move, Change Signature)
//!
//! These tests verify:
//! - Extract function refactoring
//! - Inline function refactoring
//! - Move symbol refactoring
//! - Change signature refactoring
//! - Parameter substitution with word boundaries
//! - Complex control flow detection
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)

use cognicode::application::error::AppError;
use cognicode::application::services::refactor_service::RefactorService;
use cognicode::infrastructure::parser::{Language, TreeSitterParser};
use cognicode::infrastructure::refactor::ExtractStrategy;
use cognicode::infrastructure::refactor::InlineStrategy;
use cognicode::infrastructure::safety::SafetyGate;
use std::io::Write;
use std::sync::Arc;
use tempfile::NamedTempFile;

/// Helper to create an ExtractStrategy for testing
fn create_extract_strategy(language: Language) -> ExtractStrategy {
    let parser = TreeSitterParser::new(language).unwrap();
    let safety_gate = SafetyGate::new();
    ExtractStrategy::new(Arc::new(parser), safety_gate)
}

/// Helper to create an InlineStrategy for testing
fn create_inline_strategy(language: Language) -> InlineStrategy {
    let parser = TreeSitterParser::new(language).unwrap();
    let safety_gate = SafetyGate::new();
    InlineStrategy::new(Arc::new(parser), safety_gate)
}

// ============================================================================
// Extract Function Tests
// ============================================================================

mod extract_tests {
    use super::*;

    #[test]
    fn test_extract_block_to_function_rust() {
        let strategy = create_extract_strategy(Language::Rust);

        let source = r#"
fn process_order(order_id: i32, items: Vec<f64>) {
    let total = items.iter().sum::<f64>();
    let tax = total * 0.1;
    let final_total = total + tax;
    save_order(order_id, final_total);
}
"#;

        let blocks = strategy.find_extractable_blocks(source, "test.rs").unwrap();
        // Should find extractable blocks
        assert!(
            !blocks.is_empty() || !blocks.is_empty(),
            "Should find extractable blocks in function body"
        );
    }

    #[test]
    fn test_extract_block_to_function_python() {
        let strategy = create_extract_strategy(Language::Python);

        let source = r#"
def process_order(order_id, items):
    total = sum(items)
    tax = total * 0.1
    final_total = total + tax
    save_order(order_id, final_total)
"#;

        let blocks = strategy.find_extractable_blocks(source, "test.py").unwrap();
        assert!(
            !blocks.is_empty() || !blocks.is_empty(),
            "Should find extractable blocks in Python function"
        );
    }

    #[test]
    fn test_extract_with_parameters() {
        let strategy = create_extract_strategy(Language::Rust);

        let source = r#"
fn calculate_totals(items: Vec<i32>) {
    let sum = items.iter().sum::<i32>();
    let count = items.len() as i32;
    let avg = sum / count;
    println!("Sum: {}, Avg: {}", sum, avg);
}
"#;

        let blocks = strategy.find_extractable_blocks(source, "test.rs").unwrap();
        assert!(!blocks.is_empty(), "Should find blocks with free variables");

        // Find a block with free variables
        let block_with_free_vars = blocks.iter().find(|b| !b.free_variables.is_empty());
        assert!(
            block_with_free_vars.is_some(),
            "Should detect free variables like 'items'"
        );
    }

    #[test]
    fn test_extract_with_return_value() {
        let strategy = create_extract_strategy(Language::Rust);

        let source = r#"
fn get_total(items: Vec<i32>) -> i32 {
    let sum = items.iter().sum();
    let tax = sum / 10;
    sum + tax
}
"#;

        let blocks = strategy.find_extractable_blocks(source, "test.rs").unwrap();
        let block = blocks.first();
        assert!(
            block.map(|b| b.has_return_value).unwrap_or(false),
            "Should detect return value"
        );
    }

    #[test]
    fn test_generate_function_call() {
        let strategy = create_extract_strategy(Language::Rust);

        let call = strategy.generate_function_call(
            "calculate_tax",
            &["total".to_string(), "rate".to_string()],
            Some("tax"),
        );

        assert!(call.contains("calculate_tax"));
        assert!(call.contains("total"));
        assert!(call.contains("tax"));
    }

    #[test]
    fn test_generate_function_snippet() {
        let strategy = create_extract_strategy(Language::Rust);

        let snippet = strategy.generate_function_snippet(
            "my_function",
            &["x".to_string(), "y".to_string()],
            "x + y",
            true,
            None,
        );

        assert!(snippet.contains("fn my_function"));
        assert!(snippet.contains("x"));
        assert!(snippet.contains("y"));
    }
}

// ============================================================================
// Inline Function Tests
// ============================================================================

mod inline_tests {
    use super::*;

    #[test]
    fn test_find_function_definition() {
        let strategy = create_inline_strategy(Language::Rust);

        let source = r#"
fn calculate_total(items: &[i32]) -> i32 {
    items.iter().sum()
}
"#;

        let result = strategy
            .find_function_definition(source, "calculate_total")
            .unwrap();
        assert!(result.is_some(), "Should find function definition");

        let func_def = result.unwrap();
        assert_eq!(func_def.name, "calculate_total");
        assert!(func_def.body.is_some());
    }

    #[test]
    fn test_find_call_sites() {
        let strategy = create_inline_strategy(Language::Rust);

        let source = r#"
fn calculate_total(items: &[i32]) -> i32 {
    items.iter().sum()
}

fn main() {
    let nums = vec![1, 2, 3];
    let total = calculate_total(&nums);
    let other = calculate_total(&[1, 2]);
}
"#;

        let call_sites = strategy.find_call_sites(source, "calculate_total").unwrap();
        assert!(call_sites.len() >= 2, "Should find at least 2 call sites");
    }

    #[test]
    fn test_substitute_arguments_word_boundaries() {
        let strategy = create_inline_strategy(Language::Rust);

        // Test that substitution respects word boundaries
        let body = "let sum = a + b; sum";
        let params = vec!["a".to_string(), "b".to_string()];
        let args = vec!["x".to_string(), "y".to_string()];

        let result = strategy.substitute_arguments(body, &params, &args);

        // Should substitute a->x and b->y
        assert!(result.contains("x + y"), "Should substitute arguments");
        // Should NOT substitute within other words
        assert!(
            !result.contains("xa") || !result.contains("yb"),
            "Should use word boundaries"
        );
    }

    #[test]
    fn test_substitute_arguments_no_partial_replacement() {
        let strategy = create_inline_strategy(Language::Rust);

        // Test that we don't do partial replacements
        let body = "let array_index = 5;";
        let params = vec!["index".to_string()];
        let args = vec!["i".to_string()];

        let result = strategy.substitute_arguments(body, &params, &args);

        // "array_index" should NOT become "array_i" (index should NOT be replaced inside array_index)
        // Note: "array_i" is a substring of "array_index", so we check that "array_index" is preserved
        assert!(
            result.contains("array_index"),
            "Should not do partial replacement - array_index should be preserved"
        );
    }

    #[test]
    fn test_is_recursive() {
        let strategy = create_inline_strategy(Language::Rust);

        let source = r#"
fn factorial(n: u32) -> u32 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}
"#;

        let is_recursive = strategy.is_recursive(source, "factorial").unwrap();
        assert!(is_recursive, "factorial should be detected as recursive");
    }

    #[test]
    fn test_inline_simple_helper() {
        let strategy = create_inline_strategy(Language::Rust);

        let source = r#"
fn is_valid_email(email: &str) -> bool {
    email.contains('@') && email.contains('.')
}

fn register_user(email: String) -> Result<User, Error> {
    if !is_valid_email(&email) {
        return Err(Error::InvalidEmail);
    }
    Ok(User { email })
}
"#;

        let func_def = strategy
            .find_function_definition(source, "is_valid_email")
            .unwrap();
        assert!(func_def.is_some(), "Should find function definition");

        let call_sites = strategy.find_call_sites(source, "is_valid_email").unwrap();
        assert!(!call_sites.is_empty(), "Should find call sites");
    }

    #[test]
    fn test_has_complex_control_flow_loops() {
        let strategy = create_inline_strategy(Language::Rust);

        let body_with_loop = r#"
for i in 0..10 {
    sum += i;
}
return sum;
"#;

        assert!(
            strategy.has_complex_control_flow(body_with_loop),
            "Should detect loops as complex control flow"
        );
    }

    #[test]
    fn test_has_complex_control_flow_multiple_returns() {
        let strategy = create_inline_strategy(Language::Rust);

        let body_with_multiple_returns = r#"
if x > 0 {
    return x;
}
return 0;
"#;

        assert!(
            strategy.has_complex_control_flow(body_with_multiple_returns),
            "Should detect multiple returns as complex control flow"
        );
    }

    #[test]
    fn test_has_complex_control_flow_nested_conditionals() {
        let strategy = create_inline_strategy(Language::Rust);

        let body_with_many_conditionals = r#"
if a { 
    if b {
        if c {
            return 1;
        }
    }
}
return 0;
"#;

        assert!(
            strategy.has_complex_control_flow(body_with_many_conditionals),
            "Should detect deeply nested conditionals as complex control flow"
        );
    }

    #[test]
    fn test_simple_inline_candidate() {
        let strategy = create_inline_strategy(Language::Rust);

        let simple_body = r#"
let sum = a + b;
sum
"#;

        assert!(
            !strategy.has_complex_control_flow(simple_body),
            "Simple expression should not be considered complex"
        );
    }
}

// ============================================================================
// Integration Tests with RefactorService
// ============================================================================

mod refactor_service_tests {
    use super::*;

    /// `extract_symbol(path, new_name)` looks up **the new name** as if it
    /// were the symbol to extract, so on a file that has never been
    /// called `calculate_tax` it fails with `SymbolNotFound`.
    ///
    /// That is a real defect in the two-argument form — you cannot
    /// extract into a name that does not exist yet — but the sibling
    /// `extract_symbol_with_target(path, target, new_name)` takes the
    /// existing symbol explicitly and works. This test pins what the
    /// two-argument form actually does, so the day someone fixes it
    /// this turns red instead of the fix landing silently.
    ///
    /// (It previously asserted `result.is_ok() || result.is_err()`,
    /// which is true of every `Result` and therefore of every possible
    /// behaviour of this function.)
    #[test]
    fn test_extract_symbol_preview() {
        let service = RefactorService::new();

        let mut file = NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(file, "fn process() {{").unwrap();
        writeln!(file, "    let total = 10;").unwrap();
        writeln!(file, "    let tax = total * 0.1;").unwrap();
        writeln!(file, "    println!(\"{{}}\", tax);").unwrap();
        writeln!(file, "}}").unwrap();

        let result = service.extract_symbol(file.path().to_str().unwrap(), "calculate_tax");

        let err = result.expect_err(
            "extract_symbol must not invent a preview: `calculate_tax` is the \
             NEW name and does not exist in the file yet",
        );
        assert!(
            matches!(err, AppError::SymbolNotFound(_)),
            "expected SymbolNotFound for the not-yet-existing new name, got: {err:?}"
        );
        assert!(
            err.to_string().contains("calculate_tax"),
            "the error must name the symbol it could not find, got: {err}"
        );

        // The three-argument form is the one that takes the existing
        // symbol, and it is what a caller should actually use. Same
        // file, real target, real new name — this is the positive half
        // the old tautology was standing in for.
        let preview = service
            .extract_symbol_with_target(file.path().to_str().unwrap(), "process", "calculate_tax")
            .expect("extract_symbol_with_target must produce a preview");
        assert!(
            preview.change_count > 0,
            "extracting `process` must report at least one change: {preview:?}"
        );
    }

    /// Inlining an existing symbol produces a concrete preview: the
    /// symbol is named, the file is named, and the change count covers
    /// the call sites plus the definition that gets removed.
    #[test]
    fn test_inline_symbol_preview() {
        let service = RefactorService::new();

        let mut file = NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(file, "fn helper(x: i32) -> i32 {{ x * 2 }}").unwrap();
        writeln!(file, "fn main() {{ let y = helper(5); }}").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let preview = service
            .inline_symbol(&path, "helper")
            .expect("inlining an existing symbol must produce a preview");

        assert_eq!(
            preview.symbols_affected,
            vec!["helper".to_string()],
            "the preview must name the symbol being inlined"
        );
        assert_eq!(
            preview.files_to_modify,
            vec![path.clone()],
            "the preview must name the file being rewritten"
        );
        // `inline_symbol` computes `change_count = call_sites + 1`
        // (the +1 removes the definition). This file has exactly one
        // call site, so the count must be exactly 2 — a larger value
        // means the call-site walk started matching something it
        // should not.
        assert_eq!(
            preview.change_count, 2,
            "one call site plus the definition removal must be 2 changes: {preview:?}"
        );
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

mod edge_case_tests {
    use super::*;

    #[test]
    fn test_inline_rejects_recursive_function() {
        let strategy = create_inline_strategy(Language::Rust);

        let source = r#"
fn factorial(n: u32) -> u32 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}

fn main() {
    let result = factorial(5);
}
"#;

        let func_def = strategy
            .find_function_definition(source, "factorial")
            .unwrap();
        assert!(func_def.is_some());

        // The validation should detect recursion
        let is_recursive = strategy.is_recursive(source, "factorial").unwrap();
        assert!(is_recursive, "Should detect recursive function");
    }

    /// Overruling the "documents intent" auto-debilitation.
    ///
    /// The old body was `assert!(blocks.len() >= 0)` with a comment
    /// admitting it was always true, carrying an
    /// `#[allow(clippy::absurd_extreme_comparisons)]` (and a
    /// file-level `#![allow(unused_comparisons)]`) to stop the
    /// toolchain saying so. It cost two lint suppressions and bought
    /// nothing: the test could not fail, so it could not document
    /// anything. A reader who saw `assert!(blocks.len() >= 0)` and a
    /// comment saying "empty name is validated at a higher level" had
    /// to take both on faith.
    ///
    /// It is also worth being precise about what the test name claims
    /// versus what the body exercises. `find_extractable_blocks(source,
    /// file_path)` takes no function name at all — the second argument
    /// is the *file* the blocks are attributed to. There is no "empty
    /// function name" reaching this function to reject. So the honest
    /// subject is the contract the function actually has, and that is
    /// what is pinned below: the function finds the function body,
    /// attributes it to the caller's file, and hands back code that is
    /// a real slice of the source it was given. Those are the three
    /// things a caller downstream — `extract_symbol_with_target`, and
    /// through it the `extract_symbol` preview — relies on, and all
    /// three can break silently.
    ///
    /// (Naming note: a one-statement body *is* reported as
    /// extractable. The old comment implied it might not be, which is
    /// a second thing the old test got wrong on top of asserting
    /// nothing.)
    ///
    /// The test name is kept because renaming it would silently retire
    /// a test other people may be grepping for; the doc comment is the
    /// part that has to change, and it just did.
    #[test]
    fn test_extract_rejects_empty_function_name() {
        let strategy = create_extract_strategy(Language::Rust);

        let source = r#"
fn foo() {
    let x = 1;
}
"#;

        let blocks = strategy
            .find_extractable_blocks(source, "test.rs")
            .expect("parsing a one-function file must succeed");

        assert_eq!(
            blocks.len(),
            1,
            "the single function body must be offered as one extractable \
             candidate; got {blocks:?}"
        );
        let block = &blocks[0];

        // The caller names the file; every block must be attributed to
        // that file and not to something the strategy invented.
        assert_eq!(
            (block.range.start().file(), block.range.end().file()),
            ("test.rs", "test.rs"),
            "the block must be attributed to the file the caller passed in"
        );

        // The code handed back has to be a real slice of the source
        // that was parsed. If `analyze_block` ever reconstructs text
        // instead of slicing, this goes off and the preview starts
        // showing edits that do not exist in the file.
        assert!(
            source.contains(&block.code),
            "the returned block code must be a literal slice of the \
             parsed source; got {:?}",
            block.code
        );

        // The variables it binds are what a caller needs in order to
        // synthesise a function signature, so an empty list here means
        // the analysis silently stopped working.
        assert_eq!(
            block.local_variables,
            vec!["x".to_string()],
            "the block must report the variable it binds"
        );

        // A second function is a second block: the walk recurses, it
        // does not stop at the first body it finds.
        let two_functions = r#"
fn foo() {
    let x = 1;
}

fn bar() {
    let y = 2;
}
"#;
        let blocks = strategy
            .find_extractable_blocks(two_functions, "test.rs")
            .expect("parsing a two-function file must succeed");
        assert_eq!(
            blocks.len(),
            2,
            "each function body must be offered separately; got {blocks:?}"
        );
    }
}
