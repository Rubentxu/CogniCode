//! E2E Integration Tests — Incremental Analysis with Dependency Tracking
//!
//! Tests the incremental analysis pipeline with real sandbox fixtures across
//! multiple languages (Rust, JS, Java, Python). Verifies that:
//! 1. First run detects all files as new
//! 2. Second run (after marking analyzed) shows no changes
//! 3. Modified files are detected as changed
//! 4. Dependent files (importers) are also flagged for re-analysis

use std::path::{Path, PathBuf};
use cognicode_quality::AnalysisState;
use cognicode_axiom::rules::types::*;
use cognicode_axiom::rules::RuleRegistry;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::parser::Language;

// ============================================================================
// Helper Functions
// ============================================================================

/// Recursively copy a directory from src to dst
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Get the path to a sandbox fixture, handling missing fixtures gracefully
fn get_fixture_path(fixture_name: &str) -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // manifest_dir = .../crates/cognicode-quality
    // parent() = .../crates
    // parent() = workspace root = .../CogniCode
    let fixture_path = manifest_dir
        .parent() // crates/cognicode-quality -> crates
        .and_then(|p| p.parent()) // crates -> workspace root
        .map(|root| root.join("sandbox/fixtures").join(fixture_name))?;

    if fixture_path.exists() {
        Some(fixture_path)
    } else {
        None
    }
}

/// Collect all files with given extensions from a directory
fn collect_files_by_ext(root: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let walker = walkdir::WalkDir::new(root).into_iter().filter_map(|e| e.ok());
    for entry in walker {
        if entry.path().is_file() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    files.push(entry.path().to_path_buf());
                }
            }
        }
    }
    files
}

// ============================================================================
// Test 1: Rust — Incremental with Dependency Tracking
// ============================================================================

#[test]
fn test_rust_incremental_with_deps() {
    // Skip if fixture not available
    let fixture_path = match get_fixture_path("rust-callgraph") {
        Some(p) => p,
        None => {
            println!("SKIP: rust-callgraph fixture not found");
            return;
        }
    };

    let dir = tempfile::tempdir().unwrap();
    copy_dir_all(&fixture_path, dir.path()).unwrap();

    let project_root = dir.path().to_path_buf();
    let mut state = AnalysisState::load(&project_root);

    // Find all .rs files
    let all_files = collect_files_by_ext(&project_root, &["rs"]);

    if all_files.is_empty() {
        println!("SKIP: No .rs files found in rust-callgraph fixture");
        return;
    }

    println!("Rust fixture has {} .rs files", all_files.len());

    // First analysis: all files should be changed (never analyzed)
    let first = state.find_changed_files(&all_files);
    let first_count = first.len();
    println!("First run: {} files new", first_count);

    // Mark all as analyzed + store imports
    for f in &all_files {
        state.update_file_state(f, 0);
        if let Ok(source) = std::fs::read_to_string(f) {
            let imports = AnalysisState::extract_imports(&source, &f.to_string_lossy());
            state.update_file_imports(&f.to_string_lossy(), &imports);
        }
    }

    // Second analysis: no changes
    let second = state.find_changed_files(&all_files);
    assert_eq!(second.len(), 0, "Second run: no files should have changed");

    // Modify one file (lib.rs or any .rs)
    if let Some(lib_file) = all_files.iter().find(|f| {
        f.to_string_lossy().contains("lib") || f.to_string_lossy().contains("main")
    }) {
        let mut content = std::fs::read_to_string(lib_file).unwrap();
        content.push_str("\n// Modified for test\n");
        std::fs::write(lib_file, content).unwrap();

        // Third analysis with dependency expansion
        let third = state.find_changed_with_dependents(&all_files);
        println!(
            "Third run: {} files changed (including dependents)",
            third.len()
        );

        // The modified file itself should be changed
        assert!(
            third.contains(lib_file),
            "Modified file should be in changed set"
        );

        // If there are files that import lib, they should also be re-analyzed
        let dependent_count = third.len() - 1; // minus the modified file itself
        println!("Dependents also re-analyzed: {}", dependent_count);
    } else if let Some(first_file) = all_files.first() {
        // Fallback: modify first file
        let mut content = std::fs::read_to_string(first_file).unwrap();
        content.push_str("\n// Modified for test\n");
        std::fs::write(first_file, content).unwrap();

        let third = state.find_changed_with_dependents(&all_files);
        println!(
            "Third run: {} files changed (including dependents)",
            third.len()
        );

        assert!(
            third.contains(first_file),
            "Modified file should be in changed set"
        );
    }

    println!("PASS: test_rust_incremental_with_deps");
}

