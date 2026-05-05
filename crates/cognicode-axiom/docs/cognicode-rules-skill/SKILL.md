---
name: cognicode-rules
description: >
  Master skill for developing, testing, and auditing CogniCode quality rules.
  Covers SonarQube-derived best practices, tree-sitter AST analysis, regex safety,
  test strategies (unit/integration/false-positive), and the full rule lifecycle.
  Trigger: When creating, modifying, testing, or auditing CogniCode detection rules,
  or when working with cognicode-axiom, cognicode-quality, or rule catalogs.
license: Apache-2.0
metadata:
  author: gentleman-programming
  version: "2.0"
---

# CogniCode Rules — Development & Testing Skill

## When to Use

- Creating a **new detection rule** (catalog.rs)
- **Fixing a false positive** in an existing rule
- **Writing tests** for rules (unit, integration, FP prevention)
- **Auditing** rules for correctness or performance
- **Migrating** rules from regex to tree-sitter AST
- **Reviewing** rule PRs for quality

## Architecture Overview

```
cognicode-axiom/src/rules/
├── catalog.rs          ← 854 rules (declare_rule! macro)
├── catalog_tests.rs    ← Manual tests (11 rules, 40 tests)
├── catalog_tests_generated.rs ← Auto-generated tests
├── types.rs            ← RuleContext, Issue, Severity, Category
├── mod.rs              ← RuleRegistry, inventory
├── rules/              ← Separate rule files (style.rs, complexity.rs, etc.)
│   ├── style.rs
│   ├── complexity.rs
│   ├── security.rs
│   └── ...
└── rule_factory.rs     ← Factory pattern for rule creation
```

### RuleContext — Available Data

```rust
pub struct RuleContext<'a> {
    pub tree: &'a tree_sitter::Tree,     // Full AST parse tree
    pub source: &'a str,                 // Raw source code
    pub file_path: &'a Path,             // File being analyzed
    pub language: &'a Language,          // Detected language
    pub graph: &'a CallGraph,            // Project call graph
    pub metrics: &'a FileMetrics,        // Pre-computed file metrics
}
```

## Critical Decision: Regex vs Tree-Sitter

```
╔══════════════════════════════════════════════════════════════╗
║  SIMPLE PATTERNS              COMPLEX STRUCTURAL RULES      ║
║  ──────────────               ─────────────────────────     ║
║  ✅ Use regex                  ✅ Use tree-sitter            ║
║  • TODO/FIXME detection        • SQL injection detection     ║
║  • Hardcoded IPs/creds         • Deep nesting analysis       ║
║  • Naming conventions          • Unused variable detection   ║
║  • Line length                 • Dead code detection         ║
║  • Tab/space checks            • Complexity metrics          ║
║                                                              ║
║  ⚠️  IF using regex:            ⚠️  IF using AST:             ║
║  • Use word boundaries (\b)    • Use tree queries            ║
║  • Skip comment lines           • Check node types            ║
║  • Skip string literals         • Use node ranges             ║
║  • Use (?:\b|_) for prefixes   • Walk children recursively    ║
╚══════════════════════════════════════════════════════════════╝
```

### When to Use Which

| Pattern | Regex | Tree-Sitter | Why |
|---------|-------|-------------|-----|
| `password = "xxx"` | ✅ with FP guards | — | Simple pattern, no structure needed |
| `format!("SELECT...")` | ❌ | ✅ | Needs AST to find `format!` macro + check content |
| `if x { if y { if z {} } }` | ❌ | ✅ | Nesting depth requires tree traversal |
| `let unused_var = ...` | ❌ | ✅ | Needs scope analysis + usage tracking |
| `TODO: fix this` | ✅ | — | Textual pattern, intentionally in comments |
| `http://example.com` | ✅ with FP guards | — | URL pattern, but skip lines with `https://` |

## Regex Safety Protocol (MANDATORY)

### 1. Word Boundaries

```rust
// ❌ WRONG — matches "provides", "design", "destination"
(r"des", "DES block cipher")
(r"sha1", "SHA-1")           // matches "sha1_custom"
(r"rc4", "RC4")              // matches "rc4_decrypt"

// ✅ CORRECT — only matches standalone words or _prefixed identifiers
(r"(?:\b|_)des\b", "DES block cipher")
(r"(?:\b|_)sha1\b", "SHA-1 hash function")
(r"(?:\b|_)rc4\b", "RC4 stream cipher")
```

### 2. Comment Skipping

```rust
// ❌ WRONG — matches patterns in comments
for (line_idx, line) in ctx.source.lines().enumerate() {
    if re.is_match(line) { ... }
}

// ✅ CORRECT — skip comment lines
for (line_idx, line) in ctx.source.lines().enumerate() {
    let trimmed = line.trim();
    if trimmed.starts_with("//") || trimmed.starts_with("///") 
    || trimmed.starts_with("//!") || trimmed.starts_with("#") 
    || trimmed.starts_with("/*") || trimmed.starts_with("*") {
        continue;
    }
    if re.is_match(line) { ... }
}
```

### 3. String Literal Exclusion

```rust
// For security rules (S2068, S4792), also skip lines that are string literals
fn is_inside_string(line: &str) -> bool {
    let line = line.trim();
    // Skip lines that are part of a multi-line string
    line.starts_with('"') && !line.contains('=')
}
```

## Tree-Sitter Best Practices

