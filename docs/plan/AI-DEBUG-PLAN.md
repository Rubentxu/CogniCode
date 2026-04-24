# CogniCode — AI Debug Plan (v2)

> **Fecha**: Abril 2026
> **Estado**: Plan de diseño (REVISADO)
> **Alcance**: Debugging autonomous para agentes LLM via Debug Adapter Protocol (DAP)
> **Lenguajes**: Rust, TypeScript/JavaScript, Python, Go, Java/Kotlin
> **Prerrequisitos**: Ninguno — funciona independiente de IMPROVEMENT-PLAN-V2 e IDE-FEATURES-PLAN

---

## Resumen Ejecutivo

**El problema**: DAP está diseñado para humanos que hacen debugging step-by-step. Un agente IA necesita **conclusiones**, no estado de debugger.

**La solución**: **Debug Orchestrator** — ejecuta debugging autonomously y retorna root cause + recommendation.

**Insight clave**: El agente dice "test X falla, por qué?" y recibe "validate() fue llamado con array vacío en línea 42. Recomendación: añadir check antes de llamar validate()". Cero interacción.

---

## Índice

1. [Arquitectura v2 — Debug Orchestrator](#1-arquitectura-v2--debug-orchestrator)
2. [Detección de Herramientas — Doctor v2](#2-detección-de-herramientas--doctor-v2)
3. [Gestión de Capabilities por Lenguaje](#3-gestión-de-capabilities-por-lenguaje)
4. [API y MCP Tools Simplificada](#4-api-y-mcp-tools-simplificada)
5. [DAP Client (implementación interna)](#5-dap-client-implementación-interna)
6. [Pros y Contras](#6-pros-y-contras)
7. [Roadmap de Implementación](#7-roadmap-de-implementación)

---

## 1. Arquitectura v2 — Debug Orchestrator

### 1.1 El modelo: loop eliminado

```
MODELO V1 (human loop) — NO ES OPTIMO:
┌──────────────────────────────────────────────────────┐
│ Agente → debug_start → "breakpoint en línea 15"      │
│       → debug_continue → "se paró, x=5"             │
│       → debug_get_variables → "items = []"          │
│       → debug_step → ... (20+ llamadas)             │
└──────────────────────────────────────────────────────┘

MODELO V2 (autonomous) — LO QUE QUEREMOS:
┌──────────────────────────────────────────────────────┐
│ Agente → debug_analyze("test falla", "index out")   │
│       ←───────────────────────                       │
│       Root Cause: validate() recibió array vacío     │
│       Stack: test → process → validate (línea 42)   │
│       Variables: order.items = []                    │
│       Recommendation: añadir items.empty() check     │
└──────────────────────────────────────────────────────┘
```

### 1.2 Arquitectura completa

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Agente LLM                                   │
│  "test_process_order falla con 'index out of bounds', por qué?"   │
└─────────────────────────────┬───────────────────────────────────────┘
                              │ 1. debug_analyze()
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                   Debug Orchestrator                                 │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ DebugAnalysisEngine                                           │   │
│  │                                                              │   │
│  │ 1. Recibe: target + error_description                        │   │
│  │ 2. Genera estrategia de breakpoints desde call graph         │   │
│  │ 3. Ejecuta debug autonomously (DAP interno):               │   │
│  │    - Lanza adapter                                          │   │
│  │    - Configura breakpoints automáticamente                  │   │
│  │    - Continúa hasta crash/breakpoint                        │   │
│  │    - Captura stack + variables                              │   │
│  │ 4. Analiza contexto                                         │   │
│  │ 5. Genera conclusión en lenguaje natural                    │   │
│  │ 6. Cleanup: termina sesión                                  │   │
│  │                                                              │   │
│  │ SALIDA: RootCause + Recommendation + Evidence                │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              │ DAP (interno, invisible para agente) │
│                              ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                      DapClient                               │   │
│  │  (JSON stdio, implementación interna)                       │   │
│  └─────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │ stdio
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                   Debug Adapter (externo)                            │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
│  │ codelldb │  │ debugpy  │  │ js-debug │  │  dlv     │       │
│  │ (Rust)   │  │ (Python) │  │ (TS/JS)  │  │  (Go)    │       │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘       │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.3 Componentes

| Componente | LOC | Responsabilidad |
|-----------|-----|----------------|
| `DapClient` | ~800 LOC | Cliente DAP genérico (interno, no expuesto al agente) |
| `AdapterRegistry` | ~300 LOC | Detectar adapter por lenguaje |
| `AdapterInstaller` | ~400 LOC | Auto-install de adapters faltantes |
| `Doctor` | ~300 LOC | Health check + capability detection |
| `DebugOrchestrator` | ~600 LOC | Coordinar análisis autonomous |
| `AnalysisEngine` | ~400 LOC | Generar conclusiones desde stack/variables |
| **Total** | **~2800 LOC** | |

**Reducción vs v1**: 4100 → 2800 LOC (-32%) porque eliminamos las 8+ tools de bajo nivel.

### 1.4 Crate propuesto

```
rcode-debug/
├── Cargo.toml
├── src/
│   ├── lib.rs              // public API: debug_analyze(), debug_doctor()
│   ├── client.rs           // DapClient (JSON stdio, interno)
│   ├── adapter/
│   │   ├── mod.rs         // AdapterRegistry
│   │   ├── installer.rs    // Auto-install
│   │   └── configs.rs      // per-language configs
│   ├── doctor.rs           // Health checks
│   ├── orchestrator.rs     // DebugOrchestrator
│   ├── analysis.rs         // AnalysisEngine (root cause, recommendations)
│   └── error.rs            // Error types
└── tests/
    ├── doctor_test.rs
    ├── orchestrator_test.rs
    └── integration_test.rs  // contra adapters reales
```

---

## 2. Detección de Herramientas — Doctor v2

### 2.1 Filosofía

**El agente NO debe descubrir por ensayo-error qué funcionalidades están disponibles.** El Doctor responde:
- ¿Qué lenguajes puedo debuggear?
- ¿Qué funcionalidades están disponibles?
- ¿Qué falta y cómo instalarlo?

### 2.2 Doctor Report

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub timestamp: String,
    pub debug_enabled: bool,
    pub languages: Vec<LanguageDebugStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageDebugStatus {
    pub language: String,
    pub available: bool,           // ¿Hay adapter instalado?
    pub toolchain_ok: bool,        // ¿Está el compilador/interpretador?
    pub capabilities: DebugCapabilities,
    pub install_instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugCapabilities {
    pub crash_analysis: bool,       // ✅ Análisis de crash (SIEMPRE funciona)
    pub snapshot_capture: bool,     // ✅ Captura de estado sin parar
    pub step_debugging: bool,      // ⚠️ Step-by-step (requiere adapter)
    pub evaluation: bool,           // ⚠️ Evaluar expresiones (limitado en Rust)
}
```

### 2.3 Salida del Doctor (formato XML para el agente)

```xml
<debug-health>
  <status enabled="true" />

  <rust available="true" toolchain="cargo 1.82.0">
    <capabilities
      crash_analysis="yes"
      snapshot_capture="yes"
      step_debugging="yes"
      evaluation="limited"
    />
    <note>codelldb v1.10.0 installed at ~/.local/bin/codelldb</note>
  </rust>

  <python available="false">
    <capabilities
      crash_analysis="yes"
      snapshot_capture="yes"
      step_debugging="no"
      evaluation="no"
    />
    <install>pip install debugpy</install>
    <note>python3 3.12.0 found but debugpy not installed</note>
  </python>

  <typescript available="true" toolchain="node 22.0.0">
    <capabilities
      crash_analysis="yes"
      snapshot_capture="yes"
      step_debugging="yes"
      evaluation="yes"
    />
    <note>js-debug (built-in with Node.js)</note>
  </typescript>
</debug-health>
```

### 2.4 Check inicial automático

Antes de procesar cualquier request de debug, el sistema hace check automático:

1. Si `debug_analyze()` es llamado:
   - Si el lenguaje NO tiene `crash_analysis` disponible → error con instrucciones
   - Si el lenguaje tiene `crash_analysis` pero no `step_debugging` → proceed con crash analysis

2. Si el lenguaje no está disponible:
   - Error con `install_instructions` específicas

---

## 3. Gestión de Capabilities por Lenguaje

### 3.1 Capability Matrix

| Capability | Rust (codelldb) | Python (debugpy) | TS/JS (js-debug) | Go (dlv) | Java (java-debug) |
|------------|-----------------|------------------|------------------|-----------|-------------------|
| **crash_analysis** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **snapshot_capture** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **step_debugging** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **evaluation** | ⚠️ limited | ✅ | ✅ | ✅ | ✅ |

### 3.2 Degradación por capability

```rust
pub enum AnalysisMode {
    /// Solo crash analysis - lanza proceso y espera crash
    CrashOnly,
    /// Snapshot mode - breakpoints en puntos clave, captura estado
    Snapshot,
    /// Full debugging - breakpoints configurables, step, eval
    Full,
}

impl DebugOrchestrator {
    pub fn choose_mode(&self, lang: &str, caps: &DebugCapabilities) -> AnalysisMode {
        match lang {
            "rust" if !caps.step_debugging => AnalysisMode::CrashOnly,
            "rust" if !caps.evaluation => AnalysisMode::Snapshot,
            _ if !caps.crash_analysis => AnalysisMode::CrashOnly,
            _ => AnalysisMode::Full,
        }
    }
}
```

### 3.3 Feature Flags de Usuario

```toml
# .rcode/config.toml

[debug]
enabled = true

[debug.languages.rust]
# Preferir crash analysis aunque step_debugging esté disponible
prefer_mode = "crash_only"

[debug.languages.python]
# Habilitar step debugging aunque sea más lento
prefer_mode = "full"
```

---

## 4. API y MCP Tools Simplificada

### 4.1 Tool única: debug_analyze

```rust
Tool: debug_analyze
Descripción: Analiza un test/script/binario fallido y retorna root cause + recomendación
Input: {
  target: {
    type: "test" | "script" | "binary",
    name: "test_process_order",    // para tests
    file: "src/main.py",          // para scripts
    binary: "target/debug/myapp",  // para binarios
  },
  error: {
    type: "panic" | "assertion" | "exit_code" | "timeout",
    message: "index out of bounds",   // mensaje de error
    output: "...stack trace..." ,       // optional: output completo
  },
  context?: {
    // Opcional: hints del agente para focalizar el análisis
    suspect_functions?: ["validate", "calculate_total"],
  }
}
Output: {
  success: true,
  analysis: {
    root_cause: {
      summary: "validate() fue llamado con array vacío",
      location: {
        function: "validate",
        file: "src/orders.rs",
        line: 42,
      },
      explanation: "La función validate() asume que order.items tiene elementos, pero recibe un array vacío",
    },
    stack_trace: [
      { function: "test_process_order", file: "tests/orders_test.rs", line: 12 },
      { function: "process_order", file: "src/orders.rs", line: 23 },
      { function: "validate", file: "src/orders.rs", line: 42, ← CRASH },
    ],
    variables_at_crash: {
      order: "Order { id: 42, items: [], total: 0.0 }",
      // ... más variables capturadas
    },
    recommendation: {
      action: "Añadir check antes de llamar validate()",
      code: "if order.items.is_empty() { return Err(...); }",
      alternative: "Hacer validate() idempotente para arrays vacíos",
    },
  },
  capabilities_used: {
    mode: "crash_analysis",
    adapter: "codelldb v1.10.0",
    execution_time_ms: 1250,
  }
}
```

### 4.2 Tool: debug_doctor

```rust
Tool: debug_doctor
Descripción: Verifica qué funcionalidades de debugging están disponibles
Input: {}
Output: DoctorReport (ver sección 2.2)
Cuándo usarlo: Al inicio de una sesión o cuando el agente quiere saber qué puede debuggear
```

### 4.3 Tool: debug_snapshot (futuro)

```rust
Tool: debug_snapshot
Descripción: Captura estado de variables en puntos específicos sin parar el proceso
Input: {
  target: { type: "test" | "script", name: "..." },
  capture_points: [
    { function: "validate", variables: ["order", "result"] },
    { function: "calculate_total", variables: ["total"] },
  ]
}
Output: {
  snapshots: [
    {
      at: "validate() entry",
      values: { order: "Order {...}", result: "Ok(...)" },
      timestamp_ms: 42,
    },
    {
      at: "calculate_total() entry",
      values: { total: "0.0" },
      timestamp_ms: 128,
    },
  ]
}
```

### 4.4 NO exponemos (human-loop debugging)

Las siguientes tools NO se exponen al agente porque requieren interacción:

```
NO EXPUESTAS:
- debug_start       (separado en 2: init implícito en analyze)
- debug_set_breakpoints
- debug_continue
- debug_step
- debug_get_stack
- debug_get_variables
- debug_evaluate
- debug_terminate
```

Estas son implementación interna del Orchestrator, no tools públicas.

---

## 5. DAP Client (implementación interna)

### 5.1 Interfaz simplificada

```rust
// INTERNAMENTE el Orchestrator usa estas APIs:
impl DapClient {
    /// Inicializa conexión con el adapter
    pub async fn connect(&mut self, adapter: &AdapterConfig) -> Result<(), DapError>;

    /// Lanza el proceso target y configura
    pub async fn launch(&mut self, config: LaunchConfig) -> Result<SessionId, DapError>;

    /// Establece breakpoints
    pub async fn set_breakpoints(
        &mut self,
        source: &str,
        lines: &[u32],
    ) -> Result<Vec<BreakpointStatus>, DapError>;

    /// Continúa ejecución hasta breakpoint o crash
    pub async fn continue_(&mut self) -> Result<StoppedEvent, DapError>;

    /// Captura stack trace completo
    pub async fn stack_trace(&mut self) -> Result<Vec<StackFrame>, DapError>;

    /// Captura variables en el frame actual
    pub async fn variables(&mut self, frame: u32) -> Result<Vec<Variable>, DapError>;

    /// Evalúa expresión
    pub async fn evaluate(&mut self, expr: &str) -> Result<EvalResult, DapError>;

    /// Termina sesión y limpia
    pub async fn disconnect(&mut self) -> Result<(), DapError>;
}
```

### 5.2 Por qué NO exponemos DAP crudo

| Razón | Explicación |
|-------|-------------|
| **Token cost** | Cada llamada DAP = tokens. Step-by-step = 20+ llamadas |
| **Latencia** | Cada llamada = 1-5s. Sesión de debug = minutos |
| **Complejidad** | El agente tendría que entender DAP state machine |
| **No es óptio** | El agente quiere conclusiones, no debugger state |

---

## 6. Pros y Contras

### Pros

| Pro | Detalle |
|-----|---------|
| **Cero human loop** | El agente recibe conclusiones, no estado de debugger |
| **1 LLM call** | Un `debug_analyze()` = root cause completo |
| **Autonomous** | El Orchestrator ejecuta debug sin intervención |
| **DAP powers** | Aprovecha adapters existentes, multi-lenguaje |
| **Graceful degradation** | Si adapter falta, crash analysis sigue funcionando |
| **Doctor integrado** | El agente sabe qué puede y qué no |

### Contras

| Con | Detalle | Mitigación |
|-----|---------|-----------|
| **Solo funciona con debug builds** | Release builds no tienen symbols | Tests corren en debug por defecto |
| **Crash analysis requiere crash** | Solo funciona si el test realmente crashea | Snapshot mode para casos más sutiles |
| **Step debugging sigue siendo útil** | Algunos bugs no se revelan en crash | Exponer solo si Doctor dice que está disponible |
| **Adapter auto-install** | Descargar ~15MB puede ser lento | Solo cuando se necesita, con progress |

### Contras que NO son reales

| Mito | Realidad |
|------|----------|
| "DAP requiere human loop" | ❌ Solo si lo usamos así. Con Orchestrator es autonomous |
| "El agente no puede debuggear" | ❌ Sí puede, pero recibe conclusiones, no state |
| "No sabemos qué está disponible" | ❌ Doctor detecta y reporta capabilities |

---

## 7. Roadmap de Implementación

### Fase 1: DAP Client + Doctor (1 semana)

```
Semana 1:
├── Día 1-2: DapClient básico (connect, launch, disconnect)
├── Día 3: AdapterRegistry + configs para Rust + Python
├── Día 4: Doctor + health checks
├── Día 5: AdapterInstaller (auto-download codelldb, debugpy)
└── Día 5: Tests básicos de integración
```

**Gate**: `debug_doctor()` retorna estado correcto. `debug_analyze()` funciona para crash de Rust con codelldb.

### Fase 2: Debug Orchestrator + Analysis Engine (1 semana)

```
Semana 2:
├── Día 1-2: DebugOrchestrator (lanzar + configurar breakpoints auto)
├── Día 3-4: AnalysisEngine (generar root cause desde stack/variables)
├── Día 5: Integración DapClient + Orchestrator
└── Día 5: Tests de integración completos
```

**Gate**: `debug_analyze("test", "panic")` retorna root_cause + recommendation en formato estructurado.

### Fase 3: Multi-lenguaje + polish (1 semana)

```
Semana 3:
├── Día 1-2: Python (debugpy) + TypeScript (js-debug)
├── Día 3: Go (delve) + Java (java-debug)
├── Día 4: Graceful degradation (CrashOnly mode si no hay adapter)
├── Día 5: Tests de integración por lenguaje
└── Día 5: Documentación
```

**Gate**: `debug_analyze()` funciona para los 5 lenguajes. Graceful degradation cuando adapter falta.

### Dependencias de implementación

| Fase | Qué se necesita |
|------|-----------------|
| Fase 1 | Solo DapClient - no requiere CogniCode |
| Fase 2 | Acceso a call graph para breakpoints automáticos (usa CogniCode) |
| Fase 3 | Nada adicional |

---

## Apéndice A: Diferencia con v1

| Aspecto | v1 | v2 |
|---------|----|----|
| Tools expuestas | 8+ (debug_start, debug_continue, etc) | 1-2 (debug_analyze, debug_doctor) |
| Interacción | Human loop (20+ llamadas) | Autonomous (1 llamada) |
| DAP Client | Externo (el agente lo opera) | Interno (Orchestrator lo usa) |
| Doctor | Check manual | Automático antes de cada operación |
| Capabilities | Fijas | Degradación graceful |

## Apéndice B: Formato de Salida para el Agente

```xml
<debug-analysis success="true">
  <root-cause>
    <summary>validate() recibió array vacío</summary>
    <location function="validate" file="src/orders.rs" line="42" />
    <explanation>La función espera que order.items tenga elementos pero recibe []</explanation>
  </root-cause>
  <stack-trace>
    <frame function="test_process_order" file="tests/orders_test.rs" line="12" />
    <frame function="process_order" file="src/orders.rs" line="23" />
    <frame function="validate" file="src/orders.rs" line="42" crash="true" />
  </stack-trace>
  <variables>
    <var name="order" value="Order { id: 42, items: [], total: 0.0 }" />
    <var name="result" value="Panic: index out of bounds" />
  </variables>
  <recommendation>
    <action>Añadir check antes de llamar validate()</action>
    <code-suggestion>if order.items.is_empty() { return Err(...); }</code-suggestion>
  </recommendation>
  <metadata>
    <mode>crash_analysis</mode>
    <adapter>codelldb v1.10.0</adapter>
    <execution-time-ms>1250</execution-time-ms>
  </metadata>
</debug-analysis>
```
