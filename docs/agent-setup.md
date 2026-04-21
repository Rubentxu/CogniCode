# Agent Setup Guide

This guide explains how to configure AI agents (such as Claude Desktop) to use CogniCode via the Model Context Protocol (MCP).

## Overview

CogniCode provides a **Super-LSP** server that offers advanced code analysis and refactoring capabilities to AI agents through MCP. This enables AI assistants to:

- Navigate call hierarchies and symbol relationships
- Perform safe refactoring operations with impact analysis
- Analyze code complexity and architecture health
- Search for code patterns structurally

## Architecture

```
┌─────────────────────┐     MCP (stdin/stdout)     ┌─────────────────────┐
│   AI Agent          │◄───────────────────────────►│   CogniCode MCP     │
│   (Claude Desktop)  │                            │   Server            │
└─────────────────────┘                            └─────────┬───────────┘
                                                              │
                                                    ┌─────────▼───────────┐
                                                    │   CogniCode Core    │
                                                    │   - Domain          │
                                                    │   - Application     │
                                                    │   - Infrastructure  │
                                                    └─────────────────────┘
```

## MCP Server Configuration

### Connection Method

CogniCode uses **stdin/stdout** for MCP communication, which is the standard transport mechanism for local agent integrations.

### Server Binary

The MCP server binary is built alongside the main binary:

```bash
# Build all binaries including MCP server
cargo build --release

# The MCP server binary will be at:
# target/release/cognicode-mcp
```

### Claude Desktop Configuration

To connect Claude Desktop to CogniCode, edit your Claude Desktop configuration file:

**macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`
**Linux:** `~/.config/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "cognicode": {
      "command": "/path/to/cognicode-mcp",
      "args": [],
      "env": {
        "COGNICODE_WORKSPACE": "/path/to/your/project"
      }
    }
  }
}
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `COGNICODE_WORKSPACE` | Root workspace directory for analysis | Current directory |
| `COGNICODE_MAX_FILE_SIZE` | Maximum file size to process (bytes) | 10485760 (10MB) |
| `COGNICODE_MAX_RESULTS` | Maximum results per query | 10000 |
| `COGNICODE_MAX_QUERY_LENGTH` | Maximum query string length | 1000 |
| `COGNICODE_RATE_LIMIT` | Requests per minute | 100 |
| `RUST_LOG` | Logging level | info |

### MCP Server Arguments

| Argument | Description |
|----------|-------------|
| `--port N` | TCP port for TCP-mode (optional, stdin/stdout is default) |
| `--workspace PATH` | Set workspace directory |
| `--verbose` | Enable verbose logging |

## MCP Protocol Details

### Transport

- **Default**: stdio (stdin/stdout)
- **Optional**: TCP socket mode with `--port`

### JSON-RPC 2.0

CogniCode implements JSON-RPC 2.0 specification:

```json
// Request
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "get_call_hierarchy",
    "arguments": {
      "symbol_name": "module::function",
      "direction": "outgoing",
      "depth": 1
    }
  },
  "id": 1
}

// Response (Success)
{
  "jsonrpc": "2.0",
  "result": { ... },
  "id": 1
}

// Response (Error)
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Security error: Path traversal attempt"
  },
  "id": 1
}
```

### Error Codes

| Code | Meaning |
|------|---------|
| -32600 | Invalid Request |
| -32601 | Method Not Found |
| -32602 | Invalid Parameters |
| -32603 | Internal Error |
| -32000 | Security Error |
| -32001 | Application Error |
| -32002 | Invalid Input |
| -32003 | Not Found |

## Available Tools

Once connected, the following tools are available:

| Tool | Purpose |
|------|---------|
| `get_call_hierarchy` | Navigate call graphs (incoming/outgoing) |
| `get_file_symbols` | Extract symbols from a file |
| `find_usages` | Find all usages of a symbol |
| `structural_search` | Search by AST pattern |
| `analyze_impact` | Analyze change impact |
| `check_architecture` | Detect cycles and violations |
| `safe_refactor` | Execute validated refactoring |
| `validate_syntax` | Quick syntax validation |
| `get_complexity` | Calculate complexity metrics |

See [mcp-tools-reference.md](mcp-tools-reference.md) for detailed tool documentation.

## Security Model

CogniCode implements several security measures:

### Input Validation

- **Path traversal prevention**: Blocks `..`, `~/`, `$`, `${` patterns
- **Workspace boundary enforcement**: Files must be within configured workspace
- **Query length limits**: Prevents DoS via oversized queries
- **Result size limits**: Caps maximum returned results

### Rate Limiting

- Token bucket algorithm: 100 requests per minute (configurable)
- Per-request validation of all inputs

### File Size Limits

- Default maximum: 10MB per file
- Configurable via environment

## Troubleshooting

### Connection Issues

**Problem**: Agent reports "connection refused" or timeout

**Solutions**:
1. Verify the binary path is correct and executable
2. Check that `cognicode-mcp` binary exists at the specified path
3. Try running the binary manually to see error messages:
   ```bash
   ./target/release/cognicode-mcp --verbose
   ```

### Security Errors

**Problem**: "Path traversal attempt" errors

**Solutions**:
1. Use relative paths within the workspace
2. Ensure `COGNICODE_WORKSPACE` is set correctly
3. Avoid absolute paths unless workspace includes `/`

**Problem**: "Rate limit exceeded"

**Solution**: Wait 60 seconds or increase `COGNICODE_RATE_LIMIT`

### Analysis Issues

**Problem**: Empty results for valid symbols

**Solutions**:
1. Verify the symbol name is fully qualified (e.g., `module::function`)
2. Check that the file is in the workspace
3. Use `--verbose` logging to see parsing details

## Example Claude Desktop Configurations

### Basic Configuration

```json
{
  "mcpServers": {
    "cognicode": {
      "command": "/usr/local/bin/cognicode-mcp"
    }
  }
}
```

### With Custom Workspace

```json
{
  "mcpServers": {
    "cognicode": {
      "command": "/usr/local/bin/cognicode-mcp",
      "env": {
        "COGNICODE_WORKSPACE": "/Users/me/Projects/myapp"
      }
    }
  }
}
```

### With Verbose Logging

```json
{
  "mcpServers": {
    "cognicode": {
      "command": "/usr/local/bin/cognicode-mcp",
      "env": {
        "RUST_LOG": "debug"
      }
    }
  }
}
```

### Multi-Project Setup

For agents that work across multiple projects:

```json
{
  "mcpServers": {
    "cognicode-backend": {
      "command": "/usr/local/bin/cognicode-mcp",
      "env": {
        "COGNICODE_WORKSPACE": "/Users/me/Projects/backend"
      }
    },
    "cognicode-frontend": {
      "command": "/usr/local/bin/cognicode-mcp",
      "env": {
        "COGNICODE_WORKSPACE": "/Users/me/Projects/frontend"
      }
    }
  }
}
```

## Testing the Connection

After configuration, restart Claude Desktop and try:

```
List the symbols in src/main.rs
```

Or:

```
Find all usages of the function calculate_total
```

If successful, CogniCode will return the requested information.

## Additional Resources

- [Architecture Documentation](architecture.md)
- [Conceptual Overview](concept.md)
- [MCP Tools Reference](mcp-tools-reference.md)
- [CLI Reference](cli-reference.md)
