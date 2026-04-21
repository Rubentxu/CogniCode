# CogniCode: Descripcion Conceptual

## Que es CogniCode?

CogniCode es un servidor **Super-LSP** (Language Server Protocol) escrito en Rust, disenado especificamente para proporcionar capacidades avanzadas de analisis de codigo y refactorizacion a agentes de IA. Inspirado en las capacidades de IntelliJ IDEA Ultimate pero construido con la velocidad y seguridad de Rust.

### Caracteristicas Clave

| Atributo | Descripcion |
|----------|-------------|
| **Tipo** | Servidor LSP + Proveedor de Herramientas MCP |
| **Lenguaje** | Rust (100%) |
| **Usuarios Objetivo** | Agentes de IA, Editores de Codigo con integracion LLM |
| **Protocolo Primario** | MCP (Model Context Protocol) |
| **Protocolo Secundario** | LSP (para integracion con editores) |

### Por que "Super-LSP"?

Los servidores LSP tradicionales proporcionan:
- Resaltado de sintaxis
- Completado de codigo
- Ir a definicion
- Buscar referencias

CogniCode extiende esto con:
- **Analisis de grafos de llamadas** (quien llama a quien, y quien llama a quien)
- **Analisis de impacto** (que se rompe si cambio esto?)
- **Refactorizacion segura** (con validacion y reversibilidad)
- **Validacion de arquitectura** (deteccion de ciclos, capas de dependencias)
- **Metricas de complejidad** (ciclomatica, complejidad cognitiva)

## Conceptos Fundamentales

### 1. Simbolos

Los simbolos son las entidades fundamentales que representan construcciones de codigo:

```rust
pub struct Symbol {
    name: String,           // "calculate_total"
    kind: SymbolKind,       // Function, Class, Variable, etc.
    location: Location,     // file.rs:42:5
    signature: Option<FunctionSignature>,
}
```

**Tipos de Simbolos (SymbolKind)**:

| Tipo | Descripcion | Ejemplos |
|------|-------------|----------|
| `Module` | Unidad de organizacion de codigo | `mod utils;` |
| `Class` | Clase orientada a objetos | `class Order` |
| `Struct` | Estructura de datos | `struct OrderLine` |
| `Enum` | Tipo enumeracion | `enum Status { Pending, Active }` |
| `Trait` | Definicion de interfaz | `trait Serializable` |
| `Function` | Funcion independiente | `fn calculate_total()` |
| `Method` | Metodo de clase | `impl Order { fn total() }` |
| `Field` | Campo de struct/clase | `order_id: String` |
| `Variable` | Variable local | `let total = 0;` |
| `Constant` | Valor inmutable | `const MAX_RETRIES = 3` |
| `Constructor` | Constructor de objetos | `fn new()` |
| `Interface` | Definicion de protocolo | `interface Reader` |
| `TypeAlias` | Renombrado de tipo | `type UserId = String` |
| `Parameter` | Parametro de funcion | `(order: Order)` |

### 2. Grafos de Llamadas

Un **Grafo de Llamadas** representa las relaciones de chiamada entre simbolos:

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

**Direcciones de la Jerarquia de Llamadas**:

- **Salientes**: Que llama este simbolo?
- **Entrantes**: Quien llama a este simbolo?

### 3. Refactorizacion

CogniCode soporta operaciones de refactorizacion segura:

| Accion | Descripcion |
|--------|-------------|
| `rename` | Renombrar un simbolo en todo el codigo |
| `extract` | Extraer codigo en una nueva funcion |
| `inline` | Integrar una funcion en sus llamadores |
| `move` | Mover un simbolo a una ubicacion diferente |
| `change_signature` | Modificar parametros de funcion |

**La Seguridad Primero**: Todas las operaciones de refactorizacion:
1. Validan la operacion antes de la ejecucion
2. Calculan el impacto en el codigo dependiente
3. Generan ediciones de workspace (no modificacion directa de archivos)
4. Reportan advertencias y errores

### 4. Analisis de Impacto

Antes de hacer cambios, CogniCode analiza:

- Que archivos contienen simbolos que dependen del objetivo?
- Cual es el efecto en cascada del cambio?
- Cual es el nivel de riesgo (Bajo, Medio, Alto, Critico)?

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

