# CogniCode: Conceptual Overview

## What is CogniCode?

CogniCode is a **Super-LSP** (Language Server Protocol) server written in Rust, designed specifically to provide advanced code analysis and refactoring capabilities to AI agents. Inspired by the capabilities of IntelliJ IDEA Ultimate but built with the speed and safety of Rust.

### Key Characteristics

| Attribute | Description |
|-----------|-------------|
| **Type** | LSP Server + MCP Tools Provider |
| **Language** | Rust (100%) |
| **Target Users** | AI Agents, Code Editors with LLM integration |
| **Primary Protocol** | MCP (Model Context Protocol) |
| **Secondary Protocol** | LSP (for editor integration) |

### Why "Super-LSP"?

Traditional LSP servers provide:
- Syntax highlighting
- Code completion
- Go-to-definition
- Find references

CogniCode extends this with:
- **Call graph analysis** (who calls whom, and who calls who)
- **Impact analysis** (what breaks if I change this?)
- **Safe refactoring** (with validation and rollback)
- **Architecture validation** (cycle detection, dependency layers)
- **Complexity metrics** (cyclomatic, cognitive complexity)

## Core Concepts

### 1. Symbols

Symbols are the fundamental entities representing code constructs:

```rust
pub struct Symbol {
    name: String,           // "calculate_total"
    kind: SymbolKind,       // Function, Class, Variable, etc.
    location: Location,     // file.rs:42:5
    signature: Option<FunctionSignature>,
}
```

**Symbol Types (SymbolKind)**:

| Kind | Description | Examples |
|------|-------------|----------|
| `Module` | Code organization unit | `mod utils;` |
| `Class` | Object-oriented class | `class Order` |
| `Struct` | Data structure | `struct OrderLine` |
| `Enum` | Enumeration type | `enum Status { Pending, Active }` |
| `Trait` | Interface definition | `trait Serializable` |
| `Function` | Standalone function | `fn calculate_total()` |
| `Method` | Class method | `impl Order { fn total() }` |
| `Field` | Struct/class field | `order_id: String` |
| `Variable` | Local variable | `let total = 0;` |
| `Constant` | Immutable value | `const MAX_RETRIES = 3` |
| `Constructor` | Object constructor | `fn new()` |
| `Interface` | Protocol definition | `interface Reader` |
| `TypeAlias` | Type renaming | `type UserId = String` |
| `Parameter` | Function parameter | `(order: Order)` |

### 2. Call Graphs

A **Call Graph** represents the calling relationships between symbols:

```
┌──────────────────┐         ┌──────────────────┐
│ process_order    │────────►│ validate_order   │
│ (function)       │         │ (function)       │
└──────────────────┘         └────────┬─────────┘
                                      │
                                      ▼
                             ┌──────────────────┐
                             │ check_inventory  │
                             │ (function)       │
                             └──────────────────┘
```

**Call Hierarchy Directions**:

- **Outgoing**: What does this symbol call?
- **Incoming**: Who calls this symbol?

### 3. Refactoring

CogniCode supports safe refactoring operations:

| Action | Description |
|--------|-------------|
| `rename` | Rename a symbol across the codebase |
| `extract` | Extract code into a new function |
| `inline` | Inline a function into its callers |
| `move` | Move a symbol to a different location |
| `change_signature` | Modify function parameters |

**Safety First**: All refactoring operations:
1. Validate the operation before execution
2. Calculate impact on dependent code
3. Generate workspace edits (not direct file modification)
4. Report warnings and errors

### 4. Impact Analysis

Before making changes, CogniCode analyzes:

- Which files contain symbols that depend on the target?
- What is the cascading effect of the change?
- What is the risk level (Low, Medium, High, Critical)?

```
Symbol: process_order
─────────────────────────────
Impact Score: 87/100
Risk Level: HIGH

Affected Files (3):
  - src/order.rs (calls process_order)
  - src/checkout.rs (calls process_order)
  - tests/order_test.rs (tests process_order)

Affected Symbols (12):
  - validate_order
  - calculate_totals
  - update_inventory
  - ...
```

### 5. Value Objects

