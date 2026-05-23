---
name: rust-testing
description: Use when writing Rust tests for CogniCode rules, fixtures, or any testing work involving Rust code and the #[test_rule] macro.
license: MIT
---

# Rust Testing for CogniCode Rules

## 1. Test Macros

### 1.1 #[test_rule] Macro

Tests declarativos para reglas CogniCode:

```rust
#[cfg(test)]
mod tests {
    use crate::testing::test_rule;
    use super::*;

    #[test_rule(WeakCryptoRule)]
    const CASES: &[(&str, bool)] = &[
        // (code_snippet, should_match)
        ("md5(data)", true),           // FAIL - weak hash detected
        ("sha1(input)", true),         // FAIL - weak hash detected
        ("sha256(data)", true),        // WARN - still considered weak by some
        ("blake3(data)", false),      // PASS - approved hash
        ("sha3_256(data)", false),     // PASS - approved hash
    ];
}
```

### 1.2 Case Format

| Sufijo | Significado | Ejemplo |
|--------|-------------|---------|
| `// FAIL` | El código viola la regla | `"md5(data) // FAIL"` |
| `// PASS` | El código cumple la regla | `"blake3(data) // PASS"` |
| `// WARN` | Advertencia | `"old_api() // WARN"` |

## 2. Test Conventions

### 2.1 File Structure

```
rules/
└── rust/
    └── bugs/
        └── concurrency/
            ├── mod.rs
            ├── s1872_rule.rs      # Rule implementation
            └── s1872_test.rs     # Tests
```

### 2.2 Test Naming

```rust
#[test]
fn test_rule_{rule_id}_{case_name}() { }

#[test]
fn test_s1872_race_condition_basic() { }

#[test]
fn test_s1872_arc_mutex_safe() { }
```

### 2.3 Minimal Test Per Rule

```rust
// Minimum: 2 positive, 2 negative, 1 edge case
const TEST_CASES: &[(&str, bool)] = &[
    // Positives (should match)
    ("let mut x = 0; std::thread::spawn(move || { x += 1; });", true),
    ("let data = vec![1,2,3]; thread::spawn(|| { data.push(42); });", true),

    // Negatives (should not match)
    ("let x = Arc::new(Mutex::new(0)); x.lock().unwrap();", false),
    ("let counter = AtomicI32::new(0); counter.fetch_add(1, Ordering::SeqCst);", false),

    // Edge cases
    ("fn main() { let x = 0; }", false), // Single thread, no race
];
```

## 3. Fixture Patterns

### 3.1 Positive Fixtures

Código que viola la regla:

```rust
// Race condition
fn test_race_condition() {
    let code = r#"
        fn main() {
            let mut counter = 0;
            let handle = std::thread::spawn(move || {
                counter += 1;
            });
            handle.join().unwrap();
        }
    "#;
    let rule = RaceConditionRule {};
    let issues = rule.check(&ctx(code));
    assert!(!issues.is_empty());
}
```

### 3.2 Negative Fixtures

Código que no viola la regla:

```rust
fn test_arc_mutex_safe() {
    let code = r#"
        use std::sync::{Arc, Mutex};
        fn main() {
            let counter = Arc::new(Mutex::new(0));
            let handles: Vec<_> = (0..4).map(|_| {
                let counter = Arc::clone(&counter);
                std::thread::spawn(move || {
                    let mut num = counter.lock().unwrap();
                    *num += 1;
                })
            }).collect();
        }
    "#;
    let rule = RaceConditionRule {};
    let issues = rule.check(&ctx(code));
    assert!(issues.is_empty());
}
```

### 3.3 Edge Cases

```rust
fn test_single_thread_no_race() {
    // No spawn, no race possible
    let code = r#"
        fn main() {
            let mut x = 0;
            x += 1;
        }
    "#;
    let rule = RaceConditionRule {};
    let issues = rule.check(&ctx(code));
    assert!(issues.is_empty());
}

fn test_atomic_no_race() {
    // Atomics are safe
    let code = r#"
        use std::sync::atomic::{AtomicI32, Ordering};
        fn main() {
            let counter = AtomicI32::new(0);
            counter.fetch_add(1, Ordering::SeqCst);
        }
    "#;
    let rule = RaceConditionRule {};
    let issues = rule.check(&ctx(code));
    assert!(issues.is_empty());
}
```

## 4. RuleContext for Testing

### 4.1 Mock Context

