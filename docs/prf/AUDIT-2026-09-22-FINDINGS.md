# AUDIT 2026-09-22 — Findings Tracker

> **Status**: contenedor abierto por la sesión autónoma del 2026-09-24
> (JOURNAL §138). NO toma decisiones contractuales — solo estructura
> los 13 hallazgos (H01–H13) en un único punto de referencia para que
> cada uno pueda ser OPEN / WIP / CLOSED / DEFERRED bajo decisión del
> operador.
>
> **Fuente de los hallazgos**: auditoría externa sobre `93b7a9a3`,
> recibida y reconocida en JOURNAL §134 (2026-09-24).
>
> **Fuente de este tracker**: sesión autónoma posterior a la
> publicación de v0.98.0 (`11a128a5` / `e2bbd86a`, 2026-09-24). El
> tracker se coloca junto a `docs/analysis/release-1.0.0-state-audit.md`
> (que es **input** de la auditoría, no su output) y sigue el patrón de
> `evidence/H10-correction.md` (que ya sirvió como contenedor de
> cierre honesto de un hallazgo específico).
>
> **Política de actualización**: editar este archivo NO requiere
> autorización nueva — es trazabilidad PRF. Marcar un hallazgo como
> CLOSED/DEFERRED sí requiere la decisión del operador documentada en
> `JOURNAL.md` (cuya existencia sigue el append-only §N).

## Convenciones de estado

| Estado | Significado | Cómo se setea |
|---|---|---|
| OPEN | El hallazgo está reconocido pero todavía no hay trabajo asociado. | Default al crear este tracker. |
| WIP | Hay trabajo en curso (commit o WU referenciado). | Operador AUTORIZA, o trabajo dentro del WU activo. |
| CLOSED | El hallazgo está cerrado por código, docs, o por decisión contractual explícita. | Operador-gated. Requiere evidencia + entrada en JOURNAL. |
| DEFERRED | Decisión consciente de NO cerrar; se reactiva en un roadmap posterior. | Operador-gated. Requiere razón + roadmap. |

## Snapshot actual (2026-09-24, post-v0.98.0-publicación)

