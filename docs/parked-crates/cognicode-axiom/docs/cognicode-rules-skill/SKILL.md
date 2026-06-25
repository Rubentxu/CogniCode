---
name: cognicode-rules
description: >
  Master skill for developing, testing, and auditing CogniCode quality rules.
  Covers SonarQube-derived best practices, tree-sitter AST analysis, regex safety,
  test strategies (unit/integration/false-positive), self-improvement loops via
  dashboard feedback, and full ecosystem integration (axiom → quality → dashboard).
  Trigger: When creating, modifying, testing, or auditing CogniCode detection rules,
  fixing false positives, migrating regex to tree-sitter, or working with
  cognicode-axiom, cognicode-quality, rule catalogs, or dashboard issue reports.
license: Apache-2.0
metadata:
  author: gentleman-programming
  version: "2.1"
---

# CogniCode Rules — Master Skill (v2.1)

## When to Use

- Creating a **new detection rule** (catalog.rs or `rules/` module)
- **Fixing a false positive** reported by dashboard or users
- **Writing tests** for rules (unit, integration, FP prevention)
- **Auditing** rules for correctness, performance, or FP risk
- **Migrating** rules from regex line-scanning to tree-sitter AST queries
- **Self-improving** rules using dashboard feedback loop
- **Reviewing** rule PRs for quality against this skill's standards

## Ecosystem Integration Map

```
User reports issue (Dashboard)
        │
        ▼
┌──────────────────┐     ┌─────────────────┐     ┌──────────────────┐
│  cognicode-rules │────▶│ cognicode-axiom │────▶│ cognicode-quality │
│  (this skill)    │     │  (rule engine)   │     │  (analysis)       │
└──────────────────┘     └────────┬────────┘     └────────┬─────────┘
                                  │                        │
                                  │    ┌───────────────────┘
                                  ▼    ▼
                         ┌──────────────────┐
                         │ .cognicode/      │
                         │ cognicode.db     │ ← SQLite persistence
                         └────────┬─────────┘
                                  │
                                  ▼
                         ┌──────────────────┐
                         │ Dashboard        │
                         │ (visualization)  │
                         └────────┬─────────┘
                                  │
                    ┌─────────────┘
                    ▼
              User sees issue → Reports FP → Back to top
```

## Self-Improvement Loop (Auto-Fix)

When a false positive is detected via dashboard or user report:

```
1. Identify the FP pattern
   curl http://dashboard/api/issues?rule_id=SXXXX

2. Read the rule source
   grep -A30 'id: "SXXXX"' crates/cognicode-axiom/src/rules/catalog.rs

3. Apply the fix
   A. Add word boundaries (\b) to regex patterns
   B. Add comment/string skipping logic
   C. Migrate to tree-sitter query if structural

4. Add FP regression test
   #[test] fn test_SXXXX_no_fp_{description}() { ... }

5. Run tests
   cargo test -p cognicode-axiom --lib SXXXX

6. Verify on dashboard
   curl -X POST http://dashboard/api/analysis -d '{...}' | jq '.issues'
   → Confirm the FP no longer appears

7. Commit with traceability
   git commit -m "fix(axiom): SXXXX — {what was fixed} (reported via dashboard)"
```

## Architecture Overview

```
cognicode-axiom/src/rules/
├── catalog.rs          ← 854 rules (declare_rule! macro)
├── catalog_tests.rs    ← Manual tests (11 rules, 40 tests)
├── catalog_tests_generated.rs ← Auto-generated tests
├── types.rs            ← RuleContext, Issue, Severity, Category
├── mod.rs              ← RuleRegistry, inventory
├── rule_factory.rs     ← Factory pattern for rule creation
└── rules/              ← Separate rule files
    ├── style.rs
    ├── complexity.rs
    └── security.rs
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

### The Quality Handler Flow

```
QualityAnalysisHandler::analyze_project_impl()
  │
  ├─ Load AnalysisState (from .cognicode/cognicode.db)
  ├─ Find changed files (BLAKE3 hashes)
  ├─ For each file:
  │   ├─ Parse with tree-sitter → RuleContext { tree, source, ... }
  │   ├─ Register rules from catalog (filtered by language)
  │   ├─ Run each rule: rule.check(&ctx) → Vec<Issue>
  │   └─ Collect issues
  ├─ Persist to SQLite (analysis_runs, issues)
  └─ Return ProjectAnalysisResult → Dashboard reads this
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
// ✅ ALWAYS do this when scanning ctx.source.lines()
for (line_idx, line) in ctx.source.lines().enumerate() {
    let trimmed = line.trim();
    if trimmed.is_empty() { continue; }
    if trimmed.starts_with("//") || trimmed.starts_with("///") 
    || trimmed.starts_with("//!") || trimmed.starts_with("#") 
    || trimmed.starts_with("/*") || trimmed.starts_with("*") {
        continue;
    }
    if re.is_match(trimmed) { ... }
}
```

### 3. String Literal Exclusion

```rust
fn is_inside_string(line: &str) -> bool {
    line.trim().starts_with('"') && !line.contains('=')
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
// Only matches actual code, never comments or strings
```

### Node Traversal for Structural Rules

```rust
fn walk_nodes(node: &tree_sitter::Node, depth: usize, max: usize, 
              source: &str, file: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();
    let kind = node.kind();
    
    // Check this node
    if kind == "if_statement" && depth > max {
        issues.push(Issue::new(...));
    }
    
    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        issues.extend(walk_nodes(&child, depth + 1, max, source, file));
    }
    issues
}
```

### Finding Functions/Methods

```rust
fn query_functions(tree: &tree_sitter::Tree, source: &str) -> Vec<tree_sitter::Node> {
    let query = tree_sitter::Query::new(
        &language,
        r#"(function_item name: (identifier) @name)"#
    ).unwrap();
    
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut functions = Vec::new();
    
    for m in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
        for cap in m.captures {
            functions.push(cap.node);
        }
    }
    functions
}
```

## Testing Strategy

### Test Pyramid for Rules

```
        ┌─────────┐
        │   E2E    │  ← analyze real project via dashboard
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

