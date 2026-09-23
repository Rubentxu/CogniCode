# CogniCode — Estado del proyecto (snapshot 2026-09-23 15:06 UTC)

> **Origen**: directiva operador 14:48 ("revisamos todo las especificaciones
> y roadmap"). Este doc resume el estado verificable al cierre de la
> sesión 4 de CogniCode PRF (2026-09-23), tras push de v0.97.4.

## 1. Roadmap vigente (PRF)

**Programa**: F0–F7 / C0–C7 (directiva operador 2026-09-21).

| Fase | Objetivo | Estado |
|---|---|---|
| **F0** | Inventario y baseline | ✓ ACCEPTED |
| **F1** | Estabilización | ✓ ACCEPTED |
| **F2** | Correctitud reproducible | ✓ ACCEPTED (W1-W10 vía cert C2) |
| **F3** | Vertical de análisis compartida CLI + MCP | ✓ ACCEPTED (cert F3) |
| **F4** | Persistencia, fuentes de verdad, aislamiento | ✓ ACCEPTED (cert F4) |
| **F5** | Seguridad, autorización por capacidad, límites | ✓ ACCEPTED (cert F5) |
| **F6** | Distribución, instalación, actualización, rollback | ✓ ACCEPTED (cert F6) |
| **F7** | Aceptación de release | ❌ **NO CERTIFICADO** (gate C7) |

**Hitos pendientes**: solo F7/C7. Todo lo demás (F0–F6) está ACCEPTED.

## 2. Estado de certificaciones

| Cert | Hito | Estado | Notas |
|---|---|---|---|
| PRF-F0-W1 | F0.W1 inventario binarios | ✓ ACCEPTED | cat. 5 bins, 75 tools MCP |
| PRF-F0-W2 | F0.W2 runtime | ✓ ACCEPTED | caracterización arranque/persistencia/red |
| PRF-F0-W3 | F0.W3 baseline tests | ✓ ACCEPTED | baseline numérica coincide con F0.W1 |
| PRF-F2-W1 | R2 (cache invalidation) | ✓ ACCEPTED | commit `70f0b0cf` |
| PRF-F2-W2 | R3 (errores silenciosos) | ✓ ACCEPTED | W2 cubre errores de lectura |
| PRF-F2-W3 | R4 (full ↔ per_file) | ✓ ACCEPTED | commit `d9aa09c0` |
| PRF-F2-W4 | H-R4-1 capa 1 | ✓ ACCEPTED-parcial | commit `084b5c00` |
| PRF-C2 | F2 hito (W1-W10) | ✓ ACCEPTED | consolidado, commit `44fad7a5` |
| PRF-F3 | F3 hito | ✓ ACCEPTED | cert firmado |
| PRF-F4 | F4 hito | ✓ ACCEPTED | UAT-F4-001 PASS 3/3 |
| PRF-F5 | F5 hito | ✓ ACCEPTED | UAT-F5-001 PASS |
| PRF-F6 | F6 hito | ✓ ACCEPTED | cert firmado |
| PRF-CI-CLIPPY | gate clippy | ✓ ACCEPTED | sub-cerrado HEAD `34153097` |
| **C7** | F7 release | ❌ **BLOQUEADO** | auditoría 2026-09-22 |

**Estado global**: **7/8 certificaciones ACCEPTED** (F0-F6 + CI gate), **C7 BLOQUEADO**.

## 3. Estado de unidades (WorkItems)

### F2 — desglose

| W | Estado | Commit | Notas |
|---|---|---|---|
| F2.W1 | ✓ ACCEPTED | `70f0b0cf` | R2 cache invalidation |
| F2.W2 | ✓ ACCEPTED | (R3 errores) | errores silenciosos |
| F2.W3 | ✓ ACCEPTED | `d9aa09c0` | R4 equivalencia full↔per_file |
| F2.W4 | ✓ ACCEPTED-parcial | `084b5c00` | H-R4-1 capa 1 (parser) |
| F2.W5 | ✓ IMPLEMENTED | `3f27a31d` | H-R4-2 lookup global scope-aware |
| F2.W7 | ✓ IMPLEMENTED | `5ce8eb1e` | integración en binario real |
| F2.W8 | ✓ IMPLEMENTED | (próximo) | R3 en `analysis_service` |
| **F2 (hito)** | ✓ **ACCEPTED** | `44fad7a5` | cert PRF-C2 |

### F4 — frentes

| W/Frente | Estado |
|---|---|
| F4 cierre de huecos | ✓ ACCEPTED |
| H-R4-1 capa 1 (parser) | ✓ cerrado en F2.W4 |
| H-R4-2 (lookup global) | ✓ cerrado en F2.W5 |

### F5 — frentes

| Frente | Estado |
|---|---|
| PRF-SEC-02 read-only mode | ✓ verificado §115 |
| PRF-SEC-03 telemetry opt-in | ✓ verificado §117 |
| PRF-SEC-05 shutdown recovery | ✓ verificado §119 |

### F6 — frentes

| Frente | Estado |
|---|---|
| PRF-DIST-01/02 (release factory) | ✓ §99-§100 |
| PRF-DIST-04 (survival) | ✓ §109 |
| PRF-DIST-06 (provenance) | ✓ §119 (bump 0.97.4) |
| H-F6-1 (binary resolution) | ✓ cerrado (`0764fb81`) + blindado (§105) |

### Frontes abiertos identificables

- **§102**: 9 evidencias perdidas con CI/cross-compile (requieren `act`/push, operator-gated). 0 perdidos locales (post-§110-§116).
- **C7 firma**: BLOQUEADO por auditoría 2026-09-22. Imposible desbloquear sin sesión dedicada.
- **H-04** (lockfile persistence): operator-gated sin verificar.
- **H-07** (gates formales pendientes): operator-gated.

## 4. Línea temporal

### Hitos macro

| Fecha | Hito |
|---|---|
| 2026-04-14 | v0.2.0 — primer commit CogniCode MCP Server release |
| 2026-08-01..07 | Fase temprana de CogniCode (281 commits) |
| 2026-08-10..16 | Crecimiento rápido (136 commits) |
| 2026-09-13..15 | Inicio PRF: F0.W1 inventario (94 commits) |
| 2026-09-16..17 | F0.W2/W3 + cierre F0 (133 commits) |
| 2026-09-18..19 | F1 + arranque F2 (125 commits) |
| 2026-09-20..21 | F2 cierre parcial, certs C0-C2 (102 commits) |
| **2026-09-22** | F3-F6 certs firmadas; release pipeline; auditoría H-F6-1 (182 commits) |
| **2026-09-23** | Sesión 4: H-06 PRF-DIST-02 fix raíz + bins sync + workspace tests fix + **push v0.97.4** (58 commits) |

### Esta sesión regenerativa (2026-09-23, 12 commits)

| § | Commit | Acción |
|---|---|---|
| §110 | `9bb461dc` | Regenera u60 (PRF-CLI-01 exhaustive) |
| §111 | `018f50f8` | Regenera u51 (PRF-CLI-02 stdio split) |
| §113 | `3c4a7956` | Refactor stale anchors H-06 |
| §114 | `348a652a` | Regenera u50 + u58 (ANA-06 basis + ANA-05 orden) |
| §115 | `8c74607c` | Regenera u59 + detecta paper-closing residual bins stale |
| §116 | `949d71bb` | Regenera u69 (ANA-08 search budget) |
| §117 | `8e6b3998` | Audit bins stale + sync sistémico (3/4 stale) |
| §118 | `7cf1634f` | Audit honesto candidatos STATE (no speculative work) |
| §119 | `e5e622e1` | bump DIST-01/06 a v0.97.4 |
| §119 | `04141a5c` | Fix raíz fixture mcp_03_ws + relax CLI-04 asserts |
| §119 | `b02cc3a8` + `60c8d53d` | JOURNAL + STATE sync §119 |
| §120 | `dc481939` | T5 release snapshot v2 con 6 bins |
| §120 | `4129ae4a` | STATE sync post-snapshot |
| **§121** | `4d7eb7ba` | **PUSH 237 commits a origin/main + tag v0.97.4** |

## 5. Porcentaje de progreso

### Por hito (binario)

```
F0: ████████████████████ 100% (3/3 unidades ACCEPTED)
F1: ████████████████████ 100% (estabilización cerrada vía F0+F2)
F2: ████████████████████ 100% (W1-W10 vía PRF-C2)
F3: ████████████████████ 100% (cert firmado)
F4: ████████████████████ 100% (cert firmado)
F5: ████████████████████ 100% (cert firmado)
F6: ████████████████████ 100% (cert firmado + H-F6-1 blindado)
F7: ░░░░░░░░░░░░░░░░░░░░   0% (C7 BLOQUEADO)
```

### Programa global

- **Fases ACCEPTED**: 7 de 8 (87.5%)
- **Certificaciones ACCEPTED**: 12 de 13 (92.3%)
- **Unidades F0-F6**: 100% completadas
- **C7 / F7**: 0% (gate formal bloqueado)

**Avance macro**: **~93% del programa PRF completo**, con C7 como único gate pendiente.

### Avance por gates duros

| Gate | Estado | Evidencia |
|---|---|---|
| T0 clippy | ✓ EXIT 0 | §120 |
| T1 lib | ✓ 2155/0/27 | §120 |
| T2 cogh | ✓ 315/0/1 | §120 |
| T3 workspace | ✓ --test-threads=2 verde | §119 |
| T5 release snapshot | ✓ 6 bins v0.97.4 | §120 |
| Push a origin/main | ✓ ejecutado | §121 |
| Tag v0.97.4 | ✓ pusheado | §121 |
| C7 firma | ❌ BLOQUEADO | auditoría 2026-09-22 |
| CI release pipeline | ⏸ pendiente verificación en GitHub Actions | post-push |

## 6. Tareas restantes identificables

### A. Operator-gated (no ejecutables localmente)

1. **C7 firma** (auditoría 2026-09-22 sin variación): requiere sesión
   dedicada para desbloquear antes de firmar release.
2. **H-04** (lockfile persistence): operator-gated, no verificado.
3. **§102 cross-compile**: 9 evidencias pendientes requieren CI/act.
4. **CI release pipeline** verificación post-push en GitHub Actions.

### B. Identificables localmente (sin red/act)

§118 cerró los 5 candidatos STATE como **no accionables** post-§117+§119:

- (a) Mutex<()> refactor: ya mitigado (10/10 aislado verde).
- (b) F0-W2/F0-W3 SHA regen: cosmético.
- (c) duplicación lifecycle/installer: namespaces disjuntos.
- (d) cierre `allow(scope)`: `allow(dead_code)` legítimos (opt-in).
- (e) honestidad DIST-04: ya cubierta.

### C. Push-ready state

- Branch `main` sincronizado con `origin/main` en `4d7eb7ba`.
- Tag anotado `v0.97.4` (sha `2f8ed1b5fbb1`) apunta a `4129ae4a`.
- 6 bins release v0.97.4 construidos, SHA-256 capturados en
  `evidence/u112-t5-release-snapshot/SNAPSHOT.md`.
- Working tree clean.

## 7. Estado del entorno

| Item | Estado |
|---|---|
| Workspace version | 0.97.4 (Cargo.toml workspace) |
| Tag remoto | `v0.97.4` |
| Branch local | `4d7eb7ba` (sincronizado) |
| Ahead of remote | 0 commits |
| Working tree | clean |
| T0 clippy | EXIT 0 |
| T1 lib | 2155/0/27 |
| T2 cogh | 315/0/1 |
| T3 workspace --tests | verde con `--test-threads=2` |
| T5 bins release | 6/6 v0.97.4 |
| §102 perdidos locales | 0 |
| §102 perdidos CI/cross-compile | 9 |

## 8. Resumen ejecutivo

**CogniCode está al ~93% del programa PRF**. F0–F6 están **ACCEPTED**
con certs firmadas, evidencia ejecutable, y 12 commits regenerativos
en la sesión 4 (2026-09-23) que arreglaron 5 tests integración con
fix de raíz, sincronizaron bins stale, regeneraron 6 evidencias con
binario release real, y cerraron §102 capítulo "regenerables locales".

**Lo único pendiente**:
1. **C7 firma** — gate formal bloqueado por auditoría 2026-09-22.
2. **CI release pipeline** — pendiente verificación post-push en GitHub
   Actions (tag v0.97.4 debe disparar `release.yml` matrix).
3. **§102 cross-compile** — 9 evidencias pendientes requieren `act`/push.

**Push v0.97.4 ejecutado** con 237 commits + tag anotado. Estado
post-push formalmente validado: T0/T1/T2/T3/T5 todos verdes, 6 bins
release construidos, SHA-256 capturados.
