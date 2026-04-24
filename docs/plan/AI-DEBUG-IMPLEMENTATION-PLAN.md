# AI-DEBUG Implementation Plan

> **Fecha**: Abril 2026
> **Plan basado en**: AI-DEBUG-PLAN.md v2
> **Crate destino**: `rcode-debug`
> **Dependencias**: Ninguna (funciona standalone)

---

## Meta

| Aspecto | Detalle |
|---------|---------|
| **Objetivo** | Debug autonomous para agentes LLM via DAP |
| **Entregable** | Crate `rcode-debug` con `debug_analyze()` y `debug_doctor()` |
| **Timeline** | 3 semanas |
| **LOC estimado** | ~2800 |

---

## Arquitectura objetivo

```
Agente                    rcode-debug crate                Externals
   │                           │                              │
   │ debug_analyze(...)        │                              │
   │ ────────────────────────► │                              │
   │                           │                              │
   │                           │  1. Doctor.check()           │
   │                           │ ─────────────────────────►  │ (check adapters)
   │                           │ ◄─────────────────────────  │
   │                           │                              │
   │                           │  2. Orchestrator.run()      │
   │                           │   ├── DapClient.connect()    │
   │                           │ ─────────────────────────►  │ codelldb
   │                           │   ├── DapClient.launch()     │
   │                           │ ◄─────────────────────────  │
   │                           │   ├── breakpoints.auto       │
   │                           │   ├── continue_until_crash() │
   │                           │   ├── stack_trace()         │
   │                           │   ├── variables()           │
   │                           │ ─────────────────────────►  │
   │                           │                              │
   │                           │  3. AnalysisEngine          │
   │                           │   └── generate_conclusion()  │
   │                           │                              │
   │ RootCause + Recommendation│                              │
   │ ◄────────────────────── │                              │
```

---

## Fase 1: DAP Client + Doctor

**Semana 1** | **~1000 LOC**

### Task 1.1: DapClient básico

```rust
// src/client.rs
pub struct DapClient {
    // Process que corre el adapter
    child: Child,
    // Sequences para matching request/response
    seq: u64,
    // Canales para eventos async
    event_rx: mpsc::Receiver<DapEvent>,
}

impl DapClient {
    pub async fn connect(adapter_path: &Path) -> Result<Self, DapError>;
    pub async fn initialize() -> Result<Capabilities, DapError>;
    pub async fn disconnect() -> Result<(), DapError>;
}
```

**Criterio**: `DapClient::connect("codelldb")` conecta y devuelve capabilities.

### Task 1.2: Launch y configuration

```rust
impl DapClient {
    pub async fn launch(&mut self, config: LaunchConfig) -> Result<(), DapError>;
    pub async fn configuration_done(&mut self) -> Result<(), DapError>;
}
```

**Criterio**: Puede lanzar un binario Rust y recibir evento `stopped` en entry point.

### Task 1.3: Breakpoints

```rust
impl DapClient {
    pub async fn set_breakpoints(
        &mut self,
        source: &str,
        lines: &[u32],
    ) -> Result<Vec<BreakpointStatus>, DapError>;
}
```

**Criterio**: Puede poner breakpoints en un binario y recibe `stopped` cuando se paran.

### Task 1.4: Continue y stack trace

```rust
impl DapClient {
    pub async fn continue_(&mut self) -> Result<StoppedEvent, DapError>;
    pub async fn stack_trace(&mut self) -> Result<Vec<StackFrame>, DapError>;
}
```

**Criterio**: `continue()` corre hasta breakpoint o crash, `stack_trace()` devuelve frames.

### Task 1.5: Variables

```rust
impl DapClient {
    pub async fn variables(&mut self, frame: u32) -> Result<Vec<Variable>, DapError>;
}
```

**Criterio**: Puede leer variables locales de un frame.

### Task 1.6: AdapterRegistry

```rust
// src/adapter/registry.rs
pub struct AdapterRegistry {
    configs: HashMap<String, AdapterConfig>,
}

pub struct AdapterConfig {
    pub name: String,
    pub language: String,
    pub binary: String,
    pub install_cmd: Option<String>,
    pub min_version: String,
}
```

**Criterio**: Registry tiene configs para Rust (codelldb), Python (debugpy), TypeScript (js-debug), Go (dlv).

### Task 1.7: Doctor + health checks

```rust
// src/doctor.rs
pub struct Doctor {
    registry: AdapterRegistry,
}

impl Doctor {
    pub async fn check(&self) -> DoctorReport;
    pub fn check_toolchain(&self, lang: &str) -> ToolchainStatus;
    pub fn check_adapter(&self, lang: &str) -> AdapterStatus;
}
```

**Criterio**: `doctor.check()` detecta qué adapters están instalados y qué capabilities tiene cada uno.

