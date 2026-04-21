# CogniCode: Plan de Mejora Paso a Paso

> **Fecha**: Marzo 2026
> **Estado**: Borrador
> **Alcance**: De esqueleto vacío a MVP funcional

## Resumen Ejecutivo

CogniCode tiene una arquitectura bien concebida (DDD + Clean Architecture en 4 capas) pero el código es mayoritariamente scaffolding vacío. Este plan prioriza convertir el esqueleto en un producto funcional siguiendo el enfoque **Walking Skeleton**: implementar un solo flujo end-to-end completo antes de expandir horizontalmente.

**Métricas actuales**: ~8,200 líneas | 3 servicios vacíos | 9 handlers MCP que retornan `Vec::new()` | tests no compilan

**Métricas objetivo por fase**: Ver cada fase abajo.

---

## Fase 0: Estabilización (Días 1-2)

> Prioridad: **P0 — Sin esto no hay CI ni tests**

### 0.1 Arreglar tests que no compilan

**Problema**: `RiskThreshold` define constantes como campos (`pub const NONE: RiskThreshold = ...`) pero los tests las usan como associated constants (`RiskThreshold::Medium`). Los const fields de un struct no son accesibles como associated items.

**Archivos afectados**:
- `src/infrastructure/safety/mod.rs` (tests en el mismo archivo)

**Acción**:
1. Revisar los tests de `safety/mod.rs` que referencian `RiskThreshold::Low`, `RiskThreshold::Medium`, `RiskThreshold::High`
2. Reemplazar por las constantes correctas (`RiskThreshold::NONE`, `RiskThreshold::LOW`, etc.)
3. Verificar que `cargo test` pasa sin errores

**Criterio de aceptación**: `cargo test` ejecuta todos los tests sin errores de compilación.

### 0.2 Eliminar el trait `DependencyRepository` duplicado

**Problema**: Existen dos traits con el mismo nombre en archivos distintos:

| Archivo | Firma clave | Usa |
|---------|-------------|-----|
| `domain/traits/dependency_graph.rs` | `fn add_dependency(&mut self, source: Symbol, target: Symbol, kind: DependencyType)` | `Symbol` directo |
| `domain/traits/dependency_repository.rs` | `fn add_dependency(&mut self, source_id: &SymbolId, target_id: &SymbolId, ...)` | `SymbolId` |

`PetGraphStore` implementa el de `dependency_repository.rs`. El de `dependency_graph.rs` no se usa en ningún lado.

**Acción**:
1. Eliminar `domain/traits/dependency_graph.rs`
2. Actualizar `domain/traits/mod.rs` para no exportar el trait eliminado
3. Verificar que no hay imports rotos con `cargo check`

**Criterio de aceptación**: `cargo check` pasa sin warnings sobre el trait eliminado.

### 0.3 Unificar tipos MCP duplicados

**Problema**: Dos archivos definen los mismos tipos independientemente:

| Tipo | En `server.rs` | En `schemas.rs` |
|------|---------------|-----------------|
| `McpError` | `struct { code, message }` | `struct { code, message, data }` |
| `McpRequest` | `struct { jsonrpc, method, params, id: Option<Value> }` | `struct { jsonrpc, method, params, id: Option<u64> }` |
| `McpResponse` | `struct { jsonrpc, result, error, id }` | `struct { jsonrpc, result, error, id }` |

**Acción**:
1. Definir los tipos de protocolo MCP (`McpError`, `McpRequest`, `McpResponse`) en **un solo lugar**: `interface/mcp/schemas.rs` (ya tiene el `data` field que es más completo)
2. Eliminar los tipos duplicados de `interface/mcp/server.rs`
3. Importar desde `schemas.rs` en `server.rs`
4. Verificar `cargo check`

**Criterio de aceptación**: Un solo set de tipos MCP. `cargo check` pasa.

### 0.4 Limpiar warnings del compilador

**Problema**: `cargo check` produce 22 warnings (dead code, async traits sin `Send` bound).

**Acción**:
1. Ejecutar `cargo fix --lib -p cognicode --allow-dirty` para auto-corregir los que se puedan
2. Añadir `#[allow(dead_code)]` donde sea temporal (servicios vacíos)
3. Corregir los traits async para usar `#[async_trait]` consistente o return-position `impl Trait` con `+ Send`

**Criterio de aceptación**: `cargo check` con 0 warnings o solo warnings justificados de dead code pendiente de implementar.

