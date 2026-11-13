//! Tests for InputValidator refactoring with Strategy pattern
//!
//! These tests establish the expected behavior for the Rule trait and Strategy pattern.

use cognicode_core::interface::mcp::security::{
    InputValidator, PathValidationRule, SqlValidationRule, UrlValidationRule, ValidationRule,
};

#[test]
fn test_path_validation_rule_detects_traversal() {
    let rule = PathValidationRule::new();
    let temp_dir = std::env::temp_dir();
    let workspace = temp_dir.join("test_workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    // Path traversal should be rejected
    assert!(
        rule.validate_with_context("../etc/passwd", &workspace)
            .is_err()
    );
    assert!(
        rule.validate_with_context("foo/../../etc/passwd", &workspace)
            .is_err()
    );
    assert!(
        rule.validate_with_context("foo/../bar/../../../etc/passwd", &workspace)
            .is_err()
    );
}

#[test]
fn test_path_validation_rule_allows_safe_paths() {
    let rule = PathValidationRule::new();
    let temp_dir = std::env::temp_dir();
    let workspace = temp_dir.join("test_workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let safe_file = workspace.join("src").join("main.rs");
    std::fs::create_dir_all(safe_file.parent().unwrap()).unwrap();
    std::fs::write(&safe_file, "fn main() {}").unwrap();

    // Safe path within workspace should be allowed
    assert!(
        rule.validate_with_context(safe_file.to_str().unwrap(), &workspace)
            .is_ok()
    );
}

#[test]
fn test_url_validation_rule_detects_mailto() {
    let rule = UrlValidationRule::new();

    // mailto: should be rejected
    assert!(rule.validate("mailto:user@example.com").is_err());
    assert!(rule.validate("javascript:alert(1)").is_err());
    assert!(
        rule.validate("data:text/html,<script>alert(1)</script>")
            .is_err()
    );
}

#[test]
fn test_url_validation_rule_allows_safe_urls() {
    let rule = UrlValidationRule::new();

    // Safe URLs should be allowed
    assert!(rule.validate("https://example.com/path").is_ok());
    assert!(rule.validate("http://localhost:8080/api").is_ok());
    assert!(rule.validate("/relative/path").is_ok());
}

#[test]
fn test_sql_validation_rule_detects_sql_injection() {
    let rule = SqlValidationRule::new();

    // SQL injection patterns should be rejected
    assert!(rule.validate("'; DROP TABLE users;--").is_err());
    assert!(rule.validate("1 OR 1=1").is_err());
    assert!(rule.validate("admin'--").is_err());
    assert!(rule.validate("UNION SELECT * FROM passwords").is_err());
}

#[test]
fn test_sql_validation_rule_allows_safe_queries() {
    let rule = SqlValidationRule::new();

    // Safe SQL should be allowed
    assert!(rule.validate("SELECT * FROM users WHERE id = 1").is_ok());
    assert!(
        rule.validate("INSERT INTO users VALUES ('name', 'email')")
            .is_ok()
    );
}

#[test]
fn test_input_validator_with_strategy_pattern() {
    let mut validator = InputValidator::new();
    validator.add_rule(Box::new(PathValidationRule::new()));
    validator.add_rule(Box::new(UrlValidationRule::new()));
    validator.add_rule(Box::new(SqlValidationRule::new()));

    // Should fail with path traversal
    assert!(validator.validate_input("path", "../etc/passwd").is_err());

    // Should fail with SQL injection
    assert!(
        validator
            .validate_input("sql", "'; DROP TABLE users;")
            .is_err()
    );

    // Should fail with malicious URL
    assert!(
        validator
            .validate_input("url", "javascript:alert(1)")
            .is_err()
    );
}

#[test]
fn test_input_validator_returns_first_error() {
    let mut validator = InputValidator::new();
    validator.add_rule(Box::new(PathValidationRule::new()));
    validator.add_rule(Box::new(UrlValidationRule::new()));

    // Path traversal detected first
    let result = validator.validate_input("path", "../etc/passwd");
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("Path traversal"));
    }
}

#[test]
fn test_rules_travel_with_the_validator_when_it_is_moved() {
    let mut validator = InputValidator::new();
    validator.add_rule(Box::new(PathValidationRule::new()));

    // Moving a value changes its address. The rules belong to the validator, not
    // to the place where it happened to live when the rule was added.
    let moved = Box::new(validator);

    assert!(
        moved.validate_input("path", "../etc/passwd").is_err(),
        "a moved validator must keep the rules it was given: failing open here \
         accepts a traversal path"
    );
}

#[test]
fn test_rules_travel_with_the_validator_when_it_is_cloned() {
    let mut validator = InputValidator::new();
    validator.add_rule(Box::new(SqlValidationRule::new()));

    let cloned = validator.clone();
    drop(validator);

    assert!(
        cloned
            .validate_input("sql", "'; DROP TABLE users;")
            .is_err(),
        "a cloned validator must keep the rules it was given"
    );
}

#[test]
fn test_rules_of_one_validator_do_not_leak_into_another() {
    let mut strict = InputValidator::new();
    strict.add_rule(Box::new(PathValidationRule::new()));
    let lax = InputValidator::new();

    assert!(strict.validate_input("path", "../etc/passwd").is_err());
    assert!(
        lax.validate_input("path", "../etc/passwd").is_ok(),
        "a validator with no rules applies none"
    );
}

#[test]
fn test_validation_rule_trait_object_safety() {
    // Verify that Box<dyn ValidationRule> works correctly
    let rules: Vec<Box<dyn ValidationRule>> = vec![
        Box::new(PathValidationRule::new()),
        Box::new(UrlValidationRule::new()),
        Box::new(SqlValidationRule::new()),
    ];

    for rule in rules {
        // Each rule should be callable
        let result = rule.validate("test input");
        // Should not panic - just checking trait object safety
        let _ = result;
    }
}
