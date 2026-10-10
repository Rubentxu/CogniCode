# Goal — T1-5 Mover 194 `println!` de `cognicode-core/src/interface/cli/commands.rs` a `cognicode-cli`

> **Estado**: PENDING (multi-día, riesgo de output breakage). Autorizado
> en el goal `v1015-consolidation-and-promote` como T1-5, el último item
> abierto de la auditoría de connascence de Tier 1.

## 0. Cómo se relaciona con el resto

| Artefacto | Rol |
|---|---|
| `odd/tasks/v1015-consolidation-and-promote.md` §T1-5 | origen: la auditoría senior identificó 194 `println!` en el presenter del CLI viviendo en la **librería**, no en el adapter |
| `crates/cognicode-core/src/interface/cli/commands.rs` | **target del move** — los 194 `println!` salen de aquí |
| `crates/cognicode-cli/` | **destino** — el binario CLI absorbe el presenter |
| `AGENTS.md §17` | "logs nunca en stdout" — la violación REAL del audit (no el conteo de 154 que la auditoría senior dijo; el conteo real es 194) |
| `crates/cognicode-mcp/src/mcp_client.rs:140` | la **única violación real** de AGENTS.md §17 (log en stdout en código MCP server, no CLI) — T1-5 NO toca esto directamente pero es contexto |

## 1. Objetivo

Mover los 194 `println!` que viven en `cognicode-core::interface::cli::commands.rs`
al binario `cognicode-cli` (en `crates/cognicode-cli/src/`), de forma que la
**librería** no tenga presenter output y el **binario** sea dueño del output
del CLI. Después del move, `cognicode-core` no debe tener ningún `println!`
en `commands.rs` (solo en errores que propagan por `Result` o `tracing`).

## 2. Problema

La separación library/adapter que pide arquitectura hexagonal está rota:
`cognicode-core` es la librería de dominio + aplicación, y no debería tener
output de presenter. Pero `commands.rs` (en `interface/cli/`, parte de
`cognicode-core`) tiene 194 `println!` que son el output del binario
`cognicode`. Esto significa que cualquier consumidor de la librería
(otro binario, un test, una integración) recibe output de presenter
mezclado con su propio output, y no se puede cambiar el formato del CLI
sin tocar la librería.

Riesgo concreto: `cargo test -p cognicode-core` produce 194 líneas de
output inesperado en stdout de los tests que ejercitan commands.rs.

## 3. Por qué ahora

Es el último item abierto de la auditoría senior de Tier 1 (connascence).
Cerrarlo deja el dominio (cognicode-core) sin presenter output, que es
la condición que la arquitectura hexagonal pide.

## 4. Scope

### 4.1 In-scope

* Sustituir los 194 `println!` en `commands.rs` por:
  - **Campos de retorno** en structs de resultado (preferred para output
    estructurado que tests pueden pinear)
  - **Eventos `tracing`** (preferred para output no-estructurado; el
    binario decide si escribir a stderr, a un fichero, o silenciar)
  - **Errores propagados por `Result`** (cuando el output era parte de
    un path de error)
* En `cognicode-cli`, absorber el presenter: donde antes el binario
  llamaba `core::command::run(args)`, ahora llama algo como
  `core::command::run(args).map(|result| print_to_stdout(result))` o
  equivalente.
* `cargo test -p cognicode-core` ya no debe tener output espurio de los
  tests que ejercitan `commands.rs`.
* Snapshot test del output del bin `cognicode` (binary smoke): el output
  antes y después del move debe ser **idéntico byte-a-byte** para los
  flujos principales.

### 4.2 Out-of-scope

* El `println!` de `cognicode-mcp/src/mcp_client.rs:140` (única violación
  real de AGENTS.md §17). T1-5 no lo toca; es un fix distinto, ~1 línea.
* Cambiar el formato del CLI. El output observable debe quedar igual.
* Cualquier cambio al wire format MCP. Esos tools usan MCP, no CLI.

## 5. Constraints

* **Sin cambio de output observable**: `cognicode --version`, `cognicode
  doctor`, `cognicode analyze <path>`, etc. deben producir el mismo
  output byte-a-byte que antes.
* TDD: snapshot test del output del binario antes de empezar (binario
  actual con los `println!`); RED si cambia después del move.
* Sin nuevos `unimplemented!()`.
* Sin nuevos `println!` en `cognicode-core` durante el move. Si se
  necesita output, va por `tracing` o por el struct de retorno.

## 6. Work items (propuesta inicial, refinar al empezar)