// ============================================================================
// Test 2: JS — Incremental with Dependency Tracking
// ============================================================================

#[test]
fn test_js_incremental_with_deps() {
    // Skip if fixture not available
    let fixture_path = match get_fixture_path("js-analysis") {
        Some(p) => p,
        None => {
            println!("SKIP: js-analysis fixture not found");
            return;
        }
    };

    let dir = tempfile::tempdir().unwrap();
    copy_dir_all(&fixture_path, dir.path()).unwrap();

    let project_root = dir.path().to_path_buf();
    let mut state = AnalysisState::load(&project_root);

    let all_files = collect_files_by_ext(&project_root, &["js", "ts", "jsx", "tsx"]);

    if all_files.is_empty() {
        println!("SKIP: No JS/TS files found in js-analysis fixture");
        return;
    }

    println!("JS fixture has {} JS/TS files", all_files.len());

    // First analysis
    let first = state.find_changed_files(&all_files);
    println!("JS first run: {} files new", first.len());

    // Mark as analyzed
    for f in &all_files {
        state.update_file_state(f, 0);
        if let Ok(source) = std::fs::read_to_string(f) {
            let imports = AnalysisState::extract_imports(&source, &f.to_string_lossy());
            state.update_file_imports(&f.to_string_lossy(), &imports);
        }
    }

    // Second: no changes
    let second = state.find_changed_files(&all_files);
    assert_eq!(second.len(), 0, "JS: no changes expected");

    // Modify a file
    if let Some(js_file) = all_files.first() {
        let mut content = std::fs::read_to_string(js_file).unwrap();
        content.push_str("\n// Modified\n");
        std::fs::write(js_file, content).unwrap();

        let third = state.find_changed_with_dependents(&all_files);
        assert!(
            third.contains(js_file),
            "Modified JS file should be changed"
        );
        println!("JS third run: {} files changed (incl deps)", third.len());
    }

    println!("PASS: test_js_incremental_with_deps");
}

// ============================================================================
// Test 3: Java — Incremental with Dependency Tracking
// ============================================================================

#[test]
fn test_java_incremental_with_deps() {
    // Skip if fixture not available
    let fixture_path = match get_fixture_path("java-sample") {
        Some(p) => p,
        None => {
            println!("SKIP: java-sample fixture not found");
            return;
        }
    };

    let dir = tempfile::tempdir().unwrap();
    copy_dir_all(&fixture_path, dir.path()).unwrap();

    let project_root = dir.path().to_path_buf();
    let mut state = AnalysisState::load(&project_root);

    let all_files = collect_files_by_ext(&project_root, &["java"]);

    if all_files.is_empty() {
        println!("SKIP: No Java files found in java-sample fixture");
        return;
    }

    println!("Java fixture has {} Java files", all_files.len());

    // First analysis
    let first = state.find_changed_files(&all_files);
    println!("Java first run: {} files new", first.len());

    // Mark as analyzed
    for f in &all_files {
        state.update_file_state(f, 0);
        if let Ok(source) = std::fs::read_to_string(f) {
            let imports = AnalysisState::extract_imports(&source, &f.to_string_lossy());
            state.update_file_imports(&f.to_string_lossy(), &imports);
        }
    }

    // Second: no changes
    let second = state.find_changed_files(&all_files);
    assert_eq!(second.len(), 0, "Java: no changes expected");

    // Modify a file
    if let Some(java_file) = all_files.first() {
        let mut content = std::fs::read_to_string(java_file).unwrap();
        content.push_str("\n// Modified for test\n");
        std::fs::write(java_file, content).unwrap();

        let third = state.find_changed_with_dependents(&all_files);
        assert!(
            third.contains(java_file),
            "Modified Java file should be changed"
        );
        println!(
            "Java third run: {} files changed (incl deps)",
            third.len()
        );
    }

    println!("PASS: test_java_incremental_with_deps");
}

