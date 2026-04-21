# MCP Tools Reference

Complete reference for CogniCode MCP (Model Context Protocol) tools.

## Table of Contents

1. [Overview](#overview)
2. [Tool List](#tool-list)
3. [Tool Schemas](#tool-schemas)
4. [Example Requests](#example-requests)
5. [Error Handling](#error-handling)
6. [Security](#security)

---

## Overview

CogniCode exposes code analysis and refactoring capabilities through MCP tools. Each tool follows the JSON-RPC 2.0 request/response pattern.

### Transport

- **Protocol**: JSON-RPC 2.0 over stdio or TCP
- **Content-Type**: application/json

### Request Format

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "tool_name",
    "arguments": {
      "param1": "value1"
    }
  },
  "id": 1
}
```

### Response Format (Success)

```json
{
  "jsonrpc": "2.0",
  "result": {
    "tool_specific_result": "..."
  },
  "id": 1
}
```

### Response Format (Error)

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Error description"
  },
  "id": 1
}
```

---

## Tool List

| Tool | Purpose |
|------|---------|
| `get_call_hierarchy` | Navigate call graphs (incoming/outgoing) |
| `get_file_symbols` | Extract all symbols from a file |
| `find_usages` | Find all usages of a symbol |
| `structural_search` | Search by AST pattern |
| `analyze_impact` | Analyze change impact |
| `check_architecture` | Detect cycles and violations |
| `safe_refactor` | Execute validated refactoring |
| `validate_syntax` | Quick syntax validation |
| `get_complexity` | Calculate complexity metrics |

---

## Tool Schemas

### get_call_hierarchy

Navigate the call graph for a symbol.

**Input Schema**:

```json
{
  "symbol_name": "string",      // Required: fully qualified name (e.g., "module::function")
  "direction": "incoming|outgoing",  // Required: "incoming"=who calls this, "outgoing"=what this calls
  "depth": 1,                   // Optional: traversal depth (default: 1, max: 10)
  "include_external": false      // Optional: include external deps (default: false)
}
```

**Output Schema**:

```json
{
  "symbol": "string",
  "calls": [
    {
      "symbol": "string",
      "file": "string",
      "line": 42,
      "column": 5,
      "confidence": 1.0
    }
  ],
  "metadata": {
    "total_calls": 10,
    "analysis_time_ms": 15
  }
}
```

**Direction Values**:

| Value | Description |
|-------|-------------|
| `incoming` | Find all symbols that call this symbol |
| `outgoing` | Find all symbols that this symbol calls |

---

### get_file_symbols

Extract all symbols from a file.

**Input Schema**:

```json
{
  "file_path": "string"          // Required: path to source file
}
```

**Output Schema**:

```json
{
  "file_path": "string",
  "symbols": [
    {
      "name": "string",
      "kind": "function|class|struct|...",
      "location": {
        "file": "string",
        "line": 42,
        "column": 5
      },
      "signature": "string|null"
    }
  ]
}
```

**SymbolKind Values**:

| Value | Description |
|-------|-------------|
| `module` | Module definition |
| `class` | Class definition |
| `struct` | Struct definition |
| `enum` | Enum definition |
| `trait` | Trait definition |
| `function` | Function definition |
| `method` | Method definition |
| `field` | Field definition |
| `variable` | Variable definition |
| `constant` | Constant definition |
| `constructor` | Constructor definition |
| `interface` | Interface definition |
| `type_alias` | Type alias definition |
| `parameter` | Parameter definition |

---

### find_usages

Find all usages of a symbol.

**Input Schema**:

```json
{
  "symbol_name": "string",       // Required: symbol to search for
  "include_declaration": true     // Optional: include definition (default: true)
}
```

**Output Schema**:

```json
{
  "symbol": "string",
  "usages": [
    {
      "file": "string",
      "line": 42,
      "column": 5,
      "context": "string",        // Surrounding code context
      "is_definition": false
    }
  ],
  "total": 10
}
```

---

### structural_search

Search for code patterns using AST-based matching.

**Input Schema**:

```json
{
  "pattern_type": "function_call|type_definition|import_statement|annotation|custom",
  "query": "string",              // Required: search query
  "path": "string|null",          // Optional: file/directory path
  "depth": 1                       // Optional: search depth (default: 1)
}
```

**Output Schema**:

```json
{
  "pattern": "string",
  "matches": [
    {
      "file": "string",
      "line": 42,
      "column": 5,
      "matched_text": "string",
      "context": "string"
    }
  ],
  "total": 5
}
```

**PatternType Values**:

| Value | Description |
|-------|-------------|
| `function_call` | Match function invocations |
| `type_definition` | Match type/class definitions |
| `import_statement` | Match import/require statements |
| `annotation` | Match annotations/decorators |
| `custom` | Custom pattern matching |

---

### analyze_impact

Analyze the impact of changing a symbol.

**Input Schema**:

```json
{
  "symbol_name": "string"         // Required: symbol to analyze
}
```

**Output Schema**:

```json
{
  "symbol": "string",
  "impacted_files": ["string"],
  "impacted_symbols": ["string"],
  "risk_level": "low|medium|high|critical",
  "summary": "string"
}
```

**RiskLevel Values**:

| Value | Description |
|-------|-------------|
| `low` | Change has minimal cascading effects |
| `medium` | Change affects several symbols |
| `high` | Change has significant impact |
| `critical` | Change may break multiple systems |

---

### check_architecture

Check architectural health and detect violations.

**Input Schema**:

```json
{
  "scope": "string|null"          // Optional: specific scope to check
}
```

**Output Schema**:

```json
{
  "cycles": [
    {
      "symbols": ["string"],
      "length": 3
    }
  ],
  "violations": [
    {
      "rule": "string",
      "from": "string",
      "to": "string",
      "severity": "string"
    }
  ],
  "score": 85.5,
  "summary": "string"
}
```

---

### safe_refactor

Perform a refactoring operation with validation.

**Input Schema**:

```json
{
  "action": "rename|extract|inline|move|change_signature",
  "target": "string",             // Required: target symbol
  "params": {}                     // Optional: action-specific parameters
}
```

**Output Schema**:

```json
{
  "action": "string",
  "success": true,
  "changes": [
    {
      "file": "string",
      "old_text": "string",
      "new_text": "string",
      "location": {
        "file": "string",
        "line": 42,
        "column": 5
      }
    }
  ],
  "validation_result": {
    "is_valid": true,
    "warnings": ["string"],
    "errors": []
  },
  "error_message": null
}
```

**RefactorAction Values**:

| Value | Description |
|-------|-------------|
| `rename` | Rename a symbol |
| `extract` | Extract code into function |
| `inline` | Inline a function |
| `move` | Move symbol to another location |
| `change_signature` | Modify function parameters |

---

### validate_syntax

Validate syntax of a source file.

**Input Schema**:

```json
{
  "file_path": "string"           // Required: file to validate
}
```

**Output Schema**:

```json
{
  "file_path": "string",
  "is_valid": true,
  "errors": [
    {
      "line": 42,
      "column": 5,
      "message": "string",
      "severity": "error"
    }
  ],
  "warnings": [
    {
      "line": 42,
      "column": 5,
      "message": "string",
      "severity": "warning"
    }
  ]
}
```

---

### get_complexity

Calculate complexity metrics for code.

**Input Schema**:

```json
{
  "file_path": "string",           // Required: file to analyze
  "function_name": "string|null"   // Optional: specific function
}
```

**Output Schema**:

```json
{
  "file_path": "string",
  "complexity": {
    "cyclomatic": 5,
    "cognitive": 3,
    "lines_of_code": 150,
    "parameter_count": 3,
    "nesting_depth": 4,
    "function_name": "string|null"
  }
}
```

---

## Example Requests

### Example 1: Get Call Hierarchy

**Request**:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "get_call_hierarchy",
    "arguments": {
      "symbol_name": "order::process_order",
      "direction": "outgoing",
      "depth": 2
    }
  },
  "id": 1
}
```

**Response**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "symbol": "order::process_order",
    "calls": [
      {
        "symbol": "order::validate_order",
        "file": "src/order.rs",
        "line": 25,
        "column": 1,
        "confidence": 1.0
      },
      {
        "symbol": "inventory::check_stock",
        "file": "src/order.rs",
        "line": 26,
        "column": 1,
        "confidence": 1.0
      }
    ],
    "metadata": {
      "total_calls": 2,
      "analysis_time_ms": 12
    }
  },
  "id": 1
}
```

### Example 2: Find File Symbols

**Request**:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "get_file_symbols",
    "arguments": {
      "file_path": "src/main.rs"
    }
  },
  "id": 2
}
```

**Response**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "file_path": "src/main.rs",
    "symbols": [
      {
        "name": "main",
        "kind": "function",
        "location": {
          "file": "src/main.rs",
          "line": 1,
          "column": 1
        },
        "signature": "fn main()"
      },
      {
        "name": "Config",
        "kind": "struct",
        "location": {
          "file": "src/main.rs",
          "line": 10,
          "column": 1
        },
        "signature": null
      }
    ]
  },
  "id": 2
}
```

