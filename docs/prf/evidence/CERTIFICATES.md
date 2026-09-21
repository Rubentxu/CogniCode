# Certificados PRF — Production-Ready Foundation

> Cada certificado documenta la evidencia que avala la transición de
> una unidad a su estado final (SPECIFIED → IMPLEMENTED → INTEGRATED →
> ACCEPTED → RELEASED).

---

## PRF-F0-W1 — Certificación de la unidad F0.W1 (Inventario de binarios)

| Campo | Valor |
|---|---|
| ID | `PRF-F0-W1` |
| Hito | F0 — Inventario y baseline |
| Unidad | W1 — Inventario verificable de binarios |
| Versión CogniCode | 0.97.3 |
| HEAD al cierre | `7cc6a8a7` |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 |

### Estados alcanzados

- [x] **SPECIFIED**: objetivo y criterios de salida definidos en STATE.md
      y ROADMAP.md (W1).
- [x] **IMPLEMENTED**: 5 binarios inventariados, 9 documentos PRF creados.
- [x] **INTEGRATED**: el inventario se ejecutó contra los binarios reales
      (no mocks); los comandos y herramientas documentadas son los que
      el runtime expone.
- [x] **ACCEPTED**: resultados verificados, contradicciones y hallazgos
      catalogados, criterios de salida cumplidos.
- [ ] **RELEASED**: pendiente. PRF es un programa interno; la decisión
      de "release" se aplica a su consolidación dentro del roadmap
      principal (E35+). Para PRF, ACCEPTED es el cierre práctico.

### Evidencias concretas

| Evidencia | Ubicación |
|---|---|
| Inventario de binarios | `docs/prf/evidence/F0-W1-inventory.md` |
| Baseline tests | `2083/291/7 PASS` (capturado en F0-W1-inventory) |
| Catálogo MCP | 20 (cognicode-mcp) + 55 (explorer-mcp) = 75 (F0-W1-inventory) |
| Hallazgos | H1-H5 (F0-W1-inventory.md §"Hallazgos críticos") |
| Contradicciones | C1-C2 (F0-W1-inventory.md §"Contradicciones") |

> **Nota sobre `Commit`**: la columna Commit está en blanco porque
> `docs/prf/` está cubierto por `.gitignore` (política `docs/`
> working-only). La unidad F0.W1 se cierra como `ACCEPTED` en el
> working tree; los commits que materialicen los hallazgos (H2,
> H3, H4) serán commits separados del código CogniCode cuando se
> aborden, no del directorio PRF.

### Decisiones tomadas

- D1: PRF ≠ PROG-productization (programas paralelos).
- D2: inventario con binarios reales, sin mocks.
- D3: hallazgos con severidad (todos LOW en F0.W1).
- D4: contradicciones registradas con acción propuesta.

### Firmas de aprobación

| Rol | Nombre | Estado | Notas |
|---|---|---|---|
| Operador | jcode-orchestrator | APROBADO | Sesión 2026-09-21 |
| Auto-revisión PRF | (programa PRF) | APROBADO | Criterios de salida cumplidos |

### Trabajo pendiente heredado

- F0.W2 (caracterización arranque/persistencia/red).
- F0.W3 (baseline de pruebas automatizadas).
- Investigación H3 (`cognicode-mcp-server`).
- Investigación H4 (`docs-ingest`/`issues-ingest` con feature flags).
- Actualización ADR-031 (cifra 68 → 75).

---

## PRF-F0-W2 — Certificación de la unidad F0.W2 (Caracterización runtime)

| Campo | Valor |
|---|---|
| ID | `PRF-F0-W2` |
| Hito | F0 — Inventario y baseline |
| Unidad | W2 — Caracterización arranque/persistencia/red |
| Versión CogniCode | 0.97.3 |
| HEAD al cierre | `7cc6a8a7` |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 |

### Estados alcanzados

- [x] **SPECIFIED**: objetivo y criterios de salida definidos en STATE.md
      y ROADMAP.md (W2).
- [x] **IMPLEMENTED**: 7 estudios ejecutados (arranque, strace×3,
      persistencia, red, stdio, señales, secretos).
- [x] **INTEGRATED**: las caracterizaciones se ejecutaron contra los
      binarios reales; los hallazgos H6-H9 son del runtime, no de la
      teoría del código.
- [x] **ACCEPTED**: resultados verificados, hallazgos catalogados,
      criterios de salida cumplidos. Re-validación post-cierre (10
      ejecuciones de `cargo test -p cognicode-cli --bin cogh
      --no-fail-fast`): **10/10 fallos** detectados. Root cause REAL
      (corregido tras validación profunda): GitHub API rate limit
      agotado (`api.github.com/rate_limit` → `remaining: 0`), no race
      condition entre tests. El subproceso `cogh update` falla al
      llamar a `api.github.com/repos/Rubentxu/CogniCode/releases/latest`.
      Baseline 2083/291/7 preservada cuando se re-ejecuta después del
      reset del rate limit. **No es bug del código de CogniCode**;
      es dependencia externa (GitHub API). Ver `evidence/F0-W2-runtime.md`
      Apéndice A y `evidence/H10-correction.md`.