// ============================================================================
// Test 4: Python — Incremental with Dependency Tracking
// ============================================================================

#[test]
fn test_python_incremental_with_deps() {
    // Skip if fixture not available
    let fixture_path = match get_fixture_path("python-hello") {
        Some(p) => p,
        None => {
            println!("SKIP: python-hello fixture not found");
            return;
        }
    };

    let dir = tempfile::tempdir().unwrap();
    copy_dir_all(&fixture_path, dir.path()).unwrap();

    let project_root = dir.path().to_path_buf();
    let mut state = AnalysisState::load(&project_root);

    let all_files = collect_files_by_ext(&project_root, &["py"]);

    if all_files.is_empty() {
        println!("SKIP: No Python files found in python-hello fixture");
        return;
    }

    println!("Python fixture has {} Python files", all_files.len());

    // First analysis
    let first = state.find_changed_files(&all_files);
    println!("Python first run: {} files new", first.len());

    // Mark as analyzed
    for f in &all_files {
        state.update_file_state(f, 0);
        if let Ok(source) = std::fs::read_to_string(f) {
            let imports = AnalysisState::extract_imports(&source, &f.to_string_lossy());
            state.update_file_imports(&f.to_string_lossy(), &imports);
        }
    }

    // Second: no changes
    let second = state.find_changed_files(&all_files);
    assert_eq!(second.len(), 0, "Python: no changes expected");

    // Modify a file
    if let Some(py_file) = all_files.first() {
        let mut content = std::fs::read_to_string(py_file).unwrap();
        content.push_str("\n# Modified for test\n");
        std::fs::write(py_file, content).unwrap();

        let third = state.find_changed_with_dependents(&all_files);
        assert!(
            third.contains(py_file),
            "Modified Python file should be changed"
        );
        println!(
            "Python third run: {} files changed (incl deps)",
            third.len()
        );
    }

    println!("PASS: test_python_incremental_with_deps");
}

// ============================================================================
// Test 5: Cross-language — Multi-language Project
// ============================================================================

#[test]
fn test_multi_language_incremental() {
    let dir = tempfile::tempdir().unwrap();

    // Create a project with .rs, .js, .py, .java files
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();

    // Rust
    std::fs::write(
        dir.path().join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("main.rs"),
        "use lib::add;\nfn main() { add(1, 2); }",
    )
    .unwrap();

    // JS
    std::fs::write(
        src_dir.join("app.js"),
        "import { helper } from './utils';\nhelper();",
    )
    .unwrap();
    std::fs::write(src_dir.join("utils.js"), "export function helper() {}").unwrap();

    // Python
    std::fs::write(
        dir.path().join("app.py"),
        "from utils import helper\nhelper()\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("utils.py"),
        "def helper():\n    pass\n",
    )
    .unwrap();

    // Java
    let java_dir = src_dir.join("main").join("java");
    std::fs::create_dir_all(&java_dir).unwrap();
    std::fs::write(
        java_dir.join("Main.java"),
        "public class Main {\n    public static void main(String[] args) {}\n}\n",
    )
    .unwrap();

    let project_root = dir.path().to_path_buf();
    let mut state = AnalysisState::load(&project_root);

    // Collect all code files
    let mut all_files = collect_files_by_ext(&project_root, &["rs", "js", "ts", "py", "java"]);
    // Also include .jsx, .tsx, .jsx if present
    all_files.extend(collect_files_by_ext(&project_root, &["jsx", "tsx"]));

    println!("Multi-lang project has {} files", all_files.len());

    // Verify all are detected as changed first time
    let first = state.find_changed_files(&all_files);
    println!("Multi-lang first run: {} files", first.len());
    assert!(
        !first.is_empty(),
        "Should find at least some files to analyze"
    );

    // Update all + store imports
    for f in &all_files {
        state.update_file_state(f, 0);
        if let Ok(source) = std::fs::read_to_string(f) {
            let imports = AnalysisState::extract_imports(&source, &f.to_string_lossy());
            state.update_file_imports(&f.to_string_lossy(), &imports);
        }
    }

    // Second run: 0 changes
    let second = state.find_changed_files(&all_files);
    assert_eq!(
        second.len(),
        0,
        "No changes after marking all as analyzed"
    );

    // Modify one file and verify it's detected
    if let Some(rust_file) = all_files.iter().find(|f| f.extension().unwrap_or_default() == "rs") {
        let mut content = std::fs::read_to_string(rust_file).unwrap();
        content.push_str("\n// Modified for test\n");
        std::fs::write(rust_file, content).unwrap();

        let third = state.find_changed_with_dependents(&all_files);
        assert!(
            third.contains(rust_file),
            "Modified Rust file should be changed"
        );
        println!(
            "Multi-lang after modify: {} files changed",
            third.len()
        );
    }

    println!("PASS: test_multi_language_incremental");
}