**Checklist de salida de Fase 0**:
- [ ] `cargo test` pasa
- [ ] `cargo check` sin errores y warnings mínimos
- [ ] No hay traits duplicados
- [ ] No hay tipos MCP duplicados
- [ ] CI verde (si existe)

---

## Fase 1: Walking Skeleton — `get_file_symbols` End-to-End (Días 3-7)

> Prioridad: **P1 — Demuestra que la arquitectura funciona**

### 1.1 Objetivo del Walking Skeleton

Implementar **un solo flujo completo** desde que un agente IA llama a la herramienta MCP `get_file_symbols` hasta que recibe símbolos reales parseados con tree-sitter:

```
Agente IA → MCP JSON-RPC → Handler → AnalysisService → TreeSitterParser → Symbol[] → JSON Response
```

### 1.2 Refinar `TreeSitterParser` para producción

**Estado actual**: Parsea Python, Rust y JS básico. La ubicación de los símbolos es siempre `"source"` en vez del archivo real.

**Acción**:
1. Añadir parámetro `file_path` al método `find_all_symbols()` (o al trait `Parser`)
2. Corregir `node_to_symbol()` para que use el `file_path` real en `Location::new(file_path, ...)`
3. Añadir soporte para **TypeScript** con `tree-sitter-typescript` (añadir al Cargo.toml)
4. Añadir método para detectar `impl` blocks en Rust (métodos asociados)

**Criterio de aceptación**: Tests unitarios que verifiquen que los símbolos tienen el file_path correcto.

### 1.3 Implementar `AnalysisService.get_file_symbols()`

**Estado actual**: Vacío (33 líneas, solo tiene `check_cycles` placeholder).

**Acción**:
1. Inyectar `TreeSitterParser` en `AnalysisService` via constructor
2. Implementar `get_file_symbols(file_path: &str, language: Language) -> Vec<SymbolDto>`
3. El servicio debe:
   - Leer el archivo del sistema de archivos
   - Detectar el lenguaje por extensión (`.py` → Python, `.rs` → Rust, etc.)
   - Parsear con `TreeSitterParser`
   - Convertir `Symbol` → `SymbolDto`
4. Escribir tests unitarios con archivos de ejemplo

**Firma objetivo**:
```rust
impl AnalysisService {
    pub fn get_file_symbols(&self, path: &Path) -> AppResult<Vec<SymbolDto>> {
        let source = std::fs::read_to_string(path)?;
        let language = Language::from_extension(path.extension())?;
        let symbols = self.parser.find_all_symbols(&source)?;
        Ok(symbols.iter().map(SymbolDto::from_symbol).collect())
    }
}
```

**Criterio de aceptación**: Tests que parseen un archivo Python real y devuelvan símbolos con nombres, tipos y ubicaciones correctas.

### 1.4 Conectar el handler MCP `handle_get_file_symbols`

**Estado actual**: Valida el path pero retorna `Vec::new()`.

**Acción**:
1. Añadir `AnalysisService` a `HandlerContext` (via `Arc`)
2. En `handle_get_file_symbols`:
   - Validar el path con `InputValidator`
   - Leer el archivo
   - Llamar a `AnalysisService::get_file_symbols()`
   - Convertir `SymbolDto` → `SymbolInfo` (tipo del schema MCP)
3. Escribir test de integración

**Criterio de aceptación**: Si apuntas el MCP a un archivo `.py` real, devuelve símbolos reales con nombres y ubicaciones.

### 1.5 Implementar lectura del MCP por stdin/stdout

**Estado actual**: `run_server()` solo espera Ctrl+C.

**Acción**:
1. Implementar loop de lectura JSON-RPC desde stdin (línea por línea)
2. Despachar requests al handler correspondiente por `method`
3. Serializar response a JSON y escribir a stdout
4. Implementar `initialize` y `tools/list` del protocolo MCP
5. Registrar `get_file_symbols` como herramienta disponible

**Criterio de aceptación**: El binario `cognicode-mcp` puede:
- Recibir un JSON-RPC `tools/list` por stdin y responder con la lista de herramientas
- Recibir un `tools/call` con `get_file_symbols` y responder con símbolos reales

**Checklist de salida de Fase 1**:
- [ ] `get_file_symbols` funciona end-to-end con archivos reales
- [ ] El servidor MCP lee stdin y escribe stdout correctamente
- [ ] `tools/list` retorna las herramientas registradas
- [ ] Tests unitarios del parser con file_path correcto
- [ ] Tests de integración del handler MCP

