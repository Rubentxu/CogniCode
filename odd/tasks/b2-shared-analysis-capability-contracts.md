# B2 — Shared Analysis & Capability Contracts (v0.101.12)

> **Estado**: PENDING. Desbloqueado por B1 (gate publicable).
> Detalle espeficado: [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](../docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md#bloque-b2--shared-analysis--capability-contracts).

## Valor

CLI y MCP dejan de ser dos caminos potencialmente distintos para interpretar el mismo código. Un resultado conserva identidad, cobertura, procedencia, precisión y completitud independientemente del adaptador que lo solicite.

## Prerrequisitos

- B1 cerrado: gate publicable verde, `v0.101.11` released.
- T1-5.5 cerrado (✓): Index/Graph ya pasan por `presenter::present_output`.

## Work units

- [x] **B2.W1** — Modelo común de ejecución. Contrato de aplicación agnóstico de CLI/MCP. Distingue: workspace identity + AnalysisBasis, operación + parámetros normalizados, perfil/capabilities del consumidor, budget + cancelación, resultado tipado, estado `Complete|Partial|Unsupported|Unknown|Failed`, procedencia + límites + errores, relación con snapshot/grafo. Reutiliza `WorkspaceSession`, `AnalysisService`, Evidence Kernel, DTOs.
- [ ] **B2.W2** — Descubrimiento coherente. `cognicode capabilities --json` y MCP `tools/list` derivan de misma autoridad. 73 tools preservadas. Cada tool declara parámetros, permisos, estado, lenguaje, precisión, límites, requisitos de backend, schema.
- [ ] **B2.W3** — Respuestas estructuradas y compatibilidad. Empezar por las 15 tools con schema. Schemas derivados de DTOs reales. Compatibilidad explícita cuando se cambia shape. NO `Partial → []` ni `Unsupported → false`.
- [ ] **B2.W4** — Evidence Export reparado. Independencia del orden de capabilities, extracción única de entidades/fuentes, filtros e inventario reutilizados, límites simétricos producer/consumer, identidad efectiva del worktree, cobertura incompleta honesta, referencias consistentes.
- [ ] **B2.W5 / T1-5.6** — Reducir acoplamiento de CLI. Replicar patrón T1-5.5 en `execute_navigate` (~25 println!), `execute_analyze` (~20), y los otros 8 `execute_*` con `println!` inline (~117 restantes). Multi-día.

## Acceptance criteria (resumen)

- CLI y MCP producen equivalencia semántica sobre el mismo símbolo.
- `assurance-evidence/v1` envelope validado por `pipelinek-assurance` consumer real (no solo golden Rust).
- ~117 `println!` restantes movidos a presenters en `cognicode_cli::presenter`.
- Golden tests verdes (text byte-identical, JSON parseable).
- `bash scripts/test-full.sh` exit 0.

## Out of scope

- Reescribir el catálogo de tools (B3).
- Cambios en la lógica de análisis (B3+).

## Riesgos

- **R-B2-W1**: diseñar un "contrato de aplicación" sin Repository añade riesgo de crear un service locator global (prohibido por AGENTS.md y el goal doc). El contrato debe ser UNA función/trait, no una indirección.
- **R-B2-W5**: 117 `println!` en 11 funciones → multi-día. Si se hace en una sola sesión se pierde locality y se aumenta el riesgo de regresión. Recomiendo dividir en T1-5.6a, 6b, 6c (uno por execute_*).

## Recibo (append-only)

| Fecha | WU | SHA | Notas |
|---|---|---|---|
| 2026-10-11 | B2.W1 | _pendiente_ | Ver detalle abajo. |
| _pendiente_ | B2.W2 | _pendiente_ | _pendiente_ |
| _pendiente_ | B2.W3 | _pendiente_ | _pendiente_ |
| _pendiente_ | B2.W4 | _pendiente_ | _pendiente_ |
| _pendiente_ | B2.W5 | _pendiente_ | _pendiente_ |

### B2.W1 — superficie del contrato

  - **`crates/cognicode-core/src/application/analysis_basis.rs`**
    (nuevo, ~280 lineas). Modulo del contrato.
    Tipos publicos:
      - `AnalysisBasis` (struct): workspace root canonicalizado,
        `ConsumerProfile` (Human/Agent), `Budget` (max_elapsed,
        max_entities).
      - `OutcomeState` (enum): `Complete|Partial|Unknown|Unsupported|Failed`.
        Reusa el vocabulario de `CapabilityCompleteness` y le
        anade `Failed` (que el envelope assurance-evidence/v1
        no necesita porque un envelope con Failed no se serializa).
      - `AnalysisOutcome<T>` (struct): `state + value + gaps + reason + error + provenance`.
        Constructores pinean invariantes: `partial()` panic sin
        gaps, `unsupported()` panic sin reason, `failed()` panic
        sin error. Mismo patron que `CapabilityCompleteness`.
      - `AnalysisError` (struct) + `AnalysisErrorKind` (enum):
        `Gap|Unsupported|Io|Cancelled|Generic`. Serializa con
        `message` + `capability` opcional.
      - `AnalysisProvenance` (struct): workspace + operation +
        capability + elapsed_ms. Serializable con campos opcionales.
      - `AnalysisOp` (enum): 19 variantes (Index* x4, Graph* x10,
        Navigate* x3, FindUsages, Other). `operation_id()` pinea
        el discriminante para que `AnaylyzeProvenance.operation`
        no se desvie.
      - `AnalysisValue` (enum): `Index(IndexValue) | Graph(GraphValue) | Text{summary}`.
        Cada sub-variant pinea el shape del JSON final.
      - `Analyze` (trait, UN metodo `execute`): el contrato.
        B2.W2+ lo implementa; B2.W1 lo deja declarado.
    Normalizacion: `AnalysisBasis::for_workspace` aplica
    `normalize_path` (resuelve `.`/`..` por componente sin
    tocar disco), absolutize, y `canonicalize` solo si el
    path existe. Asi un workspace que aun no tiene archivos
    (e.g. `index build` sobre un dir vacio) queda normalizado
    y no falla.

  - **`crates/cognicode-core/tests/b2_w1_analysis_outcome_contract.rs`**
    (nuevo, 12 tests). Pinean:
      - `OutcomeState` discrimina correctamente (5 variantes).
      - `Complete` / `Partial` / `Unknown` / `Unsupported` /
        `Failed` cada uno con sus invariantes.
      - `Partial` rechaza gaps vacios (panic).
      - `Unsupported` rechaza reason blanco (panic).
      - `Failed` rechaza error blanco (panic).
      - `AnalysisBasis::for_workspace` normaliza `..` y deja el
        path absoluto.
      - `AnalysisProvenance` lleva workspace + operation +
        capability + elapsed_ms.
      - JSON shape estable: `state` como discriminante, `value`
        solo cuando hay dato, `gaps[].capability` + `message`
        en Partial.

  - **Sin cambios en CLI/MCP todavia**. B2.W1 deja la
    superficie del contrato; B2.W2+ la conecta a las
    operaciones reales (B2.W2 hace `tools/list` derivar
    del contrato; B2.W3 lo enchufa en los handlers; B2.W4
    repara el evidence export; B2.W5/W6 sigue con el
    connascence de los `execute_*` que quedan).

  - **Riesgo R-B2-W1 (service locator) mitigado por diseno**:
    el trait `Analyze` es uno con un metodo, y el caller
    inyecta el `&dyn Analyze` que quiere. No hay un singleton
    global, no hay un registry, no hay un builder. El impl
    concreto se construye en el composition root del adapter
    (CLI o MCP).

## Cross-block signals

B2.W1 + B2.W2 son prerrequisito de B3 (agent code understanding usa el modelo común y catálogo coherente). B2.W5 cierra el Tier-1 connascence de T1-5.