### Task 1.8: AdapterInstaller

```rust
// src/adapter/installer.rs
impl AdapterInstaller {
    pub async fn install(&self, lang: &str) -> Result<(), DapError>;
    pub async fn verify(&self, lang: &str) -> Result<bool, DapError>;
}
```

**Criterio**: Puede descargar e instalar codelldb desde GitHub releases.

---

## Fase 2: Debug Orchestrator + Analysis Engine

**Semana 2** | **~1000 LOC**

### Task 2.1: DebugOrchestrator

```rust
// src/orchestrator.rs
pub struct DebugOrchestrator {
    client: DapClient,
    doctor: Doctor,
    analysis: AnalysisEngine,
}

impl DebugOrchestrator {
    pub async fn analyze(
        &self,
        request: DebugAnalyzeRequest,
    ) -> Result<DebugAnalysisResult, DebugError> {
        // 1. Check capabilities
        let caps = self.doctor.check_language(&request.lang);

        // 2. Choose mode (CrashOnly, Snapshot, Full)
        let mode = self.choose_mode(&request.lang, &caps);

        // 3. Connect adapter
        self.client.connect(&caps.adapter).await?;

        // 4. Launch target
        self.client.launch(&request.target).await?;

        // 5. Configure breakpoints (auto from request or call graph)
        self.set_breakpoints_auto(&request).await?;

        // 6. Run until crash/breakpoint
        let stopped = self.client.continue_().await?;

        // 7. Capture state
        let stack = self.client.stack_trace().await?;
        let vars = self.client.variables(0).await?;

        // 8. Generate conclusion
        let conclusion = self.analysis.generate(&stopped, &stack, &vars);

        // 9. Cleanup
        self.client.disconnect().await?;

        Ok(conclusion)
    }
}
```

**Criterio**: `orchestrator.analyze(request)` retorna `DebugAnalysisResult` con root_cause y recommendation.

### Task 2.2: AnalysisEngine

```rust
// src/analysis.rs
pub struct AnalysisEngine {
    // Acceso opcional a call graph para contextualización
    call_graph: Option<Arc<CallGraph>>,
}

impl AnalysisEngine {
    pub fn generate(
        &self,
        stopped: &StoppedEvent,
        stack: &[StackFrame],
        vars: &[Variable],
    ) -> DebugAnalysisResult {
        // 1. Parse crash type (panic, assertion, exit_code)
        // 2. Find crash location in stack
        // 3. Extract relevant variables
        // 4. Generate natural language explanation
        // 5. Suggest fix based on patterns
    }
}
```

**Criterio**: Dado un panic de Rust, genera root_cause inteligible.

### Task 2.3: Integración con CrashReport

```rust
pub struct DebugAnalyzeRequest {
    pub target: Target,
    pub error: ErrorInfo,
    pub context: Option<AnalysisContext>,
}

pub struct ErrorInfo {
    pub kind: ErrorKind,  // Panic, Assertion, ExitCode, Timeout
    pub message: String,
    pub output: Option<String>,
}
```

**Criterio**: El request puede incluir output de test para parsing de crash.

### Task 2.4: Graceful degradation

```rust
impl DebugOrchestrator {
    fn choose_mode(&self, lang: &str, caps: &DebugCapabilities) -> AnalysisMode {
        if !caps.crash_analysis {
            return AnalysisMode::Unavailable;
        }
        match lang {
            "rust" if !caps.step_debugging => AnalysisMode::CrashOnly,
            _ if !caps.step_debugging => AnalysisMode::CrashOnly,
            _ => AnalysisMode::Full,
        }
    }
}
```

**Criterio**: Si adapter no está, retorna error con instrucciones de instalación.

---

## Fase 3: MCP Integration + Multi-lenguaje

**Semana 3** | **~800 LOC**

### Task 3.1: MCP Tool: debug_analyze

```rust
// src/tools.rs
pub struct DebugAnalyzeTool;

impl Tool for DebugAnalyzeTool {
    const NAME: &'static str = "debug_analyze";
    const DESCRIPTION: &'static str = "...";

    type Input = DebugAnalyzeRequest;
    type Output = DebugAnalysisResult;

    async fn execute(&self, input: Self::Input) -> Result<Self::Output, ToolError> {
        let orchestrator = self.get_orchestrator()?;
        orchestrator.analyze(input).await
    }
}
```

**Criterio**: La tool está registrada en el MCP server y responde al schema.

### Task 3.2: MCP Tool: debug_doctor

```rust
pub struct DebugDoctorTool;

impl Tool for DebugDoctorTool {
    const NAME: &'static str = "debug_doctor";
    const DESCRIPTION: &'static str = "Check debugging capabilities";

    type Input = ();
    type Output = DoctorReport;

    async fn execute(&self, _input: ()) -> Result<Self::Output, ToolError> {
        self.doctor.check().await
    }
}
```

