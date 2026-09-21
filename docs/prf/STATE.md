# Production-Ready Foundation (PRF) — STATE

> **Fuente de verdad**: este archivo es el puntero de la unidad activa. La
> sección "Unidad activa" debe coincidir con la última entrada de
> `JOURNAL.md` y con el HEAD del repositorio. Si hay discrepancia, gana
> la realidad verificable (test suite + git log + binarios).

## Snapshot

| Campo | Valor |
|---|---|
| Hito activo | **F2 — Correctitud reproducible** |
| Última unidad cerrada | **F1 — Estabilización** (hito entero, ACCEPTED — 5 unidades; 8 hallazgos CLOSED, 1 WIP, 1 acción operativa) |
| Unidad activa siguiente | **F2.W1 — Caracterización de correctitud de análisis** (vertical: recorrido anidado + detección de cambios de contenido) |
| Estado de certificación | F1 = IMPLEMENTED + INTEGRATED + ACCEPTED. Pendiente RELEASED. |
| HEAD | `9628b1d3` (3 commits ahead del HEAD de inicio de sesión) |
| Working tree | Limpio en main; docs/prf/ working-only con `MANIFEST.md` de evidencia cruda |
| Bloqueos conocidos | H10 OPEN — test `cogh update` falla por GitHub API rate limit (deuda externa; no bloquea C1 porque core sin red funciona) |
| Siguiente unidad ejecutable | F2.W1 (caracterización de recorrido anidado + cambio de contenido) |
| Política git | `docs/prf/` se versiona para **documentos del programa** (.md) con `git add -f`. Evidencia cruda (strace, JSON-RPC binarios, logs de cargo test) sigue siendo local-only y está manifestada en `evidence/MANIFEST.md` |

## Última unidad cerrada: F1 (Estabilización)

**Objetivo**: cerrar los 10 hallazgos identificados durante F0
(H2, H3, H4, H6, H7, H8, H9, H10 — H1 y H5 ya cerrados en F0.W1) con
acciones concretas y trazables en el código fuente.

**Resultado**: ACCEPTED. 5 unidades ejecutadas, 4 de ellas con
cambios de código versionado en main, 1 (W5) con decisiones
documentadas sin tocar código.

**Composición por unidad**:

| Unidad | Hallazgos | Commits | Cambios |
|---|---|---|---|
| F1.W1 | H6 + H9 | `834aff67` | tracing→stderr; shutdown signals con log + axum `with_graceful_shutdown` |
| F1.W2 | H2 + H7 | `4ff514a7` | cifra 68 → 75 en 6 archivos (manifest + 4 docs) |
| F1.W3 | H8 | `9628b1d3` | sha256 reales (del propio YAML) en 5 manifests bundled |
| F1.W4 | H10 | (sin commit) | staging cubre /releases/latest; download de manifest YAML queda en red — WIP |
| F1.W5 | H3 + H4 | (sin commit) | decisión KEEP+DOCUMENT / KEEP+MARK; solo docs |

**Verificaciones ejecutadas (resumen)**:

- F1.W1: build OK; 955 tests cognicode-explorer pasan; smoke SIGTERM
  "exit 0, log 'shutdown received'"; smoke `cognicode --verbose analyze`
  con logs a stderr y datos a stdout.
- F1.W2: cero referencias residuales a "68 tools" en archivos versionados;
  tests cogh 290/0/1.