### 5. Objetos de Valor

Los objetos de valor de dominio proporcionan seguridad de tipos y validacion:

**Location**: Una posicion en el codigo fuente
```rust
Location::new("src/main.rs", 42, 5)
// Format: "file.rs:line:column"
```

**SourceRange**: Un rango en el codigo fuente
```rust
SourceRange {
    start: Location::new("src/main.rs", 10, 1),
    end: Location::new("src/main.rs", 15, 20),
}
```

**DependencyType**: La naturaleza de una dependencia
```rust
enum DependencyType {
    Calls,      // Function invocation
    Inherits,   // Class inheritance
    References, // Variable reference
    Imports,    // Module import
}
```

## Arquitectura Hexagonal

CogniCode sigue la **Arquitectura Hexagonal** (tambien conocida como Puertos y Adaptadores) para mantener una separacion limpia de responsabilidades.

### Estructura de Capas

```
┌─────────────────────────────────────────────────────────────────┐
│                         INTERFAZ                                │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │    MCP Server   │  │   LSP Server    │  │   CLI Commands │  │
│  │  (AI Agents)    │  │   (Editors)     │  │   (Terminal)   │  │
│  └────────┬────────┘  └────────┬────────┘  └───────┬────────┘  │
└───────────┼─────────────────────┼───────────────────┼───────────┘
            │                     │                    │
            ▼                     ▼                    ▼
┌─────────────────────────────────────────────────────────────────┐
│                       APLICACION                               │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    Capa de Servicios                       │  │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐ │  │
│  │  │ Navigation  │ │ Refactoring │ │    Analysis         │ │  │
│  │  │ Service     │ │ Service     │ │    Service          │ │  │
│  │  └─────────────┘ └─────────────┘ └─────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
            │                     │                    │
            ▼                     ▼                    ▼
┌─────────────────────────────────────────────────────────────────┐
│                          DOMINIO                                │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Agregados              Objetos de Valor                  │  │
│  │  ┌─────────────┐        ┌─────────────────────┐        │  │
│  │  │ Symbol      │        │ Location             │        │  │
│  │  │ CallGraph   │        │ SourceRange          │        │  │
│  │  │ Refactor    │        │ DependencyType       │        │  │
│  │  └─────────────┘        └─────────────────────┘        │  │
│  │                                                            │  │
│  │  Servicios de Dominio       Traits (Interfaces)              │  │
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
│                       INFRAESTRUCTURA                            │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │ TreeSitter      │  │ PetGraph        │  │ LSP Client     │  │
│  │ Parser          │  │ Graph Store     │  │                │  │
│  └─────────────────┘  └─────────────────┘  └────────────────┘  │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐  │
│  │ VirtualFileSystem│  │ SafetyGate     │  │ TestGenerator │  │
│  └─────────────────┘  └─────────────────┘  └────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Por que Arquitectura Hexagonal?

| Beneficio | Impacto en CogniCode |
|-----------|----------------------|
| **Testabilidad** | Logica de dominio probada sin infraestructura |
| **Flexibilidad** | Intercambiar tree-sitter por otro parser |
| **Dependencias Limpias** | Dominio sin dependencias externas |
| **Evolucionabilidad** | Agregar nuevas interfaces (MCP, LSP, CLI) sin cambiar el dominio |

### Regla de Dependencias

> **Las dependencias siempre apuntan hacia adentro.**

- El dominio es el centro (no tiene dependencias)
- La aplicacion depende del dominio
- La infraestructura implementa los traits del dominio
- La interfaz usa la aplicacion

## Garantias de Seguridad

CogniCode implementa multiples mecanismos de seguridad para garantizar refactorizacion y analisis confiables:

### 1. Validacion de Entrada

Cada entrada se valida antes del procesamiento:

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

### 2. Validacion Pre-vuelo

Antes de cualquier refactorizacion:

1. **Validacion de sintaxis**: Asegurar que el codigo se parsea correctamente
2. **Validacion semantica**: Verificar compatibilidad de tipos
3. **Analisis de impacto**: Calcular alcance afectado
4. **Deteccion de ciclos**: Verificar que no hay dependencias circulares

### 3. Modelo de Edicion de Workspace

La refactorizacion devuelve **workspace edits**, no modificaciones directas:

```rust
pub struct SafeRefactorOutput {
    pub action: RefactorAction,
    pub success: bool,
    pub changes: Vec<ChangeEntry>,  // Edits to apply
    pub validation_result: ValidationResult,
    pub error_message: Option<String>,
}
```

Esto permite:
- Revision antes de aplicar
- Aplicacion por lotes
- Aplicacion selectiva
- Capacidad de reversibilidad

### 4. Recuperacion de Errores

Todos los errores son tipados y recuperables:

```rust
pub enum HandlerError {
    Security(SecurityError),    // Input rejected
    App(AppError),              // Business logic error
    InvalidInput(String),       // Malformed request
    NotFound(String),           // Missing resource
}
```

## Ejemplo de Flujo de Datos

```
┌─────────────────────────────────────────────────────────────────┐
│  AI Agent                                                       │
│  "Extract the method calculateTotal from Order"                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ MCP JSON-RPC
┌─────────────────────────────────────────────────────────────────┐
│  INTERFAZ: Servidor MCP                                         │
│  - Recibir solicitud                                             │
│  - Validar entrada (seguridad)                                  │
│  - Deserializar a DTO                                           │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  APLICACION: RefactorService                                    │
│  - Crear RefactorContext                                        │
│  - Seleccionar ExtractMethodStrategy                            │
│  - Orquestar proceso                                            │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│  DOMINIO        │ │  DOMINIO        │ │  DOMINIO        │
│  Symbol         │ │  Refactor       │ │  ImpactAnalyzer │
│  - Validate name│ │  - Prepare edits│ │  - Calculate    │
│  - Get signature│ │  - Validate     │ │    impact       │
└─────────────────┘ └─────────────────┘ └─────────────────┘
              │               │               │
              └───────────────┼───────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  APLICACION: SafetyGate                                         │
