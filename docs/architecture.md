# CogniCode: Super-LSP Architecture

## Resumen Ejecutivo

Este documento presenta la arquitectura técnica final para CogniCode, un servidor LSP "premium" en Rust diseñado para proporcionar funcionalidades avanzadas de análisis de código y refactorización a agentes IA a través del protocolo MCP (Model Context Protocol).

**Inspiración**: Capacidades de IntelliJ IDEA Ultimate, impulsado por la velocidad y seguridad de Rust.

---

## 1. Stack Tecnológico

### 1.1 Componentes Principales

| Componente | Biblioteca | Versión | Justificación |
|------------|-----------|---------|--------------|
| **LSP Protocol** | `language-server-protocol` + `lsp-types` | 0.14 / 0.93 | Reemplazo de tower-lsp (abandonware). Mantenido activamente por rust-analyzer |
| **Parser** | `tree-sitter` + `tree-sitter-*` | 0.20 | Incremental parsing de clase mundial. Usado por GitHub, Neovim |
| **Grafos** | `petgraph` | 0.6 | Más maduro y estable que alternativas. Excelente para dependency/call graphs |
| **Runtime Async** | `tokio` | 1.x | Estándar de facto. Usar features específicos (no "full") |
| **Serialización** | `serde` + `serde_json` | 1.0 | Industria estándar |
| **Async Utils** | `futures-util` | 0.3 | Preferido sobre `futures` directamente |
| **Errores** | `anyhow` + `thiserror` | 1.0 | `thiserror` para library errors, `anyhow` para aplicación |

### 1.2 Cargo.toml Base

```toml
[package]
name = "cognicode"
version = "0.1.0"
edition = "2021"

[dependencies]
# LSP Protocol (CRÍTICO: reemplazar tower-lsp)
language-server-protocol = "0.14"
lsp-types = "0.93"

# Parsing
tree-sitter = "0.20"
tree-sitter-python = "0.20"
tree-sitter-rust = "0.20"

# Grafos
petgraph = "0.6"

# Async Runtime
tokio = { version = "1", features = ["rt-multi-thread", "net", "sync", "time", "macros"] }

# Serialización
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Async Utils
futures-util = "0.3"

# Error Handling
anyhow = "1.0"
thiserror = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Additional
async-trait = "0.1"
arc-swap = "1.0"
parking-lot = "0.12"
```

### 1.3 Crítica Original y Cambios Aplicados

| Problema Identificado | Cambio Aplicado |
|---------------------|----------------|
| tower-lsp en abandonware | Migrado a `language-server-protocol` + `lsp-types` |
| `tokio` con features "full" | Features específicos por módulo |
| Solo `anyhow` | Combinación `anyhow` + `thiserror` |

---

## 2. Arquitectura General

### 2.1 Capas (Clean Architecture + DDD)