**Criterio**: `debug_doctor` retorna el health report.

### Task 3.3: Python adapter (debugpy)

**Criterio**: `debug_analyze()` funciona para tests Python usando debugpy.

### Task 3.4: TypeScript adapter (js-debug)

**Criterio**: `debug_analyze()` funciona para scripts TypeScript usando js-debug (built-in con Node).

### Task 3.5: Go adapter (delve)

**Criterio**: `debug_analyze()` funciona para tests Go usando dlv.

### Task 3.6: Tests de integración

```rust
// tests/integration/
// test_rust_crash.rs      - crash analysis con codelldb
// test_python_crash.rs    - crash analysis con debugpy
// test_typescript_debug.rs - debug con js-debug
// test_graceful_degrad.rs - cuando adapter falta
```

**Criterio**: Todos los tests de integración pasan.

---

## Dependencias entre Tasks

```
Fase 1 (Semana 1)
├── Task 1.1 DapClient básico
├── Task 1.2 Launch
├── Task 1.3 Breakpoints
├── Task 1.4 Continue + stack
├── Task 1.5 Variables
│   └── Task 1.1-1.5 son secuenciales (DapClient crece)
│
├── Task 1.6 AdapterRegistry
│   └── Task 1.1-1.5 completados
│
├── Task 1.7 Doctor
│   └── Task 1.6 completado
│
└── Task 1.8 AdapterInstaller
    └── Task 1.6 completado

Fase 2 (Semana 2)
├── Task 2.1 DebugOrchestrator
│   └── Task 1.1-1.5, 1.7 completados
│
├── Task 2.2 AnalysisEngine
│   └── Task 2.1 completado
│
├── Task 2.3 Request/Response types
│   └── Task 2.1 completado
│
└── Task 2.4 Graceful degradation
    └── Task 2.1 completado

Fase 3 (Semana 3)
├── Task 3.1 debug_analyze MCP tool
│   └── Task 2.1-2.4 completados
│
├── Task 3.2 debug_doctor MCP tool
│   └── Task 1.7 completado
│
├── Task 3.3 Python adapter
│   └── Task 3.1 completado
│
├── Task 3.4 TypeScript adapter
│   └── Task 3.1 completado
│
├── Task 3.5 Go adapter
│   └── Task 3.1 completado
│
└── Task 3.6 Integration tests
    └── Task 3.1-3.5 completados
```

---

## Definition of Done

### Fase 1 Done
- [ ] DapClient puede conectar a codelldb
- [ ] Puede lanzar un binario Rust
- [ ] Puede poner breakpoints y pararse en ellos
- [ ] Doctor detecta qué adapters están instalados
- [ ] AdapterInstaller puede instalar codelldb desde GitHub

### Fase 2 Done
- [ ] DebugOrchestrator.analyze() retorna DebugAnalysisResult
- [ ] Root cause está en lenguaje natural
- [ ] Recommendation incluye código sugerido
- [ ] Graceful degradation cuando adapter falta

### Fase 3 Done
- [ ] debug_analyze MCP tool expuesta
- [ ] debug_doctor MCP tool expuesta
- [ ] Funciona para Rust, Python, TypeScript, Go
- [ ] Todos los tests de integración pasan

---

## Testing Strategy

### Unit Tests
- DapClient: mock del adapter process
- Doctor: mock de filesystem checks
- AnalysisEngine: mock de crash output

### Integration Tests (requieren adapters reales)
- `test_rust_crash_analysis`: compila test que hace panic, analiza, verifica root cause
- `test_python_crash_analysis`: similar para Python
- `test_graceful_degradation`: cuando debugpy no está instalado, verifica mensaje de error

### Test Environment
```bash
# Instalar adapters para tests
cargo test --features integration_tests

# Los integration tests solo corren si el adapter está
# disponible (usando #[cfg(feature = "integration_tests")])
```

---

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|-------------|---------|------------|
| codelldb no funciona en CI | Media | Alto | Usar mock en tests de CI, integración manual |
| DAP protocol más complejo de lo esperado | Baja | Medio | Empezar con subset (solo crash analysis) |
| Auto-install de adapters falla | Baja | Medio | Fallback a mensaje de instalación manual |
| Multi-lenguaje retrasa | Alta | Bajo | Priorizar Rust primero, otros después |

---

## Milestones

| Semana | Milestone |
|--------|-----------|
| **Semana 1** | DAP Client + Doctor funcionales para Rust |
| **Semana 2** | DebugOrchestrator genera root cause |
| **Semana 3** | MCP tools expuestas, multi-lenguaje |