Domain value objects provide type safety and validation:

**Location**: A position in source code
```rust
Location::new("src/main.rs", 42, 5)
// Format: "file.rs:line:column"
```

**SourceRange**: A range in source code
```rust
SourceRange {
    start: Location::new("src/main.rs", 10, 1),
    end: Location::new("src/main.rs", 15, 20),
}
```

**DependencyType**: The nature of a dependency
```rust
enum DependencyType {
    Calls,      // Function invocation
    Inherits,   // Class inheritance
    References, // Variable reference
    Imports,    // Module import
}
```

## Hexagonal Architecture

CogniCode follows **Hexagonal Architecture** (also known as Ports and Adapters) to maintain clean separation of concerns.

### Layer Structure

```
┌─────────────────────────────────────────────────────────────────┐
│                         INTERFACE                                │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │    MCP Server   │  │   LSP Server    │  │   CLI Commands │  │
│  │  (AI Agents)    │  │   (Editors)     │  │   (Terminal)   │  │
│  └────────┬────────┘  └────────┬────────┘  └───────┬────────┘  │
└───────────┼─────────────────────┼───────────────────┼───────────┘
            │                     │                    │
            ▼                     ▼                    ▼
┌─────────────────────────────────────────────────────────────────┐
│                       APPLICATION                               │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    Service Layer                          │  │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐ │  │
│  │  │ Navigation  │ │ Refactoring │ │    Analysis         │ │  │
│  │  │ Service     │ │ Service     │ │    Service          │ │  │
│  │  └─────────────┘ └─────────────┘ └─────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
            │                     │                    │
            ▼                     ▼                    ▼
┌─────────────────────────────────────────────────────────────────┐
│                          DOMAIN                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Aggregates              Value Objects                    │  │
│  │  ┌─────────────┐        ┌─────────────────────┐        │  │
│  │  │ Symbol      │        │ Location             │        │  │
│  │  │ CallGraph   │        │ SourceRange          │        │  │
│  │  │ Refactor    │        │ DependencyType       │        │  │
│  │  └─────────────┘        └─────────────────────┘        │  │
│  │                                                            │  │
│  │  Domain Services           Traits (Interfaces)              │  │
│  │  ┌─────────────────────────┐  ┌───────────────────────┐  │  │
│  │  │ ImpactAnalyzer          │  │ CodeIntelligenceProvider│  │  │
│  │  │ CycleDetector           │  │ DependencyRepository   │  │  │
│  │  │ ComplexityCalculator   │  │ RefactorStrategy      │  │  │
│  │  └─────────────────────────┘  └───────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
            │                     │                    │
            ▼                     ▼                    ▼
┌─────────────────────────────────────────────────────────────────┐
│                       INFRASTRUCTURE                            │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │ TreeSitter      │  │ PetGraph        │  │ LSP Client     │  │
│  │ Parser          │  │ Graph Store     │  │                │  │
│  └─────────────────┘  └─────────────────┘  └────────────────┘  │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │ VirtualFileSystem│  │ SafetyGate     │  │ TestGenerator │  │
│  └─────────────────┘  └─────────────────┘  └────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Why Hexagonal Architecture?

| Benefit | Impact on CogniCode |
|---------|---------------------|
| **Testability** | Domain logic tested without infrastructure |
| **Flexibility** | Swap tree-sitter for another parser |
| **Clean Dependencies** | Domain has no external dependencies |
| **Evolvability** | Add new interfaces (MCP, LSP, CLI) without changing domain |

### Dependency Rule

> **Dependencies always point inward.**

- Domain is the center (has no dependencies)
- Application depends on Domain
- Infrastructure implements Domain traits
- Interface uses Application

## Safety Guarantees

CogniCode implements multiple safety mechanisms to ensure reliable refactoring and analysis:

### 1. Input Validation

Every input is validated before processing:

```rust
impl InputValidator {
    // Path traversal prevention
    pub fn validate_file_path(&self, path: &str) -> Result<PathBuf, SecurityError>

    // Size limits
    pub fn validate_file_size(&self, content: &str) -> Result<(), SecurityError>