// ============================================================================
// Test 6: Empty Project Handling
// ============================================================================

#[test]
fn test_incremental_empty_project() {
    let dir = tempfile::tempdir().unwrap();
    let state = AnalysisState::load(dir.path());

    let all_files: Vec<PathBuf> = Vec::new();

    let changed = state.find_changed_files(&all_files);
    assert_eq!(changed.len(), 0, "Empty file list should return empty changes");

    println!("PASS: test_incremental_empty_project");
}

// ============================================================================
// Test 7: Single File Project
// ============================================================================

#[test]
fn test_incremental_single_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("solo.rs");
    std::fs::write(&file, "fn main() {}").unwrap();

    let mut state = AnalysisState::load(dir.path());

    // First run: file is new
    let first = state.find_changed_files(&[file.clone()]);
    assert_eq!(first.len(), 1, "Single new file should be detected");

    // Mark as analyzed
    state.update_file_state(&file, 0);
    if let Ok(source) = std::fs::read_to_string(&file) {
        let imports = AnalysisState::extract_imports(&source, &file.to_string_lossy());
        state.update_file_imports(&file.to_string_lossy(), &imports);
    }

    // Second run: no changes
    let second = state.find_changed_files(&[file.clone()]);
    assert_eq!(second.len(), 0, "No changes after marking analyzed");

    // Modify file
    std::fs::write(&file, "fn main() {}\n// Modified").unwrap();

    let third = state.find_changed_with_dependents(&[file.clone()]);
    assert!(
        third.contains(&file),
        "Modified file should be in changed set"
    );

    println!("PASS: test_incremental_single_file");
}

// ============================================================================
// Test 8: Verify Dependency Graph - Files that DON'T import changed file
// ============================================================================

#[test]
fn test_no_false_positives_on_dependents() {
    let dir = tempfile::tempdir().unwrap();

    // Create 3 files: lib.rs, main.rs (imports lib), utils.rs (standalone)
    let lib = dir.path().join("src/lib.rs");
    let main = dir.path().join("src/main.rs");
    let utils = dir.path().join("src/utils.rs");

    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(&lib, "pub struct User;").unwrap();
    std::fs::write(&main, "use crate::User;\nfn main() {}").unwrap();
    std::fs::write(&utils, "pub fn helper() {}").unwrap();

    let mut state = AnalysisState::load(dir.path());

    // First run: all new
    let all = vec![lib.clone(), main.clone(), utils.clone()];
    let first = state.find_changed_files(&all);
    assert_eq!(first.len(), 3, "First run: all 3 files new");

    // Mark all as analyzed, with imports
    state.update_file_state(&lib, 0);
    state.update_file_state(&main, 0);
    state.update_file_state(&utils, 0);
    state.update_file_imports("src/main.rs", &["src/lib.rs".to_string()]);

    // Second run: no changes
    let second = state.find_changed_files(&all);
    assert_eq!(second.len(), 0, "Second run: no changes");

    // Modify lib.rs
    std::fs::write(&lib, "pub struct User { pub name: String }").unwrap();

    // Third run: lib changed, main should also be flagged, but utils should NOT
    let third = state.find_changed_with_dependents(&all);

    assert!(
        third.contains(&lib),
        "lib.rs should be changed"
    );
    assert!(
        third.contains(&main),
        "main.rs should be re-analyzed (depends on lib.rs)"
    );
    assert!(
        !third.contains(&utils),
        "utils.rs should NOT be re-analyzed (independent file)"
    );

    println!("PASS: test_no_false_positives_on_dependents");
}

