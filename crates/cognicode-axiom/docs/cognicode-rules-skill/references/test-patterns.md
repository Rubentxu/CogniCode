# CogniCode Rule Testing — Patterns & Templates

## Test Infrastructure

All rule tests use the `with_rule_context` helper:

```rust
fn with_rule_context<R>(source: &str, language: Language, f: impl FnOnce(&RuleContext) -> R) -> R {
    let tree = TreeSitterParser::parse(source, &language);
    let graph = CallGraph::new();
    let metrics = FileMetrics::default();
    let ctx = RuleContext {
        tree: &tree,
        source,
        file_path: Path::new("test.rs"),
        language: &language,
        graph: &graph,
        metrics: &metrics,
    };
    f(&ctx)
}
```

## Test Categories

### 1. Detection Tests (Positive)

Must test at least 2 different variants of the pattern:

```rust
#[test]
fn test_sXXXX_detects_variant_1() {
    let source = r#"..."#;  // First way to trigger
    let issues = ...;
    assert!(!issues.is_empty());
}

#[test]
fn test_sXXXX_detects_variant_2() {
    let source = r#"..."#;  // Second way to trigger
    let issues = ...;
    assert!(!issues.is_empty());
}
```

### 2. False Positive Tests (Negative) — MANDATORY

Must test 3 categories:

```rust
// FP Category A: Comment containing the pattern
#[test]
fn test_sXXXX_no_fp_comment() {
    let source = r#"
// This is a comment about password="test" — should NOT trigger
//! Doc comment about DES encryption — should NOT trigger
/// Example: md5("data") — should NOT trigger
fn main() {}
"#;
    let issues = ...;
    assert!(issues.is_empty(), "Rule should NOT trigger on comments");
}

// FP Category B: Pattern inside a variable/function name  
#[test]
fn test_sXXXX_no_fp_identifier() {
    let source = r#"
fn main() {
    let password_hash = argon2("data");
    let description = get_description();
}
"#;
    let issues = ...;
    assert!(issues.is_empty(), "Rule should NOT trigger on identifiers");
}

// FP Category C: Common English words containing the pattern
#[test]
fn test_sXXXX_no_fp_english_word() {
    let source = r#"
//! This module provides functionality — 'provides' contains 'des'
//! Design: the description includes details
fn main() {}
"#;
    let issues = ...;
    assert!(issues.is_empty(), "Rule should NOT trigger on English words");
}
```

### 3. Edge Case Tests

```rust
#[test]
fn test_sXXXX_edge_empty_file() {
    let source = "";
    let issues = ...;
    assert!(issues.is_empty());
}

#[test]
fn test_sXXXX_edge_boundary() {
    let source = r#"fn main() { let x = something_at_threshold; }"#;
    let issues = ...;
    // Check exact boundary behavior
}
```

### 4. Integration Tests

Test multiple rules together on a realistic codebase snippet:

```rust
#[test]
fn test_integration_security_rules() {
    let source = r#"
fn configure_db() {
    let password = "admin123";        // Should trigger S2068
    let hash = md5("data");           // Should trigger S4792
    let query = format!("SELECT *");  // Should trigger S5122
}
fn safe_function() {
    let hash = argon2id("data");      // Should NOT trigger S4792
    let query = conn.query("SELECT ?", &[id]); // Should NOT trigger S5122
}
"#;
    let all_issues = run_all_rules(source, Language::Rust);
    // Verify specific rules triggered
    assert!(all_issues.iter().any(|i| i.rule_id == "S2068"));
    assert!(all_issues.iter().any(|i| i.rule_id == "S4792"));
    assert!(all_issues.iter().any(|i| i.rule_id == "S5122"));
    // Verify no false positives from safe code
    // ...
}
```

## Test Quality Checklist

| Check | Why |
|-------|-----|
| □ Tests use `r#"..."#` raw strings | Cleaner multi-line test data |
| □ Assert messages are descriptive | `"Expected SXXXX to detect XYZ"` |
| □ Each test has a single assertion focus | Easier debugging |
| □ FP tests include actual source of FP | Like real-world `provides` case |
| □ Test covers all regex alternatives in the rule | Each `\|` branch in the pattern |
| □ Properties test uses `assert_eq!` not just `assert!` | Exact value checking |