    // Query length limits
    pub fn validate_query(&self, query: &str) -> Result<(), SecurityError>

    // Rate limiting
    pub fn check_rate_limit(&self) -> Result<(), SecurityError>
}
```

### 2. Pre-flight Validation

Before any refactoring:

1. **Syntax validation**: Ensure code parses correctly
2. **Semantic validation**: Check type compatibility
3. **Impact analysis**: Calculate affected scope
4. **Cycle detection**: Verify no circular dependencies

### 3. Workspace Edit Model

Refactoring returns **workspace edits**, not direct modifications:

```rust
pub struct SafeRefactorOutput {
    pub action: RefactorAction,
    pub success: bool,
    pub changes: Vec<ChangeEntry>,  // Edits to apply
    pub validation_result: ValidationResult,
    pub error_message: Option<String>,
}
```

This allows:
- Review before applying
- Batch application
- Selective application
- Rollback capability

### 4. Error Recovery

All errors are typed and recoverable:

```rust
pub enum HandlerError {
    Security(SecurityError),    // Input rejected
    App(AppError),              // Business logic error
    InvalidInput(String),       // Malformed request
    NotFound(String),           // Missing resource
}
```

## Data Flow Example

```
┌─────────────────────────────────────────────────────────────────┐
│  AI Agent                                                       │
│  "Extract the method calculateTotal from Order"                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ MCP JSON-RPC
┌─────────────────────────────────────────────────────────────────┐
│  INTERFACE: MCP Server                                          │
│  - Receive request                                              │
│  - Validate input (security)                                    │
│  - Deserialize to DTO                                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  APPLICATION: RefactorService                                   │
│  - Create RefactorContext                                      │
│  - Select ExtractMethodStrategy                                 │
│  - Orchestrate process                                         │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│  DOMAIN         │ │  DOMAIN         │ │  DOMAIN         │
│  Symbol         │ │  Refactor       │ │  ImpactAnalyzer │
│  - Validate name│ │  - Prepare edits│ │  - Calculate    │
│  - Get signature│ │  - Validate     │ │    impact       │
└─────────────────┘ └─────────────────┘ └─────────────────┘
              │               │               │
              └───────────────┼───────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  APPLICATION: SafetyGate                                        │
│  - Apply changes in virtual filesystem                          │
│  - Validate syntax with tree-sitter                             │
│  - Verify no errors                                             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  INTERFACE: MCP Server                                          │
│  - Serialize result                                             │
│  - Return WorkspaceEdit to Agent                               │
└─────────────────────────────────────────────────────────────────┘
```

## Key Design Decisions

### 1. Rust for Core

Rust provides:
- **Memory safety**: No GC pauses, safe concurrent access
- **Speed**: Competitive with C/C++ for parsing
- **Expression**: Type system captures domain invariants

### 2. Tree-sitter for Parsing

- Incremental parsing (only re-parse changed sections)
- Multiple language support (Python, Rust via tree-sitter-*)
- Battle-tested (used by GitHub, Neovim)

### 3. PetGraph for Graphs

- Mature and stable graph library
- Excellent for dependency/call graphs
- Strong algorithm support (Tarjan SCC, Dijkstra, etc.)

### 4. Separation of Parsing and Analysis

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Parsing    │────►│  Symbol      │────►│  Call Graph  │
│   (tree-sitter)    │  Extraction  │     │  Construction│
└──────────────┘     └──────────────┘     └──────────────┘
```

This allows:
- Swap parsing technology independently
- Cache parsed results
- Build graphs incrementally

## Roadmap

| Phase | Features | Status |
|-------|----------|--------|
| 1 | Navigation & Mapping | In Progress |
| 2 | Local Refactoring | Planned |
| 3 | Impact Analysis | Planned |
| 4 | Advanced Refactoring | Planned |
| 5 | Deep Analysis (DFA) | Future |

See [architecture.md](architecture.md) for detailed implementation roadmap.

## Additional Resources

- [Architecture Documentation](architecture.md)
- [Bounded Contexts](bounded-contexts.md)
- [Agent Setup Guide](agent-setup.md)
- [MCP Tools Reference](mcp-tools-reference.md)