---

## Fase 2: El Grafo Real — Call Graph + Impact Analysis (Días 8-14)

> Prioridad: **P2 — Lo que el agente IA no puede hacer solo**

### 2.1 Resolver la duplicación CallGraph vs PetGraphStore

**Problema**: El dominio tiene `CallGraph` (HashMap puro) y la infraestructura tiene `PetGraphStore` (petgraph). Son dos grafos paralelos. `PetGraphStore.get_call_graph()` tiene `unimplemented!()`.

**Decisión de diseño**: Mantener `CallGraph` como el modelo de dominio rico (con BFS, path finding, roots/leaves). Hacer que `PetGraphStore` sea la implementación eficiente del almacenamiento, pero que **construya** un `CallGraph` de dominio cuando se necesite (patrón Repository).

**Acción**:
1. `PetGraphStore` almacena nodos y aristas eficientemente con petgraph
2. Añadir método `PetGraphStore::to_call_graph(&self) -> CallGraph` que construya el `CallGraph` de dominio
3. Eliminar `get_call_graph()` del trait `DependencyRepository` (el trait devuelve datos primitivos, no el aggregate completo)
4. Tests que verifiquen la conversión bidireccional

**Criterio de aceptación**: Puedo añadir símbolos y dependencias al `PetGraphStore`, convertir a `CallGraph`, y ejecutar `find_path()` exitosamente.

### 2.2 Implementar el scáner de relaciones entre símbolos

**Estado actual**: `TreeSitterParser` solo extrae definiciones. No extrae llamadas entre funciones.

**Acción**:
1. Añadir método `find_call_relationships(&self, source: &str) -> Vec<(Symbol, Symbol)>` a `TreeSitterParser`
2. Para cada función definida, encontrar las llamadas a función dentro de su cuerpo
3. Emitir pares `(caller, callee)` que alimenten el grafo
4. Implementar queries tree-sitter específicas por lenguaje:
   - Python: `(call function: (identifier) @callee)` dentro de `(function_definition)`
   - Rust: `(call_expression function: (identifier) @callee)` dentro de `(function_item)`
5. Tests con código real multi-función

**Criterio de aceptación**: Dado un archivo con `fn a() { b(); c(); }` y `fn b() { c(); }`, el scáner detecta las aristas `a→b`, `a→c`, `b→c`.

### 2.3 Implementar `AnalysisService.build_project_graph()`

**Acción**:
1. Recorrer un directorio recursivamente
2. Por cada archivo, parsear con `TreeSitterParser`
3. Extraer símbolos y relaciones
4. Alimentar el `PetGraphStore`
5. Almacenar el grafo en `GraphCache` (thread-safe)

**Firma objetivo**:
```rust
impl AnalysisService {
    pub fn build_project_graph(&self, project_dir: &Path) -> AppResult<()> {
        for file in walk_rust_files(project_dir) {
            let source = std::fs::read_to_string(&file)?;
            let symbols = self.parser.find_all_symbols(&source)?;
            let relations = self.parser.find_call_relationships(&source)?;
            // Alimentar el grafo
        }
        Ok(())
    }
}
```

### 2.4 Implementar handlers MCP con datos reales

**Acción**: Conectar los siguientes handlers al grafo real:

| Handler | Qué debe hacer |
|---------|----------------|
| `handle_get_call_hierarchy` | BFS/DFS en el grafo por `symbol_name` |
| `handle_analyze_impact` | `find_all_dependents()` + calcular risk level |
| `handle_check_architecture` | `detect_cycles()` via Tarjan SCC |

**Criterio de aceptación**: Si apuntas el MCP al propio código fuente de CogniCode, `analyze_impact` puede decir qué funciones dependen de `Symbol::new()`.

**Checklist de salida de Fase 2**:
- [ ] `PetGraphStore.to_call_graph()` funciona
- [ ] Scáner de relaciones detecta calls entre funciones
- [ ] `build_project_graph()` indexa un directorio completo
- [ ] `get_call_hierarchy` devuelve datos reales del grafo
- [ ] `analyze_impact` calcula impacto real
- [ ] `check_architecture` detecta ciclos reales

---

## Fase 3: Refactorización Real — Rename End-to-End (Días 15-21)

> Prioridad: **P2 — Valor premium tipo IntelliJ**

