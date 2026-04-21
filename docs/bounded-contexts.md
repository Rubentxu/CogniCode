# CogniCode: Bounded Contexts y Abstracciones

## 1. Visión General de Contextos

CogniCode se divide en 4 bounded contexts principales, cada uno con responsabilidad clara y límites bien definidos:

```
┌──────────────────────────────────────────────────────────────┐
│                      COGNICODE                               │
│                                                              │
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

---

## 2. Domain Context (Core)

### 2.1 Propósito

> **"¿Qué hacemos aquí?"**
> Representar el conocimiento del dominio de análisis de código y refactorización, sin conocer detalles de implementación.

### 2.2 Agregados

#### Symbol (Aggregate Root)

```rust
// Represents a code symbol (function, class, variable, etc.)

pub struct Symbol {
    name: String,
    kind: SymbolKind,
    location: Location,
    signature: Option<FunctionSignature>,
}

impl Symbol {
    // Comportamiento del dominio
    pub fn fully_qualified_name(&self) -> String { ... }
    pub fn is_callable(&self) -> bool { ... }
    pub fn can_be_renamed(&self, new_name: &str) -> RenameValidation { ... }
    pub fn extract_references(&self, scope: &Scope) -> Vec<Reference> { ... }
}
```

**Responsabilidades:**
- Conocer su identidad (nombre, ubicación)
- Validar operaciones sobre sí mismo
- Proporcionar información para análisis de impacto

#### CallGraph (Aggregate)

```rust
// Represents the call relationships between symbols

pub struct CallGraph {
    nodes: HashMap<SymbolId, Symbol>,
    edges: Vec<(SymbolId, SymbolId, CallKind)>,
}

impl CallGraph {
    pub fn add_call(&mut self, caller: Symbol, callee: Symbol) { ... }
    pub fn get_callers(&self, symbol: &Symbol) -> Vec<&Symbol> { ... }
    pub fn get_callees(&self, symbol: &Symbol) -> Vec<&Symbol> { ... }
    pub fn find_cycles(&self) -> Vec<Cycle> { ... }
}
```

**Responsabilidades:**
- Mantener el grafo de llamadas
- Detectar ciclos y anomalías
- Proporcionar navegación del grafo

#### Refactor (Aggregate)

```rust
// Represents a refactoring operation

pub struct Refactor {
    id: RefactorId,
    kind: RefactorKind,
    target: Symbol,
    changes: Vec<Change>,
    validation: ValidationResult,
}

impl Refactor {
    pub fn validate(&self, context: &ValidationContext) -> Result<(), RefactorError> { ... }
    pub fn prepare_workspace_edit(&self) -> WorkspaceEdit { ... }
    pub fn is_safe(&self) -> bool { ... }
}
```

**Responsabilidades:**
- Representar una operación de refactorización
- Validar precondiciones
- Generar cambios

### 2.3 Value Objects

| Value Object | Propósito | Ejemplo |
|-------------|-----------|---------|
| `Location` | Posición en código fuente | `file.rs:42:5` |
| `SourceRange` | Rango de texto en código | `start: Location, end: Location` |
| `DependencyType` | Tipo de dependencia | `Calls`, `Inherits`, `References`, `Imports` |
| `SymbolKind` | Tipo de símbolo | `Function`, `Class`, `Variable` |
| `ComplexityMetrics` | Métricas de complejidad | `cyclomatic: u32, lines: u32` |

### 2.4 Domain Services

```rust
// domain/services/impact_analyzer.rs

/// Servicio de dominio para análisis de impacto
pub struct ImpactAnalyzer;

impl ImpactAnalyzer {
    /// Calcula el impacto de cambiar un símbolo
    pub fn calculate_impact(
        symbol: &Symbol,
        graph: &CallGraph,
    ) -> ImpactReport {
        // Algoritmo puro, sin dependencias de infraestructura
    }

    /// Determina si un cambio es seguro
    pub fn is_safe_to_change(
        symbol: &Symbol,
        graph: &CallGraph,
        threshold: ImpactThreshold,
    ) -> bool {
        // Lógica de negocio pura
    }
}
```

### 2.5 Traits (Contratos)

```rust
// domain/traits/code_intelligence.rs

