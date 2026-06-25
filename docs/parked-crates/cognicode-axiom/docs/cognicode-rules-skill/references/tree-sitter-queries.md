# Tree-Sitter Query Patterns for Rule Detection

## Available Languages

```rust
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Java,
    Go,
    // ...
}

impl Language {
    pub fn to_ts_language(&self) -> tree_sitter::Language {
        match self {
            Language::Rust => tree_sitter_rust::LANGUAGE.into(),
            Language::Python => tree_sitter_python::LANGUAGE.into(),
            // ...
        }
    }
}
```

## Common Query Patterns by Language

### Rust

```lisp
;; Find all function definitions
(function_item
  name: (identifier) @func_name
  parameters: (parameters) @params
  body: (block) @body)

;; Find all macro invocations (format!, println!, etc.)
(macro_invocation
  macro: (identifier) @macro_name
  (token_tree) @args)

;; Find string literals assigned to variables
(let_declaration
  pattern: (identifier) @var
  value: (string_literal) @value)

;; Find unsafe blocks
(unsafe_block) @unsafe

;; Find match expressions without wildcard arm
(match_expression
  body: (match_block
    (match_arm
      pattern: (wildcard_pattern)? @wildcard)))
;; ^ If @wildcard is missing, the match is non-exhaustive

;; Find unwrap() calls
(call_expression
  function: (field_expression
    field: (field_identifier) @method_name)
  (#eq? @method_name "unwrap"))
```

### Python

```lisp
;; Find exec() calls (dangerous)
(call
  function: (identifier) @func
  (#eq? @func "exec"))

;; Find pickle.loads calls
(call
  function: (attribute
    object: (identifier) @obj
    attribute: (identifier) @attr)
  (#eq? @obj "pickle")
  (#match? @attr "^(load|loads)$"))

;; Find hardcoded secrets in assignments
(assignment
  left: (identifier) @var
  right: (string) @value
  (#match? @var "^(password|secret|token)$"))
```

### JavaScript

```lisp
;; Find eval() calls
(call_expression
  function: (identifier) @func
  (#eq? @func "eval"))

;; Find innerHTML assignments (XSS risk)
(assignment_expression
  left: (member_expression
    property: (property_identifier) @prop)
  (#eq? @prop "innerHTML"))

;; Find console.log calls
(call_expression
  function: (member_expression
    object: (identifier) @obj
    property: (property_identifier) @prop)
  (#eq? @obj "console")
  (#eq? @prop "log"))
```

### Java

```lisp
;; Find System.out.println
(method_invocation
  object: (field_access
    object: (identifier) @obj
    field: (identifier) @field)
  name: (identifier) @method
  (#eq? @obj "System")
  (#eq? @field "out"))

;; Find Thread.sleep in try blocks
(try_statement
  body: (block
    (expression_statement
      (method_invocation
        name: (identifier) @method
        (#eq? @method "sleep")))))
```

## Pattern Matching Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `#eq?` | Exact string match | `(#eq? @name "unwrap")` |
| `#match?` | Regex match | `(#match? @name "^(get\|set)_")` |
| `#not-eq?` | Negated exact match | `(#not-eq? @name "main")` |
| `#not-match?` | Negated regex | `(#not-match? @name "^test_")` |
| `?` | Optional node (0 or 1) | `(parameters)?` |
| `*` | Zero or more | `(statement)*` |
| `+` | One or more | `(statement)+` |
| `[]` | Field access | `function: [ (identifier) (field_expression) ]` |

## Capturing Node Ranges

```rust
let query = tree_sitter::Query::new(&lang, pattern)?;
let mut cursor = tree_sitter::QueryCursor::new();
let matches = cursor.matches(&query, root, source.as_bytes());

for m in matches {
    for cap in m.captures {
        let node = cap.node;
        let start = node.start_position();
        let end = node.end_position();
        let text = node.utf8_text(source.as_bytes())?;
        // start.row (line), start.column
        // end.row, end.column
    }
}
```

## Performance Tips

1. **Compile queries once**: Store in a `Lazy<tree_sitter::Query>` or `OnceLock`
2. **Limit traversal depth**: Use `node.child_count()` to skip leaf nodes
3. **Filter early**: Check `node.kind()` before running expensive queries
4. **Use `ctx.metrics`**: Pre-computed metrics (LOC, functions, complexity) are cheaper than tree walking
5. **Cache tree**: The tree is already parsed — reuse it across rules