### 3.1 Implementar `RenameStrategy` (Strategy Pattern)

**Acción**:
1. Crear `infrastructure/refactor/rename_strategy.rs`
2. Implementar el trait `RefactorStrategy` para rename:
   - `validate()`: Verificar que el símbolo existe, calcular impacto, verificar que el nuevo nombre no colisiona
   - `prepare_edits()`: Generar `TextEdit` para cada ocurrencia del símbolo
3. Usar tree-sitter para encontrar TODAS las referencias (no solo definición)
4. Generar edits que reemplacen en todas las ubicaciones

**Criterio de aceptación**: Dado un archivo con `fn foo() { foo(); }` y rename a `bar`, genera 2 TextEdits (definición + llamada).

### 3.2 Implementar `RefactorService` real

**Acción**:
1. Inyectar `PetGraphStore`, `TreeSitterParser`, `SafetyGate`
2. Implementar `rename_symbol(command: RenameSymbolCommand) -> RefactorPreviewDto`
3. El servicio debe:
   - Validar con `SafetyGate`
   - Calcular impacto con el grafo
   - Preparar edits con la estrategia
   - Retornar preview para aprobación del agente

### 3.3 Conectar handler `handle_safe_refactor` para rename

**Acción**:
1. Conectar el handler MCP al `RefactorService`
2. Para `RefactorAction::Rename`, ejecutar el rename real
3. Retornar los `ChangeEntry` con los diffs reales

**Criterio de aceptación**: El agente IA puede llamar a `safe_refactor` con `action: "rename"`, `target: "foo"`, `params: { new_name: "bar" }` y recibir los cambios reales.

**Checklist de salida de Fase 3**:
- [ ] `RenameStrategy` implementada y testeada
- [ ] `RefactorService.rename_symbol()` funciona
- [ ] `safe_refactor` MCP handler devuelve diffs reales
- [ ] SafetyGate valida antes de ejecutar
- [ ] Preview mode funciona (sin aplicar cambios)

---

## Fase 4: Madurez — Calidad y Robustez (Días 22-28)

> Prioridad: **P3 — Producto publicable**

### 4.1 Actualizar tree-sitter a 0.24+

**Razón**: La versión 0.20 está obsoleta. La API cambió significativamente en 0.22+.

**Acción**:
1. Actualizar `Cargo.toml`: `tree-sitter = "0.24"`, `tree-sitter-python = "0.23"`, etc.
2. Migrar cambios de API:
   - `Parser::set_language()` ahora usa `LANGUAGE` constantes
   - `Language` es un tipo opaque diferente
   - `Node::kind()` sin cambios significativos
3. Ejecutar tests y corregir
4. Añadir `tree-sitter-typescript` como dependencia separada

**Criterio de aceptación**: `cargo test` pasa con tree-sitter actualizado.

### 4.2 Añadir VirtualFileSystem al flujo de refactorización

**Estado actual**: `VirtualFileSystem` existe pero no se usa.

**Acción**:
1. Antes de aplicar refactor edits, clonar archivos al VFS
2. Aplicar edits en el VFS
3. Parsear los archivos modificados con tree-sitter para validar sintaxis
4. Si hay errores de parseo, rechazar el refactor
5. Si todo está bien, retornar los edits para que el agente/confirme

**Criterio de aceptación**: Un refactor que rompe la sintaxis es detectado y rechazado.

### 4.3 Añadir tests de integración end-to-end

**Acción**:
1. Crear `tests/e2e/` con fixtures de código en múltiples lenguajes
2. Test: parsear archivo Python → verificar símbolos
3. Test: construir grafo → verificar relaciones
4. Test: rename → verificar edits generados
5. Test: flujo MCP completo (stdin/stdout simulado)

### 4.4 Implementar `find_usages` real

**Acción**:
1. Usar tree-sitter queries para encontrar todas las referencias a un símbolo
2. Distinguir entre definición, lectura, escritura, llamada
3. Conectar al handler MCP

### 4.5 Implementar `get_complexity` real

**Acción**:
1. `ComplexityCalculator` ya existe en el dominio — conectarlo
2. Calcular complejidad ciclomática contando puntos de decisión (if, match, loop, &&, ||)
3. Calcular profundidad de anidamiento
4. Conectar al handler MCP