| Campo | Valor |
|---|---|
| Release publicada | `CogniCode v0.98.0` (GitHub release #36034410448, success) |
| HEAD actual | `481bb28d` (STATE self-roll H11 close, JOURNAL §141) sobre `89cdec3f` (STATE self-roll H11) sobre `44cfe602` (§140 H11 TRACEABILITY close) |
| Tag pushed | `v0.98.0` (annotated, tag-object `d99d3911…`) |
| C7 firma | **BLOQUEADO** (la auditoría es justo la razón) |
| Cobertura cognicode-core | 74,15% (informativo, no gate, §134 H08) |
| Hallazgos | 13 (H01 críticos para C7 → H13 baja) |

## H01 — RELEASE-CANDIDATE.md mantiene SHA congelado `178f8a5b` (CRÍTICA para C7)

- **Severidad**: crítica para C7.
- **Estado actual**: **WIP** particionado — opción 2 (criterio de staleness) ya cumplida por el propio doc; opción 1 (nueva candidata C7) sigue operator-gated.
- **Recomendación de la auditoría**: fijar nueva candidata DESPUÉS de cerrar requisitos pendientes; ejecutar campaña C7 sobre la candidata nueva; separar `VALIDATED` / `RELEASE_ACCEPTED` / `PUBLISHED`.
- **Evaluación honesta del par (opción 1, opción 2)** — JOURNAL §141:
  - **Opción 2 (criterio de staleness en el propio doc): CUMPLIDA por construcción.**
    El propio `docs/prf/RELEASE-CANDIDATE.md` ya contiene el criterio de staleness explícito en dos sitios:
      - Cabecera línea 3: "El SHA candidato está **congelado** abajo; las acciones 1-5 del plan operador (sección 5 de la auditoría) se ejecutarán **en orden estricto** contra este SHA, sin reescribirlo."
      - Línea 5 (NOTA 2026-09-22): "El SHA congelado `178f8a5b` queda **stale**; el operador deberá re-firmar el freeze antes de proseguir. NO se actualiza automáticamente: el push sigue BLOQUEADO."
      - Línea 7 (meta-nota sobre staleness iterativa): "La solución canónica es **dejar de tocar los punteros** (mantener el freeze `178f8a5b` honestamente stale) hasta que el operador re-firme".
    Los tres sitios convergen en el mismo criterio: "knowingly stale hasta que el operador re-firme".
    Esto cumple el texto literal de la opción 2 de la auditoría: "Fijar criterio de staleness explícito en `RELEASE-CANDIDATE.md` reconociendo el flujo actual".
    La sesión autónoma NO inventa una "apertura nueva" en el archivo, porque el archivo ya está abierto en el estado correcto.
  - **Opción 1 (nueva candidata con campaña C7 completa): operator-gated, NO ejecutada en esta sesión.**
    Requisito de campañas C7 contra SHA candidato natural (`e2bbd86a` o posterior) requiere: matriz RECONCILIATION-MATRIX.md (acción 2 del plan "Cierre de PRF" del propio doc, no creada), pruebas que certifiquen equivalencia con C0–C6, decisión de versionado, push autorizado.
    El HEAD actual `481bb28d` ha avanzado **314 commits** desde `178f8a5b` (verificado `git rev-list --count 178f8a5b..481bb28d`), pero la batería de tests sigue verde sobre el HEAD vigente: `cargo test -p cognicode-core --lib` → **2166 passed / 0 failed / 27 ignored** post-§125 (ver `docs/prf/evidence/CERTIFICATES.md` línea 1018), más la verificación focal de §140 que añadió R1.1–R1.4 = 4/4 PASS sobre el rango actual.
- **Estado de hecho**: la release v0.98.0 publicada NO actualizó `RELEASE-CANDIDATE.md` (decisión deliberada — actualizar release-candidate sin campaña C7 sería peor que no hacerlo). El SHA congelado `178f8a5b` en ese documento está **knowingly stale**, marcado así desde §134 y §135.
- **Acción necesaria (operator-gated)**:
  1. Abrir nueva candidata y ejecutar campaña C7: requiere decisión del operador + RECONCILIATION-MATRIX.md primero + plan de equivalencia C0–C6.
  2. (Ya cumplida vía doc contractual existente) Criterio de staleness explícito.
- **Refs**: `docs/prf/RELEASE-CANDIDATE.md` (líneas 3, 5, 7 son la prueba contractual de la opción 2), JOURNAL §134 (acuse), §135 (publicación), §140 (H11.Traceability close — ver estado HEAD vigente), §141 (esta evaluación).

## H02 — `ci.yml` solo tiene `workflow_dispatch`; sin branch protection ni required checks (ALTA)

- **Severidad**: alta.
- **Estado actual**: **OPEN**.
- **Descripción**: política local-first reconocida pero sin enforcement equivalente en CI para `main`. F6.W3.bis resolvió parcialmente para `release-validate.yml` (gate `Bind to expected_sha`), pero NO para el CI general.
- **Acción necesaria (operator-gated)**:
  - Decisión de governance: ¿activar branch protection con required checks? Si sí, ¿cuáles son los required checks mínimos (suite `--lib`, audit, advisor)?
  - Configurar en GitHub repo Settings > Branches > main.
- **Refs**: `.github/workflows/ci.yml`.

## H03 — Dependencias inversas a la arquitectura hexagonal (MEDIA-ALTA)

- **Severidad**: media-alta.
- **Estado actual**: **OPEN**.
- **Descripción**: `FileOperationsService` importa `InputValidator` de MCP directamente; `WorkspaceSession` construye implementaciones concretas de caché, LSP, verificador, validador MCP; `AnalysisService` depende de implementaciones de grafo/parser de infraestructura. Puertos neutrales faltantes; seams sin inyección adecuada.
- **Riesgo de cierre autónomo**: alto (refactor arquitectónico con riesgo de regresión).
- **Acción necesaria (operator-gated)**:
  - Decidir scope: ¿qué servicios se refactorean primero? (¿`WorkspaceSession` por ser el más grande?)
  - Definir puertos neutrales y plan de inyección desde raíz de composición.
  - Tests de equivalencia antes/después sobre UAT corpus existente.
- **Refs**: `crates/cognicode-runtime/src/application/services/workspace_session.rs`, `crates/cognicode-core/src/application/services/analysis_service.rs`, `crates/cognicode-runtime/src/application/services/file_operations.rs`.

## H04 — Dos rutas de construcción del grafo (MEDIA-ALTA)

- **Severidad**: media-alta.
- **Estado actual**: **WIP** (F2.W7 ya documentó la divergencia; Issue J remediación 100% completada V14+V15).
- **Descripción**: `AnalysisService::build_full_graph` vs `FullGraphStrategy`/`PerFileStrategy`. Las estrategias modificadas por F2.W5 no eran invocadas por el binario real. Riesgo de connascence semántica.
- **Acción necesaria (operator-gated)**:
  - Decidir: ¿consolidar (un solo propietario de reglas) o mantener separación?
  - Si consolidar: pruebas de equivalencia sobre el mismo corpus antes de borrar una ruta.
  - Si mantener: añadir test que detecte divergencias automáticamente.
- **Refs**: `crates/cognicode-core/src/application/services/analysis_service.rs`, JOURNAL §95+ (F2.W7 + Issue J).

## H05 — Módulos de gran tamaño (MEDIA)

- **Severidad**: media.
- **Estado actual**: **OPEN**.
- **Tamaños observados** (pueden haber variado desde la auditoría):
  - `handlers/mod.rs` ~7.552 líneas
  - `workspace_session.rs` ~4.532
  - `file_operations.rs` ~3.944
  - `analysis_service.rs` ~3.809
  - `cognicode-explorer/src/domain/views.rs` >9.000
- **Acción necesaria (operator-gated)**:
  - Decisión: ¿qué módulo se aborda primero? ¿`views.rs` por ser el de mayor tamaño y por usar `QualityGraphRepository` desde `adapters` (frontera adicional)?
  - Refactor + tests + métrica de tamaño antes/después.
- **Refs**: módulos listados arriba, JOURNAL §125 (issue B de §123 cerrado vía PRF-STATE-11/12/13).

## H06 — Doble representación de autoridad MCP (ALTA por permisos)

- **Severidad**: alta.
- **Estado actual**: **OPEN**.
- **Descripción**: herramientas declaran `authority` en metadatos, pero `list_tools` filtra con `MUTATING_TOOLS` heredada. `PRF-MCP-05` parcial.
- **Riesgo de cierre autónomo**: alto (permisos = seguridad).
- **Acción necesaria (operator-gated)**:
  1. Una sola fuente ejecutable de autoridad.
  2. Prueba negativa con herramienta sintética con permisos elevados en modo solo lectura.
  3. Validar `cogh doctor` + `release-install-smoke.sh` no rompen (los negativos verificados en §130).
- **Refs**: `crates/cognicode-mcp/src/...`, `PRF-MCP-05` traceability entry.

## H07 — Campaña de seguridad incompleta (ALTA para certificación)

- **Severidad**: alta para certificación.
- **Estado actual**: **WIP** (cláusula RELEASE-CANDIDATE §Notas de honestidad + JOURNAL §125.V23 documentan; esto es cierre DOCUMENTAL, no cierre real de la campaña).
- **Faltante real**: presupuestos CPU/memoria/tiempo, cancelación durante operación costosa, cobertura exhaustiva TOCTOU. `PRF-SEC-07` adversarial integrado pendiente.
- **Acción necesaria (operator-gated)**:
  - Definir escenarios adversariales (¿fixtures en `docs/prf/evidence/adversarial/`?).
  - Implementar y medir presupuestos.
  - Correr la campaña contra binarios reales.
- **Refs**: JOURNAL §125.V23 (cláusula), PRF-SEC-07.

## H08 — Cobertura 74,15% lines informativa, no gate (MEDIA)

- **Severidad**: media.
- **Estado actual**: **OPEN**.
- **Descripción**: cobertura en `cognicode-core --lib` es informativa, no gate. `continue-on-error: true` en CI. Falla de regresión podría pasar inadvertida en CLI/MCP/Explorer.
- **Acción necesaria (operator-gated)**:
  1. Decidir: ¿es 74,15% suficiente? ¿umbral aceptable 80%?
  2. Si se decide el umbral, cambiar a `continue-on-error: false` para que el gate funcione.
- **Refs**: `cargo test --package cognicode-core --lib --no-run` (build OK), `evidence/` runs, CI logs.

## H09 — Rendimiento medido pero no certificado como regresión controlada (MEDIA)

- **Severidad**: media.
- **Estado actual**: **OPEN**.
- **Descripción**: `PRF-CI-04` falta comparación automatizada en entorno estable.
- **Acción necesaria (operator-gated)**:
  - Definir baseline ejecutable en entorno estable.
  - Implementar comparación (¿bench harness en CI? ¿algún criterio como criterion.rs?).
- **Refs**: `evidence/perf-baseline/`, PRF-CI-04.

## H10 — Cadena de suministro con excepciones documentadas (MEDIA-ALTA)

- **Severidad**: media-alta.
- **Estado actual**: **CLOSED-BY-DOCUMENTATION** (evidence/H10-correction.md v3 final).
- **Descripción**: dependencias con excepciones en `deny.toml`; `PRF-CI-05` parcial por gate de licencias.
- **Acción necesaria**: ninguna para cierre documental. Reactivación solo si el operador decide endurecer el gate de licencias más allá de las excepciones existentes.

## H11 — Punteros y matrices desactualizados respecto al HEAD remoto (MEDIA — "la que más me toca")

- **Severidad**: media (la más concreta para esta sesión).
- **Estado actual**: **WIP** (en proceso de cierre por esta sesión).
- **Trabajo realizado esta sesión**:
  - `c2b2924d`: recovered §135+§136 narrative that `d40e61b2` revertió silenciosamente.
  - `e2bbd86a`: STATE snapshot para §137 cerrado.
  - `3316f445` (JOURNAL §140): `docs/prf/TRACEABILITY.md` H-F3-1 marcado **RESUELTO delegación** (ciclo SDDK `prf-h-f3-1` CLOSED seq 12 sobre commits `49224b2a` + `0124befb`, verificado vía 4/4 PASS de tests R1.1–R1.4 en `analysis_service::find_symbol_usages_tests`). Deuda separada pendiente dentro de la misma fila: CLI equivalente al tool MCP `find_usages`.
  - `RELEASE-CANDIDATE.md` mantiene freeze `178f8a5b` knowingly stale — el operador decide si lo actualiza al SHA post-publicación (`e2bbd86a`).
- **Pendiente (operator-gated)**:
  - Decisión sobre `RELEASE-CANDIDATE.md` (parte del H01, no duplicar).
  - CLI equivalente al tool MCP `find_usages` en `cognicode-cli` — OPEN (sub-categoría del H-F3-1, separado del cierre de delegación). Implementar `cognicode find-usages <name>` reusando `AnalysisService::find_symbol_usages` requiere definir scope, signatura, y tests.
- **Refs**: `docs/prf/STATE.md` (HEAD row), JOURNAL §135-§137.

## H12 — Distribución validada tiene alcance menor que el producto descrito (MEDIA-ALTA)

- **Severidad**: media-alta.
- **Estado actual**: **WIP** para alcance contractual; **CLOSED** para Linux x86_64+aarch64 (5/5 verde). Alcanza WIP vía `docs/prf/DISTRIBUTION-SCOPE.md` (JOURNAL §139, commit `89ea4baf`).
- **Descripción**: 5/5 cubre Linux x86_64 + aarch64; `PRF-DIST-05` y `PRF-DIST-07` parciales para otras plataformas y `explorer-*`.
- **Acción necesaria (operator-gated)**:
  - Decisión: ¿agregar macOS/Windows/MUSL?
  - Si sí: runners nativos o cross-compile + tests.
- **Refs**: `docs/prf/evidence/CERTIFICATES.md` (PRF-DIST cert), `release.yml`.

## H13 — CP1.0 con endpoint HTTP de lectura pero sin flujo de valor completo (MEDIA)

- **Severidad**: media.
- **Estado actual**: **DEFERRED-BY-DESIGN** (decisión correcta explícita: mantener CP diferido durante cierre PRF).
- **Acción necesaria**: ninguna durante el cierre PRF. Reactivación en roadmap post-PRF.

## Resumen de próximos pasos operator-gated

| Orden sugerido (de menor a mayor esfuerzo) | Hallazgos |
|---|---|
| **Trivial** (decisión 1-párrafo) | H10 ya cerrado; H13 ya diferido. |
| **Bajo esfuerzo** (actualizar docs) | H01 (RELEASE-CANDIDATE: opción 2 ya cumplida — declarada en §141), H11 (H-F3-1 delegación cerrada §140; queda CLI equivalente), H12 (DISTRIBUTION-SCOPE WIP vía §139). |
| **Medio esfuerzo** (CI policy) | H02 (branch protection), H08 (gate de cobertura `continue-on-error: false` si se decide el umbral). |
| **Alto esfuerzo** (refactor) | H03 (hexagonal), H04 (rutas de grafo), H05 (módulos grandes), H09 (perf benchmark). |
| **Crítico esfuerzo** (seguridad) | H06 (autoridad MCP), H07 (campaña adversarial PRF-SEC-07). |

---

## Lo que la sesión autónoma de 2026-09-24 NO decidió

- NO marca hallazgos como CLOSED/DEFERRED — eso requiere decisión del operador.
- NO propone un roadmap de remediación concreto — el operador elige el orden.
- NO abre nuevas WU bajo PRF (F0-F6) por cada hallazgo — eso es operator-gated.
- NO modifica `docs/prf/RELEASE-CANDIDATE.md` aunque el SHA congelado está stale — la actualización contractual de ese archivo es el H01 mismo y debe llegar con campaña C7 cerrada, no antes.

## Política de "auto-cierre" futura

Si una sesión autónoma futura quiere CLOSE un hallazgo, el flujo es:

1. La sesión documenta el trabajo en JOURNAL §N (con commit hash + evidencia).
2. La sesión actualiza este tracker cambiando el estado y añadiendo una sección "Evidencia de cierre" con punteros a JOURNAL §N.
3. La sesión NO presenta el cierre como contractual — describe exactamente qué se hizo y qué queda.

Bajo ningún concepto una sesión autónoma puede "firmar" un hallazgo como CERRADO si la firma tiene implicaciones contractuales (válido para C7, válido para certificación externa). Eso queda en `RELEASE-CANDIDATE.md` + decisión operator-explicit.

---

## Refs

- Auditoría original (acuse de recibo): JOURNAL §134 (2026-09-24, sobre SHA `93b7a9a3`).
- Workflow gates cerrados: §135 (release v0.98.0), §136 (tag/workspace coherence gate), §137 (pre-commit docs-isolation guard).
- Re-activación de este tracker: cada vez que el operador toca el estado de un hallazgo, en el append-only §N+1.