| ID | Tarea | Estimación |
|----|-------|------------|
| **T1-5.1** | Snapshot test del output actual del binario `cognicode` para los flujos principales (`--version`, `doctor`, `analyze <fixture>`, `find-usages`, etc.) | 1 día |
| **T1-5.2** | Clasificar los 194 `println!` en categorías: structured-result vs tracing-event vs error | 1 día |
| **T1-5.3** | Crear struct de resultado o evento `tracing` para cada categoría | 1-2 días |
| **T1-5.4** | Migrar los `println!` a los nuevos mecanismos en `commands.rs` | 2-3 días |
| **T1-5.5** | En `cognicode-cli`, añadir el presenter que materializa los resultados/eventos a stdout | 1 día |
| **T1-5.6** | Verificación: snapshot test verde, `cargo test --workspace` verde, `cognicode --version` etc. output idéntico | 1 día |

Estimación total: **7-10 días persona**. Substancial.

## 7. Acceptance criteria

1. T1-5.1..6 en DONE.
2. `cargo test -p cognicode-core` no produce output espurio en stdout
   para los tests que ejercitan `commands.rs`.
3. `cognicode --version`, `cognicode doctor`, etc. producen el mismo
   output byte-a-byte que antes (snapshot test).
4. `cargo test --workspace` exit 0 sin regresión.
5. `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
6. `odd/tasks/v1015-consolidation-and-promote.md` T1-5 row actualizado
   con recibos y commit.

## 8. Risks

* **R-T1-5** (ya documentado en v1015 §8): mover los `println!` puede
  romper el output del bin si los `CommandExecutor::execute` no se han
  diseñado para ser output-agnostic. Mitigación: snapshot test del
  output (T1-5.1) ANTES de migrar.
* **R-1**: tests de `cognicode-core` que asumían que `commands.rs`
  imprimía a stdout pueden romperse al cambiar el flujo. Mitigación:
  actualizar los tests al nuevo modelo (result struct / tracing), no
  reintroducir `println!`.
* **R-2**: si el presenter de `cognicode-cli` diverge del formato
  actual, el snapshot test falla. Mitigación: pin el snapshot al
  inicio, refactor solo cuando el snapshot lo permite.

## 9. Recibo (append-only)

| Fecha | WU | Commit | Notas |
|-------|----|--------|-------|
| 2026-10-10 | T1-5.1 | `0182595` | Snapshot test `crates/cognicode-cli/tests/t1_5_snapshot.rs` (4/4 verde). Captura el output actual de `cogh --version` (exact byte match: `"cogh {CARGO_PKG_VERSION}"`), `cogh current` (structural: single line, contains digits), `cogh doctor` (structural: `==> cogh doctor` header, `PASS`/`FAIL`/`UNAVAILABLE` subsystem lines, `==> overall:` final), y `cogh --help` (structural: `Usage:` + `Commands:`). El test usa `env!("CARGO_BIN_EXE_cogh")` para localizar el binario que cargo construye por test. Cuando T1-5.3..6 migre los `println!` de `cognicode-core::interface::cli::commands.rs` al presenter de `cognicode-cli`, este test RED si el output observable cambia (incluso un byte), forzando una decisión explícita sobre cada cambio. |
| 2026-10-10 | T1-5.2 | (en `odd/tasks/t1-5-2-classification.md`) | Clasificación completa: 194 calls = ~150 structured-result + ~21 tracing-event (14 progress + 7 warning) + ~16 error + ~4 decoration + ~3 usage-hint. Top emitters: `execute_graph` (~50), `execute_index` (~30), `execute_navigate` (~25), `execute_analyze` (~20). Recomendación: T1-5.3a..d ejecuta el patrón sobre 1 subcomando representativo (`execute_index`) primero, replica después. Análisis en `odd/tasks/t1-5-2-classification.md`. |
| 2026-10-10 | T1-5.3a | (en `odd/tasks/t1-5-3a-index-design.md`) | Design completo del patrón sobre `execute_index`. `IndexResult` enum (5 variantes con `#[serde(tag = "kind")]`), `IndexError` enum (4 variantes con `#[source]`), `present()` para text + JSON, tracing scaffolding con `EnvFilter` + stderr writer, snapshot test design con 2 tests (text exact + JSON parseable). El doc incluye el before/after del código de `execute_index`. Implementación es T1-5.3b..d; este commit es solo la especificación. |
| _pendiente_ | T1-5.3b..d | _pendiente_ | implementar el patrón sobre `execute_index`: `IndexResult` + `IndexError` + `present()` + tracing init + snapshot test. Bounded ~2 días. |
| _pendiente_ | T1-5.4..6 | _pendiente_ | replicar el patrón para `execute_graph`, `execute_navigate`, `execute_analyze`, etc. Multi-día. Fuera de scope de una sesión. |