- F1.W3: 5 sha256 reales; bundled_manifests_parse OK; tests cogh 290/0/1.
- F1.W4: scaffolda test que apuntaba a `--staging` con el nombre canónico
  del manifest (`bundle-<v>-<p>.yaml`); el download del bundle va a la
  URL real (no cubierto por el staging actual). Decisión: WIP honesto,
  no fix para flake externo (Cardinal Sin #5).
- F1.W5: solo docs (TRACEABILITY.md marca los hallazgos como
  CLOSED — KEEP+DOCUMENT / KEEP+MARK).

**Estado de hallazgos al cierre**:

| Hallazgo | Estado | Acción |
|---|---|---|
| H1 (50 tests rojos anteriores) | Cerrado (sesión previa) | — |
| H2 (cifra 68) | CLOSED (F1.W2) | ADR-031 + manifest actualizado |
| H3 (cognicode-mcp-server no usado) | CLOSED (F1.W5) | Variante HTTP/SSE válida; documentar |
| H4 (multimodal en --help) | CLOSED (F1.W5) | Marca honesta en el comando; KEEP |
| H5 (LSPs faltantes) | Sin acción de código | Setup UAT |
| H6 (logs a stdout) | CLOSED (F1.W1) | tracing → stderr |
| H7 (manifest 68 → 75) | CLOSED (F1.W2) | Manifest actualizado |
| H8 (5/6 sha256 placeholder) | CLOSED (F1.W3) | sha256 reales del propio YAML |
| H9 (SIGTERM silencioso) | CLOSED (F1.W1) | Hook axum graceful shutdown |
| H10 (rate limit GH) | OPEN (WIP) | Extender staging_dir en F2 |

**Evidencias archivadas** (local-only, working tree):

- `evidence/F0-W1-inventory.md`, `evidence/F0-W2-runtime.md`,
  `evidence/F0-W3-baseline.md`, `evidence/H10-correction.md`.
- `evidence/F0-W2-runs/`, `evidence/F0-W3-runs/` (logs observables).
- `evidence/CERTIFICATES.md` (certificados PRF-F0-W1, PRF-F0-W2,
  PRF-F0-W3).

**Commits nuevos en main** (no push):

```
9628b1d3 fix(prf-f1.w3): replace placeholder sha256 in bundled plugin manifests (H8)
4ff514a7 fix(prf-f1.w2): update MCP tool count from 68 to 75 (H2 H7)
834aff67 fix(prf-f1.w1): route tracing to stderr + log shutdown signals (H6 H9)
```

## Hito F1 (Estabilización) → CERRADO

| Unidad | Estado |
|---|---|
| F1.W1 — Stdout/signals (H6+H9) | ACCEPTED (commit 834aff67) |
| F1.W2 — Cifra 68 → 75 (H2+H7) | ACCEPTED (commit 4ff514a7) |
| F1.W3 — sha256 reales (H8) | ACCEPTED (commit 9628b1d3) |
| F1.W4 — Mock GitHub API (H10) | WIP (sin commit — staging incompleto para downloads) |
| F1.W5 — Decisiones UX (H3+H4) | ACCEPTED (solo docs) |

**Hito F1 cerrado**.

## Hito F0 (Inventario y baseline) → CERRADO

| Unidad | Estado |
|---|---|
| F0.W1 — Inventario de binarios | ACCEPTED (sesión previa) |
| F0.W2 — Caracterización arranque/persistencia/red | ACCEPTED (este turn) |
| F0.W3 — Baseline de pruebas automatizadas | ACCEPTED (este turn) |

**Hito F0 cerrado**. Siguiente hito sugerido: **F1 (Estabilización)**.

**Objetivo**: caracterizar el arranque, la inicialización de
dependencias, la persistencia, el uso de red y la salida por
stdout/stderr de cada uno de los 5 binarios inventariados en F0.W1.

**Resultado**: ACCEPTED. Detalle en `evidence/F0-W2-runtime.md`.

**Resumen de resultados**:

- 5 binarios caracterizados (arranque ≤10ms, exit 0).
- 3 strace ejecutados: `cogh`, `cognicode-mcp`, `explorer-api`.
- `cognicode-mcp`: 130 threads Tokio; 0 sockets; logs a stderr, JSON-RPC
  a stdout (patrón correcto).
- `explorer-api`: 193 threads; `bind+listen` en 127.0.0.1 (backlog 128);
  abre `.lbug.wal` continuamente (52× en 4s).
- `cogh`: monolítico, 0 red, salida limpia.
- 0 secretos en logs (positivo).
- **4 hallazgos nuevos**: H6 (cognicode logs a stdout), H7 (manifest
  dice "68 tools"), H8 (5/6 sha256 placeholder), H9 (explorer-api
  SIGTERM silencioso).
- 6 comprobaciones BLOCKED/NOT_RUN con motivo.

**Evidencias archivadas**:
- `evidence/F0-W2-runtime.md` (310 líneas, fuente de verdad).
- `evidence/F0-W2-runs/` (63 archivos, ~5.2 MB: strace, stdout,
  stderr, time-files, JSON-RPC frames).

## Estado de certificaciones

Ver `evidence/CERTIFICATES.md`.

## Próxima unidad a abrir (F1 — Estabilización)

## Notas sobre la bootstrap de PRF

Este programa PRF se inicializa en esta sesión. La estructura `docs/prf/`
no existía previamente. Los 9 documentos base (README, ROADMAP,
CERTIFICATION, UAT, TEST-PLAN, TRACEABILITY, STATE, JOURNAL,
evidence/CERTIFICATES) se crearon como primer paso del trabajo de F0.W1.

Justificación documentada en JOURNAL.md, primera entrada.