- [ ] **RELEASED**: pendiente (PRF es programa interno; ver D1 de F0.W1).

### Evidencias concretas

| Evidencia | Ubicación |
|---|---|
| Caracterización runtime | `docs/prf/evidence/F0-W2-runtime.md` (310 líneas) |
| Strace logs | `docs/prf/evidence/F0-W2-runs/*_strace.log` (3 archivos, ~5.2 MB) |
| Output --version/--help | `docs/prf/evidence/F0-W2-runs/*_{--version,help,version}.{out,err,time}` (30 archivos) |
| JSON-RPC frames | `docs/prf/evidence/F0-W2-runs/cognicode-mcp_strace.out` (11782 bytes) |
| explorer-api health | `docs/prf/evidence/F0-W2-runs/explorer-api_health.out` |
| Hallazgos | H6-H9 (F0-W2-runtime.md §9) |
| Contradicciones | H7 (manifest 68 vs runtime 75) |

### Decisiones tomadas

- D5: binarios no filtran secretos en logs (verificado).
- D6: H8 (sha256 placeholders) es bloqueante para F1, no para F0.
- D7: corregir cifra 68 → 75 en ADR-031 y plugin.yaml antes de F1.

### Firmas de aprobación

| Rol | Nombre | Estado | Notas |
|---|---|---|---|
| Operador | jcode-orchestrator | APROBADO | Sesión 2026-09-21 |
| Auto-revisión PRF | (programa PRF) | APROBADO | Criterios de salida cumplidos |

### Trabajo pendiente heredado

- F0.W3 (baseline pruebas automatizadas): ejecutado, certificado abajo.
- H6 (separar logs/datos en `cognicode analyze`).
- H7 (corregir cifra 68 → 75 en docs).
- H8 (calcular sha256 reales para 5 plugins).
- H9 (terminación limpia de explorer-api).
- H3, H4 de F0.W1 siguen abiertos.

---

## PRF-F0-W3 — Certificación de la unidad F0.W3 (Baseline pruebas automatizadas)

| Campo | Valor |
|---|---|
| ID | `PRF-F0-W3` |
| Hito | F0 — Inventario y baseline |
| Unidad | W3 — Baseline de pruebas automatizadas |
| Versión CogniCode | 0.97.3 |
| HEAD al cierre | `7cc6a8a7` |
| Operador | jcode-orchestrator |
| Fecha | 2026-09-21 |

### Estados alcanzados

- [x] **SPECIFIED**: objetivo y criterios de salida definidos en STATE.md
      y ROADMAP.md (W3).
- [x] **IMPLEMENTED**: 3 suites canónicas ejecutadas + smoke L3 con 5
      binarios + verificación de causa raíz H10.
- [x] **INTEGRATED**: las pruebas se ejecutaron contra binarios reales
      (sin mocks para los smoke; con `--skip` para 1 test bloqueante
      por dependencia externa documentada).
- [x] **ACCEPTED**: baseline numérica coincide con F0.W1
      (2083/291/7 → 2083/290/7 + 1 skip justificado). Criterios
      de salida cumplidos.
- [ ] **RELEASED**: pendiente (PRF es programa interno).

### Evidencias concretas

| Evidencia | Ubicación |
|---|---|
| Documento fuente | `docs/prf/evidence/F0-W3-baseline.md` (138 líneas) |
| Log core lib | `docs/prf/evidence/F0-W3-runs/cognicode-core-lib.txt` |
| Log cogh sin skip | `docs/prf/evidence/F0-W3-runs/cognicode-cli-cogh-no-update.txt` |
| Log ide adapter | `docs/prf/evidence/F0-W3-runs/cognicode-ide-adapter.txt` |
| Hallazgos | sin cambios respecto a F0.W2 (H6-H9) |
| Causa raíz H10 v3 | `docs/prf/evidence/H10-correction.md` |

### Decisiones tomadas

- D8: ejecutar `--skip test_cogh_update_respects_lockfile` cuando el
  rate limit de GitHub API esté agotado. Documentar la causa externa
  antes de cualquier fix.
- D9: F0.W3 NO mockea GitHub API para ese test (alcance de F0.W3 =
  baseline, no fix). Trabajar el mock en F1.

### Firmas de aprobación

| Rol | Nombre | Estado | Notas |
|---|---|---|---|
| Operador | jcode-orchestrator | APROBADO | Sesión 2026-09-21 |
| Auto-revisión PRF | (programa PRF) | APROBADO | Criterios de salida cumplidos |

### Trabajo pendiente heredado

- F1 (Estabilización): H6, H7, H8, H9, + mock GitHub API.
- H3, H4 de F0.W1 siguen abiertos.

---

## Hito F0 — CERRADO (PRF-F0-W1 + PRF-F0-W2 + PRF-F0-W3)

Tres certificados firmados, hito Inventario y baseline cerrado.
Siguiente hito: F1 (Estabilización).