## Common Anti-Patterns

### ❌ Scanning raw lines without comment filtering
```rust
// 100+ rules do this — causes FP on documentation
for line in ctx.source.lines() { ... }
```
**Fix**: Add comment/string skip (see Protocol above)

### ❌ Regex without word boundaries
```rust
(r"des", ...)  // Matches "provides", "design", "destination"
```
**Fix**: `r"(?:\b|_)des\b"` (word boundary before/underscore, word boundary after)

### ❌ No false positive tests
**Fix**: Add 3 FP test categories (see Test Templates)

### ❌ Using regex when tree-sitter would be better
**Fix**: Use tree-sitter queries for structural patterns (SQL injection, nesting, unused vars)

### ❌ No self-improvement feedback loop
**Fix**: Monitor dashboard for user-reported FPs, fix + add regression test in same commit

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
- [ ] **Dashboard verified**: Check that issue appears correctly in dashboard
- [ ] **Self-improvement trace**: FP fix includes dashboard issue reference

## Dashboard Integration Commands

```bash
# Register project in dashboard
curl -X POST http://localhost:3000/api/projects/register \
  -H "Content-Type: application/json" \
  -d '{"name":"CogniCode","path":"/home/rubentxu/Proyectos/rust/CogniCode"}'

# Run analysis and check issues
curl -X POST http://localhost:3000/api/analysis \
  -H "Content-Type: application/json" \
  -d '{"project_path":"/home/rubentxu/Proyectos/rust/CogniCode","quick":true}'

# Check specific rule issues in dashboard
curl -X POST http://localhost:3000/api/issues \
  -H "Content-Type: application/json" \
  -d '{"project_path":"/home/rubentxu/Proyectos/rust/CogniCode","rule_id":"SXXXX"}'

# Verify FP is fixed (should return 0 issues)
curl -s ... | jq '.issues | length'
```

## Development Commands

```bash
# Run all tests for a specific rule
cargo test -p cognicode-axiom --lib SXXXX

# Run all rule tests
cargo test -p cognicode-axiom --lib

# Run with output to debug FP issues  
cargo test -p cognicode-axiom --lib SXXXX -- --nocapture

# Full quality analysis (test your rule on real code)
cargo run -p cognicode-quality -- analyze /path/to/project

# Check what rules fire on a specific file
cargo test -p cognicode-axiom --lib SXXXX -- --nocapture 2>&1 | grep "assert"
```

## Resources

- **Catalog**: `crates/cognicode-axiom/src/rules/catalog.rs` (854 rules)
- **Types**: `crates/cognicode-axiom/src/rules/types.rs` (RuleContext, Issue, Rule trait)
- **Parser**: `crates/cognicode-core/src/infrastructure/parser/` (tree-sitter infra)
- **Handler**: `crates/cognicode-quality/src/handler.rs` (analysis flow)
- **Dashboard**: `crates/cognicode-dashboard/` (visualization of issues)
- **References**: See [references/](references/) for detailed patterns
  - [sonarqube-patterns.md](references/sonarqube-patterns.md) — SonarQube architecture
  - [test-patterns.md](references/test-patterns.md) — Test templates
  - [tree-sitter-queries.md](references/tree-sitter-queries.md) — Query patterns

## Compact Rules (for agent injection)

```
- ALWAYS use word boundaries in regex: (?:\b|_)des\b not just "des"
- ALWAYS skip comment lines in line-scanning rules (//, ///, //!, #, /*)
- ALWAYS add 3+ false positive tests per rule (comment, identifier, English word)
- RuleContext has tree_sitter::Tree available — use queries instead of regex for structural patterns
- Dashboard is the feedback loop: monitor issues, fix FPs, add regression tests
- Self-improvement: FP report → fix rule → add test → verify on dashboard → commit
- Tree-sitter queries match actual code nodes, never comments or strings
- 854 rules exist; only 11 have tests; prioritize security/vulnerability rules for testing
- Use ctx.graph (CallGraph) and ctx.metrics (FileMetrics) for semantic analysis
- cognicode-quality handler: analyze_project_impl() → RuleContext → rule.check() → Issue → SQLite → Dashboard
```
