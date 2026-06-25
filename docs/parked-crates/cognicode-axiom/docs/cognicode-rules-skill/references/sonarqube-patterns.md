# SonarQube Rule Patterns — Reference

## SonarQube Architecture for Rules

SonarQube uses a **layered analysis pipeline** for each rule:

```
Source Code
    │
    ▼
┌─────────────┐
│  Lexer       │  ← Tokenization (like tree-sitter lexer)
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  AST Parser  │  ← Full semantic tree (like tree-sitter parser)
└──────┬──────┘
       │
       ▼
┌──────────────────────┐
│  Rule Visitors       │  ← Each rule is a visitor over the AST
│  (SyntaxTreeVisitor) │
└──────┬───────────────┘
       │
       ▼
┌─────────────┐
│  Checks      │  ← Rule-specific logic
└─────────────┘
```

## SonarQube Rule Categories

| Category | Our Equivalent | Detection Method |
|----------|---------------|------------------|
| **Bug** | Bug | AST visitor + data flow |
| **Vulnerability** | Vulnerability | Regex + AST visitor |
| **Code Smell** | CodeSmell | AST visitor + metrics |
| **Security Hotspot** | SecurityHotspot | Regex + AST visitor |

## Key SonarQube Patterns We Should Adopt

### 1. Visitor Pattern (AST-based)
Instead of iterating source lines, SonarQube uses **tree visitors** that walk the AST:

```java
// SonarQube Java rule
public class MyRule extends BaseTreeVisitor {
    @Override
    public void visitMethod(MethodTree tree) {
        // Only triggered on actual method nodes, not comments
    }
}
```

**Our equivalent**: Use `tree_sitter::Query` or manual tree walking.

### 2. Semantic API
SonarQube provides a **semantic API** that understands:
- Type resolution (`tree.type()`)
- Symbol references (`tree.symbol()`)
- Scope analysis (`tree.scope()`)

**Our equivalent**: `ctx.graph` (call graph), `ctx.metrics` (pre-computed).

### 3. Issue Suppression
SonarQube supports `// NOSONAR` comments to suppress issues on specific lines.

**Our equivalent**: Not implemented yet. Could add `// cognicode:disable=SXXXX`.

### 4. Secondary Locations
SonarQube can show multiple locations for a single issue (e.g., "declared here", "used here").

**Our equivalent**: `Issue.secondary_locations` field available but not widely used.

## Regex Patterns That SonarQube Uses (Correctly)

SonarQube also uses regex for simple patterns, but always with:

1. **Comment exclusion**: Pattern is checked only on non-comment tokens
2. **String exclusion**: Pattern skips string literals
3. **Word boundaries**: Always uses `\b` or equivalent
4. **Case-insensitive**: Uses `(?i)` flag

Example from SonarQube's S2068:
```java
private static final Pattern PASSWORD_PATTERN = 
    Pattern.compile("(?i)password\\s*[=:]\\s*[\"'][^\"']{4,}[\"']");
```

They then check this pattern against **tokens** (lexer output), not raw source lines.
Tokens exclude comments and strings automatically.