### Example 3: Analyze Impact

**Request**:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "analyze_impact",
    "arguments": {
      "symbol_name": "order::calculate_total"
    }
  },
  "id": 3
}
```

**Response**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "symbol": "order::calculate_total",
    "impacted_files": [
      "src/order.rs",
      "src/checkout.rs",
      "tests/order_test.rs"
    ],
    "impacted_symbols": [
      "checkout::finalize",
      "order::apply_discount",
      "report::generate_summary"
    ],
    "risk_level": "high",
    "summary": "Impact analysis completed in 8ms - 3 files affected, 12 symbols depend on this function"
  },
  "id": 3
}
```

### Example 4: Safe Refactor

**Request**:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "safe_refactor",
    "arguments": {
      "action": "rename",
      "target": "order::process_order",
      "params": {
        "new_name": "order::handle_order"
      }
    }
  },
  "id": 4
}
```

**Response**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "action": "rename",
    "success": true,
    "changes": [
      {
        "file": "src/order.rs",
        "old_text": "process_order",
        "new_text": "handle_order",
        "location": {
          "file": "src/order.rs",
          "line": 42,
          "column": 5
        }
      }
    ],
    "validation_result": {
      "is_valid": true,
      "warnings": ["Consider updating related documentation"],
      "errors": []
    },
    "error_message": null
  },
  "id": 4
}
```

