# Production-Ready Foundation (PRF) — STATE

> **Fuente de verdad**: este archivo es el puntero de la unidad activa. La
> sección "Unidad activa" debe coincidir con la última entrada de
> `JOURNAL.md` y con el HEAD del repositorio. Si hay discrepancia, gana
> la realidad verificable (test suite + git log + binarios).

## Snapshot

| Campo | Valor |
|---|---|
| Hito activo | **F2 — Correctitud reproducible** |
| Última unidad cerrada | **F2.W1 — Caracterización de la correctitud del análisis (R2)** |
| Unidad activa siguiente | **F2.W2 — Errores de lectura silenciosos en PerFileStrategy** (R3) |
| Estado de certificación | F1 = IMPLEMENTED + INTEGRATED + ACCEPTED. F2.W1 = IMPLEMENTED + INTEGRATED + ACCEPTED. Pendiente RELEASED. |
| HEAD | `70f0b0cf` (17 commits ahead de origin/main) |
| Working tree | Limpio |
| Bloqueos conocidos | H10 OPEN — test `cogh update` falla por GitHub API rate limit (deuda externa; no bloquea C1). Bug preexistente del binario `cognicode` (workspace con dos crates `name = "cognicode"`) — fuera del alcance F2.W1. |
| Siguiente unidad ejecutable | F2.W2 (errores de lectura silenciosos en `PerFileStrategy::build_full_graph`) |
| Política git | `docs/prf/` se versiona para **documentos del programa** (.md, fixtures) con `git add -f`. Evidencia cruda (strace, JSON-RPC binarios, logs de cargo test) sigue siendo local-only y está manifestada en `evidence/MANIFEST.md` |

## Última unidad cerrada: F2.W1 (Correctitud del análisis — invalidación de cache)

**Objetivo**: caracterizar la correctitud de la vertical `PerFileStrategy`
(`cognicode graph per-file <file>`, `get_per_file_graph` MCP tool) y
cerrar el primer defecto reproducible: el cache no detecta cambios de
contenido entre llamadas.

**Vertical elegida**: `PerFileGraphCache::get_or_build` en
`crates/cognicode-core/src/infrastructure/graph/per_file_graph.rs`.
Justificación: accesible desde el binario real (CLI y MCP), aislada,
con test de regresión natural.

**Riesgos catalogados** (de los 4 descritos en el brief):

| ID | Riesgo | Estado |
|---|---|---|
| R1 | Recorrido anidado (subdirectorios) | Caracterizado en F2.W1 — test `test_per_file_strategy_build_full_graph_nested_corpus` PASS (≥3 símbolos sobre corpus de 3 archivos anidados) |
| R2 | Detección de cambios de contenido | **CLOSED en F2.W1** — fix mínimo con fingerprint (mtime+size), 2 tests nuevos GREEN |
| R3 | Errores de lectura silenciosos | Diferido a **F2.W2** (defecto real: `PerFileStrategy::build_full_graph` usa `filter_map(\|e\| e.ok())` y `merge` usa `unwrap_or_else(\|_\| CallGraph::new())`; cobertura insuficiente se reporta como éxito) |
| R4 | Equivalencia `full` vs `per_file` | Diferido a **F2.W3** (caracterización sin corrección; las dos estrategias tienen propósitos distintos) |

**Causa raíz (R2)**: `get_or_build` solo consultaba `entry.valid`; nunca
preguntó a `fs::metadata` por el estado del archivo. Resultado: después de
editar un archivo, el cache devolvía el grafo viejo.

**Fix mínimo**:
- `FileGraphCacheEntry` gana dos campos: `mtime_secs: Option<u64>` y
  `size: Option<u64>`.
- Helpers privados `file_fingerprint(path)` y `system_time_to_secs(t)`.
- `get_or_build` ahora comprueba `valid && mtime == fp.mtime && size == fp.size`.
- Si `fs::metadata` falla (archivo borrado, etc.), se reconstruye
  conservadoramente.

**Tests**:
- `test_per_file_graph_cache_detects_content_change` (RED → GREEN):
  escribe 1 función, cachea, reescribe con 2 funciones tras 1.1s (para
  superar granularidad de mtime), re-pide. Sin fix: 1 símbolo (stale).
  Con fix: >1 símbolo (nuevo).
- `test_per_file_strategy_build_full_graph_nested_corpus` (GREEN desde
  inicio): ejercita `PerFileStrategy::build_full_graph` sobre el corpus
  `docs/prf/fixtures/per_file_correctness/` y verifica ≥3 símbolos en
  3 archivos anidados (2 niveles).

**Corpus nuevo** (versionado en este commit):

- `docs/prf/fixtures/per_file_correctness/CORPUS.md` — describe el
  oráculo (3 funciones, 2 edges) y por qué es independiente de la
  implementación.
