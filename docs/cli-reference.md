# CLI Reference

Complete reference for CogniCode command-line interface.

## Table of Contents

1. [Synopsis](#synopsis)
2. [Global Options](#global-options)
3. [Commands](#commands)
4. [Output Formats](#output-formats)
5. [Exit Codes](#exit-codes)
6. [Examples](#examples)

---

## Synopsis

```bash
cognicode [OPTIONS] [COMMAND]
```

The CogniCode CLI provides commands for code analysis, MCP server management, and refactoring operations.

---

## Global Options

| Option | Description | Default |
|--------|-------------|---------|
| `-v, --verbose` | Enable verbose logging (sets RUST_LOG=debug) | false |
| `-h, --help` | Print help information | - |
| `--version` | Print version information | - |

---

## Commands

### analyze

Analyze code in a directory for symbols, dependencies, and metrics.

```bash
cognicode analyze [PATH] [OPTIONS]
```

**Arguments**:

| Argument | Description | Default |
|----------|-------------|---------|
| `PATH` | Directory to analyze | Current directory (`.`) |

**Options**:

| Option | Description |
|--------|-------------|
| `-v, --verbose` | Enable verbose output |

**Output Format**:

```
Analyzing: ./src
  Found 42 symbols
  3 cycles detected
  Average complexity: 4.2
Analysis completed in 125ms
```

---

### serve

Start the MCP server for AI agent connections.

```bash
cognicode serve [OPTIONS]
```

**Options**:

| Option | Description | Default |
|--------|-------------|---------|
| `-p, --port <PORT>` | TCP port to listen on | 8080 |
| `-v, --verbose` | Enable verbose logging | false |

**Output Format**:

```
Starting MCP server...
Listening on port 8080
MCP server ready
```

**Connection Modes**:

1. **TCP Mode** (default with --port):
   ```bash
   cognicode serve --port 8080
   # Connects via TCP socket
   ```

2. **Stdio Mode** (default without --port):
   ```bash
   cognicode serve
   # Listens on stdin/stdout for MCP protocol
   ```

---

### refactor

Perform refactoring operations on code symbols.

```bash
cognicode refactor [OPTIONS]
```

**Options**:

| Option | Description | Default |
|--------|-------------|---------|
| `-o, --operation <OPERATION>` | Refactoring operation | rename |
| `-s, --symbol <SYMBOL>` | Symbol to refactor | - |
| `-n, --new-name <NAME>` | New name (for rename) | - |
| `-v, --verbose` | Enable verbose output | false |

**Operations**:

| Operation | Description |
|-----------|-------------|
| `rename` | Rename a symbol across the codebase |
| `extract` | Extract code into a new function |
| `inline` | Inline a function into its callers |
| `move` | Move a symbol to a different location |
| `change-signature` | Modify function parameters |

---

### help

Print help information.

```bash
cognicode help [COMMAND]
```

**Arguments**:

| Argument | Description |
|----------|-------------|
| `COMMAND` | Optional specific command to get help for |

---

## Output Formats

### Default Output (Human-Readable)

```
$ cognicode analyze ./src
Analyzing: ./src
  Processing files...  [####################] 100%
  Found 156 symbols in 23 files
  Complexity analysis complete
  Average cyclomatic complexity: 3.4

Results:
  High complexity functions (>10):
    - calculate_totals (src/order.rs:42) - complexity: 12
    - process_payment (src/payment.rs:88) - complexity: 15

Analysis completed in 342ms
```

### JSON Output (Machine-Readable)

When output is piped or redirected, CogniCode may output JSON:

```json
{
  "status": "success",
  "command": "analyze",
  "results": {
    "symbols": 156,
    "files": 23,
    "complexity": {
      "average": 3.4,
      "max": 15,
      "high_risk": 2
    }
  },
  "timing_ms": 342
}
```

### Verbose Output (Debug)

```
$ cognicode analyze ./src --verbose
DEBUG [cognicode] Initializing parser for Rust
DEBUG [cognicode] Loading workspace from ./src
DEBUG [cognicode] Found 47 .rs files
DEBUG [cognicode] Parsing file: src/main.rs
DEBUG [cognicode] Found 5 symbols in src/main.rs
DEBUG [cognicode] Building call graph...
DEBUG [cognicode] Running cycle detection...
INFO  [cognicode] Analysis complete
Analyzing: ./src
  Found 156 symbols
Analysis completed in 342ms
```

---

## Exit Codes

| Code | Description |
|------|-------------|
| `0` | Success |
| `1` | General error |
| `2` | Invalid arguments |
| `3` | Analysis failed |
| `4` | Server error |
| `5` | Security error (path traversal, rate limit) |

**Examples**:

```bash
# Success
cognicode analyze ./src
echo $?  # 0

# Invalid arguments
cognicode analyze --nonexistent-flag
echo $?  # 2

# Analysis failure
cognicode analyze ./empty_dir
echo $?  # 3
```

---

## Examples

### Basic Analysis

```bash
# Analyze current directory
cognicode analyze

# Analyze specific directory
cognicode analyze ./src

# Analyze with verbose output
cognicode analyze ./src --verbose
```

### Server Management

```bash
# Start MCP server on default port (8080)
cognicode serve

# Start server on custom port
cognicode serve --port 9090

# Start server with debug logging
cognicode serve --verbose
```

### Refactoring

```bash
# Rename a symbol
cognicode refactor \
  --operation rename \
  --symbol "process_order" \
  --new-name "handle_order"

# Extract a function
cognicode refactor \
  --operation extract \
  --symbol "order.total()"

# Change function signature
cognicode refactor \
  --operation change-signature \
  --symbol "create_user"
```

### Help

```bash
# Show general help
cognicode --help

# Show analyze command help
cognicode analyze --help

# Show refactor command help
cognicode refactor --help

# Show serve command help
cognicode serve --help
```

### Piping and Scripting

```bash
# Capture exit code
cognicode analyze ./src && echo "Success" || echo "Failed"

# Use in scripts
#!/bin/bash
if cognicode analyze ./src --verbose; then
  echo "Analysis successful"
else
  exit 1
fi

# Chain commands
cognicode analyze ./src -v | grep -i "complexity"
```

### Error Handling

```bash
# Check exit code
cognicode analyze ./nonexistent 2>/dev/null
exit_code=$?
if [ $exit_code -ne 0 ]; then
  echo "Analysis failed with code: $exit_code"
fi

# Capture error output
cognicode analyze ./src 2>&1 | grep -i error
```

---

## Additional Resources

- [User Manual](user-manual.md)
- [Agent Setup Guide](agent-setup.md)
- [MCP Tools Reference](mcp-tools-reference.md)