### Query-Based Detection (Recommended)

```rust
// Instead of iterating source lines, use tree-sitter queries
let query = tree_sitter::Query::new(
    &ctx.language.to_ts_language(),
    r#"(call_expression
        function: (identifier) @func
        arguments: (arguments (string) @arg))"#
)?;

let mut cursor = tree_sitter::QueryCursor::new();
let matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());

for m in matches {
    for cap in m.captures {
        let node = cap.node;
        // Only matches actual code, not comments or strings
    }
}
```

### Node Traversal for Structural Rules

```rust
fn check_nesting(node: &tree_sitter::Node, depth: usize, max: usize) -> Vec<Issue> {
    let mut issues = Vec::new();
    if node.kind() == "if_statement" || node.kind() == "for_statement" {
        if depth > max {
            issues.push(/* create issue */);
        }
    }
    for child in node.children(&mut node.walk()) {
        issues.extend(check_nesting(&child, depth + 1, max));
    }
    issues
}
```

## Testing Strategy

### Test Pyramid for Rules

```
        ┌─────────┐
        │   E2E    │  ← analyze real project, check dashboard
        ├─────────┤
        │Integration│ ← multi-file, multi-rule, call graph
        ├───────────┤
        │  Rule Unit │ ← single rule, multiple test cases
        ├─────────────┤
        │  FP Guard   │ ← MANDATORY: test DOES NOT trigger
        └─────────────┘
```

### Required Test Cases Per Rule

| Test Type | Minimum | Description |
|-----------|---------|-------------|
| **Detection** | 2+ | At least 2 positive cases (different variants) |
| **False Positive** | 3+ | Comment, variable name, and string literal |
| **Edge Case** | 1+ | Boundary condition (empty file, single char) |
| **Rule Properties** | 1 | id(), name(), severity(), category(), language() |

### Test Template

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sXXXX_detects_pattern() {
        let source = r#"
fn main() {
    let password = "secret123";  // Should detect
}
"#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            SXXXXRule::new().check(ctx)
        });
        assert!(!issues.is_empty(), "Expected SXXXX to detect hardcoded password");
        assert_eq!(issues[0].rule_id, "SXXXX");
    }

    #[test]
    fn test_sXXXX_no_fp_in_comment() {
        let source = r#"
// Example: let password = "secret";
//! This module provides password hashing
fn main() {
    let hash = argon2("data");
}
"#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            SXXXXRule::new().check(ctx)
        });
        assert!(issues.is_empty(), "SXXXX should NOT trigger on comments");
    }

    #[test]
    fn test_sXXXX_no_fp_in_variable_name() {
        let source = r#"
fn main() {
    let password_hash = get_env("SECRET");
}
"#;
        let issues = with_rule_context(source, Language::Rust, |ctx| {
            SXXXXRule::new().check(ctx)
        });
        assert!(issues.is_empty(), "SXXXX should NOT trigger on env vars");
    }
}
```

## Common Anti-Patterns

### ❌ Scanning raw lines without comment filtering
```rust
// 100+ rules do this — causes FP on documentation
for line in ctx.source.lines() { ... }
```

### ❌ Regex without word boundaries
```rust
(r"des", ...)  // Matches "provides", "design", "destination"
```

### ❌ No false positive tests
```rust
// Only tests that the rule triggers, never tests it doesn't
assert!(!issues.is_empty())
```

### ❌ Using regex when tree-sitter would be better
```rust
// Trying to detect function calls with regex
r"\w+\s*\(.*\)"  // Matches any parenthesized expression
```

### ✅ The SonarQube Way: detect structure, not text
```rust
// Use tree-sitter to find actual function calls
let query = tree_sitter::Query::new(&lang, "(call_expression) @call")?;
```

## Rule Development Checklist

Before submitting a rule PR:

- [ ] **Approach**: Is regex or tree-sitter the right tool?
- [ ] **Regex safety**: Word boundaries (`\b`), prefix handling (`(?:\b|_)`)
- [ ] **Comment skip**: Does the rule skip `//`, `///`, `//!`, `/*` comments?
- [ ] **String skip**: Does the rule skip string literals?
- [ ] **Detection tests**: 2+ positive cases
- [ ] **FP tests**: Comment, variable name, string literal
- [ ] **Edge case test**: Empty input, boundary
- [ ] **Properties test**: id, name, severity, category
- [ ] **No hardcoded paths**: Use `ctx.file_path`, not absolute paths
- [ ] **Remediation message**: Clear, actionable guidance

## Commands

```bash
# Run all tests for a specific rule
cargo test -p cognicode-axiom --lib SXXXX

# Run all rule tests
cargo test -p cognicode-axiom --lib

# Run with output to debug FP issues  
cargo test -p cognicode-axiom --lib SXXXX -- --nocapture

# Format code
cargo fmt -p cognicode-axiom

# Check compilation
cargo check -p cognicode-axiom
```

## Resources

- **Catalog**: `crates/cognicode-axiom/src/rules/catalog.rs` (854 rules)
- **Types**: `crates/cognicode-axiom/src/rules/types.rs` (RuleContext, Issue, Rule trait)
- **Parser**: `crates/cognicode-core/src/infrastructure/parser/` (tree-sitter infra)
- **Handler**: `crates/cognicode-quality/src/handler.rs` (analysis flow)
- **Dashboard**: `crates/cognicode-dashboard/` (visualization of issues)