- `docs/prf/fixtures/per_file_correctness/src/lib.rs` — `top_level`.
- `docs/prf/fixtures/per_file_correctness/src/nested/mod.rs` — `mid_level`.
- `docs/prf/fixtures/per_file_correctness/src/nested/deeply_nested/mod.rs` — `leaf`.

**Verificaciones ejecutadas**:

- `cargo test -p cognicode-core --lib per_file_graph` → **8/8 pass**.
- `cargo test -p cognicode-core --lib` → **2085/0/27** (baseline F0.W3
  era 2083/0/27, **+2 tests** sin regresión).
- `cargo test -p cognicode-cli --bin cogh` → 277/13/1 + 1 filtered. Los
  **13 fallos son preexistentes** (verificado con `git stash`): el
  workspace tiene dos crates con `name = "cognicode"` y cargo no
  produce `target/debug/cognicode`, lo que rompe los tests que lanzan
  ese binario como subproceso. **No es regresión de F2.W1**.
- Manual: revertido el fix del cache localmente, el nuevo test falla
  (RED confirmado); re-aplicado, pasa (GREEN confirmado).
- UAT sobre el código real: el test runner de `cognicode-core` ejecuta
  el código real de `PerFileStrategy::build_full_graph` y
  `PerFileGraphCache::get_or_build` sobre el corpus, no un mock.

**Composición del commit**:

```
70f0b0cf fix(per-file-cache): invalidate entries on content change (R2 from F2.W1)
05ba121e docs(prf): reconcile F1 closure and version the PRF program documents
9628b1d3 fix(prf-f1.w3): replace placeholder sha256 in bundled plugin manifests (H8)
```

**Certificación**: F2.W1 = **IMPLEMENTED + INTEGRATED + ACCEPTED**.
Detalle en `evidence/CERTIFICATES.md` (cert PRF-F2-W1).

**Política respetada**:

- No se cambió la API pública de `PerFileGraphCache` (mismas firmas).
- No se introdujeron nuevos módulos, ports, event bus ni representaciones
  alternativas.
- No se rebajó el oráculo: el test exige `new > original` con mensaje de
  error explícito.
- El fix es mínimo (~50 líneas de código nuevo) y se aplica solo donde
  es necesario.

**Próxima unidad concreta**: **F2.W2 — Errores de lectura silenciosos en
PerFileStrategy::build_full_graph (R3)**. Defecto a corregir: cuando un
archivo no se puede parsear, el merge continúa silenciosamente con un
grafo vacío para ese archivo, llevando a una conclusión falsa de
"análisis completo". Solución: cambiar el contrato de `build_full_graph`
para que devuelva un tipo que incluya tanto el grafo como la lista de
archivos omitidos, y documentar el comportamiento en UAT.

## Hito F2 (Correctitud reproducible) — En curso

| Unidad | Estado |
|---|---|
| F2.W1 — Invalidación de cache por cambio de contenido (R2) | **ACCEPTED** (commit 70f0b0cf) |
| F2.W2 — Errores de lectura silenciosos (R3) | Pendiente |
| F2.W3 — Equivalencia full vs per_file (R4) | Pendiente |

## Hito F1 (Estabilización) → CERRADO (referencia histórica)

| Unidad | Estado |
|---|---|
| F1.W1 — Stdout/signals (H6+H9) | ACCEPTED (commit 834aff67) |
| F1.W2 — Cifra 68 → 75 (H2+H7) | ACCEPTED (commit 4ff514a7) |
| F1.W3 — sha256 reales (H8) | ACCEPTED (commit 9628b1d3) |
| F1.W4 — Mock GitHub API (H10) | WIP → migrado a deuda de F2 |
| F1.W5 — Decisiones UX (H3+H4) | ACCEPTED (solo docs) |

**Hito F1 cerrado**. Detalle completo en `JOURNAL.md` §6.

## Hito F0 (Inventario y baseline) → CERRADO (referencia histórica)

| Unidad | Estado |
|---|---|
| F0.W1 — Inventario de binarios | ACCEPTED |
| F0.W2 — Caracterización arranque/persistencia/red | ACCEPTED |
| F0.W3 — Baseline de pruebas automatizadas | ACCEPTED |

**Hito F0 cerrado**. Detalle completo en `evidence/F0-W2-runtime.md` y
`evidence/F0-W3-baseline.md`.

## Estado de certificaciones

Ver `evidence/CERTIFICATES.md`.

## Próxima unidad a abrir (F1 — Estabilización)

## Notas sobre la bootstrap de PRF

Este programa PRF se inicializa en esta sesión. La estructura `docs/prf/`
no existía previamente. Los 9 documentos base (README, ROADMAP,
CERTIFICATION, UAT, TEST-PLAN, TRACEABILITY, STATE, JOURNAL,
evidence/CERTIFICATES) se crearon como primer paso del trabajo de F0.W1.

Justificación documentada en JOURNAL.md, primera entrada.
