# CogniCode — AI Debug Plan

> **Fecha**: Abril 2026
> **Estado**: Plan de diseño
> **Alcance**: Integración de debugging para agentes LLM via Debug Adapter Protocol (DAP)
> **Lenguajes**: Rust, TypeScript/JavaScript, Python, Go, Java/Kotlin
> **Prerrequisitos**: Ninguno — funciona independiente de IMPROVEMENT-PLAN-V2 e IDE-FEATURES-PLAN

---

## Resumen Ejecutivo

La propuesta: **CogniCode/RCode actúa como DAP client genérico**, permitiendo que agentes LLM debugueen código en cualquier lenguaje soportado — sin modificar el código fuente del usuario, usando los mismos debug adapters que usa VS Code/IntelliJ.

**Insight clave**: DAP (Debug Adapter Protocol) es un protocolo JSON sobre stdio diseñado por Microsoft para ser máquina-máquina. Un LLM es una máquina. No hay que inventar nada — solo ser un DAP client inteligente.

---

## Índice

1. [Arquitectura](#1-arquitectura)
2. [Dependencias externas y disponibilidad](#2-dependencias-externas-y-disponibilidad)
3. [Función Doctor](#3-función-doctor)
4. [Feature Flags y degradación graceful](#4-feature-flags-y-degradación-graceful)
5. [API y MCP Tools](#5-api-y-mcp-tools)
6. [Pensamiento lateral — Propuestas de mejora](#6-pensamiento-lateral--propuestas-de-mejora)
7. [Pros y Contras](#7-pros-y-contras)
8. [Veredicto](#8-veredicto)
9. [Roadmap de implementación](#9-roadmap-de-implementación)

---

## 1. Arquitectura

### 1.1 Vista general

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Agente LLM                                   │
│  "Debuguea test_process_order, quiero ver por qué falla"           │
│        │                                                            │
│        │ tool calls (8 tools MCP idénticas para todo lenguaje)     │
│        ▼                                                            │
│  ┌──────────────────────────────────────────────────────────┐      │
│  │              rcode-cognicode (DAP Client)                │      │
│  │                                                          │      │
│  │  ┌────────────────┐  ┌────────────────┐  ┌────────────┐ │      │
│  │  │ AdapterManager │  │  DapClient      │  │ Doctor     │ │      │
│  │  │ (detect+spawn) │  │  (JSON stdio)   │  │ (health)   │ │      │
│  │  └────────────────┘  └────────────────┘  └────────────┘ │      │
│  │         │                    │                           │      │
│  │    ¿qué adapter?     DAP protocol (genérico)            │      │
│  └─────────┼───────────────────┼────────────────────────────┘      │
│            │                   │                                     │
│   ┌────────┼───────────────────┼──────────────┐                     │
│   │        ▼                   ▼              │                     │
│   │  ┌──────────┐  ┌──────────┐  ┌──────────┐│                     │
│   │  │ codelldb │  │ debugpy  │  │ js-debug ││                     │
│   │  │ (Rust)   │  │ (Python) │  │ (TS/JS)  ││                     │
│   │  └────┬─────┘  └────┬─────┘  └────┬─────┘│                     │
│   │       │              │              │      │                     │
│   │  ┌────┴─────┐  ┌────┴─────┐  ┌────┴─────┐│                     │
│   │  │  LLDB    │  │ CPython  │  │   V8     ││                     │
│   │  └────┬─────┘  └────┬─────┘  └────┬─────┘│                     │
│   │       │              │              │      │                     │
│   │  ┌────┴──────────────┴──────────────┴─────┐│                     │
│   │  │     Proceso target (código del usuario) ││                     │
│   │  │     ZERO modificaciones al source       ││                     │
│   │  └─────────────────────────────────────────┘│                     │
│   │          Debug Adapters (externos)          │                     │
│   └─────────────────────────────────────────────┘                     │
│            SISTEMA OPERATIVO                                          │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 Componentes nuevos necesarios

| Componente | Dónde vive | LOC estimado | Responsabilidad |
|-----------|-----------|-------------|-----------------|
| `DapClient` | `rcode-cognicode` o crate nuevo `rcode-debug` | ~1500 LOC | Cliente DAP genérico: conectar, enviar requests, recibir responses/events |
| `AdapterRegistry` | mismo crate | ~400 LOC | Registry de adapters por lenguaje, config, rutas |
| `AdapterInstaller` | mismo crate | ~600 LOC | Detectar, descargar, verificar adapters |
| `Doctor` | mismo crate | ~300 LOC | Health check de todas las dependencias externas |
| `DebugSessionManager` | mismo crate | ~800 LOC | Lifecycle de sesiones: prepare → launch → interact → terminate |
| `DebugTools` (MCP) | mismo crate | ~500 LOC | 8-10 tools MCP que el agente llama |
| **Total** | | **~4100 LOC** | |

### 1.3 Crate nuevo vs dentro de rcode-cognicode

**Opción A**: Dentro de `rcode-cognicode` — más simple, pero acopla debug con code intelligence.

**Opción B**: Crate nuevo `rcode-debug` — separación limpia, se puede usar independientemente de CogniCode.

**Recomendación**: **Opción B** — `rcode-debug`. El debugging es una capability ortogonal al code indexing. Un usuario podría querer debugging sin CogniCode, o CogniCode sin debugging. Además, DAP es un protocolo estable y bien especificado — merece su propio crate.

```
rcode-debug/
├── Cargo.toml
├── src/
│   ├── lib.rs              // public API
│   ├── client.rs           // DapClient (JSON stdio protocol)
│   ├── protocol.rs         // DAP types (requests, responses, events)
│   ├── adapter/
│   │   ├── mod.rs          // AdapterRegistry
│   │   ├── installer.rs    // AdapterInstaller
│   │   ├── configs.rs      // per-language configs
│   │   └── health.rs       // Doctor (health checks)
│   ├── session.rs          // DebugSessionManager (lifecycle)
│   ├── tools.rs            // MCP tool implementations
│   └── error.rs            // Error types
└── tests/
    ├── client_test.rs
    ├── adapter_test.rs
    └── integration_test.rs
```

---

## 2. Dependencias externas y disponibilidad

### 2.1 Mapa completo de dependencias

#### Dependencias por lenguaje

| Lenguaje | Debug Adapter | Cómo se obtiene | Binario | ¿Preinstalado? | Plataformas |
|----------|-------------|----------------|---------|----------------|-------------|
| **Rust** | `codelldb` v1.10+ | GitHub releases (vadimcn/codelldb) | `~15MB` | ❌ No | Linux x64, macOS x64/ARM, Windows x64 |
| **Rust** (alt) | `lldb-dap` (LLVM 18+) | Viene con LLVM toolchain | `~2MB` | ⚠️ Si LLVM instalado | Linux, macOS |
| **TypeScript/JS** (Node) | `js-debug` | Built-in Node.js >=12 | `0MB` | ✅ Sí (con Node) | All |
| **TypeScript/JS** (browser) | Chrome DevTools Protocol | Viene con Chrome/Chromium | `0MB` | ⚠️ Si Chrome instalado | All |
| **TypeScript/JS** (Deno) | `deno` (built-in) | Viene con Deno | `0MB` | ⚠️ Si Deno instalado | All |
| **Python** | `debugpy` v1.8+ | `pip install debugpy` | `~5MB` | ❌ No | All |
| **Go** | `delve` (dlv) v1.22+ | `go install github.com/go-delve/delve/cmd/dlv@latest` | `~15MB` | ❌ No | Linux, macOS, Windows |
| **Java/Kotlin** | `java-debug` v0.53+ | Maven Central (com.microsoft.java.debug) | `~2MB` | ❌ No | All |
| **C/C++** | `codelldb` (mismo que Rust) | Mismo binario | `~15MB` | ❌ No | All |

#### Dependencias de build (para compilar con debug symbols)

| Lenguaje | Qué necesita | Comando de build con debug |
|----------|-------------|---------------------------|
| **Rust** | `cargo` (ya necesario para RCode) | `cargo build` (debug=2 por defecto) |
| **TypeScript/JS** | `node` o `bun` | No necesita compilar |
| **Python** | `python3` | No necesita compilar |
| **Go** | `go` toolchain | `go build -gcflags="all=-N -l"` |
| **Java** | `javac` o Maven/Gradle | `javac -g` o `mvn compile` |

#### Dependencias del sistema

| Componente | Propósito | ¿Siempre necesario? |
|-----------|----------|---------------------|
| `rustup` / `cargo` | Build de Rust + RCode | ✅ Sí (ya necesario para RCode) |
| `node` >= 18 | JS/TS debugging | Solo si el proyecto tiene JS/TS |
| `python3` >= 3.8 | Python debugging | Solo si el proyecto tiene Python |
| `go` >= 1.21 | Go debugging | Solo si el proyecto tiene Go |
| `java` >= 11 | Java debugging | Solo si el proyecto tiene Java |

### 2.2 Árbol de dependencias

```
Debug disponible para lenguaje X
├── Lenguaje X detectado por CogniCode (Tree-sitter) → ya funciona
├── Toolchain X instalado (cargo, node, python, go, javac)
│   └── Doctor verifica: which cargo, which node, which python3, which go, which javac
├── Debug Adapter X instalado (codelldb, debugpy, dlv, java-debug)
│   └── Doctor verifica: adapter binario existe y es ejecutable
└── Proyecto X compila con debug symbols
    └── Doctor verifica: test build exitoso (dry-run)
```

---

## 3. Función Doctor

### 3.1 Diseño

El Doctor es un health-checker que verifica **todas las dependencias** y reporta qué capacidades de debug están disponibles. Se ejecuta:

1. **Al iniciar RCode** — check rápido (qué adapters están instalados)
2. **Bajo demanda** — `debug_doctor` tool que el agente puede llamar
3. **Antes de cada sesión de debug** — check del adapter específico necesario

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub timestamp: String,
    pub overall_status: HealthStatus,
    pub languages: Vec<LanguageHealth>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageHealth {
    pub language: String,
    pub status: HealthStatus,
    pub toolchain: ToolchainHealth,
    pub adapter: AdapterHealth,
    pub can_debug: bool,
    pub setup_instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,           // Todo listo, se puede debuguear
    Degraded,          // Parcialmente disponible (ej: adapter viejo)
    Unavailable,       // No disponible, pero instalable
    NotApplicable,     // Lenguaje no presente en el proyecto
    Error(String),     // Error inesperado
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolchainHealth {
    pub name: String,           // "cargo", "node", "python3", "go", "javac"
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub min_version: String,    // versión mínima requerida
    pub version_ok: bool,       // versión instalada >= min_version
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterHealth {
    pub name: String,           // "codelldb", "debugpy", "dlv", etc.
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub auto_installable: bool, // ¿se puede instalar automáticamente?
    pub install_command: Option<String>,
}
```

### 3.2 Checks por lenguaje

```rust
impl Doctor {
    pub async fn check_rust(&self) -> LanguageHealth {
        // 1. Toolchain
        let cargo = which("cargo");
        let cargo_version = cargo.as_ref().ok().map(|p| run(p, ["--version"]));
        
        // 2. Adapter
        let codelldb = self.check_adapter("codelldb");
        let lldb_dap = self.check_adapter("lldb-dap"); // alternativa
        
        // 3. Debug build capability (dry-run)
        let can_build_debug = cargo.is_ok() && self.test_build_check("rust");

        LanguageHealth {
            language: "rust".into(),
            status: match (cargo.is_ok(), codelldb.installed || lldb_dap.installed, can_build_debug) {
                (true, true, true) => HealthStatus::Healthy,
                (true, true, false) => HealthStatus::Degraded,
                (true, false, _) => HealthStatus::Unavailable,
                (false, _, _) => HealthStatus::Unavailable,
            },
            toolchain: ToolchainHealth {
                name: "cargo".into(),
                installed: cargo.is_ok(),
                version: cargo_version,
                path: cargo.ok(),
                min_version: "1.70.0".into(),
                version_ok: self.version_gte(&cargo_version, "1.70.0"),
            },
            adapter: if codelldb.installed { codelldb } else { lldb_dap },
            can_debug: cargo.is_ok() && (codelldb.installed || lldb_dap.installed) && can_build_debug,
            setup_instructions: if !codelldb.installed && !lldb_dap.installed {
                Some("Install codelldb: download from https://github.com/vadimcn/codelldb/releases".into())
            } else { None },
        }
    }

    pub async fn check_python(&self) -> LanguageHealth {
        let python3 = which("python3");
        let debugpy = self.check_python_package("debugpy");
        
        LanguageHealth {
            language: "python".into(),
            status: match (python3.is_ok(), debugpy.installed) {
                (true, true) => HealthStatus::Healthy,
                (true, false) => HealthStatus::Unavailable,
                (false, _) => HealthStatus::Unavailable,
            },
            toolchain: ToolchainHealth {
                name: "python3".into(),
                installed: python3.is_ok(),
                version: python3.as_ref().ok().map(|_| run("python3", ["--version"])),
                path: python3.ok(),
                min_version: "3.8".into(),
                version_ok: true,
            },
            adapter: debugpy,
            can_debug: python3.is_ok() && debugpy.installed,
            setup_instructions: if !debugpy.installed {
                Some("pip install debugpy".into())
            } else { None },
        }
    }

    // Similar para TypeScript, Go, Java...
}
```

### 3.3 Output del Doctor (para el agente)

```xml
<debug-health>
  <rust status="healthy">
    <toolchain name="cargo" version="1.82.0" path="/home/user/.cargo/bin/cargo" />
    <adapter name="codelldb" version="1.10.0" path="/home/user/.local/bin/codelldb" />
  </rust>
  <python status="unavailable">
    <toolchain name="python3" version="3.12.0" installed="true" />
    <adapter name="debugpy" installed="false" auto_install="true" />
    <setup>pip install debugpy</setup>
  </python>
  <typescript status="healthy">
    <toolchain name="node" version="22.0.0" installed="true" />
    <adapter name="js-debug" installed="true" note="built-in with Node.js" />
  </typescript>
</debug-health>
```

El agente recibe esto en `<code-intelligence>` y sabe qué lenguajes puede debuguear.

---

## 4. Feature Flags y degradación graceful

### 4.1 Modelo de capacidades

El debugging no es todo-o-nada. Hay niveles de capacidad:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugCapabilities {
    /// Debug está habilitado globalmente (config del usuario)
    pub enabled: bool,
    /// Capacidades por lenguaje
    pub languages: HashMap<String, LanguageCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageCapability {
    pub language: String,
    pub level: CapabilityLevel,
    pub adapter_name: Option<String>,
    pub features: Vec<DebugFeature>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CapabilityLevel {
    /// Full debugging: breakpoints, stepping, variables, evaluation
    Full,
    /// Limited: breakpoints and variables, pero no evaluation o stepping limitado
    Limited,
    /// Observability only: stdout/stderr capture, sin breakpoints
    ObserveOnly,
    /// No disponible
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DebugFeature {
    Breakpoints,
    ConditionalBreakpoints,
    LogPoints,
    StepOver,
    StepInto,
    StepOut,
    Variables,
    Evaluate,
    SetVariable,
    HotCodeReplace,
    MultiThread,
    AsyncDebugging,
}
```

### 4.2 Tabla de capacidades reales por adapter

| Feature | codelldb (Rust) | js-debug (TS) | debugpy (Python) | dlv (Go) | java-debug |
|---------|-----------------|---------------|------------------|----------|------------|
| Breakpoints | ✅ | ✅ | ✅ | ✅ | ✅ |
| Conditional | ✅ | ✅ | ✅ | ✅ | ✅ |
| LogPoints | ✅ | ✅ | ✅ | ✅ | ✅ |
| Step Over | ✅ | ✅ | ✅ | ✅ | ✅ |
| Step Into | ✅ | ✅ | ✅ | ✅ | ✅ |
| Step Out | ✅ | ✅ | ✅ | ✅ | ✅ |
| Variables | ✅ | ✅ | ✅ | ✅ | ✅ |
| Evaluate | ⚠️ Limitado | ✅ | ✅ | ✅ | ✅ |
| Set Variable | ⚠️ | ✅ | ✅ | ✅ | ✅ |
| Hot Replace | ❌ | ✅ | ✅ | ❌ | ✅ |
| Multi Thread | ✅ | ✅ | ✅ | ✅ | ✅ |
| Async | ❌ (tokio) | ⚠️ | ⚠️ | ✅ (goroutines) | ⚠️ |

### 4.3 Degradación graceful

```rust
impl DebugSessionManager {
    pub async fn start_session(&self, request: DebugRequest) -> Result<DebugSession, DebugError> {
        // 1. Verificar que debug está habilitado
        if !self.capabilities.enabled {
            return Err(DebugError::Disabled(
                "Debug is disabled. Enable in .rcode/config.toml or set RCODE_DEBUG=1".into()
            ));
        }

        // 2. Detectar lenguaje
        let language = self.detect_language(&request.file)?;

        // 3. Verificar capacidad del lenguaje
        let cap = self.capabilities.languages.get(&language)
            .ok_or(DebugError::LanguageNotSupported(language.clone()))?;

        match cap.level {
            CapabilityLevel::None => {
                return Err(DebugError::AdapterUnavailable {
                    language: language.clone(),
                    instructions: cap.limitations.join(". "),
                });
            }
            CapabilityLevel::ObserveOnly => {
                // Solo capturar stdout/stderr, sin breakpoints
                return self.start_observe_session(request, language).await;
            }
            CapabilityLevel::Limited | CapabilityLevel::Full => {
                // Proceder con debug completo o limitado
            }
        }

        // 4. Auto-instalar adapter si es posible
        if !self.is_adapter_installed(&language) {
            if self.can_auto_install(&language) {
                self.auto_install_adapter(&language).await?;
            } else {
                return Err(DebugError::AdapterNotInstalled {
                    language: language.clone(),
                    install_instructions: self.get_install_instructions(&language),
                });
            }
        }

        // 5. Compilar con debug symbols si es necesario
        self.ensure_debug_build(&request, &language).await?;

        // 6. Lanzar adapter y session
        self.launch_session(request, language, cap).await
    }
}
```

### 4.4 Configuración del usuario

```toml
# .rcode/config.toml

[debug]
# Habilitar/deshabilitar debug globalmente
enabled = true

# Auto-instalar adapters si faltan
auto_install = true

# Directorio donde buscar/installar adapters
adapter_dir = "~/.local/share/rcode/debug-adapters"

# Máximo tiempo de una sesión de debug (evita sesiones colgadas)
session_timeout_secs = 300

# Auto-cleanup: terminar sesión si el agente no interactúa en N segundos
idle_timeout_secs = 60

# Capturar automáticamente stdout/stderr del proceso
capture_output = true

# Número máximo de breakpoints por sesión
max_breakpoints = 50

# Número máximo de variables a retornar (evita token explosion)
max_variables = 100

[debug.languages.rust]
# Forzar un adapter específico
adapter = "codelldb"  # o "lldb-dap"

[debug.languages.python]
adapter = "debugpy"
```

---

## 5. API y MCP Tools

### 5.1 Herramientas MCP (lo que el agente ve)

```
Tool: debug_doctor
Descripción: Check debugging capabilities and dependency health
Input: {}
Output: DoctorReport (ver sección 3)
Cuándo usarlo: Al inicio de una sesión de debugging, o cuando el usuario pregunta "¿puedo debuguear?"

---

Tool: debug_start
Descripción: Start a debug session for a test or script
Input: {
  target: {
    type: "test" | "script" | "binary",
    name: "test_process_order",     // para tests
    file: "src/main.py",            // para scripts
    binary: "target/debug/myapp",   // para binarios
  },
  args?: string[],                  // argumentos del proceso
  env?: Record<string, string>,     // variables de entorno
  stop_on_entry?: boolean,          // parar en la primera línea (default: false)
}
Output: {
  session_id: "abc123",
  language: "rust",
  adapter: "codelldb v1.10.0",
  capabilities: ["breakpoints", "step_over", "step_into", "variables", "evaluate"],
  status: "running" | "stopped",
}
Nota: Cero código modificado. CogniCode detecta lenguaje, compila si hace falta, lanza adapter.

---

Tool: debug_set_breakpoints
Descripción: Set breakpoints in source files
Input: {
  file: "src/orders.rs",
  lines: [15, 28, 42],
  conditions?: [{ line: 15, condition: "order.id > 100" }],  // breakpoints condicionales
  log_messages?: [{ line: 28, message: "order = {order}" }],  // logpoints
}
Output: {
  breakpoints: [
    { id: 1, file: "src/orders.rs", line: 15, verified: true },
    { id: 2, file: "src/orders.rs", line: 28, verified: true },
    { id: 3, file: "src/orders.rs", line: 42, verified: false, reason: "no executable code" },
  ]
}

---

Tool: debug_continue
Descripción: Continue execution until next breakpoint or end
Input: { thread_id?: number }
Output: {
  status: "stopped" | "terminated",
  reason?: "breakpoint" | "exception" | "step" | "entry",
  stopped_at?: { file: "src/orders.rs", line: 28, thread_id: 1 },
  exception?: { type: "Panic", message: "index out of bounds" },
}

---

Tool: debug_step
Descripción: Step over, into, or out
Input: {
  action: "over" | "into" | "out",
  thread_id?: number,
  granularity?: "statement" | "line" | "instruction",
}
Output: {
  status: "stopped",
  stopped_at: { file: "src/orders.rs", line: 29, function: "calculate_total" },
}

---

Tool: debug_get_stack
Descripción: Get current call stack
Input: { thread_id?: number, start_frame?: number, levels?: number }
Output: {
  frames: [
    { id: 0, function: "calculate_total", file: "src/orders.rs", line: 29, column: 5 },
    { id: 1, function: "process_order", file: "src/orders.rs", line: 18, column: 20 },
    { id: 2, function: "test_process_order", file: "tests/orders_test.rs", line: 12, column: 3 },
  ],
  total_frames: 3,
}

---

Tool: debug_get_variables
Descripción: Get variables in current scope
Input: {
  frame_id?: number,        // default: top of stack
  scope?: "locals" | "globals" | "closure" | "return",
  filter?: string,          // regex filter para no traer todo
  max_depth?: number,       // profundidad de nested objects (default: 2)
}
Output: {
  variables: [
    {
      name: "order",
      type: "Order",
      value: "Order { id: 42, items: [], total: 0.0 }",
      has_children: true,
      children_ref: 1001,   // para expandir con debug_get_variables(ref=1001)
    },
    {
      name: "validated",
      type: "Result<Order, Error>",
      value: "Ok(Order { ... })",
      has_children: true,
      children_ref: 1002,
    },
    {
      name: "total",
      type: "f64",
      value: "0.0",
      has_children: false,
    },
  ],
}

---

Tool: debug_evaluate
Descripción: Evaluate an expression in the current context
Input: {
  expression: "order.items.len()",
  frame_id?: number,
  context?: "repl" | "watch" | "hover",
}
Output: {
  result: "0",
  type: "usize",
  variables_reference: null,   // si retorna un objeto complejo
  error: null,
}

---

Tool: debug_terminate
Descripción: Terminate debug session
Input: {}
Output: { exit_code: 0, cleanup: "ok" }
Nota: Siempre termina la sesión, limpia recursos, mata procesos hijo.
```

### 5.2 Control de tokens

El mayor riesgo es **token explosion** — un stack trace con 50 frames y 200 variables puede consumir 20K+ tokens. Mitigaciones:

```rust
pub struct DebugOutputConfig {
    /// Máximo de variables a retornar por llamada
    pub max_variables: usize,           // default: 100
    
    /// Profundidad máxima de nested objects
    pub max_variable_depth: usize,      // default: 2
    
    /// Máximo de frames en stack trace
    pub max_stack_frames: usize,        // default: 20
    
    /// Longitud máxima de un valor string
    pub max_value_length: usize,        // default: 500 chars
    
    /// Truncar arrays/vectors con más de N elementos
    pub max_array_elements: usize,      // default: 20
    
    /// Resumen automático: si el output > N tokens, generar summary
    pub auto_summarize_threshold: usize, // default: 3000 tokens
}
```

---

## 6. Pensamiento lateral — Propuestas de mejora

### Propuesta 1: **Reverse Debugging** — Ir hacia atrás en el tiempo

**Problema**: El debug normal solo va hacia adelante. Si el bug ya pasó, tienes que reiniciar y volver a llegar.

**Idea**: Usar `rr` (Mozilla Record and Replay) para Rust/C/C++. `rr` graba toda la ejecución y permite ir hacia atrás.

```rust
// API adicional si rr está disponible
pub enum DebugDirection {
    Forward,
    Reverse,  // ejecutar hacia atrás
}

Tool: debug_reverse_continue  // ir al breakpoint anterior
Tool: debug_step_back          // deshacer el último step
```

**Limitación**: `rr` solo funciona en Linux x86_64 con performance counters. Pero cuando funciona, es **magia** — puedes ir del crash al origen del bug en segundos.

**Veredicto**: Feature avanzada, solo Linux. Se habilita solo si `rr` está instalado (el Doctor lo detecta).

---

### Propuesta 2: **Automated Debug Strategy** — El agente decide cómo debuguear

**Problema**: El agente tiene que decidir manualmente dónde poner breakpoints, cuándo step, cuándo evaluar. Es ineficiente.

**Idea**: CogniCode genera una estrategia de debug automáticamente basada en el call graph:

```rust
pub struct DebugStrategy {
    /// Breakpoints automáticos basados en el call graph
    pub breakpoints: Vec<AutoBreakpoint>,
    /// Variables a observar
    pub watch_expressions: Vec<String>,
    /// Puntos donde pausar y reportar al agente
    pub report_points: Vec<ReportPoint>,
}

pub struct AutoBreakpoint {
    pub function: String,
    pub file: String,
    pub line: u32,
    pub reason: String,  // "Hot path (fan-in: 15)", "Error handling branch"
}

pub struct ReportPoint {
    pub function: String,
    pub variables_to_capture: Vec<String>,
    pub condition: Option<String>,  // "if result.is_err()"
}

impl WorkspaceSession {
    /// Generate a debug strategy for a specific function or test.
    /// Uses call graph + complexity + hot paths to suggest optimal breakpoints.
    pub fn generate_debug_strategy(
        &self,
        target_function: &str,
        bug_description: &str,  // "returns None when it shouldn't"
    ) -> WorkspaceResult<DebugStrategy> { ... }
}
```

**Flujo**:
```
Usuario: "process_order retorna error, ¿por qué?"

Agente:
  1. CogniCode: generate_debug_strategy("process_order", "returns error")
     → Breakpoints en: process_order (entrada), validate (llamada), calculate_total (llamada)
     → Watch: order.items, validated.is_ok(), total
     → Report: cuando validate retorne Err, capturar el error

  2. debug_start(test: "test_process_order")
  3. debug_set_breakpoints(estrategia automática)
  4. debug_continue()
  5. [Auto-report]: "validate() returned Err(EmptyItems) at line 23"
  
  6. Agente: "Encontrado: validate rechaza orders con items vacíos"
```

**Veredicto**: Esto es **la killer feature**. Combina CogniCode (call graph) con DAP (runtime values). Solo CogniCode puede hacerlo porque tiene el grafo de llamadas.

---

### Propuesta 3: **Snapshot Debugging** — Capturar estado sin parar

**Problema**: Para el código asíncrono o concurrente, parar en un breakpoint altera el timing y puede hacer que el bug desaparezca.

**Idea**: En vez de parar, capturar snapshots del estado en puntos clave:

```rust
pub struct DebugSnapshot {
    pub timestamp: u64,
    pub function: String,
    pub line: u32,
    pub variables: Vec<(String, String)>,  // (name, value)
    pub thread_id: i64,
}

Tool: debug_add_snapshot_point
Input: {
  file: "src/orders.rs",
  line: 28,
  capture: ["order", "validated", "result"],  // variables a capturar
}
// No para la ejecución. Captura los valores cuando pasa por esa línea.
// El proceso corre a velocidad normal.
// Al terminar, el agente puede revisar todos los snapshots.
```

**Implementación**: Usa **logpoints** de DAP — el adapter inserta un log statement que captura las variables sin detenerse.

**Veredicto**: Realizable con DAP logpoints. Sin overhead de parar. Ideal para bugs de timing.

---

### Propuesta 4: **Differential Debugging** — Comparar ejecución buena vs mala

**Problema**: "En mi máquina funciona" — el bug solo se reproduce en CI o con ciertos inputs.

**Idea**: Ejecutar el test dos veces (con inputs diferentes), capturar traces, y CogniCode los compara:

```rust
pub struct DifferentialTrace {
    pub passing: ExecutionTrace,
    pub failing: ExecutionTrace,
    pub divergences: Vec<DivergencePoint>,
}

pub struct DivergencePoint {
    pub function: String,
    pub line: u32,
    pub variable: String,
    pub value_passing: String,
    pub value_failing: String,
    pub description: String,
}

Tool: debug_differential
Input: {
  test: "test_process_order",
  env_passing: { "ORDER_COUNT": "5" },
  env_failing: { "ORDER_COUNT": "10000" },
}
Output: DifferentialTrace con divergences
```

**Veredicto**: Potente pero requiere ejecución doble. Implementable con snapshot debugging.

---

### Propuesta 5: **Test-to-Debug Bridge** — De test fallido a debug automático

**Problema**: Cuando un test falla en CI, el agente no tiene acceso interactivo al debug.

**Idea**: Si un test falla, CogniCode automáticamente:
1. Detecta el test que falló (parse del output)
2. Identifica la función relevante (call graph)
3. Genera una estrategia de debug
4. El agente puede decir "debuguea ese test" sin setup manual

```rust
Tool: debug_failed_test
Input: {
  test_output: "test process_order ... FAILED\n  thread 'main' panicked at 'index out of bounds'",
  test_name: "test_process_order",
}
Output: {
  session_id: "abc123",
  strategy: DebugStrategy,  // breakpoints automáticos
  stack_trace_at_failure: Vec<StackFrame>,
  failing_variables: Vec<Variable>,  // capturadas del panic
}
```

**Veredicto**: Esto convierte un test failure en una sesión de debug lista. El agente va directo a la raíz.

---

### Propuesta 6: **Debug Replay para humanos** — Visualización del debug del agente

**Problema**: El usuario no ve lo que el agente hizo durante el debug — es una caja negra.

**Idea**: CogniCode guarda un log estructurado de toda la sesión de debug:

```rust
pub struct DebugReplay {
    pub session_id: String,
    pub steps: Vec<DebugStep>,
    pub total_duration_ms: u64,
    pub conclusion: String,
}

pub struct DebugStep {
    pub timestamp: u64,
    pub action: String,           // "set_breakpoint", "continue", "get_variables"
    pub agent_reasoning: String,  // "Quiero ver qué valor tiene order en línea 28"
    pub result_summary: String,   // "order.items = [], total = 0.0"
    pub files_viewed: Vec<String>,
}

// Genera un HTML/markdown interactivo que el usuario puede revisar
Tool: debug_replay
Input: { session_id: "abc123", format: "html" | "markdown" | "json" }
Output: { replay: DebugReplay, rendered: String }
```

**Veredicto**: Bajo esfuerzo, alto valor para transparencia. Los usuarios pueden ver exactamente qué hizo el agente y por qué.

---

## 7. Pros y Contras

### Pros

| Pro | Detalle |
|-----|---------|
| **Zero código modificado** | El código fuente del usuario no se toca en absoluto. DAP se conecta al proceso en runtime. |
| **Multi-lenguaje con una sola implementación** | DAP es genérico. El DAP client es el mismo para Rust, Python, TS, Go, Java. Los adapters ya existen. |
| **Estándar industrial** | DAP lo usa VS Code, IntelliJ, Emacs, Neovim. No es un protocolo experimental. |
| **Debug real, no simulado** | Valores reales de runtime, no estimaciones. Si `user.name` es `"Alice"`, el agente ve `"Alice"`. |
| **Complementa CogniCode** | El call graph de CogniCode puede generar estrategias de debug automáticas (Propuesta 2). Sin CogniCode, el agente tendría que adivinar dónde poner breakpoints. |
| **Degradación graceful** | Si un adapter no está instalado, el Doctor dice cómo instalarlo. Si un lenguaje no tiene adapter, las otras herramientas siguen funcionando. |
| **Mismo modelo mental que IntelliJ** | Los usuarios ya entienden breakpoints, stepping, variables. No hay que aprender un concepto nuevo. |
| **Testeable** | Los debug adapters están muy testeados. CogniCode solo tiene que ser un buen DAP client. |

### Contras

| Con | Detalle | Mitigación |
|-----|---------|-----------|
| **Solo funciona con debug builds** | Release builds no tienen debug symbols. | Cero impacto: los tests se corren en debug por defecto. RCode ya compila en debug para testing. |
| **No funciona en producción** | No puedes attacharte a un proceso en producción. | Fuera de scope. Para prod, usar tracing/logging tradicional. |
| **Consumo de tokens** | Cada interacción de debug consume 500-2000 tokens. Una sesión de 20 pasos = 10-40K tokens. | Control de tokens (sección 5.2). Auto-summarize. Snapshot mode reduce round-trips. |
| **Debug adapters son dependencias externas** | codelldb, debugpy, dlv — no se compilan con RCode. | Doctor + auto-install + feature flags. Degradación graceful. |
| **Async debugging es difícil** | En Rust con tokio, step-over en `.await` salta a otro task. | Mismo problema que tiene IntelliJ. Mitigación: snapshot debugging (Propuesta 3). |
| **Plataformas** | codelldb no funciona en ARM Linux. `rr` solo en x86_64 Linux. | Doctor detecta plataforma y adapta recommendations. lldb-dap como alternativa. |
| **Latencia** | Cada step = 1 LLM call = 1-5 segundos. Debug de 20 pasos = 20-100 segundos. | Aceptable para debugging (IntelliJ users también esperan). Auto-strategy reduce pasos. |
| **No determinismo** | Cada ejecución puede ser diferente (timing, random, network). | Record & Replay (Propuesta 1) para Rust/C++. Differential Debug (Propuesta 4) para comparar. |
| **Evaluación limitada en Rust** | codelldb no puede evaluar expresiones complejas como IntelliJ. | Realidad del ecosistema. Python/TS tienen evaluación completa. |

### Contras que NO son reales

| Mito | Realidad |
|------|----------|
| "Necesitamos escribir debug adapters" | ❌ No. Los adapters ya existen (codelldb, debugpy, dlv, js-debug, java-debug). Solo somos DAP client. |
| "DAP es inestable" | ❌ DAP lleva desde 2018, es el protocolo de facto para debugging. VS Code lo usa para todos los lenguajes. |
| "El agente necesitaría entender DWARF/LLDB internals" | ❌ No. DAP abstrae todo eso. El agente solo ve JSON con nombres y valores legibles. |
| "Solo funciona para Rust" | ❌ No. DAP es multi-lenguaje por diseño. Esta es la razón principal de usar DAP. |
| "Modifica el código fuente" | ❌ No. Zero código modificado. El debug se hace via protocolo, no via instrumentación. |

---

## 8. Veredicto

### ¿Vale la pena? **Sí, absolutamente.**

**Razón 1: Es el missing piece**. CogniCode tiene static analysis (call graph, complexity, hot paths). DAP le da **dynamic analysis** (runtime values, execution flow). La combinación es única — ningún otro agente de IA tiene esto.

**Razón 2: El esfuerzo es razonable**. ~4100 LOC para el DAP client + adapter management. Los debug adapters ya existen. No inventamos nada nuevo.

**Razón 3: El modelo es correcto**. Zero código modificado, estándar industrial, multi-lenguaje, degradación graceful. No es un hack — es como debería funcionar.

**Razón 4: Sinergia con CogniCode**. El call graph de CogniCode puede generar estrategias de debug automáticas (Propuesta 2). El agente no adivina dónde poner breakpoints — CogniCode se lo dice basándose en el call graph. Esto es algo que IntelliJ NO puede hacer para sus usuarios.

### ¿Cuándo? **Después de IMPROVEMENT-PLAN-V2 P0-P3.**

No es urgente. La base (persistencia, incremental indexing, event streaming) es más importante. Pero debería ser **el siguiente proyecto grande** después de que CogniCode tenga sus fundamentals sólidos.

### ¿Qué NO hacer?

1. ❌ No escribir debug adapters — ya existen
2. ❌ No inyectar tracing en código fuente — DAP es más limpio
3. ❌ No empezar con Record & Replay — muy específico de plataforma
4. ❌ No intentar hot code replace en Rust — no es posible

### ¿Qué SÍ hacer primero?

1. ✅ DAP client genérico (funciona para todos los lenguajes)
2. ✅ Doctor + auto-install (garantiza disponibilidad)
3. ✅ Feature flags (degradación graceful)
4. ✅ Test-to-Debug Bridge (Propuesta 5 — alto ROI)
5. ✅ Automated Debug Strategy (Propuesta 2 — killer feature)

---

## 9. Roadmap de implementación

### Fase 0: Prototipo DAP Client (1 semana)

```
Semana 1:
├── Día 1-2: protocol.rs — DAP types (requests, responses, events)
│   └── Generado desde el JSON schema oficial de DAP
├── Día 3-4: client.rs — DapClient (connect, request, listen events)
│   └── JSON over stdio, seq numbers, response matching
├── Día 5: session.rs — DebugSessionManager básico (launch + terminate)
└── Día 5: tests/integration_test.rs — test contra codelldb con proyecto Rust simple
```

**Gate**: `debug_start` + `debug_set_breakpoints` + `debug_continue` + `debug_get_variables` + `debug_terminate` funcionan con codelldb en un proyecto Rust simple.

### Fase 1: Multi-lenguaje + Doctor (2 semanas)

```
Semana 2-3:
├── Día 1-2: AdapterRegistry — configs para Rust, Python, TypeScript, Go, Java
├── Día 3-4: AdapterInstaller — detect, download, verify adapters
├── Día 5-6: Doctor — health checks por lenguaje
├── Día 7: Feature flags — CapabilityLevel, degradación
├── Día 8: tools.rs — 8 MCP tools implementadas
└── Día 9-10: Tests de integración por lenguaje (Python con debugpy, TS con js-debug)
```

**Gate**: Doctor reporta estado correcto. `debug_start` funciona para Rust, Python, TypeScript. Feature flags deshabilitan debug si adapter falta.

### Fase 2: Integración con CogniCode (1 semana)

```
Semana 4:
├── Día 1-2: Propuesta 2 — generate_debug_strategy() 
│   └── Usa call graph de CogniCode para sugerir breakpoints
├── Día 3: Propuesta 5 — debug_failed_test()
│   └── Parse test output → auto-setup debug session
├── Día 4: Propuesta 3 — Snapshot debugging (logpoints)
└── Día 5: Propuesta 6 — Debug replay log
```

**Gate**: `generate_debug_strategy("process_order")` sugiere breakpoints basados en el call graph. `debug_failed_test` detecta el test y configura la sesión automáticamente.

### Fase 3: Polish y avanzadas (1 semana)

```
Semana 5:
├── Día 1-2: Control de tokens (auto-summarize, max_variables, truncation)
├── Día 3: Propuesta 4 — Differential debugging
├── Día 4: Propuesta 1 — rr integration (solo Linux x86_64)
└── Día 5: Documentación, examples, cleanup
```

**Gate**: Token consumption < 5K por sesión típica de 10 pasos. rr funciona en Linux si está instalado.

### Dependencias temporales

```
IMPROVEMENT-PLAN-V2:
  Fase A (P0+P1) ───────┐
  Fase B (P2 persist) ──┤
  Fase C (P3+P5) ───────┤
                         │
                         ▼
IDE-FEATURES-PLAN:       │  (puede empezar en paralelo)
  Fase 1 (F3) ──────────┤
  Fase 2 (F1,F4,F5...) ─┤
                         │
                         ▼
AI DEBUG PLAN:           │  (después de P0-P3)
  Fase 0 (DAP client) ──┤
  Fase 1 (Multi-lang) ──┤
  Fase 2 (CogniCode) ───┤  ← necesita call graph (F3)
  Fase 3 (Polish) ──────┘
```

**Total**: 5 semanas de trabajo. ~4100 LOC de código nuevo. 0 debug adapters escritos (todos reutilizados).

---

## Apéndice A: Dependencias Rust

| Crate | Versión | Propósito | Pure Rust? |
|-------|---------|-----------|------------|
| `serde_json` | 1.x | DAP messages son JSON | ✅ |
| `tokio` | 1.x | Async I/O para stdio del adapter | ✅ (ya en workspace) |
| `reqwest` | 0.12.x | Download adapters | ✅ |
| `sha2` | 0.10.x | Verify adapter checksums | ✅ |
| `zip` | 2.x | Unpack adapter archives | ✅ |
| `dirs` | 6.x | Resolver `~/.local/share/` | ✅ |
| `tempfile` | 3.x | Archivos temporales | ✅ |

**Total overhead**: ~200KB. Todas pure Rust.

## Apéndice B: Flujo completo de debug (diagrama de secuencia)

```
Agente          RCode          DapClient       codelldb         Proceso Target
  │               │               │               │                  │
  │ debug_start   │               │               │                  │
  │──────────────►│               │               │                  │
  │               │ [Doctor check]│               │                  │
  │               │ [cargo build] │               │                  │
  │               │ [spawn adapter]               │                  │
  │               │               │ initialize    │                  │
  │               │               │──────────────►│                  │
  │               │               │ capabilities  │                  │
  │               │               │◄──────────────│                  │
  │               │               │ launch        │                  │
  │               │               │──────────────►│                  │
  │               │               │               │ spawn process ──►│
  │               │               │ initialized   │                  │
  │               │               │◄──────────────│                  │
  │ session_id    │               │               │                  │
  │◄──────────────│               │               │                  │
  │               │               │               │                  │
  │ set_breakpoints               │               │                  │
  │──────────────►│               │               │                  │
  │               │               │ setBreakpoints│                  │
  │               │               │──────────────►│                  │
  │               │               │               │ inject INT3 ────►│
  │               │               │ breakpoints   │                  │
  │               │               │◄──────────────│                  │
  │ {breakpoints} │               │               │                  │
  │◄──────────────│               │               │                  │
  │               │               │               │                  │
  │ continue      │               │               │                  │
  │──────────────►│               │               │                  │
  │               │               │ continue      │                  │
  │               │               │──────────────►│                  │
  │               │               │               │ resume ─────────►│
  │               │               │               │                  │ running...
  │               │               │               │◄── SIGTRAP ──────│ hit line 28!
  │               │               │ stopped event │                  │
  │               │               │◄──────────────│                  │
  │ {stopped at 28}               │               │                  │
  │◄──────────────│               │               │                  │
  │               │               │               │                  │
  │ get_variables │               │               │                  │
  │──────────────►│               │               │                  │
  │               │               │ threads       │                  │
  │               │               │──────────────►│                  │
  │               │               │ stackTrace    │                  │
  │               │               │──────────────►│                  │
  │               │               │ scopes        │                  │
  │               │               │──────────────►│                  │
  │               │               │ variables     │                  │
  │               │               │──────────────►│ read memory ────►│
  │               │               │ {order: ...}  │                  │
  │               │               │◄──────────────│                  │
  │ {variables}   │               │               │                  │
  │◄──────────────│               │               │                  │
  │               │               │               │                  │
  │ "items empty!" │              │               │                  │
  │ terminate     │               │               │                  │
  │──────────────►│               │ disconnect    │                  │
  │               │               │──────────────►│ kill ────────────►│
  │ {done}        │               │               │                  │
  │◄──────────────│               │               │                  │
```