// ============================================================================
// Test 9: Full Pipeline — No Panics on Real Projects (Rust)
// ============================================================================

#[test]
fn test_full_pipeline_rust_no_panics() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir_all("../../sandbox/fixtures/rust-callgraph", dir.path()).unwrap();

    let runner = cognicode_axiom::rules::RuleRegistry::discover();
    let project_root = dir.path().to_path_buf();

    let mut all_files = Vec::new();
    for entry in walkdir::WalkDir::new(&project_root) {
        let entry = entry.unwrap();
        if entry.path().extension().map(|e| e == "rs").unwrap_or(false) {
            all_files.push(entry.path().to_path_buf());
        }
    }

    println!("Analyzing {} Rust files with {} rules...", all_files.len(), runner.all().len());

    let mut total_issues = 0;
    let mut files_analyzed = 0;
    let start = std::time::Instant::now();

    for file in &all_files {
        let source = std::fs::read_to_string(file).unwrap();
        let ext = file.extension().and_then(|e| e.to_str());
        let language = Language::from_extension(ext.map(|e| std::ffi::OsStr::new(e))).unwrap_or(Language::Rust);

        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(&language.to_ts_language()).is_ok() {
            if let Some(tree) = parser.parse(&source, None) {
                let graph = CallGraph::default();
                let metrics = FileMetrics::default();
                let ctx = RuleContext {
                    tree: &tree, source: &source, file_path: file,
                    language: &language, graph: &graph, metrics: &metrics,
                };

                let lang_name = &language.name().to_lowercase();
                let rules = runner.for_language(lang_name);

                let mut file_issues = 0;
                for rule in &rules {
                    let issues = rule.check(&ctx);
                    file_issues += issues.len();
                }
                total_issues += file_issues;
                files_analyzed += 1;
            }
        }
    }

    let elapsed = start.elapsed();
    println!("Rust: {} files, {} issues, {:?}", files_analyzed, total_issues, elapsed);
    // Just verify no panics — any issue count is fine
}

// ============================================================================
// Test 10: Full Pipeline — No Panics on JS
// ============================================================================

#[test]
fn test_full_pipeline_js_no_panics() {
    let dir = tempfile::tempdir().unwrap();
    // Use a simpler fixture without node_modules
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();

    // Create a realistic JS file with various patterns
    std::fs::write(src_dir.join("app.js"), r#"
import React from 'react';
import { helper } from './utils';

function App() {
    const [count, setCount] = React.useState(0);
    var oldStyle = "hello";
    let password = "secret123";

    useEffect(() => {
        document.getElementById('root').innerHTML = '<p>' + count + '</p>';
    });

    debugger;
    eval("alert('xss')");

    return React.createElement('div', null, 'Hello');
}
"#).unwrap();

    std::fs::write(src_dir.join("utils.js"), r#"
export function helper(x) {
    if (x == null) return 0;
    return x * 2;
}
"#).unwrap();

    let runner = cognicode_axiom::rules::RuleRegistry::discover();

    let all_files: Vec<PathBuf> = walkdir::WalkDir::new(dir.path())
        .into_iter().filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|e| e == "js").unwrap_or(false))
        .map(|e| e.path().to_path_buf()).collect();

    println!("JS files: {}, rules: {}", all_files.len(), runner.for_language("javascript").len());

    let mut total_issues = 0;
    for file in &all_files {
        if let Ok(source) = std::fs::read_to_string(file) {
            let lang = Language::JavaScript;
            let mut parser = tree_sitter::Parser::new();
            if parser.set_language(&lang.to_ts_language()).is_ok() {
                if let Some(tree) = parser.parse(&source, None) {
                    let ctx = RuleContext {
                        tree: &tree, source: &source, file_path: file,
                        language: &lang, graph: &CallGraph::default(),
                        metrics: &FileMetrics::default(),
                    };
                    for rule in runner.for_language("javascript") {
                        let issues = rule.check(&ctx);
                        if !issues.is_empty() {
                            println!("  {}: {} issues", rule.id(), issues.len());
                            total_issues += issues.len();
                        }
                    }
                }
            }
        }
    }
    println!("JS total issues: {}", total_issues);
    // Verify we found SOME issues (the fixture has intentional smells)
    assert!(total_issues > 0, "Should detect issues in smelly JS code");
}