### Example 5: Get Complexity

**Request**:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "get_complexity",
    "arguments": {
      "file_path": "src/order.rs",
      "function_name": "calculate_total"
    }
  },
  "id": 5
}
```

**Response**:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "file_path": "src/order.rs",
    "complexity": {
      "cyclomatic": 8,
      "cognitive": 5,
      "lines_of_code": 45,
      "parameter_count": 2,
      "nesting_depth": 4,
      "function_name": "calculate_total"
    }
  },
  "id": 5
}
```

---

## Error Handling

### Error Response Format

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Human-readable error message",
    "data": {}
  },
  "id": 1
}
```

### Error Codes

| Code | Name | Description |
|------|------|-------------|
| -32600 | InvalidRequest | Malformed JSON-RPC request |
| -32601 | MethodNotFound | Unknown tool name |
| -32602 | InvalidParams | Invalid tool parameters |
| -32603 | InternalError | Server-side error |
| -32000 | SecurityError | Input validation failed |
| -32001 | AppError | Application logic error |
| -32002 | InvalidInput | Invalid input data |
| -32003 | NotFound | Requested resource not found |

### Common Errors

**Path Traversal Attempt**:

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Security error: Path traversal attempt detected: '../etc/passwd'",
    "data": null
  },
  "id": 1
}
```

**Rate Limit Exceeded**:

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Security error: Rate limit exceeded",
    "data": null
  },
  "id": 1
}
```

**Symbol Not Found**:

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32003,
    "message": "Not found: Symbol 'nonexistent::function' not found in workspace",
    "data": null
  },
  "id": 1
}
```

**Invalid Parameters**:

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32602,
    "message": "Invalid params: 'direction' must be 'incoming' or 'outgoing'",
    "data": null
  },
  "id": 1
}
```

---

## Security

All MCP tools are protected by the `InputValidator` which enforces:

### Path Validation

- Prevents path traversal (`..`, `~/`, `$`)
- Validates paths are within workspace
- Checks for null bytes and invalid characters
- Limits path component depth

### Size Limits

| Limit | Default | Description |
|-------|---------|-------------|
| `max_file_size` | 10MB | Maximum file content size |
| `max_query_length` | 1000 | Maximum query string length |
| `max_results` | 10000 | Maximum results per query |

### Rate Limiting

- Token bucket algorithm
- Default: 100 requests per minute
- Configurable via environment variables

### Best Practices for Clients

1. **Always validate responses**: Check `is_valid` fields
2. **Handle errors gracefully**: Implement retry with backoff
3. **Limit result sizes**: Use pagination for large result sets
4. **Sanitize inputs**: Escape special characters in queries

---

## Additional Resources

- [Agent Setup Guide](agent-setup.md)
- [Conceptual Overview](concept.md)
- [Architecture Documentation](architecture.md)
