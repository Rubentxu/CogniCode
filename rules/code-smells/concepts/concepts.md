# Code Smells Concepts

## CC_CS_001: TODO/FIXME Comments Should Be Resolved

**Problem**: TODO and FIXME comments indicate incomplete code that should be addressed. Leaving them in production code creates technical debt and can hide important incomplete work.

**Rule**: CC_CS_001 (derived from SonarQube S113)
**Severity**: Minor
**Languages**: rust

**Detection Approach**:
- AST-based comment token analysis
- Search for comments containing `TODO`, `FIXME`, `XXX`, `HACK` patterns
- Flag any block or line comments containing these markers

**References**:
- SonarQube S113: TODO comments should be resolved
- SQALE: http://www.sqale.org/managing/technical-debt

**AST Query Strategy**:
```tree-sitter
(comment) @todo_comment
  ; Check if comment text contains TODO/FIXME/XXX/HACK
```

**False Positives to Avoid**:
- TODO in string literals (e.g., `"TODO: implement later"`)
- Generated code markers
- Test files (often have intentional placeholders)

**Remediation**: Replace TODO comments with actual implementations or create GitHub issues to track the work.

---

## CC_CS_002: Empty Nested Blocks Should Be Removed

**Problem**: Empty blocks `{}` in if/else statements, loops, or closures add no logic but reduce code readability and can indicate forgotten implementation.

**Rule**: CC_CS_002 (derived from SonarQube S1764)
**Severity**: Minor
**Languages**: rust

**Detection Approach**:
- AST-based empty block detection
- Find block nodes (`block` in tree-sitter) with no statements
- Exclude function bodies (legitimate empty for declarations)
- Exclude trait/impl blocks with only type signatures

**References**:
- SonarQube S1764: Empty nested blocks should be removed

**AST Query Strategy**:
```tree-sitter
(block (statements) @empty_block)
  ; When block has no statements
```

**False Positives to Avoid**:
- Empty function bodies in declared functions
- Empty impl blocks with only method signatures
- Empty trait definitions
- Unit tests with `#[test]` and empty body (placeholder)

---

## CC_CS_003: Duplicate Branches in If-Else Chain

**Problem**: When multiple branches in an if-else chain have identical code, it indicates dead code and confusion about which condition is actually being checked.

**Rule**: CC_CS_003 (derived from SonarQube S1871)
**Severity**: Major
**Languages**: rust

**Detection Approach**:
- AST-based branch comparison
- Compare AST structure of consecutive branch bodies
- Flag when two branches produce identical AST representations

**References**:
- SonarQube S1871: Two branches in an if/else chain should not have the same implementation

**AST Query Strategy**:
```tree-sitter
(alternative_branch
  condition: _ @cond
  consequence: (block (statements)) @body)
@alt
; Compare @body across siblings
```

**False Positives to Avoid**:
- Branches with same semantic meaning but different AST (e.g., `x > 0` vs `0 < x`)
- Early return patterns where fallthrough is intentional
- Macro-expanded code that looks identical but has different effects

---

## CC_CS_004: Empty Statements Should Be Removed

**Problem**: Standalone semicolons or empty statements add no value and indicate incomplete work or copy-paste errors.

**Rule**: CC_CS_004 (derived from SonarQube S1116)
**Severity**: Minor
**Languages**: rust

**Detection Approach**:
- AST-based empty statement detection
- Find expression statements with no expression (just semicolon)

**References**:
- SonarQube S1116: Empty statements should be removed

**AST Query Strategy**:
```tree-sitter
(expression_statement) @empty_stmt
  ; When expression_statement has no expression child
```

**False Positives to Avoid**:
- None expected in Rust - empty statements are always errors

---

## CC_CS_005: Redundant Semicolons Should Be Removed

**Problem**: Extra semicolons after expressions or at the end of blocks are unnecessary and indicate sloppy code.

**Rule**: CC_CS_005 (derived from SonarQube S1117)
**Severity**: Minor
**Languages**: rust

**Detection Approach**:
- AST-based semicolon analysis
- Find semicolons in positions where they add no value

**References**:
- SonarQube S1117: Redundant semicolons should be removed