// ============================================================================
// Test 11: Full Pipeline — No Panics on Python
// ============================================================================

#[test]
fn test_full_pipeline_python_no_panics() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("app.py"), r#"
import os
from flask import Flask, request

app = Flask(__name__)
password = "admin123"

@app.route('/login', methods=['POST'])
def login():
    user = request.form['username']
    query = f"SELECT * FROM users WHERE name = '{user}'"
    eval(user)
    return "ok"
"#).unwrap();

    let runner = cognicode_axiom::rules::RuleRegistry::discover();
    let files: Vec<PathBuf> = walkdir::WalkDir::new(dir.path())
        .into_iter().filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|e| e == "py").unwrap_or(false))
        .map(|e| e.path().to_path_buf()).collect();

    let mut total = 0;
    for f in &files {
        if let Ok(source) = std::fs::read_to_string(f) {
            let lang = Language::Python;
            let mut parser = tree_sitter::Parser::new();
            parser.set_language(&lang.to_ts_language()).unwrap();
            if let Some(tree) = parser.parse(&source, None) {
                let ctx = RuleContext { tree: &tree, source: &source, file_path: f, language: &lang, graph: &CallGraph::default(), metrics: &FileMetrics::default() };
                for rule in runner.for_language("python") {
                    total += rule.check(&ctx).len();
                }
            }
        }
    }
    println!("Python: {} files, {} issues", files.len(), total);
    assert!(total > 0, "Should detect issues in smelly Python code");
}

// ============================================================================
// Test 12: Full Pipeline — No Panics on Java
// ============================================================================

#[test]
fn test_full_pipeline_java_no_panics() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Smelly.java"), r#"
public class Smelly {
    private String password = "secret123";

    public void doStuff() {
        System.out.println("debug");
        try {
            String query = "SELECT * FROM users WHERE id = " + userId;
            stmt.executeQuery(query);
        } catch (Exception e) {
        }
    }
}
"#).unwrap();

    let runner = cognicode_axiom::rules::RuleRegistry::discover();
    let files: Vec<PathBuf> = walkdir::WalkDir::new(dir.path())
        .into_iter().filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|e| e == "java").unwrap_or(false))
        .map(|e| e.path().to_path_buf()).collect();

    let mut total = 0;
    for f in &files {
        if let Ok(source) = std::fs::read_to_string(f) {
            let lang = Language::Java;
            let mut parser = tree_sitter::Parser::new();
            if parser.set_language(&lang.to_ts_language()).is_ok() {
                if let Some(tree) = parser.parse(&source, None) {
                    let ctx = RuleContext { tree: &tree, source: &source, file_path: f, language: &lang, graph: &CallGraph::default(), metrics: &FileMetrics::default() };
                    for rule in runner.for_language("java") {
                        total += rule.check(&ctx).len();
                    }
                }
            }
        }
    }
    println!("Java: {} files, {} issues", files.len(), total);
    assert!(total > 0, "Should detect issues in smelly Java code");
}

// ============================================================================
// Test 13: Incremental Full Pipeline with Caching
// ============================================================================

#[test]
fn test_incremental_full_pipeline_cache_works() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.rs");
    std::fs::write(&file, "pub fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();

    let project_root = dir.path().to_path_buf();
    let mut state = AnalysisState::load(&project_root);

    let all_files = vec![file.clone()];

    // First run: analyze
    let first = state.find_changed_files(&all_files);
    assert_eq!(first.len(), 1);
    state.update_file_state(&file, 0);

    // Second run: should be cached
    let second_start = std::time::Instant::now();
    let second = state.find_changed_files(&all_files);
    let second_elapsed = second_start.elapsed();
    assert_eq!(second.len(), 0);
    println!("Cache lookup: {:?}", second_elapsed);

    // Verify the SQLite file exists
    let db_path = project_root.join(".cognicode/cognicode.db");
    assert!(db_path.exists(), "SQLite DB should exist after analysis");
}