/// Provider de inteligencia de código
/// Este trait define QUÉ operaciones existen, no CÓMO se implementan
#[async_trait]
pub trait CodeIntelligenceProvider: Send + Sync {
    async fn parse_source(&self, source: &str) -> Result<ParseTree, ParseError>;
    async fn find_symbols(&self, source: &str) -> Result<Vec<Symbol>, ProviderError>;
    async fn find_references(&self, symbol: &Symbol) -> Result<Vec<Location>, ProviderError>;
    async fn get_hierarchy(&self, symbol: &Symbol) -> Result<Hierarchy, ProviderError>;
}
```

---

## 3. Application Context

### 3.1 Propósito

> **"¿Cómo orquestamos las operaciones?"**
> Coordinar los servicios del dominio, manejar casos de uso, y representar la lógica de aplicación sin conocimiento de detalles de infraestructura.

### 3.2 Servicios de Aplicación

#### NavigationService

```rust
// application/services/navigation_service.rs

pub struct NavigationService<P: CodeIntelligenceProvider, G: DependencyRepository> {
    provider: Arc<P>,
    graph: Arc<RwLock<G>>,
}

impl<P, G> NavigationService<P, G>
where
    P: CodeIntelligenceProvider,
    G: DependencyRepository,
{
    /// Obtiene el call hierarchy de un símbolo
    pub async fn get_call_hierarchy(
        &self,
        symbol_name: &str,
        direction: CallDirection,
        depth: u8,
    ) -> Result<CallHierarchyResult, AppError> {
        // 1. Encontrar el símbolo
        let symbol = self.provider.find_symbol_by_name(symbol_name).await?;

        // 2. Obtener el subgrafo de llamadas
        let calls = match direction {
            CallDirection::Outgoing => self.graph.read().get_callees(&symbol),
            CallDirection::Incoming => self.graph.read().get_callers(&symbol),
        };

        // 3. Limitar por profundidad
        let filtered = self.limit_depth(calls, depth);

        Ok(CallHierarchyResult { symbol, calls: filtered })
    }
}
```

#### RefactorService

```rust
// application/services/refactor_service.rs

pub struct RefactorService<R: RefactorStrategy> {
    strategy: R,
    safety_gate: Arc<SafetyGate>,
}

impl<R: RefactorStrategy> RefactorService<R> {
    pub async fn execute_refactor(
        &self,
        command: RefactorCommand,
    ) -> Result<RefactorResult, AppError> {
        // 1. Crear contexto de validación
        let context = RefactorContext::from(command.clone());

        // 2. Validar precondiciones
        let validation = self.strategy.validate(&context)?;
        if !validation.is_valid() {
            return Err(AppError::ValidationFailed(validation.reasons()));
        }

        // 3. Pasar por Safety Gate
        let edits = self.strategy.prepare_edits(&context)?;
        self.safety_gate.validate_edits(&edits)?;

        // 4. Ejecutar
        self.strategy.execute(&context)
    }
}
```

#### AnalysisService

```rust
// application/services/analysis_service.rs

pub struct AnalysisService {
    impact_analyzer: ImpactAnalyzer,
    cycle_detector: CycleDetector,
    complexity_calculator: ComplexityCalculator,
}

impl AnalysisService {
    /// Análisis completo de arquitectura
    pub fn analyze_architecture(
        &self,
        graph: &CallGraph,
    ) -> ArchitectureReport {
        ArchitectureReport {
            has_cycles: self.cycle_detector.has_cycles(graph),
            cycles: self.cycle_detector.find_cycles(graph),
            complexity_metrics: self.complexity_calculator.calculate(graph),
            dependency_layers: self.analyze_layers(graph),
        }
    }
}
```

### 3.3 DTOs (Data Transfer Objects)

```rust
// application/dto/mod.rs

