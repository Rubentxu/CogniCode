# CogniCode API Reference

> **Version**: 0.1.0
> **Protocol**: MCP (Model Context Protocol) via JSON-RPC 2.0
> **Transport**: stdin/stdout

## Quick Start

### Build

```bash
cargo build --release
```

### Run MCP Server

```bash
cargo run --bin cognicode-mcp
```

### Test with JSON-RPC

```bash
# Initialize
echo '{"jsonrpc":"2.0","method":"initialize","params":{},"id":1}' | cargo run --bin cognicode-mcp

# List tools
echo '{"jsonrpc":"2.0","method":"tools/list","params":{},"id":2}' | cargo run --bin cognicode-mcp

# Get file symbols
echo 'def hello(): pass' > /tmp/test.py
echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"get_file_symbols","arguments":{"file_path":"/tmp/test.py"}},"id":3}' | cargo run --bin cognicode-mcp
```

---

## MCP Protocol

### Message Format

All messages use JSON-RPC 2.0 format:

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "method_name",
  "params": { ... },
  "id": 1
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": { ... },
  "id": 1
}
```

**Error Response:**
```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32600,
    "message": "Error message"
  },
  "id": 1
}
```

### Methods

#### `initialize`
Initializes the MCP server connection.

**Request:** `{}`

**Response:**
```json
{
  "protocolVersion": "1.0",
  "serverInfo": {
    "name": "cognicode",
    "version": "0.1.0"
  },
  "capabilities": {
    "tools": true
  }
}
```

#### `tools/list`
Lists all available MCP tools.

**Response:**
```json
{
  "tools": [
    { "name": "get_file_symbols", "description": "...", "inputSchema": {...} },
    { "name": "get_call_hierarchy", ... },
    ...
  ]
}
```

#### `tools/call`
Calls a specific tool with arguments.

**Request:**
```json
{
  "name": "tool_name",
  "arguments": { ... }
}
```

---

## Available Tools

### `get_file_symbols`

Extracts all symbols (functions, classes, variables) from a file.

**Input:**
```json
{
  "file_path": "/path/to/file.py",
  "compressed": false
}
```

**Output (compressed=false):**
```json
{
  "file_path": "/path/to/file.py",
  "symbols": [
    {
      "name": "hello",
      "kind": "function",
      "location": { "file": "/path/to/file.py", "line": 1, "column": 0 },
      "signature": "def hello()"
    }
  ]
}
```

**Output (compressed=true):**
```json
{
  "summary": "file.py: 1 function (hello). No external deps."
}
```

---

### `get_call_hierarchy`

Traverses the call graph to find callers or callees.

**Input:**
```json
{
  "symbol_name": "process_order",
  "direction": "outgoing",
  "depth": 1,
  "include_external": false,
  "compressed": false
}
```

**Output:**
```json
{
  "symbol": "process_order",
  "calls": [
    { "symbol": "validate", "file": "order.rs", "line": 42, "column": 5, "confidence": 1.0 }
  ],
  "metadata": { "total_calls": 1, "analysis_time_ms": 15 }
}
```

**Directions:** `incoming` | `outgoing`

---

### `analyze_impact`

Analyzes the impact of changing a symbol.

**Input:**
```json
{
  "symbol_name": "Symbol::new",
  "compressed": false
}
```

**Output:**
```json
{
  "symbol": "Symbol::new",
  "impacted_files": ["src/parser.rs", "src/handler.rs"],
  "impacted_symbols": ["parse_symbols", "handle_request"],
  "risk_level": "medium",
  "summary": "Symbol::new is used by 2 files and 4 symbols"
}
```

**Risk Levels:** `low` | `medium` | `high` | `critical`

---

### `check_architecture`

Detects cycles and architecture violations using Tarjan SCC algorithm.

**Input:**
```json
{
  "scope": null
}
```

**Output:**
```json
{
  "cycles": [
    { "symbols": ["a", "b", "a"], "length": 3 }
  ],
  "violations": [
    { "rule": "no_circular_deps", "from": "module_a", "to": "module_b", "severity": "error" }
  ],
  "score": 0.85,
  "summary": "Architecture check passed with 1 warning"
}
```

---

### `safe_refactor`

Performs safe refactoring with validation and preview.

**Input:**
```json
{
  "action": "rename",
  "target": "old_function_name",
  "params": { "new_name": "new_function_name" }
}
```

**Actions:** `rename` | `extract` | `inline` | `move` | `change_signature`

**Output:**
```json
{
  "action": "rename",
  "success": true,
  "changes": [
    {
      "file": "src/lib.rs",
      "old_text": "fn old_function_name()",
      "new_text": "fn new_function_name()",
      "location": { "file": "src/lib.rs", "line": 42, "column": 0 }
    }
  ],
  "validation_result": {
    "is_valid": true,
    "warnings": [],
    "errors": []
  }
}
```

---

### `find_usages`

Finds all usages of a symbol across the project.

**Input:**
```json
{
  "symbol_name": "calculate_total",
  "include_declaration": true
}
```

**Output:**
```json
{
  "symbol": "calculate_total",
  "usages": [
    { "file": "order.rs", "line": 15, "column": 5, "context": "result = calculate_total(order)", "is_definition": true },
    { "file": "order.rs", "line": 42, "column": 12, "context": "total = calculate_total(items)", "is_definition": false }
  ],
  "total": 2
}
```

---

### `get_complexity`

Calculates code complexity metrics.

**Input:**
```json
{
  "file_path": "/path/to/file.py",
  "function_name": "process_order"
}
```

**Output:**
```json
{
  "file_path": "/path/to/file.py",
  "complexity": {
    "cyclomatic": 5,
    "cognitive": 3,
    "lines_of_code": 42,
    "parameter_count": 2,
    "nesting_depth": 3,
    "function_name": "process_order"
  }
}
```

**Metrics:**
- **cyclomatic**: Cyclomatic complexity (decision points + 1)
- **cognitive**: Cognitive complexity (nested structure complexity)
- **nesting_depth**: Maximum nesting level
- **parameter_count**: Number of function parameters

---

### `structural_search`

Searches for code patterns using tree-sitter queries.

**Input:**
```json
{
  "pattern_type": "function_call",
  "query": "validate_*",
  "path": "/path/to/search",
  "depth": 3
}
```

**Pattern Types:** `function_call` | `type_definition` | `import_statement` | `annotation` | `custom`

---

### `validate_syntax`

Validates syntax of a file using tree-sitter.

**Input:**
```json
{
  "file_path": "/path/to/file.py"
}
```

**Output:**
```json
{
  "file_path": "/path/to/file.py",
  "is_valid": true,
  "errors": [],
  "warnings": []
}
```

---

## Context Compression

The `compressed: true` flag transforms JSON responses into natural language summaries, reducing token usage by 50%+.

**Example:**

```json
// Without compression (458 tokens)
{
  "file_path": "order_service.rs",
  "symbols": [
    {"name": "process_order", "kind": "function", "location": {"file": "order_service.rs", "line": 42, "column": 0}, "signature": "fn process_order(order: Order) -> Result<()>"},
    {"name": "validate", "kind": "function", "location": {"file": "order_service.rs", "line": 58, "column": 0}, "signature": "fn validate(order: &Order) -> bool"},
    ...
  ]
}