│  - Apply changes in virtual filesystem                          │
│  - Validar sintaxis con tree-sitter                             │
│  - Verificar que no hay errores                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  INTERFAZ: Servidor MCP                                         │
│  - Serializar resultado                                         │
│  - Devolver WorkspaceEdit al Agente                             │
└─────────────────────────────────────────────────────────────────┘
```

## Decisiones de Diseno Clave

### 1. Rust para el Nucleo

Rust proporciona:
- **Seguridad de memoria**: Sin pausas de GC, acceso concurrente seguro
- **Velocidad**: Competitivo con C/C++ para parsing
- **Expresividad**: El sistema de tipos captura invariantes de dominio

### 2. Tree-sitter para Parsing

- Parsing incremental (solo re-parsea secciones cambiadas)
- Soporte multiple de lenguajes (Python, Rust via tree-sitter-*)
- Probado en batalla (usado por GitHub, Neovim)

### 3. PetGraph para Grafos

- Libreria de grafos madura y estable
- Excelente para grafos de dependencias/llamadas
- Fuerte soporte de algoritmos (Tarjan SCC, Dijkstra, etc.)

### 4. Separacion de Parsing y Analisis

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Parsing    │────►│  Symbol      │────►│  Call Graph  │
│   (tree-sitter)    │  Extraction  │     │  Construction│
└──────────────┘     └──────────────┘     └──────────────┘
```

Esto permite:
- Intercambiar tecnologia de parsing independientemente
- Cache de resultados parseados
- Construir grafos incrementalmente

## Hoja de Ruta

| Fase | Caracteristicas | Estado |
|------|----------------|--------|
| 1 | Navegacion y Mapeo | En Progreso |
| 2 | Refactorizacion Local | Planeado |
| 3 | Analisis de Impacto | Planeado |
| 4 | Refactorizacion Avanzada | Planeado |
| 5 | Analisis Profundo (DFA) | Futuro |

Ver [architecture-es.md](architecture-es.md) para la hoja de implementacion detallada.

## Recursos Adicionales

- [Documentacion de Arquitectura](architecture-es.md)
- [Contextos Delimitados](bounded-contexts-es.md)
- [Guia de Configuracion de Agentes](agent-setup-es.md)
- [Referencia de Herramientas MCP](mcp-tools-reference-es.md)