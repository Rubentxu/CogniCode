# Spec: truthful-analysis — resultados sin falsos éxitos

**Status:** PROPOSED. **Refs existentes:** `docs/prf/specs/SPEC-ANALYSIS.md`, `docs/prf/specs/SPEC-MCP.md` (PRF-MCP-04/06), `docs/adr/ADR-012-ui-visible-capability-contract.md`; `crates/cognicode-explorer/src/moldql/compile.rs`, `src/domain/views.rs`, `src/api.rs`.

## Requirements

### VDH-TRUTH-01 — Los resultados expresan una suma exhaustiva de estados

**MUST** representar operaciones observables mediante ADT equivalente a `Complete<T,Basis> | Partial<T,Basis,Missing> | Unavailable<Reason> | Failed<Error>`. **MUST NOT** transformar indisponibilidad/error en `Complete([])`/`Complete(0)`. **SHOULD** preservar una base `workspace + revision + config_digest` donde exista soporte real; no fabricar campos desconocidos. **MUST** mapear exhaustivamente cada estado al contrato público, sin perder causa y con compatibilidad versionada.

**Scenario A: resultado vacío verdadero**
- GIVEN un snapshot conocido con cero matches y puerto operativo.
- WHEN se ejecuta la consulta completa.
- THEN se devuelve `Complete([])` con base y cobertura real, no `Unavailable`.

**Scenario B: puerto ausente**
- GIVEN `GraphQueryPort=None` o ejecutor no wireado.
- WHEN se pide `fan_in`, `fan_out`, calidad o graph query.
- THEN el resultado es `Unavailable/Unsupported`, nunca `0` o `[]` con autoridad de dato.

**Scenario C: error al leer calidad**
- GIVEN `QualityStore::issues_for_file` devuelve `Err`.
- WHEN se construye `build_file_quality_view`.
- THEN la vista contiene error/degradación explícita sin etiquetar el archivo `clean`.

### VDH-TRUTH-02 — Ejecución real o rechazo honesto

**MUST** ejecutar `PetgraphPlan` contra el grafo inyectado o fallar tipadamente como `Unsupported/Unavailable`; **MUST NOT** devolver `Ok(total=0)` solo porque el algoritmo es MVP. Plan/target mismatch **MUST** ser error. **MUST** asegurar que `GraphPlan` usa revisión pinneada o rechaza ejecución.

**Scenario D: consulta con símbolo conocido**
- GIVEN un fixture determinista con A→B y una consulta `callees(A)`.
- WHEN el API ejecuta MoldQL.
- THEN devuelve B con identidad estable y `total=1`, o `Unsupported` si la capacidad está deshabilitada; nunca éxito vacío sin ejecutar.

**Scenario E: composición no soportada**
- GIVEN un `BooleanOp` todavía sin ejecutor.
- WHEN se invoca con dos planes válidos.
- THEN responde `Unsupported` y no `Complete([])`.

### VDH-TRUTH-03 — Taxonomías abiertas no se convierten en identidades incorrectas

**MUST** conservar `UnknownKind(raw)` / `unknown` para un tipo de nodo nuevo; **SHOULD** dar estilo visual neutral sin afirmar `function`.

**Scenario F:** GIVEN `kind="some-future-node"`; WHEN se construye GraphNode; THEN su clase visual no es `function` y el kind crudo sigue inspeccionable.

## UAT/E2E/AAT del ciclo

- `B1-UAT-01` errores de lectura vs vacío real.
- `B1-E2E-01` resultados reales desde API/CLI/MCP sobre fixture A→B.
- `B1-AAT-01` inyección de `Err` en QualityStore y puerto ausente.

**Regresión DURADERA de reemplazo:** leyes de `map`/preservación, shape público, ejecución de plan y negativas de read error; las pruebas con prefijo de ciclo deben desaparecer tras seal/guard. Cobertura fallida o no ejecutada bloquea release.