// With compression (48 tokens)
{
  "summary": "order_service.rs: 4 functions (process_order@42, validate@58, compute_total@75, save@92). process_order calls validate + compute_total. No external deps."
}
```

---

## Incremental Graph Updates

For efficient updates when files change:

### GraphEvent Enum

```rust
pub enum GraphEvent {
    SymbolAdded { symbol: Symbol },
    SymbolRemoved { symbol_id: SymbolId },
    SymbolModified { symbol_id: SymbolId, old_symbol: Symbol, new_symbol: Symbol },
    DependencyAdded { source: SymbolId, target: SymbolId, kind: DependencyType },
    DependencyRemoved { source: SymbolId, target: SymbolId },
}
```

### Usage

```rust
use crate::domain::events::{GraphDiffCalculator, GraphEvent};

let old_symbols = parser.find_all_symbols(&old_source, path)?;
let new_symbols = parser.find_all_symbols(&new_source, path)?;

let events = GraphDiffCalculator::calculate_diff(&old_symbols, &new_symbols);
graph.apply_events(&events)?;  // Incremental update
```

---

## LSP Proxy Mode

CogniCode can proxy to external LSPs (rust-analyzer, pyright) for basic operations while handling premium operations internally.

### Configuration

```rust
use crate::application::services::LspProxyService;

let proxy = LspProxyService::new(analysis_service);
proxy.enable_proxy_mode();
```

### Delegated Operations (to external LSP)
- `hover` - Type information on hover
- `completion` - Auto-completion
- `definition` - Go to definition
- `references` - Find references (basic)

### Premium Operations (CogniCode)
- `analyze_impact` - Dependency-aware impact analysis
- `check_architecture` - Cycle detection
- `get_complexity` - Complexity metrics
- `safe_refactor` - Safe refactoring with SafetyGate

---

## Error Types

### SecurityError

| Variant | Description |
|---------|-------------|
| `PathTraversalAttempt` | Path contains `..` or traversal patterns |
| `PathOutsideWorkspace` | Path not within allowed workspace |
| `SymlinkDetected` | Path or parent contains symlink |
| `PathNotAccessible` | Cannot access path |
| `InvalidPathCharacters` | Path contains null bytes |
| `PathTooDeep` | Exceeds maximum path depth |

### AppError

| Variant | Description |
|---------|-------------|
| `AnalysisError` | Analysis operation failed |
| `ParseError` | Tree-sitter parsing failed |
| `GraphError` | Graph operation failed |

---

## SymbolKind Enum

| Kind | Description |
|------|-------------|
| `module` | Module or file |
| `class` | Class definition |
| `struct` | Struct definition |
| `enum` | Enum definition |
| `trait` | Trait definition |
| `function` | Function definition |
| `method` | Method within class/struct |
| `field` | Field within struct/class |
| `variable` | Variable declaration |
| `constant` | Constant value |
| `constructor` | Constructor |
| `interface` | Interface definition |
| `type_alias` | Type alias |
| `parameter` | Function parameter |

---

## Supported Languages

| Language | Extensions | Parser |
|----------|------------|--------|
| Python | `.py` | tree-sitter-python |
| Rust | `.rs` | tree-sitter-rust |
| JavaScript | `.js` | tree-sitter-javascript |
| TypeScript | `.ts`, `.tsx` | tree-sitter-typescript |

---

## Testing

```bash
# Run all tests
cargo test --lib

# Run with output
cargo test --lib -- --nocapture

# Run specific test
cargo test test_get_file_symbols
```