#[derive(Debug, Serialize, Deserialize)]
pub struct SymbolDto {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub location: String,
    pub signature: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImpactAnalysisDto {
    pub symbol: String,
    pub impact_score: u32,
    pub affected_symbols: Vec<String>,
    pub risk_level: RiskLevel,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}
```

---

## 4. Infrastructure Context

### 4.1 Propósito

> **"¿Cómo implementamos las abstracciones?"**
> Proporcionar implementaciones concretas de los traits del dominio usando tecnologías específicas (tree-sitter, petgraph, LSP).

### 4.2 Implementaciones

#### TreeSitterProvider

```rust
// infrastructure/parser/tree_sitter_provider.rs

pub struct TreeSitterProvider {
    parser: Arc<Mutex<Parser>>,
    language: Language,
}

#[async_trait::async_trait]
impl CodeIntelligenceProvider for TreeSitterProvider {
    async fn find_symbols(&self, source: &str) -> Result<Vec<Symbol>, ProviderError> {
        let symbols = tokio::task::spawn_blocking({
            let source = source.to_string();
            let language = self.language;
            move || {
                let parser = Parser::new();
                parser.set_language(language)?;
                let tree = parser.parse(&source, None)?;
                // Parse tree and extract symbols
            }
        }).await?;

        Ok(symbols)
    }
}
```

#### PetGraphRepository

```rust
// infrastructure/graph/pet_graph_repository.rs

pub struct PetGraphRepository {
    graph: DiGraph<Symbol, DependencyType>,
    index: HashMap<String, NodeIndex>,
}

impl DependencyRepository for PetGraphRepository {
    // Implementación con petgraph
}
```

### 4.3 Componentes de Infraestructura

| Componente | Responsabilidad | Tecnología |
|------------|----------------|------------|
| `TreeSitterParser` | Parsing de código fuente | tree-sitter |
| `PetGraphStore` | Almacenamiento de grafos | petgraph |
| `LspClient` | Comunicación con LSP servers | lsp-types |
| `VirtualFileSystem` | Filesystem en memoria | HashMap |
| `SafetyGate` | Validación de cambios | tree-sitter |
| `TestGenerator` | Generación de tests | Template engine |

---

## 5. Interface Context

### 5.1 Propósito

> **"¿Cómo nos comunicamos con el exterior?"**
> Adaptar los servicios de aplicación a protocolos externos (MCP, LSP, CLI).

### 5.2 Servidores

#### McpServer

```rust
// interface/mcp/server.rs

pub struct McpServer {
    navigation: Arc<NavigationService>,
    refactor: Arc<RefactorService>,
    analysis: Arc<AnalysisService>,
    validator: InputValidator,
}

impl McpServer {
    pub async fn handle_request(&self, request: McpRequest) -> McpResponse {
        match request.method.as_str() {
            "tools/list" => self.list_tools(),
            "tools/call" => self.call_tool(request.params).await,
            _ => McpResponse::error("Method not found"),
        }
    }

    async fn call_tool(&self, params: Value) -> McpResponse {
        let tool_name = params["name"].as_str().unwrap();
        let arguments = &params["arguments"];

        match tool_name {
            "get_call_hierarchy" => {
                let input: GetCallHierarchyInput = serde_json::from_value(arguments.clone())?;
                self.validate_input(&input)?;
                let result = self.navigation.get_call_hierarchy(
                    &input.symbol_name,
                    input.direction,
                    input.depth,
                ).await?;
                McpResponse::success(result)
            }
            // ... otras herramientas
        }
    }
}
```

#### LspServer

```rust
// interface/lsp/server.rs

pub struct LspServer {
    editor_state: EditorState,
    providers: ProviderRegistry,
}

#[lsp_server::async_trait]
impl LanguageServer for LspServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult, Error> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(Full)),
                definition_provider: Some(true),
                references_provider: Some(true),
                rename_provider: Some(true),
                // ...
            },
        })
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.editor_state.open_document(params.text_document.uri.clone());
        self.providers.parse_and_index(&params.text_document);
    }
}
```

### 5.3 Seguridad

```rust
// interface/mcp/security.rs

pub struct InputValidator {
    max_file_size: usize,
    max_results: usize,
    allowed_paths: Vec<PathBuf>,
    rate_limiter: RateLimiter,
}

