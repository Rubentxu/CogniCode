<p align="center">
  <h1 align="center">CogniCode</h1>
  <p align="center">
    <strong>Super-LSP code intelligence server for AI agents</strong>
  </p>
  <p align="center">
    <a href="#features">Features</a> · <a href="#installation">Installation</a> · <a href="#mcp-tools">MCP Tools</a> · <a href="#cli">CLI</a> · <a href="#architecture">Architecture</a> · <a href="README.es.md">Español</a>
  </p>
</p>

---

CogniCode is a Rust-based code intelligence server that provides deep analysis, call graphs, semantic search, and safe refactoring to AI agents via the [Model Context Protocol (MCP)](https://modelcontextprotocol.io). Think IntelliJ IDEA's capabilities — exposed as tools your AI can call.

Built with **Domain-Driven Design** and **Clean Architecture**, it supports six languages out of the box.

## Features

- **32+ MCP tools** — call graphs, impact analysis, semantic search, safe refactoring, complexity metrics, and more
- **6 languages** — Rust, Python, TypeScript, JavaScript, Go, Java (via Tree-sitter)
- **4 graph strategies** — `full`, `lightweight`, `on_demand`, `per_file`
- **Persistent graph cache** — RedbGraphStore survives across sessions (embedded `redb` database)
- **Safe refactoring** — rename, extract, inline, move, change signature with impact preview
- **LSP navigation** — go-to-definition, hover, find references
- **Architecture analysis** — cycle detection (Tarjan SCC), risk assessment, hot-path identification, dead code detection
- **Mermaid export** — generate call graph diagrams as code or rendered SVG
- **Context compression** — return natural language summaries instead of raw JSON
- **Sandbox orchestrator** — automated scenario testing and benchmarking
- **Zero-config startup** — works out of the box with `cognicode-mcp --cwd /your/project`
- **OpenTelemetry integration** — metrics and observability support

## Installation

### Pre-built binary

Download the latest release from [GitHub Releases](https://github.com/Rubentxu/CogniCode/releases):

```bash
# Linux (x86_64)
chmod +x cognicode-mcp
./cognicode-mcp --cwd /path/to/your/project
```

### From source

```bash
git clone https://github.com/Rubentxu/CogniCode.git
cd CogniCode
cargo build --release -p cognicode-mcp
./target/release/cognicode-mcp --cwd /path/to/your/project
```

### Claude Desktop / Cursor / Windsurf

Add CogniCode as an MCP server in your AI client configuration:

```json
{
  "mcpServers": {
    "cognicode": {
      "command": "cognicode-mcp",
      "args": ["--cwd", "/path/to/your/project"]
    }
  }
}
```

## Quick Start

1. **Build the graph** — CogniCode needs to analyze your project first:

```json
{ "tool": "build_graph", "arguments": { "directory": "/path/to/project" } }
```

2. **Analyze impact** of changing a symbol:

```json
{ "tool": "analyze_impact", "arguments": { "symbol_name": "my_function" } }
```

3. **Trace execution path** between two symbols:

```json
{ "tool": "trace_path", "arguments": { "source": "main", "target": "handle_request" } }
```

4. **Find hot paths** — most-called functions:

```json
{ "tool": "get_hot_paths", "arguments": { "limit": 10, "min_fan_in": 3 } }
```

5. **Refactor safely** with impact preview:

```json
{
  "tool": "safe_refactor",
  "arguments": {
    "action": "rename",
    "new_name": "new_function_name",
    "file_path": "src/lib.rs",
    "line": 42,
    "column": 4
  }
}
```

## MCP Tools

### Graph Analysis (12 tools)

| Tool | Description |
|------|-------------|
| `build_graph` | Build call graph for a project. Persists to disk automatically. |
| `get_call_hierarchy` | Traverse callers/callees of a symbol. |
| `analyze_impact` | Analyze impact of changing a symbol. Returns risk level. |
| `check_architecture` | Detect cycles and architecture violations (Tarjan SCC). |
| `get_entry_points` | Find entry-point symbols (no incoming edges). |
| `get_leaf_functions` | Find leaf functions (no outgoing edges). |
| `get_hot_paths` | Find most-called functions by fan-in. |
| `trace_path` | Find execution path between two symbols (BFS). |
| `export_mermaid` | Export call graph as Mermaid flowchart or SVG. |
| `build_lightweight_index` | Build fast symbol-only index. |
| `query_symbol_index` | Case-insensitive symbol lookup. |
| `find_dead_code` | Find unused symbols across the project. |

### Graph Operations (5 tools)

| Tool | Description |
|------|-------------|
| `build_call_subgraph` | Build on-demand subgraph centered on a symbol. |
| `get_per_file_graph` | Get call graph for a single file. |
| `merge_graphs` | Merge graphs from multiple files. |
| `get_module_dependencies` | Analyze module-level dependencies. |
| `get_all_symbols` | Get all symbols in the workspace. |

### Symbols & Semantics (9 tools)

| Tool | Description |
|------|-------------|
| `get_file_symbols` | Extract symbols from a file. Supports compressed summaries. |
| `get_outline` | Hierarchical symbol outline (tree structure). |
| `get_symbol_code` | Get full source code of a symbol including docstrings. |
| `get_complexity` | Cyclomatic, cognitive, and nesting complexity metrics. |
| `semantic_search` | Fuzzy symbol search with kind filtering. |
| `find_usages` | Find all usages of a symbol across the project. |
| `find_usages_with_context` | Find usages with surrounding context lines. |
| `structural_search` | AST-based structural pattern matching. |
| `validate_syntax` | Validate file syntax using Tree-sitter. |

### LSP Navigation (3 tools)

| Tool | Description |
|------|-------------|
| `go_to_definition` | Navigate to symbol definition. |
| `hover` | Get type info and documentation. |
| `find_references` | Find all references to a symbol. |

### File Operations (5 tools)

| Tool | Description |
|------|-------------|
| `read_file` | Smart file reader with outline/symbols/compressed modes. |
| `search_content` | Regex search with .gitignore awareness. |
| `list_files` | List project files with glob filtering. |
| `write_file` | Create or overwrite files within workspace. |
| `edit_file` | Edit files with syntax validation. |

### Refactoring (1 tool)

| Tool | Description |
|------|-------------|
| `safe_refactor` | Safe refactoring with validation and preview (rename, extract, inline, move, change signature). |

## CLI

CogniCode ships with a full-featured CLI (`cognicode`) for direct terminal use:

```
cognicode analyze [path]                          # Full code analysis
cognicode doctor [--format text|json]             # Check environment setup

cognicode index build [path] [--strategy full|lightweight|per_file|on_demand]
cognicode index query <symbol> [path]
cognicode index outline <file>
cognicode index symbol-code <file> <line> <col>

cognicode graph full [--rebuild] [path]
cognicode graph hot-paths [-n 10] [path]
cognicode graph trace-path <from> <to> [path]
cognicode graph mermaid [path] [--format svg|txt]
cognicode graph complexity [path]
cognicode graph impact <symbol> [path]

cognicode refactor rename|extract|inline|move <symbol> [new_name]

cognicode navigate definition|hover|references <file:line:col> [path]
```

## Graph Strategies

Choose the right strategy for your use case:

| Strategy | Speed | Edges | Best For |
|----------|-------|-------|----------|
| `lightweight` | Fastest | None | Symbol lookups, search |
| `on_demand` | Fast | Targeted | Analyzing specific functions |
| `per_file` | Medium | Per-file | Modular analysis |
| `full` | Slowest | Complete | Impact analysis, hot paths, architecture checks |

## Supported Languages

| Language | Extensions |
|----------|------------|
| Rust | `.rs` |
| Python | `.py` |
| TypeScript | `.ts`, `.tsx` |
| JavaScript | `.js`, `.jsx` |
| Go | `.go` |
| Java | `.java` |

## Architecture

CogniCode follows **Domain-Driven Design** with Clean Architecture and 4 bounded contexts:

```
┌──────────────────────────────────────────────────────────────┐
│                      COGNICODE                               │
│                                                               │
│  ┌────────────────┐  ┌────────────────┐  ┌──────────────┐  │
│  │   DOMAIN       │  │  APPLICATION   │  │ INFRASTRUCTURE│ │
│  │   (Core)       │  │   (Services)   │  │  (Impl)       │ │
│  └───────┬────────┘  └───────┬────────┘  └──────┬───────┘  │
│          │                    │                   │          │
│          └────────────────────┼───────────────────┘          │
│                               │                              │
│                    ┌──────────┴──────────┐                    │
│                    │     INTERFACE       │                    │
│                    │   (MCP, LSP, CLI)  │                    │
│                    └────────────────────┘                    │
└──────────────────────────────────────────────────────────────┘
```

**Domain Context** (Core business logic):
- Aggregates: `Symbol`, `CallGraph`, `Refactor`
- Value Objects: `Location`, `SourceRange`, `DependencyType`
- Domain Services: `ImpactAnalyzer`, `CycleDetector`, `ComplexityCalculator`
- Traits: `CodeIntelligenceProvider`, `DependencyRepository`, `RefactorStrategy`

**Application Context** (Orchestration):
- Services: `NavigationService`, `RefactorService`, `AnalysisService`
- DTOs: Request/response contracts
- Commands: Use case orchestrators

**Infrastructure Context** (Implementations):
- Parsers: `TreeSitterParser`
- Graph Stores: `PetGraphStore`, `RedbGraphStore`
- LSP: `LspClient`
- Persistence: `RedbGraphStore` (embedded `redb` database)

**Interface Context** (External protocols):
- MCP Server (Model Context Protocol)
- CLI Commands
- LSP Server

**Key design decisions:**

- **Trait-based strategies** — Graph building, refactoring, and parsing are pluggable via traits
- **ArcSwap graph cache** — Atomic, lock-free reads across async tasks
- **Rayon parallelism** — Heavy computation runs on a dedicated thread pool (8MB stack per thread)
- **Workspace sandboxing** — All file operations are restricted to the declared workspace
- **Cancellation propagation** — MCP cancellation tokens flow through all handlers
- **OpenTelemetry metrics** — Built-in observability with OTLP export

## Workspace Crates

| Crate | Description |
|-------|-------------|
| `cognicode` | Shared types and utilities |
| `cognicode-core` | Domain logic, application services, infrastructure |
| `cognicode-mcp` | MCP server (`cognicode-mcp`) and test client (`mcp-client`) |
| `cognicode-cli` | Terminal interface (`cognicode`) |
| `cognicode-sandbox` | Automated scenario testing and benchmarking |
| `rcode-debug` | Time-travel debugging integration (Chronos MCP) |

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | OpenTelemetry metrics endpoint |

### Feature Flags (`cognicode-core`)

| Feature | Default | Description |
|---------|---------|-------------|
| `persistence` | enabled | RedbGraphStore for persistent graph cache |
| `rig` | disabled | `rig-core` AI agent framework integration |

## Development

```bash
# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Build release binary
cargo build --release -p cognicode-mcp

# Check environment
cargo run -p cognicode-cli -- doctor
```

## Using with AI Agents

CogniCode is designed to be the **code intelligence backbone** for AI agents.
Instead of asking an agent to read files and guess at structure, you give it
tools that return precise, structured answers.

📖 **[docs/agent-prompts.md](docs/agent-prompts.md)** contains 20 ready-to-use
prompt scenarios with full reasoning chains and tool call sequences. Here's a
taste:

---

### Onboarding a New Codebase

> *"I just cloned this repo. Help me understand what it does, what the main
> entry points are, and which functions are called the most."*

**Agent reasoning:** Build the full graph first, then get entry points (public
API surface), leaf functions (low-level primitives), and hot paths (most
interconnected code). Together these three give a 360° view of any unfamiliar
codebase.

```
1. build_graph      → strategy: "full"
2. get_entry_points → the public API surface
3. get_leaf_functions → low-level primitives
4. get_hot_paths    → min_fan_in: 3  (the load-bearing functions)
```

---

### Analyzing Change Impact Before a PR

> *"I'm about to change `UserRepository.find_by_email`. What will break?"*

```
1. build_lightweight_index
2. query_symbol_index  → symbol_name: "find_by_email"
3. analyze_impact      → symbol_name: "UserRepository.find_by_email"
4. get_call_hierarchy  → direction: "incoming", depth: 3
```

The agent gets a risk level (`low` / `medium` / `high`), a list of impacted
files, and the full call chain — before touching a single line of code.

---

These are just 2 of 20 scenarios. The full guide covers **dead code detection,
safe rename refactoring, complexity audits, execution path tracing**, and more.

👉 [Read the full Agent Prompt Guide →](docs/agent-prompts.md) · [Versión en español →](docs/agent-prompts-es.md)

## License

See [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