```rust
fn ctx(source: &str) -> RuleContext {
    RuleContext::mock(source)
}

impl RuleContext {
    pub fn mock(source: &str) -> Self {
        let language = Language::Rust;
        let tree = parse_rust(source).expect("valid rust");

        RuleContext {
            source,
            ast: &tree,
            symbol_table: None,
            language,
            file_path: Path::new("test.rs"),
        }
    }
}
```

### 4.2 Parsing Helper

```rust
use tree_sitter::Parser;

fn parse_rust(source: &str) -> Result<Tree, Box<dyn Error>> {
    let language = tree_sitter_rust::LANGUAGE.into();
    let mut parser = Parser::new();
    parser.set_language(language)?;
    parser.parse(source, None).map_err(Into::into)
}
```

## 5. Test Organization

### 5.1 Test Module Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{test_rule, assert_issue};

    // Test the rule
    test_rule!(MyRule => [
        // Positive cases
        ("vulnerable_code()", true),
        ("weak_hash(md5)", true),

        // Negative cases
        ("safe_code()", false),
        ("strong_hash(blake3)", false),
    ]);

    // Test specific aspects
    mod metadata {
        use super::*;

        #[test]
        fn test_rule_id() {
            assert_eq!(MyRule.id(), "bug/my-rule");
        }

        #[test]
        fn test_severity() {
            assert_eq!(MyRule.severity(), Severity::Critical);
        }

        #[test]
        fn test_layer() {
            assert_eq!(MyRule.layer(), 1);
        }
    }

    // Test performance
    mod performance {
        use super::*;

        #[test]
        fn test_within_budget() {
            let code = generate_large_code(1000);
            let start = Instant::now();
            MyRule.check(&ctx(&code));
            let elapsed = start.elapsed();
            assert!(elapsed.as_millis() < 3, "Rule exceeded 3ms budget");
        }
    }
}
```

## 6. Benchmark Tests

### 6.1 Performance Fixtures

```rust
#[cfg(test)]
mod benchmarks {
    use super::*;
    use test::Bencher;

    #[bench]
    fn bench_simple_case(b: &mut Bencher) {
        let code = "fn foo() { let x = 0; x += 1; }";
        b.iter(|| MyRule.check(&ctx(code)));
    }

    #[bench]
    fn bench_realistic_code(b: &mut Bencher) {
        let code = generate_realistic_module(500); // 500 lines
        b.iter(|| MyRule.check(&ctx(&code)));
    }
}
```

## 7. Integration Tests

### 7.1 Multi-rule Catalog

```rust
#[test]
fn test_all_rules_registered() {
    let catalog = RuleCatalog::load_all();

    // Verify expected rules are present
    assert!(catalog.find("sec/sql-injection").is_some());
    assert!(catalog.find("bug/race-condition").is_some());
    assert!(catalog.find("style/no-trailing-whitespace").is_some());
}

#[test]
fn test_rules_have_valid_metadata() {
    let catalog = RuleCatalog::load_all();
    for rule in catalog.rules() {
        assert!(!rule.id().is_empty());
        assert!(rule.severity().is_valid());
        assert!(rule.category().is_valid());
    }
}
```

## 8. Test Utilities

### 8.1 RuleContext Builder

```rust
pub struct RuleContextBuilder {
    source: String,
    language: Language,
    file_path: PathBuf,
}

impl RuleContextBuilder {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.to_string(),
            language: Language::Rust,
            file_path: PathBuf::from("test.rs"),
        }
    }

    pub fn language(mut self, lang: Language) -> Self {
        self.language = lang;
        self
    }

    pub fn file_path(mut self, path: &str) -> Self {
        self.file_path = PathBuf::from(path);
        self
    }

    pub fn build(self) -> RuleContext {
        let tree = parse(&self.source, self.language)
            .expect("parse failed");
        RuleContext {
            source: self.source.into(),
            ast: tree,
            symbol_table: None,
            language: self.language,
            file_path: self.file_path,
        }
    }
}
```

### 8.2 Assertion Helpers

```rust
pub fn assert_issue(issues: &[Issue], rule_id: &str) -> &Issue {
    assert!(!issues.is_empty(), "Expected issues for rule {}", rule_id);
    assert_eq!(issues[0].rule_id, rule_id);
    &issues[0]
}

pub fn assert_no_issue(issues: &[Issue], rule_id: &str) {
    assert!(issues.is_empty(), "Unexpected issues for rule {}: {:?}", rule_id, issues);
}
```

---

*Skill para testing de reglas CogniCode*
*Última actualización: 15 de Mayo de 2026*