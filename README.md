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

CogniCode is a Rust-based code intelligence server that provides deep analysis, call graphs, fuzzy symbol search, and safe refactoring to AI agents via the [Model Context Protocol (MCP)](https://modelcontextprotocol.io). Think IntelliJ IDEA's capabilities — exposed as tools your AI can call.

Built with **Domain-Driven Design** and **Clean Architecture**, it analyses 31
languages out of the box — 20 with full acceptance evidence, 11 experimental.

## Features

- **88 MCP tools** — 78 tools and the 10 goals that answer them: call graphs, impact analysis, fuzzy symbol search, safe refactoring, complexity metrics, and more
- **31 languages** — 20 `supported` (Rust, Python, TypeScript, JavaScript, Java, Go, C, C++, C#, Ruby, PHP, Swift, Scala, Haskell, Lua, Bash, JSON, YAML, Kotlin, Groovy) and 11 `experimental`, all via Tree-sitter. The exact split, with per-language evidence, lives in [`product/languages.json`](product/languages.json)
- **3 graph strategies** — `full`, `lightweight`, `on_demand`
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

### One-line bootstrap (Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/Rubentxu/CogniCode/main/install.sh | sh
```

Installs only the `cogh` executable to `~/.cognicode/bin` (checksum-verified against the official GitHub release). Pin a version with `COGNICODE_VERSION=v0.103.0`. Then run `cogh init && cogh install mcp-server --profile reviewer --ide opencode` to install the CLI, MCP daemon and portable skills. See [Distribution and installation](docs/distribution/INSTALL.md) for upgrade, rollback, uninstall, platform support and recovery.

### mise

```bash
mise install "github:Rubentxu/CogniCode[matching=cogh-]"
```

Pin a version with `@v0.103.0`. Both channels install the same published release asset — the digest identity proof is no longer versioned in this tree (known limitation). Ownership contract: when installed via mise, mise owns the Layer-0 `cogh` binary (upgrade with `mise upgrade`); `cogh` itself owns only the Layer-1 runtime (`~/.cognicode`).

### Published, pre-built binary (standalone fallback)

For an unmanaged installation, extract the platform-specific tarball from
[GitHub Releases](https://github.com/Rubentxu/CogniCode/releases) **after**
checking it against the release's `SHA256SUMS`. Its executable is under
`bin/cognicode-mcp` inside the archive, not a bare file in the download
directory. Do not point a persistent IDE configuration to
`target/release/cognicode-mcp`: that is a local development build. Prefer
the managed `~/.cognicode/shims/cognicode-mcp` path installed by `cogh`.

### From source

> **Not the recommended path.** Use `install.sh` (Layer 0) + `cogh install mcp-server` (Layer 1)
> for normal use. From source is for contributors and reproducible builds.

```bash
git clone https://github.com/Rubentxu/CogniCode.git
cd CogniCode
# CLI binaries (cogh, cognicode, cognicode-release)
cargo build --release -p cognicode-cli --features cognicode-core/evidence-kernel
# MCP daemon
cargo build --release -p cognicode-mcp
./target/release/cogh --version         # expect: cogh 0.103.0
./target/release/cognicode --version    # expect: cognicode 0.103.0
./target/release/cognicode-mcp --version # expect: cognicode-mcp 0.103.0
```

> **Note:** the published binaries also include the binaries for the cross-toolchain
> the host does not natively run (e.g. `aarch64-unknown-linux-gnu`); those are produced
> by the release pipeline. Building them locally needs the matching target via
> `rustup target add <triple>` and a configured linker. From-source builds are
> host-native by default.

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
    "target": "my_function",
    "params": { "new_name": "new_function_name", "file_path": "src/lib.rs" }
  }
}
```

## MCP Tools

CogniCode ships **88 MCP tools**: 78 tools and the 10 goals that answer them.
The tables below are a **curated selection**,
grouped by the capability a reader is most likely to care about — not the full
surface. The authoritative list, generated from the code and checked in CI, is
[`crates/cognicode-core/product/tools.json`](crates/cognicode-core/product/tools.json); per-tool input and output schemas are
in [`product/README.md`](product/README.md).

### Graph Analysis (11 tools)

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

### Graph Operations (3 tools)

| Tool | Description |
|------|-------------|
| `build_call_subgraph` | Build on-demand subgraph centered on a symbol. |
| `get_per_file_graph` | Get call graph for a single file. |
| `merge_graphs` | Merge graphs from multiple files. |

### Symbols & Semantics (4 tools)

| Tool | Description |
|------|-------------|
| `get_file_symbols` | Extract symbols from a file. Supports compressed summaries. |
| `get_symbol_code` | Get full source code of a symbol including docstrings. |
| `get_complexity` | Cyclomatic, cognitive, and nesting complexity metrics. |
| `find_usages` | Find all usages of a symbol across the project. |

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

cognicode index build [path] [--strategy full|lightweight|on_demand]
cognicode index query <symbol> [path]
cognicode index outline <file>
cognicode index symbol-code <file> <line> <col>

cognicode graph full [--rebuild] [path]
cognicode graph hot-paths [-n 10] [path]
cognicode graph trace-path <from> <to> [path]
cognicode graph mermaid [path] [--format svg|txt]
cognicode graph complexity [path]
cognicode graph impact <symbol> [path]

cognicode refactor <symbol> [new_name] [--operation rename|extract|inline|move]

cognicode navigate definition|hover|references <file:line:col> [path]
```

## Graph Strategies

Choose the right strategy for your use case:

| Strategy | Speed | Edges | Best For |
|----------|-------|-------|----------|
| `lightweight` | Fastest | None | Symbol lookups, search |
| `on_demand` | Fast | Targeted | Analyzing specific functions |
| `full` | Slowest | Complete | Impact analysis, hot paths, architecture checks |

## Supported Languages

31 languages are parsed and indexed. **20 are `supported`**, meaning they have
focused acceptance evidence for the analysis paths; **11 are `experimental`**,
meaning they parse and index but are not yet held to the same bar. The
authoritative per-language list, with the evidence behind each level, is
[`product/languages.json`](product/languages.json).

| Language | Extensions | Support |
|----------|------------|---------|
| Rust | `.rs` | supported |
| Python | `.py` | supported |
| TypeScript | `.ts`, `.tsx` | supported |
| JavaScript | `.js`, `.jsx` | supported |
| Java | `.java` | supported |
| Go | `.go` | supported |
| C | `.c`, `.h` | supported |
| C++ | `.cpp`, `.hpp`, `.cc` | supported |
| C# | `.cs` | supported |
| Ruby | `.rb` | supported |
| PHP | `.php` | supported |
| Swift | `.swift` | supported |
| Scala | `.scala` | supported |
| Haskell | `.hs` | supported |
| Lua | `.lua` | supported |
| Bash | `.sh`, `.bash` | supported |
| JSON | `.json` | supported |
| YAML | `.yaml`, `.yml` | supported |
| Dart | `.dart` | experimental |
| Elixir | `.ex`, `.exs` | experimental |
| Erlang | `.erl` | experimental |
| Fortran | `.f90` | experimental |
| Groovy | `.groovy` | supported |
| HCL | `.hcl`, `.tf` | experimental |
| Julia | `.jl` | experimental |
| Kotlin | `.kt`, `.kts` | supported |
| PowerShell | `.ps1` | experimental |
| R | `.r` | experimental |
| SystemVerilog | `.sv` | experimental |
| Verilog | `.v` | experimental |
| Zig | `.zig` | experimental |

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
| `cognicode-graph-algos` | SDK library: dependency-free graph algorithms |
| `cognicode-macros` | SDK library: procedural macros used by the domain types |
| `cognicode-graph-wasm` | SDK library: WebAssembly bindings for the graph algorithms |

The `cognicode-core`, `cognicode-graph-algos`, `cognicode-macros` and `cognicode-graph-wasm` crates are the SDK surface. They are published on crates.io; see [Consuming the SDK](docs/distribution/INSTALL.md#consuming-the-sdk) for the exact commands.

## Binaries

Only `cogh`, `cognicode` and `cognicode-mcp` are published as release assets. Every other binary is built from source and has no release asset or compatibility promise.

| Binary | Package | Published |
|--------|---------|-----------|
| `cogh` | `cognicode-cli` | yes |
| `cognicode` | `cognicode-cli` | yes |
| `cognicode-mcp` | `cognicode-mcp` | yes |
| `cognicode-release` | `cognicode-cli` | no (release tooling) |
| `cognicode-mcp-server` | `cognicode-mcp` | no (`src/server.rs`) |
| `mcp-client` | `cognicode-mcp` | no (test client) |
| `sandbox-orchestrator` | `cognicode-sandbox` | no (scenario runner) |
| `cognicode-control-plane` | `cognicode-explorer` | no (Explorer, see [handoff](docs/distribution/EXPLORER_HANDOFF.md)) |
| `explorer-api`, `explorer-mcp` | `cognicode-runtime` | no (need the `ladybug` feature) |

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | OpenTelemetry metrics endpoint; only used when `COGNICODE_TELEMETRY=1` |
| `COGNICODE_TELEMETRY` | unset (off) | `COGNICODE_TELEMETRY=1` opts in to metric export; any other value leaves it off |
| `COGNICODE_LINK_TRACE` | unset (off) | Path of a file that receives one JSON line per tool call (sensitive: may hold paths and symbol names) |

### Feature Flags (`cognicode-core`)

| Feature | Default | Description |
|---------|---------|-------------|
| `persistence` | enabled | RedbGraphStore for persistent graph cache |

## Security and limitations

CogniCode is a local tool: it analyses a repository on your machine and does
not need the network. The boundaries that matter when you run it, stated in
full in [`SECURITY.md`](SECURITY.md):

- **The analysed repository is data, not instructions.** Files, comments and
  tool results are never treated as commands.
- **Read-only mode.** Start the server with `--read-only` to hide the mutating
  tools from `tools/list` and reject them.
- **No telemetry by default.** `COGNICODE_TELEMETRY=1` opts in to metric
  export; no source code is sent. `COGNICODE_LINK_TRACE=<file>` opts in to a
  local record of tool calls; it can hold paths and symbol names, so treat the
  file as sensitive.
- **Streams.** `cognicode-mcp` writes JSON-RPC on stdout and logs on stderr,
  never the reverse.

Limitations: a `Partial`, `Unknown`, `Unsupported` or `Failed` result is not a
complete one, and an analysis answer states its coverage. Report a
vulnerability privately through GitHub Security Advisories, not in a public
issue.

## Troubleshooting

- **The MCP client reports a parse error or "invalid JSON" when it starts the
  server.** The client reads JSON-RPC from stdout, so anything else printed
  there breaks it, for example a wrapper script or a shell profile that echoes.
  `cognicode-mcp` itself writes its logs to stderr; raise them with `RUST_LOG`
  and read them there.
- **The server exits at start with exit status 2.** `--profile` received a
  value that is not a surface profile. The accepted values are `default`,
  `full` and `legacy`; the error on stderr lists them.
- **The server exits at start with exit status 1.** The directory given to
  `--cwd` does not exist; stderr says `Directory '...' does not exist`.
  Pass a path that exists.
- **A language is not analysed.** Run the doctor, which reports which language
  servers are available for the workspace:

```bash
cognicode doctor --cwd . --format json
```

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

Here is a taste of the prompt scenarios, with their reasoning chains and tool
call sequences:

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

## Versioning

- **Current release:** `v0.103.0` — cut from this tree as `v0.103.0`.
  The release published from that tag is the one an unpinned `install.sh`
  serves. The previous published release is
  [`v0.102.1`](https://github.com/Rubentxu/CogniCode/releases/tag/v0.102.1);
  `v0.101.24`, `v0.101.25` and `v0.101.26` carry tags but never reached
  publication, and a tag that was never published does not move.
  v0.103.0 is a minor release: the four SDK library crates are published on
  crates.io, the MCP and sandbox adapters leave the core, and the release
  ships without a provenance attestation (an open key-custody decision).
- **Binaries report:** `0.103.0` (`cogh`, `cognicode`, `cognicode-mcp`).
- **Tag:** [`origin/v0.103.0`](https://github.com/Rubentxu/CogniCode/releases/tag/v0.103.0).
- **Tags without a release:** `v0.99.2`, `v0.100.0`, `v0.101.6`, `v0.101.7`,
  `v0.101.8`, `v0.101.18`, `v0.101.19`, `v0.101.24`, `v0.101.25` and
  `v0.101.26` exist and were never published. Each
  has a changelog entry
  explaining why. `v0.101.6` was cut before the staging-root fix and the Release
  Truth gate repair landed; `v0.101.7` was cut before the candidate pipeline
  compiled; `v0.101.8` is the first cut whose pipeline compiles, and its lane
  built the whole candidate —eleven artifacts that verify against their
  `SHA256SUMS`— but exited non-zero because the install UAT lost its verdict in
  its own cleanup. `v0.101.24` and `v0.101.25` each reached their candidate
  gate and each was turned back by a real defect —the symbol index answering
  by disk order, and then the release smoke filter running as a command—
  and a tag that was never published does not move. This release supersedes
  all of them.
- **Published releases with an incomplete asset set:** `v0.101.11` through
  `v0.101.16` each publish 3 of 8 assets. The cause is recorded in the
  v0.101.17 changelog entry: the `skill-bundles` stage read the
  `cognicode-release skills --published` answer through a pipe, the subcommand
  did not exist in the binary that ran, the loop consumed nothing, and the
  stage passed with zero bundles. The fix checks the answer and asserts the
  count staged.
- **Tracking authority:** the `PRODUCT-1.0` row in [`docs/roadmap/ROADMAP.md` §2](docs/roadmap/ROADMAP.md).
- **Changelog:** [CHANGELOG.md](CHANGELOG.md).

The product is in active community-productization phase. Releases are
derived from `Cargo.toml` `[workspace.package] version` and checked by the
tag/workspace coherence check in `scripts/ci/release-tag-coherence.sh` (R8).

## License

Released under **MIT OR Apache-2.0**, at your option. The full texts are
[`LICENSE-MIT`](LICENSE-MIT) and [`LICENSE-APACHE`](LICENSE-APACHE); see
[`LICENSE`](LICENSE) for the grant itself and
[`docs/adr/ADR-053-cp1-license-publication.md`](docs/adr/ADR-053-cp1-license-publication.md)
for the decision record.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the workflow this project actually
enforces, [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) for the behaviour expected
of everyone participating, and [`SECURITY.md`](SECURITY.md) before reporting a
vulnerability. Support channels are in [`SUPPORT.md`](SUPPORT.md).