```
┌─────────────────────────────────────────────────────────────────┐
│                         INTERFACE                                │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │    McpServer    │  │   LspServer     │  │   CliCommands  │  │
│  │  (Agente IA)    │  │   (Editor)      │  │   (Terminal)   │  │
│  └────────┬────────┘  └────────┬────────┘  └───────┬────────┘  │
└───────────┼─────────────────────┼───────────────────┼───────────┘
            │                     │                    │
            ▼                     ▼                    ▼
┌─────────────────────────────────────────────────────────────────┐
│                       APPLICATION                               │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    Service Locator                         │  │
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
│  │  Aggregates                    Value Objects              │  │
│  │  ┌─────────────┐              ┌─────────────────────┐   │  │
│  │  │ Symbol      │              │ Location             │   │  │
│  │  │ CallGraph   │              │ SourceRange          │   │  │
│  │  │ Refactor    │              │ DependencyType       │   │  │
│  │  └─────────────┘              └─────────────────────┘   │  │
│  │                                                           │  │
│  │  Domain Services                 Traits (Interfaces)     │  │
│  │  ┌─────────────────────────┐  ┌───────────────────────┐  │  │
│  │  │ ImpactAnalyzer          │  │ CodeIntelligenceProvider│  │  │
│  │  │ CycleDetector           │  │ DependencyRepository   │  │  │
│  │  │ ComplexityCalculator    │  │ RefactorStrategy       │  │  │
│  │  └─────────────────────────┘  └───────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
            │                     │                    │
            ▼                     ▼                    ▼
┌─────────────────────────────────────────────────────────────────┐
│                       INFRASTRUCTURE                            │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │ TreeSitter      │  │ PetGraph        │  │ LspClient      │  │
│  │ Parser          │  │ Manager         │  │                │  │
│  └─────────────────┘  └─────────────────┘  └────────────────┘  │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │ VirtualFileSystem│  │ SafetyGate      │  │ TestGenerator  │  │
│  │                 │  │                 │  │                │  │
│  └─────────────────┘  └─────────────────┘  └────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Crítica Original y Mejoras Aplicadas

| Problema Identificado | Solución Implementada |
|----------------------|-----------------------|
| Dominio anémico (Symbol como data container) | Symbol con comportamiento, Location como Value Object |
| Feature Envy en Application Layer | Traits genéricos con DI |
| Location como String | Value Object `Location` con validación |
| petgraph no mockeable | Wrapper con trait `DependencyRepository` |
| Provider trait muy grande | Dividido en sub-traits especializados |

---

## 3. Bounded Contexts y Módulos

### 3.1 Estructura de Directorios

```
src/
├── main.rs                      # Entry point
├── lib.rs                       # Library root
│
├── domain/                      # CORE DOMAIN (sin dependencias externas)
│   ├── mod.rs
│   ├── aggregates/
│   │   ├── mod.rs
│   │   ├── symbol.rs            # Symbol aggregate root
│   │   ├── call_graph.rs        # CallGraph aggregate
│   │   └── refactor.rs          # Refactor aggregate
│   │
│   ├── value_objects/
│   │   ├── mod.rs
│   │   ├── location.rs           # Location (file, line, column)
│   │   ├── source_range.rs      # Range en código fuente
│   │   ├── dependency_type.rs   # Calls, Inherits, References
│   │   └── symbol_kind.rs       # Function, Class, Variable, etc.
│   │
│   ├── services/
│   │   ├── mod.rs
│   │   ├── impact_analyzer.rs   # Domain service: análisis de impacto
│   │   ├── cycle_detector.rs    # Domain service: detección de ciclos
│   │   └── complexity.rs        # Domain service: métricas
│   │
│   └── traits/
│       ├── mod.rs
│       ├── code_intelligence.rs   # Provider trait
│       ├── dependency_repository.rs # Repository trait
│       ├── refactor_strategy.rs    # Strategy pattern
│       └── search_provider.rs      # Structural search
│
├── application/                  # APPLICATION LAYER
│   ├── mod.rs
│   ├── services/
│   │   ├── mod.rs
│   │   ├── navigation_service.rs   # Call hierarchy, find usages
│   │   ├── refactor_service.rs     # Orchestrator de refactors
│   │   ├── analysis_service.rs     # DFA, complexity metrics
│   │   └── inspection_service.rs   # Dependency analysis
│   │
│   ├── commands/
│   │   ├── mod.rs
│   │   └── refactor_commands.rs    # Use cases
│   │
│   └── dto/
│       ├── mod.rs
│       ├── symbol_dto.rs
│       ├── impact_dto.rs
│       └── refactor_dto.rs
│
├── infrastructure/               # INFRASTRUCTURE LAYER
│   ├── mod.rs
│   │
│   ├── parser/
│   │   ├── mod.rs
│   │   ├── tree_sitter_parser.rs  # Tree-sitter implementation
│   │   └── ast_scanner.rs         # AST traversal utilities
│   │
│   ├── graph/
│   │   ├── mod.rs
│   │   ├── pet_graph_store.rs     # petgraph implementation
│   │   └── graph_cache.rs         # Cache layer
│   │
│   ├── lsp/
│   │   ├── mod.rs
│   │   ├── client.rs              # LSP client implementation
│   │   └── protocol.rs            # Protocol helpers
│   │
│   ├── vfs/
│   │   ├── mod.rs
│   │   └── virtual_file_system.rs # Virtual filesystem
│   │
│   ├── safety/
│   │   ├── mod.rs
│   │   ├── syntax_validator.rs    # Tree-sitter validation
│   │   └── safety_gate.rs         # Validation pipeline
│   │
│   └── testing/
│       ├── mod.rs
│       └── test_generator.rs       # Test scaffolding
│
└── interface/                    # INTERFACE LAYER
    ├── mod.rs
    │
    ├── mcp/
    │   ├── mod.rs
    │   ├── server.rs              # MCP server implementation
    │   ├── handlers.rs            # Tool handlers
    │   ├── schemas.rs             # Tool schemas
    │   └── security.rs            # Input validation, rate limiting
    │
    ├── lsp/
    │   ├── mod.rs
    │   └── server.rs              # LSP server (using lsp-types)
    │
    └── cli/
        ├── mod.rs
        └── commands.rs            # CLI commands