**AST Query Strategy**:
```tree-sitter
; In Rust, semicolons at end of block are usually statement terminators
; Detect redundant ones like: let x = 5;;
```

**False Positives to Avoid**:
- Semicolons that are required (most expression statements)
- Semicolons in macros like `println!();`

---

## CC_CS_006: Wildcard Patterns Should Not Precede Specific Patterns in Match

**Problem**: When a wildcard (`_`) pattern appears before more specific patterns in a match arm, the specific patterns will never be matched - this is likely a bug.

**Rule**: CC_CS_006 (derived from SonarQube S3353)
**Severity**: Major
**Languages**: rust

**Detection Approach**:
- AST-based match arm pattern analysis
- Find match expressions where `_` appears before other patterns in the same arm list
- Note: In Rust, `_` at top level of a match arm catches remaining values, but multiple `_` in Or patterns are redundant

**References**:
- SonarQube S3353: Match cases should be properly sorted

**AST Query Strategy**:
```tree-sitter
(match_arm
  patterns: (patterns
    (wildcard_pattern)
    _+) @wildcard_first
```

**False Positives to Avoid**:
- `_` as the only pattern (it's correctly exhaustive)
- Or patterns where `_` is last: `A | B | _`
- Guarded arms where order is intentional

---

## CC_CS_007: Function Names Should Follow snake_case Convention

**Problem**: Rust convention is snake_case for function names. Using camelCase or other conventions violates the Rust API guidelines.

**Rule**: CC_CS_007 (derived from SonarQube S100)
**Severity**: Minor
**Languages**: rust

**Detection Approach**:
- AST-based identifier pattern analysis
- Find function declarations and method names
- Check if they match `snake_case` pattern: lowercase + optional underscores

**References**:
- SonarQube S100: Method names should comply with a naming convention
- Rust API Guidelines: https://rust-lang.github.io/api-guidelines/naming.html

**AST Query Strategy**:
```tree-sitter
(function_item name: (identifier) @fn_name)
(method_declaration name: (identifier) @method_name)
```

**False Positives to Avoid**:
- `main` function (convention allows it)
- Test functions: `#[test]` functions often use camelCase for readability
- Foreign function interface (FFI) declarations
- Generated code with markers

---

## CC_CS_008: Empty Function Bodies with Only TODO Markers

**Problem**: Functions that contain only `todo!()`, `unimplemented!()`, or `panic!()` macros indicate incomplete implementation that will fail at runtime.

**Rule**: CC_CS_008 (Rust-specific)
**Severity**: Minor
**Languages**: rust

**Detection Approach**:
- AST-based function body analysis
- Find functions whose body contains only:
  - `todo!()` macro call
  - `unimplemented!()` macro call
  - `panic!()` macro call with string
  - `loop {}` infinite empty loop (dead code)

**References**:
- Rust RFC: https://github.com/rust-lang/rfcs/blob/master/text/2195-xfail\s.md

**AST Query Strategy**:
```tree-sitter
(function_item
  body: (block
    (expression_statement
      (macro_invocation
        (identifier) @macro_name
        (#match? @macro_name "^(todo|unimplemented)$")))))
```

**False Positives to Avoid**:
- `panic!()` in `main()` functions (valid for intentionally failing programs)
- `todo!()` in test functions (acceptable placeholder)
- Skeleton code in example binaries

---

## CC_CS_009: Redundant Parenthesized Expressions Should Be Removed

**Problem**: Unnecessary parentheses around expressions add visual noise and suggest the developer was unsure of operator precedence.

**Rule**: CC_CS_009 (Rust-specific)
**Severity**: Minor
**Languages**: rust

**Detection Approach**:
- AST-based expression structure analysis
- Find `parenthesized_expression` nodes that contain only a simple expression
- Flag when parentheses don't affect evaluation order

**References**:
- Rust Style Guidelines

**AST Query Strategy**:
```tree-sitter
(parenthesized_expression
  (expression) @inner)
; Flag when inner is a simple expression, not affecting precedence
```

**False Positives to Avoid**:
- Expressions where parentheses affect evaluation order
- Expressions in macro contexts
- Return statements: `return (x);` (unnecessary but harmless)
