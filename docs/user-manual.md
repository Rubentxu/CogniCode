# CogniCode User Manual

This manual provides practical guidance for end-users who want to install, configure, and use CogniCode.

## Table of Contents

1. [Installation](#installation)
2. [Quick Start](#quick-start)
3. [CLI Usage](#cli-usage)
4. [Configuration](#configuration)
5. [Troubleshooting](#troubleshooting)

---

## Installation

### Prerequisites

- **Rust 1.70+**: Required for building from source
- **Cargo**: Rust package manager (comes with Rust)

### Build from Source

```bash
# Clone the repository
git clone https://github.com/your-org/cognicode.git
cd cognicode

# Build release version
cargo build --release

# The binaries will be in target/release/
ls target/release/
# cognicode      - Main CLI
# cognicode-mcp  - MCP server binary
# cognicode-lsp  - LSP server binary
```

### Verify Installation

```bash
# Check CLI version
./target/release/cognicode --version

# Expected output:
# cognicode 0.1.0
```

### Directory Setup

Create a working directory for CogniCode:

```bash
# Create a bin directory in your home
mkdir -p ~/bin

# Copy binaries
cp target/release/cognicode ~/bin/
cp target/release/cognicode-mcp ~/bin/
cp target/release/cognicode-lsp ~/bin/

# Add to PATH (add to ~/.bashrc or ~/.zshrc)
export PATH="$HOME/bin:$PATH"

# Reload shell
source ~/.bashrc
```

---

## Quick Start

### CLI Basic Usage

```bash
# Analyze a directory
cognicode analyze ./src

# Start MCP server (for AI agents)
cognicode serve --port 8080

# Perform a refactoring check
cognicode refactor --operation rename "OldName" --new-name "NewName"
```

### MCP Server Quick Start

For AI agent integration:

```bash
# Start the MCP server
cognicode-mcp --workspace /path/to/your/project

# Server will listen on stdin/stdout by default
# Configure your AI agent to connect to this server
```

---

## CLI Usage

### Global Options

| Option | Description |
|--------|-------------|
| `-v, --verbose` | Enable verbose/debug logging |
| `--help` | Show help information |
| `--version` | Show version information |

### Commands Overview

```
cognicode [OPTIONS] [COMMAND]

Commands:
  analyze    Analyze code in the given directory
  serve      Start the MCP server
  refactor   Perform a refactoring operation
  help       Print this help message
```

### Analyze Command

Analyze code in a directory for symbols, dependencies, and complexity.

```bash
cognicode analyze [PATH] [OPTIONS]

Arguments:
  PATH    Directory to analyze (default: current directory)

Options:
  -v, --verbose    Enable verbose output
```

**Examples**:

```bash
# Analyze current directory
cognicode analyze

# Analyze specific directory
cognicode analyze ./src

# Analyze with verbose output
cognicode analyze ./src --verbose
```

### Serve Command

Start the MCP server for AI agent connections.

```bash
cognicode serve [OPTIONS]

Options:
  -p, --port <PORT>    Port to listen on (default: 8080)
  -v, --verbose        Enable verbose output
```

**Examples**:

```bash
# Start server on default port
cognicode serve

# Start server on custom port
cognicode serve --port 9090

# Start with verbose logging
cognicode serve --verbose
```

### Refactor Command

Perform refactoring operations on code symbols.

```bash
cognicode refactor [OPTIONS]

Options:
  -o, --operation <OPERATION>    Refactoring operation (default: rename)
  -s, --symbol <SYMBOL>          Symbol to refactor
  -n, --new-name <NAME>          New name (for rename operation)
```

**Operations**:

| Operation | Description |
|-----------|-------------|
| `rename` | Rename a symbol |
| `extract` | Extract code into a function |
| `inline` | Inline a function |
| `move` | Move a symbol |
| `change-signature` | Change function parameters |

**Examples**:

```bash
# Rename a symbol
cognicode refactor --operation rename --symbol "process_order" --new-name "handle_order"

# Extract a function
cognicode refactor --operation extract --symbol "order.total()"

# Change function signature
cognicode refactor --operation change-signature --symbol "create_user"
```

---

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `COGNICODE_WORKSPACE` | Root workspace directory | Current directory |
| `COGNICODE_MAX_FILE_SIZE` | Max file size in bytes | 10485760 (10MB) |
| `COGNICODE_MAX_RESULTS` | Max results per query | 10000 |
| `COGNICODE_MAX_QUERY_LENGTH` | Max query string length | 1000 |
| `COGNICODE_RATE_LIMIT` | Requests per minute | 100 |
| `RUST_LOG` | Logging level | info |

### Configuration File

CogniCode reads configuration from `cognicode.toml` in the project root:

```toml
[workspace]
path = "/path/to/project"
max_file_size = 10485760

[security]
rate_limit = 100
max_query_length = 1000
max_results = 10000

[logging]
level = "info"
```

### Workspace Configuration

The workspace is the root directory for code analysis:

```bash
# Via environment variable
export COGNICODE_WORKSPACE=/path/to/project
cognicode analyze

# Via cognicode.toml
[workspace]
path = "/path/to/project"
```

### Security Settings

```toml
[security]
# Path traversal prevention is always enabled
# Rate limiting
rate_limit = 100  # requests per minute

# Query limits
max_query_length = 1000  # characters
max_results = 10000       # per query

# File size limits
max_file_size = 10485760  # 10MB
```

---

## Troubleshooting

### Common Issues

#### Issue: "Binary not found"

**Symptom**: `bash: cognicode: command not found`

**Solution**:
```bash
# Check if binary exists
ls -la ~/bin/cognicode

# If not, copy from build directory
cp target/release/cognicode ~/bin/

# Ensure ~/bin is in PATH
echo $PATH | grep -q ~/bin || export PATH="$HOME/bin:$PATH"
```

#### Issue: "Connection refused" on serve

**Symptom**: Cannot connect to MCP server on specified port

**Solutions**:
1. Check if another process is using the port:
   ```bash
   lsof -i :8080
   # or
   netstat -an | grep 8080
   ```

2. Try a different port:
   ```bash
   cognicode serve --port 9090
   ```

3. Check firewall settings:
   ```bash
   # Allow port 8080
   sudo ufw allow 8080/tcp
   ```

#### Issue: "Path traversal attempt" errors

**Symptom**: Security error when using file paths

**Solution**: Use relative paths within your workspace:
```bash
# Instead of absolute paths
cognicode analyze ./src

# Or set workspace and use relative paths
export COGNICODE_WORKSPACE=/home/user/project
cd /home/user/project
cognicode analyze ./src
```

#### Issue: "Rate limit exceeded"

**Symptom**: Too many requests error

**Solution**:
1. Wait 60 seconds for rate limit reset
2. Or increase limit in environment:
   ```bash
   export COGNICODE_RATE_LIMIT=200
   ```

#### Issue: Empty analysis results

**Symptom**: Analysis completes but returns no symbols

**Possible Causes**:
1. Workspace not set correctly
2. File type not supported
3. Parsing errors in source files

**Debugging**:
```bash
# Run with verbose logging
cognicode analyze ./src --verbose

# Check output for parsing errors
# Look for "Parse error" or "Symbol not found" messages
```

#### Issue: MCP server disconnects

**Symptom**: AI agent disconnects from MCP server

**Solutions**:
1. Check server logs for errors
2. Verify stdin/stdout connection
3. Try TCP mode instead:
   ```bash
   cognicode-mcp --port 8080
   # Then connect via TCP instead of stdio
   ```

### Logging

Enable detailed logging for troubleshooting:

```bash
# Debug level logging
export RUST_LOG=debug
cognicode analyze ./src --verbose

# Or for a specific module
export RUST_LOG=cognicode=debug
cognicode serve --verbose
```

### Performance Issues

#### Issue: Slow analysis on large codebase

**Solutions**:
1. Limit analysis scope:
   ```bash
   # Analyze specific subdirectory
   cognicode analyze ./src/module1
   ```

2. Increase rate limit for MCP:
   ```bash
   export COGNICODE_RATE_LIMIT=200
   ```

3. Exclude unnecessary directories:
   ```toml
   [workspace]
   exclude = ["**/tests/**", "**/target/**", "**/node_modules/**"]
   ```

### Getting Help

```bash
# Show all available commands
cognicode --help

# Show help for specific command
cognicode analyze --help
cognicode serve --help
cognicode refactor --help
```

### Reporting Bugs

When reporting issues, include:

1. **Version**: `cognicode --version`
2. **Command**: The exact command that failed
3. **Environment**: OS, Rust version, workspace path
4. **Logs**: Verbose output with `RUST_LOG=debug`
5. **Minimal reproduction**: Smallest code sample that demonstrates the issue

---

## Additional Resources

- [Architecture Documentation](architecture.md)
- [Conceptual Overview](concept.md)
- [Agent Setup Guide](agent-setup.md)
- [CLI Reference](cli-reference.md)
- [MCP Tools Reference](mcp-tools-reference.md)