```

### 3.2 Responsabilidades por Bounded Context

| Context | Responsabilidad | Dependencias |
|---------|---------------|--------------|
| **Domain** | Lógica de negocio pura, modelos, Value Objects | Ninguna (innermost) |
| **Application** | Orquestación, casos de uso, servicios | Domain |
| **Infrastructure** | Implementaciones concretas (tree-sitter, petgraph) | Domain (traits) |
| **Interface** | Adaptadores externos (MCP, LSP, CLI) | Application |

---

## 4. Abstracciones SOLID

### 4.1 Traits del Dominio

```rust
// domain/traits/code_intelligence.rs

/// Provider de inteligencia de código (LSP o Tree-sitter)
#[async_trait::async_trait]
pub trait CodeIntelligenceProvider: Send + Sync {
    /// Extrae todos los símbolos de un archivo
    async fn get_symbols(&self, source: &str) -> Result<Vec<Symbol>, ProviderError>;

    /// Encuentra referencias a un símbolo
    async fn find_references(&self, symbol: &Symbol) -> Result<Vec<Location>, ProviderError>;

    /// Obtiene la jerarquía de tipos (herencia)
    async fn get_type_hierarchy(&self, symbol: &Symbol) -> Result<TypeHierarchy, ProviderError>;

    /// Obtiene el call hierarchy
    async fn get_call_hierarchy(&self, symbol: &Symbol) -> Result<CallHierarchy, ProviderError>;
}

/// Errors específicos del provider
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Symbol not found: {0}")]
    NotFound(String),

    #[error("Provider error: {0}")]
    Internal(String),
}
```

```rust
// domain/traits/dependency_repository.rs

/// Repository para el grafo de dependencias
pub trait DependencyRepository: Send + Sync {
    /// Añade una dependencia al grafo
    fn add_dependency(&mut self, source: Symbol, target: Symbol, kind: DependencyType);

    /// Obtiene todos los símbolos afectados por un cambio en el símbolo dado
    fn find_impact_scope(&self, symbol: &Symbol) -> Vec<Symbol>;

    /// Obtiene todas las dependencias de un símbolo
    fn find_dependencies(&self, symbol: &Symbol) -> Vec<Symbol>;

    /// Detecta ciclos en el grafo
    fn detect_cycles(&self) -> CycleDetectionResult;

    /// Verifica si añadir una dependencia crearía un ciclo
    fn would_create_cycle(&self, source: &Symbol, target: &Symbol) -> bool;
}
```

```rust
// domain/traits/refactor_strategy.rs

/// Strategy para refactorizaciones
pub trait RefactorStrategy: Send + Sync {
    /// Valida si el refactor puede ejecutarse
    fn validate(&self, context: &RefactorContext) -> Result<ValidationResult, RefactorError>;

    /// Prepara los edits sin aplicarlos
    fn prepare_edits(&self, context: &RefactorContext) -> Result<Vec<WorkspaceEdit>, RefactorError>;