**Checklist de salida de Fase 4**:
- [ ] tree-sitter actualizado a 0.24+
- [ ] VFS valida sintaxis post-refactor
- [ ] Tests e2e pasan para Python, Rust y JS
- [ ] `find_usages` devuelve resultados reales
- [ ] `get_complexity` calcula métricas reales

---

## Fase 5: Diferenciación — Lo que IntelliJ no hace para IA (Días 29+)

> Prioridad: **P3 — Ventaja competitiva**

### 5.1 Context Compression

**Concepto**: Transformar respuestas técnicas en contexto comprimido para el agente IA.

**Ejemplo**:
```
En vez de:
  {"symbols": [{"name": "process_order", "kind": "Function", "line": 42, ...}, ...]}

Comprimir a:
  "order_service.rs: 4 funciones (process_order, validate, compute_total, save). 
   process_order calls validate + compute_total. No external deps."
```

**Acción**:
1. Añadir capa de compresión en el handler MCP
2. Añadir flag `compressed: bool` al input de cada herramienta
3. Si `compressed=true`, generar resumen en lenguaje natural

### 5.2 Incremental Graph Updates

**Concepto**: En lugar de rebuild-every-time, actualizar el grafo incrementalmente cuando cambian archivos.

**Acción**:
1. Definir `GraphEvent` enum en el dominio
2. Cuando un archivo cambia: calcular diff de símbolos, emitir eventos
3. Aplicar eventos al grafo existente
4. Mantener el grafo caliente entre sesiones del agente

### 5.3 LSP Proxy Mode

**Concepto**: Conectarse a LSPs existentes (rust-analyzer, pyright) y añadir inteligencia por encima.

**Acción**:
1. `LspClient` ya existe — conectarlo de verdad
2. Lanzar LSP externo como subproceso
3. Delegar operaciones básicas (hover, completion) al LSP externo
4. Añadir operaciones premium (impact, cycles, complexity) con CogniCode

---

## Diagrama de Dependencias entre Fases

```
Fase 0: Estabilización
    │
    ▼
Fase 1: Walking Skeleton (get_file_symbols)
    │
    ├──▶ Fase 2: Grafo Real (call hierarchy, impact)
    │       │
    │       ├──▶ Fase 3: Refactorización Real (rename)
    │       │       │
    │       │       └──▶ Fase 4: Madurez (tests e2e, VFS, tree-sitter upgrade)
    │       │               │
    │       │               └──▶ Fase 5: Diferenciación (compression, incremental, LSP proxy)
    │       │
    │       └──▶ (Paralelo) Fase 4
    │
    └──▶ (Solo después de Fase 1) Cualquier otra herramienta MCP
```

---

## Notas de Implementación

### Principio guía: "Un solo flujo completo vale más que 10 esqueletos vacíos"

Antes de añadir CUALQUIER nueva funcionalidad, pregúntate:
1. ¿Ya tengo un flujo end-to-end funcionando?
2. ¿Este cambio conecta dos capas que aún no están conectadas?
3. ¿O es solo más scaffolding?

### Testing strategy

| Capa | Cobertura objetivo | Tipo de test |
|------|-------------------|--------------|
| Domain | 95%+ | Unit tests puros (sin IO, sin dependencias) |
| Application | 80%+ | Unit tests con mocks de traits |
| Infrastructure | 70%+ | Integration tests con archivos reales |
| Interface | 60%+ | E2E tests simulando stdin/stdout |

### Métricas de progreso

| Métrica | Fase 0 | Fase 1 | Fase 2 | Fase 3 | Fase 4 |
|---------|--------|--------|--------|--------|--------|
| Handlers MCP funcionales | 0/9 | 1/9 | 4/9 | 5/9 | 8/9 |
| Servicios con lógica real | 0/4 | 1/4 | 2/4 | 3/4 | 4/4 |
| Tests pasando | ~30 | ~45 | ~65 | ~80 | ~120+ |
| Líneas de código real (no scaffolding) | ~500 | ~1500 | ~3000 | ~4500 | ~6000 |

### Reglas de oro para el código

1. **Nunca añadir un servicio vacío**. Si no puedes implementar al menos un método con lógica real, no lo añadas.
2. **Nunca añadir un handler que retorna `Vec::new()`**. Si no tienes datos reales, conecta un test fixture.
3. **Cada PR debe conectar al menos dos capas**. PRs que solo añaden tipos sin conectar nada se rechazan.
4. **Los tests son first-class citizens**. Si un cambio no tiene test, no se mergea.