impl InputValidator {
    pub fn validate(&self, input: &impl Validatable) -> Result<(), ValidationError> {
        input.validate_schema()?;
        input.validate_size(self.max_file_size)?;
        input.validate_paths(&self.allowed_paths)?;
        self.rate_limiter.check()?;
        Ok(())
    }
}
```

---

## 6. Flujo de Datos Entre Contextos

### 6.1 Ejemplo: Refactorización

```
┌─────────────────────────────────────────────────────────────────┐
│  Agente IA (LLM)                                                │
│  "Extrae el método calculateTotal de Order"                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ JSON-RPC (MCP)
┌─────────────────────────────────────────────────────────────────┐
│  INTERFACE: McpServer                                           │
│  - Recibe request                                               │
│  - Valida input (seguridad)                                     │
│  - Deserializa a DTO                                            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  APPLICATION: RefactorService                                   │
│  - Crea RefactorContext                                         │
│  - Selecciona estrategia (ExtractMethodStrategy)                 │
│  - Orchestrates el proceso                                      │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│  DOMAIN         │ │  DOMAIN         │ │  DOMAIN         │
│  Symbol         │ │  Refactor       │ │  ImpactAnalyzer │
│  - Valida nombre│ │  - Prepara edits│ │  - Calcula      │
│  - Get signature│ │  - Valida       │ │    impacto      │
└─────────────────┘ └─────────────────┘ └─────────────────┘
              │               │               │
              └───────────────┼───────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  APPLICATION: SafetyGate                                        │
│  - Aplica cambios en VFS virtual                                │
│  - Valida sintaxis con tree-sitter                              │
│  - Verifica no hay errores                                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  INTERFACE: McpServer                                           │
│  - Serializa resultado                                          │
│  - Devuelve WorkspaceEdit al Agente                             │
└─────────────────────────────────────────────────────────────────┘
```

---

## 7. Testing Strategy por Contexto

### 7.1 Domain (100% Coverage)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_can_detect_name_conflict() {
        let symbol = Symbol::new("foo", SymbolKind::Function, Location::default());
        let conflicts = symbol.find_conflicts_in_scope(&["foo", "bar"]);
        assert!(conflicts.contains(&"foo"));
    }

    #[test]
    fn call_graph_detects_simple_cycle() {
        let mut graph = CallGraph::new();
        graph.add_call("a", "b");
        graph.add_call("b", "c");
        graph.add_call("c", "a"); // Ciclo!

        let cycles = graph.find_cycles();
        assert!(cycles.len() == 1);
    }
}
```

### 7.2 Application (90% Coverage con Mocks)

```rust
#[cfg(test)]
mod tests {
    use mockall::mock;
    use super::*;

    mock! {
        pub Provider {}

        #[async_trait]
        impl CodeIntelligenceProvider for Provider {
            async fn find_symbols(&self, source: &str) -> Result<Vec<Symbol>, ProviderError>;
        }
    }

    #[tokio::test]
    async fn navigation_returns_hierarchy() {
        let mut mock_provider = MockProvider::new();
        mock_provider.expect_find_symbol_by_name()
            .returning(|_| Ok(Symbol::new("test", SymbolKind::Function, Location::default())));

        let service = NavigationService::new(Arc::new(mock_provider), Arc::new(RwLock::new(MockGraph::new())));
        let result = service.get_call_hierarchy("test", CallDirection::Outgoing, 1).await;

        assert!(result.is_ok());
    }
}
```

### 7.3 Infrastructure (80% Integration Tests)

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn tree_sitter_parses_python_correctly() {
        let parser = TreeSitterParser::new(tree_sitter_python::language());
        let source = "def foo():\n    pass";
        let symbols = parser.find_function_definitions(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "foo");
    }
}
```

---

## 8. Resumen de Responsabilidades

| Contexto | Responsabilidad | No responsabilidad |
|----------|----------------|---------------------|
| **Domain** | Lógica de negocio pura, modelos, validación intrínseca | Persistencia, parsing, UI |
| **Application** | Orquestación, casos de uso, coordinación | Implementación de algoritmos |
| **Infrastructure** | Implementaciones concretas de traits | Lógica de negocio |
| **Interface** | Comunicación con exterior, adaptación de protocolos | Lógica de negocio |

### Dependencias entre Contextos

```
Interface ──────► Application
       └────► Domain ◄──── Infrastructure
       (usa)      (implementa)
```

**Regla sagrada**: Las dependencias siempre apuntan hacia el Domain. Infrastructure implementa traits definidos en Domain. Application usa Domain. Interface usa Application.