    /// Ejecuta el refactor
    fn execute(&self, context: &RefactorContext) -> Result<RefactorResult, RefactorError>;
}

/// Contexto para refactorizaciones
pub struct RefactorContext {
    pub symbol: Symbol,
    pub source_range: SourceRange,
    pub workspace: Workspace,
    pub graph: Arc<dyn DependencyRepository>,
}

#[derive(Debug, Clone)]
pub enum ValidationResult {
    Valid,
    Invalid { reason: String, suggestions: Vec<String> },
}
```

### 4.2 Value Objects del Dominio

```rust
// domain/value_objects/location.rs

/// Value Object para ubicación en código fuente
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Location {
    file: String,
    line: u32,
    column: u32,
}

impl Location {
    pub fn new(file: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }

    pub fn from_str(s: &str) -> Result<Self, ParseLocationError> {
        // Formato: "file.rs:10:5"
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return Err(ParseLocationError::InvalidFormat(s.to_string()));
        }

        let file = parts[0].to_string();
        let line: u32 = parts[1].parse().map_err(|_| ParseLocationError::InvalidLine(s.to_string()))?;
        let column: u32 = parts[2].parse().map_err(|_| ParseLocationError::InvalidColumn(s.to_string()))?;

        Ok(Self { file, line, column })
    }

    pub fn file(&self) -> &str { &self.file }
    pub fn line(&self) -> u32 { self.line }
    pub fn column(&self) -> u32 { self.column }

    /// Convierte a formato LSP
    pub fn to_lsp_position(&self) -> lsp_types::Position {
        lsp_types::Position::new(self.line - 1, self.column - 1) // LSP es 0-indexed
    }

    /// Nombre fully qualified
    pub fn fully_qualified_name(&self) -> String {
        format!("{}:{}:{}", self.file, self.line, self.column)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseLocationError {
    #[error("Invalid format: expected 'file.rs:line:column'")]
    InvalidFormat(String),

    #[error("Invalid line number: {0}")]
    InvalidLine(String),

    #[error("Invalid column number: {0}")]
    InvalidColumn(String),
}
```

### 4.3 Aggregate: Symbol

```rust
// domain/aggregates/symbol.rs

/// Aggregate Root para símbolos de código
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol {
    name: String,
    kind: SymbolKind,
    location: Location,
    signature: Option<FunctionSignature>,
}

impl Symbol {
    pub fn new(name: impl Into<String>, kind: SymbolKind, location: Location) -> Self {
        Self {
            name: name.into(),
            kind,
            location,
            signatures: None,
        }
    }

    pub fn with_signature(mut self, signature: FunctionSignature) -> Self {
        self.signature = Some(signature);
        self
    }

    // Comportamiento del dominio
    pub fn fully_qualified_name(&self) -> String {
        format!("{}::{}", self.location.file(), self.name)
    }

    pub fn is_callable(&self) -> bool {
        matches!(self.kind, SymbolKind::Function | SymbolKind::Method | SymbolKind::Constructor)
    }

    pub fn is_type_definition(&self) -> bool {
        matches!(self.kind, SymbolKind::Class | SymbolKind::Struct | SymbolKind::Enum | SymbolKind::Trait)
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn kind(&self) -> &SymbolKind { &self.kind }
    pub fn location(&self) -> &Location { &self.location }
    pub fn signature(&self) -> Option<&FunctionSignature> { self.signature.as_ref() }
}

/// Kind de símbolo
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Module,
    Class,
    Struct,
    Enum,
    Trait,
    Function,
    Method,
    Field,
    Variable,
    Constant,
    Constructor,
    Interface,
    TypeAlias,
    Parameter,
}

impl SymbolKind {
    pub fn is_definition(&self) -> bool {
        matches!(
            self,
            SymbolKind::Class | SymbolKind::Struct | SymbolKind::Enum
                | SymbolKind::Trait | SymbolKind::Function | SymbolKind::Method
                | SymbolKind::Module | SymbolKind::Interface | SymbolKind::TypeAlias
        )
    }

    pub fn is_reference(&self) -> bool {
        matches!(
            self,
            SymbolKind::Variable | SymbolKind::Constant | SymbolKind::Field | SymbolKind::Parameter
        )
    }
}

/// Firma de función (para análisis de tipos)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionSignature {
    parameters: Vec<Parameter>,
    return_type: Option<String>,
    is_async: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Parameter {
    name: String,
    type_annotation: Option<String>,
    is_variadic: bool,
}
```

---

## 5. Interface MCP: Protocolo y Herramientas

### 5.1 Herramientas MCP (Mejoradas)

| Herramienta | Propósito | Input Schema |
|-------------|-----------|--------------|
| `get_call_hierarchy` | Navega el grafo de llamadas | `{symbol_name, direction, depth, include_external}` |
| `get_file_symbols` | Extrae símbolos de un archivo | `{file_path}` |
| `find_usages` | Encuentra usages context-aware | `{symbol_name, include_declaration}` |
| `structural_search` | Búsqueda por patrones AST | `{pattern_type, path, depth}` |
| `analyze_impact` | Análisis de impacto de cambio | `{symbol_name}` |
| `check_architecture` | Detecta ciclos y deuda | `{scope}` |
| `safe_refactor` | Refactor con validación | `{action, target, params}` |
| `validate_syntax` | Validación rápida | `{file_path}` |
| `get_complexity` | Métricas de complejidad | `{file_path, function_name}` |

### 5.2 Schema Mejorado (Ejemplo)

```rust
// interface/mcp/schemas.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GetCallHierarchyInput {
    /// Fully qualified name (e.g., 'module::function' or 'Class.method')
    pub symbol_name: String,

    /// Direction: incoming (who calls this) or outgoing (what this calls)
    #[serde(rename = "direction")]
    pub direction: CallDirection,

    /// Profundidad de traversal (default: 1, max: 10)
    #[serde(default = "default_depth")]
    pub depth: u8,

    /// Incluir dependencias externas (crates/packages)
    #[serde(default)]
    pub include_external: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CallDirection {
    Incoming,
    Outgoing,
}

fn default_depth() -> u8 { 1 }

#[derive(Debug, Serialize, Deserialize)]
pub struct GetCallHierarchyOutput {
    pub symbol: String,
    pub calls: Vec<CallEntry>,
    pub metadata: AnalysisMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallEntry {
    pub symbol: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub confidence: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisMetadata {
    pub total_calls: usize,
    pub analysis_time_ms: u64,
}
```

### 5.3 Seguridad (Mejoras Críticas)

```rust
// interface/mcp/security.rs

/// Validador de inputs para prevenir DoS y path traversal
pub struct InputValidator {
    max_file_size: usize,
    max_results: usize,
    allowed_paths: Vec<PathBuf>,
    rate_limiter: RateLimiter,
}

impl InputValidator {
    pub fn validate_file_path(&self, path: &str) -> Result<PathBuf, SecurityError> {
        // 1. Prevenir path traversal
        if path.contains("..") {
            return Err(SecurityError::PathTraversalAttempt);
        }

        // 2. Normalizar y validar
        let path = PathBuf::from(path);
        let canonical = path.canonicalize()
            .map_err(|_| SecurityError::PathNotAccessible)?;

        // 3. Verificar que está dentro del workspace
        let is_allowed = self.allowed_paths.iter().any(|p| {
            canonical.starts_with(p)
        });

        if !is_allowed {
            return Err(SecurityError::PathOutsideWorkspace);
        }

        Ok(path)
    }

    pub fn validate_file_size(&self, content: &str) -> Result<(), SecurityError> {
        if content.len() > self.max_file_size {
            return Err(SecurityError::FileTooLarge {
                size: content.len(),
                max: self.max_file_size,
            });
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("Path traversal attempt detected")]
    PathTraversalAttempt,

    #[error("Path not accessible")]
    PathNotAccessible,

    #[error("Path is outside allowed workspace")]
    PathOutsideWorkspace,

    #[error("File too large: {size} bytes (max: {max})")]
    FileTooLarge { size: usize, max: usize },
}
```

---

## 6. Infrastructure: Implementaciones

### 6.1 Tree-sitter Parser (Refactorizado)

```rust
// infrastructure/parser/tree_sitter_parser.rs

use tree_sitter::{Parser, Query, QueryCursor};
use std::sync::Arc;
use parking_lot::Mutex;

pub struct TreeSitterParser {
    parser: Arc<Mutex<Parser>>,
    language: tree_sitter::Language,
}

impl TreeSitterParser {
    pub fn new(language: tree_sitter::Language) -> Self {
        let mut parser = Parser::new();
        parser.set_language(language).expect("Language grammar加载失败");
        Self {
            parser: Arc::new(Mutex::new(parser)),
            language,
        }
    }

    pub fn parse(&self, source: &str) -> Result<ParseResult, ParseError> {
        let mut parser = self.parser.lock();
        let tree = parser.parse(source, None)
            .ok_or(ParseError::ParseFailed)?;
        Ok(ParseResult { tree, source: source.to_string() })
    }

    /// Query para encontrar todas las definiciones de funciones
    pub fn find_function_definitions(&self, source: &str) -> Result<Vec<Symbol>, ParseError> {
        let parse_result = self.parse(source)?;
        let query = Query::new(
            self.language,
            "(function_definition name: (identifier) @name)"
        )?;

        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, parse_result.tree.root_node(), source.as_bytes());

        let symbols = matches
            .map(|m| {
                let node = m.captures[0].node;
                let name = &source[node.start_byte()..node.end_byte()];
                Symbol::new(name, SymbolKind::Function, Location::default())
            })
            .collect();

        Ok(symbols)
    }
}
```

### 6.2 PetGraph Store (Wrapper Testeable)

```rust
// infrastructure/graph/pet_graph_store.rs

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::tarjan_scc;
use std::collections::HashMap;
use domain::traits::DependencyRepository;
use domain::aggregates::symbol::Symbol;
use domain::value_objects::dependency_type::DependencyType;

pub struct PetGraphStore {
    graph: DiGraph<Symbol, DependencyType>,
    node_indices: HashMap<String, NodeIndex>,
}

impl PetGraphStore {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
        }
    }

    fn get_or_create_node(&mut self, symbol: Symbol) -> NodeIndex {
        let key = symbol.fully_qualified_name();
        if let Some(&idx) = self.node_indices.get(&key) {
            return idx;
        }
        let idx = self.graph.add_node(symbol);
        self.node_indices.insert(key, idx);
        idx
    }
}

impl DependencyRepository for PetGraphStore {
    fn add_dependency(&mut self, source: Symbol, target: Symbol, kind: DependencyType) {
        let source_idx = self.get_or_create_node(source);
        let target_idx = self.get_or_create_node(target);
        self.graph.add_edge(source_idx, target_idx, kind);
    }

    fn find_impact_scope(&self, symbol: &Symbol) -> Vec<Symbol> {
        let start_idx = match self.node_indices.get(&symbol.fully_qualified_name()) {
            Some(idx) => *idx,
            None => return vec![],
        };

        // BFS inverso para encontrar todos los que dependen de este símbolo
        let mut result = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = vec![start_idx];

        while let Some(idx) = queue.pop() {
            if visited.contains(&idx) {
                continue;
            }
            visited.insert(idx);

            // Buscar nodos que tienen aristas hacia este nodo
            for edge in self.graph.edges_directed(idx, petgraph::Direction::Incoming) {
                let source_idx = edge.source();
                if !visited.contains(&source_idx) {
                    result.push(self.graph[source_idx].clone());
                    queue.push(source_idx);
                }
            }
        }

        result
    }

    fn detect_cycles(&self) -> Option<Vec<Vec<Symbol>>> {
        let sccs = tarjan_scc(&self.graph);

        // Filtrar SCCs con más de un nodo (indican ciclos)
        let cycles: Vec<Vec<Symbol>> = sccs
            .into_iter()
            .filter(|scc| scc.len() > 1)
            .map(|indices| {
                indices
                    .iter()
                    .map(|&idx| self.graph[idx].clone())
                    .collect()
            })
            .collect();

        if cycles.is_empty() { None } else { Some(cycles) }
    }

    fn would_create_cycle(&self, source: &Symbol, target: &Symbol) -> bool {
        // Simular añadir la arista y verificar si crea ciclo
        let mut temp_graph = self.graph.clone();
        let source_idx = self.node_indices.get(&source.fully_qualified_name()).copied();
        let target_idx = self.node_indices.get(&target.fully_qualified_name()).copied();

        match (source_idx, target_idx) {
            (Some(s), Some(t)) => {
                temp_graph.add_edge(s, t, DependencyType::Calls);
                !tarjan_scc(&temp_graph).iter().any(|scc| scc.len() > 1 && scc.contains(&s) && scc.contains(&t))
            }
            _ => false,
        }
    }
}
```

---

## 7. Roadmap de Implementación

### Fase 1: Navegación y Mapeo (Semanas 1-3)

| Herramienta | Prioridad | Componentes |
|-------------|-----------|-------------|
| `get_file_symbols` | 1 | TreeSitterParser, Symbol aggregate |
| `get_call_hierarchy` | 2 | PetGraphStore, CallGraph aggregate |
| `find_usages` | 3 | NavigationService |

### Fase 2: Refactorización Local (Semanas 4-6)

| Herramienta | Prioridad | Componentes |
|-------------|-----------|-------------|
| `safe_refactor` (rename) | 1 | RefactorService, RenameStrategy |
| `safe_refactor` (extract) | 2 | ExtractStrategy |
| `validate_syntax` | 3 | SafetyGate |

### Fase 3: Análisis de Impacto (Semanas 7-10)

| Herramienta | Prioridad | Componentes |
|-------------|-----------|-------------|
| `analyze_impact` | 1 | ImpactAnalyzer domain service |
| `check_architecture` | 2 | CycleDetector |
| `get_complexity` | 3 | ComplexityCalculator |

### Fase 4: Refactorización Avanzada (Semanas 11-14)

| Herramienta | Prioridad | Componentes |
|-------------|-----------|-------------|
| Change Signature | 1 | ChangeSignatureStrategy |
| Extract Method | 2 | MethodExtractorStrategy |
| Encapsulate Field | 3 | FieldEncapsulationStrategy |

### Fase 5: Análisis Profundo (Semanas 15+)

| Herramienta | Prioridad | Componentes |
|-------------|-----------|-------------|
| `structural_search` | 1 | PatternMatcher |
| DFA | 2 | DataFlowEngine (POSTPONE hasta v1.0) |

---

## 8. Métricas de Calidad

### 8.1 Cobertura de Tests

- **Domain**: 100% (sin dependencias externas, fácil de testear)
- **Application**: 90% (unit tests con mocks)
- **Infrastructure**: 80% (integration tests)
- **Interface**: 70% (integration tests con stubs)

### 8.2 Métricas de Código

| Métrica | Objetivo |
|---------|----------|
| Cyclomatic Complexity | < 10 por función |
| Lines per module | < 500 |
| Acoplamiento eferente (CE) | < 10 |
| Cobertura de tests | > 80% |

---

## 9. Conclusiones

La arquitectura propuesta en `ideas.md` tenía una base sólida pero sufrió de:

1. **Problemas técnicos críticos**: tower-lsp en abandonware
2. **Dominio anémico**: Falta de comportamiento en modelos
3. **Acoplamiento**: Feature envy entre capas
4. **Seguridad**: Sin validación de inputs
5. **Interfaces ambiguas**: Schemas inconsistentes

Las mejoras implementadas:

1. ✅ Migración a `language-server-protocol` + `lsp-types`
2. ✅ Domain con comportamiento y Value Objects
3. ✅ Traits para DI y testabilidad
4. ✅ Seguridad con validación de inputs
5. ✅ Schemas consistentes y documentados

La arquitectura final está lista para implementación MVP siguiendo las 5 fases definidas.
