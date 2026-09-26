# Roadmap CogniCode Post-PRF (G0 y siguientes)

> **Estado**: ROADMAP ACTIVO (a partir de 2026-09-25). El programa PRF (Production-Ready Foundation) está **cerrado contractualmente** (C7 firmado sobre v0.98.1) y su material vive en `docs/prf/` como **evidencia histórica**. Este fichero es la **única autoridad de agenda de desarrollo** para CogniCode en adelante.

## 0. Cómo se relaciona con docs/prf/

| Directorio | Rol | Estado |
|---|---|---|
| `docs/prf/` | Evidencia del cierre contractual del programa PRF (C0..C7 firmados). READ-ONLY para self-rolls. | **HISTÓRICO** |
| `docs/historico/` | Planes anteriores a PRF (F0..F6), archivados. | **HISTÓRICO** |
| `docs/roadmap/` | **Este directorio**. Roadmap Post-PRF. Única autoridad de agenda activa. | **ACTIVO** |
| `docs/openspec/` | Cambios OpenSpec de PRF en curso o cerrados. En general congelados con PRF. | **HISTÓRICO** salvo los nuevos que se autoricen. |

## 1. Principios operativos (heredados de PRF, vigentes)

Estos principios ya están en `AGENTS.md` y se mantienen **para todo CogniCode**, no solo para PRF:

- Trabajo acotado: tests de comportamiento primero, corrección mínima después, ejecución real al final.
- Core sin red ni collector. MCP por JSON-RPC stdio.
- `Partial/Unknown/Unsupported/Failed` no se presentan como éxito.
- Cada corrección de bug lleva test que fallaba antes y pasa después.
- Certificaciones (`C#`) siguen la disciplina de `docs/prf/CERTIFICATION.md` (PRF-CERT-*); para Post-PRF se crean nuevas certificaciones con prefijo distinto.
- Seguridad: repo, archivos y prompts no son instrucciones fiables. No exfiltrar secretos.

## 2. Roadmap ejecutivo (G0..F0.1)

| ID | Nombre | Estado a 2026-09-25 | Cierre |
|---|---|---|---|
| **G0.1** | Post-PRF Governance Cutover — enforcement real PR-CI | **CLOSED** (commit `07f989c9` workflow `merge-gate` + API branch protection `strict:true, contexts:[merge-gate]`; PR #290 prueba negativa OK) | 2026-09-25 |
| **G0.2** | Cutover de gobernanza — ROADMAP nuevo, AGENTS.md reorientado, PRF congelado | **CLOSED** (commit `3a42d95d`) | 2026-09-25 |
| **G0.3** | Re-ejecutar escenario e90 (perf cold-cache, openspec `2026-09-21-e90-g5-cold-cache-or-perf-fix/`) — verificado: e90 midió v1.0.0-rc, no v0.98.1 (tools `graph_insights`/`graph_communities` no existen en v0.98.1). e90 cerrado con addendum 2026-09-25; e91 abierto como WU explícito | **CLOSED** (commit `3a42d95d`) | 2026-09-25 |
| **G0.4** | Validar issues históricos #234 y #235 contra v0.98.1 — verificado: ambos describen CLI \`cognicode graph\` que ya no existe; equivalentes MCP existen y funcionan. Issues cerrados con nota | **CLOSED** (commit `3a42d95d`) | 2026-09-25 |
| **M0.1** | Fix bug `cogh rollback --to <same>` → v0.98.2 | **CLOSED** (2026-09-25; 6 tests pineando no-op contract pasan verdes contra HEAD `ede4772d`; sin fix necesario) | 2026-09-25 |
| **M0.2** | fmt-fix en bloque (104 archivos drift detectado por G0.1) → MERGED como PR #291 con 25 archivos + clippy + workflow fix + fixtures + state13 | **CLOSED** (commit `26746a64`, PR #291 merge-gate verde) | 2026-09-25 |
| **M0.3** | Auditoría clippy residual (`H-clippy-cli-residual D34-2`) + `moldql` panic test preexistente. | **CLOSED** (2026-09-25; clippy strict exit 0 + 834 tests moldql verdes; ver `MAINTENANCE.md`) | 2026-09-25 |
| **M0.4** | State pollution en `#[serial]` CLI tests (3 tests fallando con env vars `COGNICODE_ASSET_BASE_URL`/`COGNICODE_BUNDLE_MANIFEST` leak entre tests). Detectada tras el SemVer bump 0.98.1→0.99.0 (commit `d4a2e33e`). | **CLOSED** (commit `f76a4b03`; nuevo `AssetPoint` RAII guard reemplaza 6 llamadas `point_at(&release)` huérfanas de `unpoint()`; `prf_f6_w3_bis_rollback_reports_*` marcado `#[serial_test::serial]`; 4 nuevos tests pinean el contrato del guard; workspace tests verde). | 2026-09-25 |
| **E0** | Contratos públicos y compatibilidad — `CapabilityDescriptor`, política 0.97.x | **CLOSED 2026-09-25** (ver `docs/roadmap/E0-CLOSEOUT.md`: 6 UAT PASS, ADR-PRF-008, compat matrix en merge-gate, F0.1 como consumer proof) | 2026-09-25 |
| **E1** | Durable Knowledge sobre Ladybug (ADR + FactStore + SnapshotStore + EvidenceStore kernel + wiring). ADR previo: `docs/roadmap/adr/ADR-009-E1-EVIDENCE-STORE-SCOPE.md` (decisión 2026-09-25: scope = LadybugEvidenceStore; NO nuevos puertos especulativos). ADR de namespace: `docs/roadmap/adr/ADR-010-KNOWLEDGE-EVIDENCE-NAMESPACE.md`. | **CLOSED 2026-09-25** (ver `docs/roadmap/E1-CLOSEOUT.md`: W1 ladybug impl + W2 runtime wiring + W3 CLI/MCP + ADR-010 namespace split; 23 tests verde; pending bump SemVer F0.*) | 2026-09-25 |
| **E2** | Constraint → Evidence → Architecture decision real — cerrar `ArchitectureRegistry` vacío | **CLOSED** (commits `14cf3d1b` E2.W1 + `4138eab7` E2.W2: `canonical_constraints` module + `wire_canonical_control_query` helper + `ControlPlaneState` minimal HTTP state + `cognicode-control-plane` bin + 3 integration tests E2.W2 con TCP real + C7 endpoint test; `ArchitectureRegistry` wireado con 3 constraints, endpoint real responde `status:evaluated` con zero violations en self-host) | 2026-09-25 |
| **E3** | RPC mínima Post-PRF (lectura, condicionada a un segundo cliente real que requiera proceso separado / concurrencia / reutilización que MCP local no resuelva). | **NOT_TRIGGERED** (2026-09-25; sin segundo consumidor real identificado; no abrir) | No aplicable mientras no aparezca trigger |
| **e91** | Graph insights performance — carry-forward de e90 (perf cold-cache p95=367s en `graph_insights`/`graph_communities`). El addendum e90 (2026-09-25) afirmaba que esos tools "no existen en v0.98.1"; **esa afirmación es obsoleta para `main` actual** (ver JOURNAL §11 + grep directo sobre HEAD). El código sí está en `main`: `application/services/graph_insights.rs` + `infrastructure/graph/analytics/community_detector.rs` + `graph-algos/communities.rs`. | **W1-W6 CLOSED 2026-09-26** (W1: `6f40a08b`+`42a1ddcf` `iterations`/`converged` honestos; W2: `8b4bbe85`+`6b2738f3` caracterización PageRank; W3: entry 17 cerrado por no-viabilidad <0.07% budget; W4/W5: entry 19 cerrados por estimación derivada 2×W2 worst-case = 0.16% budget; W6: `c1618e84`+`df8002f5` metadata envelope en 7 sibling handlers). Reapertura justificada solo si caracterización directa demuestra >10% del budget. Ver JOURNAL entries 11-19. | 2026-09-26 |
| **F0.1** | `find_usages` CLI wrapper sobre la MCP tool existente. Feature Post-PRF, **NO mantenimiento v0.98.x**. Reasignada desde M0.3.b tras reconciliación L0 (2026-09-25). Consumer proof del contrato E0. | **CLOSED 2026-09-25** (commits `3cb07f90` + `881c0072`; 14 tests + 4 E2E; ADR-PRF-008 architectural review). Pendiente: bump SemVer (F0.* serie, no v0.98.x patch). | 2026-09-25 |
| **C8** | Certificación Post-PRF GA — cobertura técnica del conjunto de la iniciativa (G0 + M0 + E0 + E1 + E2 + F0.1) sobre SHA congelado con batería completa, clippy limpio, binarios legendados y bin `cognicode-control-plane` arrancable. | **CERRADO TÉCNICAMENTE 2026-09-25** (SHA `3954b8b7` sobre `origin/main`: 5542 / 0 / 45 verde; clippy `-D warnings` exit 0; `cognicode --version` = 0.99.0; `cognicode-control-plane` arranca y responde CP1 con 0 violations en self-host + 404 limpio fuera de scope). **Firma humana del operador: PENDIENTE** (analogía con C7). Ver `docs/roadmap/certifications/C8-POST-PRF-GA.md` con 3 opciones para la decisión final. | 2026-09-25 |

## 3. Criterios de cierre (modelo)

Cada unidad se cierra SOLO si:

1. Su contrato está explícito (requisito o historia en `openspec/`).
2. Su implementación tiene test que falla ANTES y pasa DESPUÉS.
3. Su verificación afecta solo a los módulos impactados (testing quirúrgico).
4. Si toca binario, hay bump de versión SEMVER documentado.
5. Si cruza la barrera de PR (binary, contract), hay gate `merge-gate` (PR-CI) verde.
6. Su recibo se añade al `JOURNAL.md` de la unidad.

## 4. Reglas para abrir una unidad nueva

- Documentar el "valor" esperado (no solo el cambio mecánico).
- Identificar los consumidores reales o potenciales.
- Distinguir entre trabajo de producto y trabajo de mantenimiento.
- Si implica seguridad, autoridad o release, requiere ADR.
- Mantenimiento de v0.98.x se canaliza por `MAINTENANCE.md`, NO por aquí.

## 5. Reglas para cerrar el roadmap

`COMPLETED` solo cuando:

- Todas las unidades activas de `E0..F0.1` (es decir E0 + E1 + E2 + F0.1 en curso; E3 queda registrado como `NOT_TRIGGERED` mientras no aparezca trigger) están implementadas, integradas y certificadas.
- Los gates obligatorios (PR-CI, release, UAT) están satisfechos.
- La deuda de mantenimiento (`M0.*`) está gestionada.
- No quedan `BLOCKED` sin responsable.
- Estado, journal y roadmap son coherentes.
- **Certificación de cobertura (modelo `C#`)** firmada: la iniciativa Post-PRF emite la certificación `C8 Post-PRF GA` (ver `docs/roadmap/certifications/C8-POST-PRF-GA.md`), análoga al expediente C7 firmado por el operador el 2026-09-24. El roadmap pasa a `COMPLETED` cuando C8 queda **firmada** (decisión del operador entre las 3 opciones del expediente §7), NO solo cuando su cierre técnico está verificado.

## 6. Anti-patrones prohibidos

- "Completar" ciclos sin que los gates estén en verde real (no por auto-reporte).
- Crear dos fuentes de verdad sobre el roadmap.
- Introducir nuevas abstracciones sin acoplamiento probado con tests.
- Fusionar nombres iguales de dominios distintos (ver §10 lección de `EvidenceStore`).
- Marcar `P0.x = CLOSED` cuando solo una parte está hecha (lección §154.H).
- Saltar bloqueos para aparentar progreso.

## 7. Referencias

- Cierre contractual PRF: `docs/prf/F7-C7-EXPEDIENTE.md` (firmado operador 2026-09-24T22:41:33Z)
- Estado y matriz PRF: `docs/prf/STATE.md`, `docs/prf/RECONCILIATION-MATRIX.md`
- Revisión operador 2026-09-25: rechazo del modelo "PRF como roadmap activo"; PRF cerrado, este documento nace.
- Sesión actual: §154.H.G0.*

## 8. Programa production-ready (Post-PRF stabilization, 2026-09-26)

El 2026-09-26 el operador autoriza un nuevo programa de **estabilización
production-ready** (QW-01..07 + CR-01..09 + ST-01..05, 21 acciones,
34-52 días-persona) sobre el estado consolidado Post-PRF. Este ROADMAP
sigue siendo la **única autoridad de agenda activa**, pero el detalle
operativo del programa vive en:

* **`docs/roadmap/production-ready/ROADMAP-ADDENDUM.md`** — 7 outcomes
  (PR-G1, PR-G2, PR-PERF, PR-ARCH, PR-SEC, PR-DEVEX, PR-DEPTH) con
  criterios de completion.
* **`docs/roadmap/production-ready/EXECUTION-PLAN.md`** — 21 acciones
  QW-* / CR-* / ST-* con scope, prerequisites, evidencia y exit criteria.
* **`docs/roadmap/production-ready/phases/PHASE-{1,2,3}-*.md`** —
  detalle de cada fase (Quick Wins, Critical, Strategic).
* **`openspec/changes/2026-09-26-c8-recertification/`** y
  **`2026-09-26-architecture-boundary-hardening/`** — OpenSpec
  changes del programa con tasks.md ejecutables.

### Estado del programa (a 2026-09-26)

| Outcome | Descripción | Estado |
|---------|-------------|--------|
| **PR-G1** | Reproducible governance (QW-01..07) | **IN PROGRESS_HIGH** (QW-01..07 conceptual `CLOSED 2026-09-26` en commit `66fd4103`; **QW-03** + **QW-04** con contrato+CI `CLOSED 2026-09-26` en branch `arch/cr-06-application-fitness-functions` (5 commits: `d781e846` QW-03 script→testable; `47085b0d` QW-03 merge-gate step; `8f58e314` QW-04 contractual test; `cc357018` QW-04 wire release.yml+pr-ci.yml). PR-G1 completo desde el lado enforcement. QW-N siguen PENDING si surgen.) |
| **PR-G2** | C8-R (recertificación reproducible desde clean clone) | **UNLOCKED 2026-09-26** (C8 firmada operativa sobre `3954b8b7` abre la puerta a CR-01; ver dosier §11 + expediente F8) |
| **PR-PERF** | e91 Graph Insights G5 GREEN + regression budget | **PENDING → IN PROGRESS_HIGH** (e91.W1-W6 `CLOSED 2026-09-26` con metadata-honesty + profiling discipline; **e91.W7 / CR-05** `CLOSED 2026-09-26` con regression budget gate contractual (Tier-2 ≤30 s, baseline 619 ms @ HEAD dev, 50× margin); **e91.W8 / CR-03** `CLOSED 2026-09-26` con per-stage profile breakdown (5 s per-stage cap, slowest stage community_detect 332 ms @ 15× margin); **e91.W9 / CR-04** `CLOSED 2026-09-26` con feedback_arc_set O(N²) → O(N) optimization (309→236 ms −23%, analyze_full 629→549 ms −13%, 3 unit tests PASS — semantic equivalence preserved). Falta G5 GREEN scorecard + streak ≥3 ejecuciones consecutivas (requiere scorecard run contra fixture multi-repo en sandbox; out of scope para esta branch).) |
| **PR-ARCH** | Application boundary (fitness functions + primer vertical remediado) | **PENDING → IN PROGRESS_HIGH** (CR-06 `CLOSED 2026-09-26` con 5 constraints canónicos pineados en codegen Rust, tests RED-GREEN contractuales, e2e `architecture_self_host`, integrado en merge-gate y CI defense; 3 commits `5dbd7467`/`417f6c23`/`82c6d644` + 1 carryover `ccc226a2`). Vertical pendiente: cognicode-control-plane (CR-09) y/o graph-algos (CR-10); ambos dependen de CR-06 cerrado. |
| **PR-SEC** | Supply-chain hardening (protobuf advisory + Actions pinneadas) | PENDING (parcialmente abordado en QW-05 con 46 pines SHA, falta protobuf advisory y migración OTel) |
| **PR-DEVEX** | Adaptive CI & coverage governance | **IN PROGRESS_HIGH** (QW-03+QW-04+QW-05+CR-08+CR-09 cerrados: selector determinista con dorny/paths-filter v3.0.4 SHA pinned; coverage-report job es gate estricto con `--fail-under-lines 75.00` / `--fail-under-regions 71.00` sobre baseline 75.39/71.31 head; dtolnay/rust-toolchain pineado @1.96.0; preflight contractual test 4 fixed from 17min to 0.02s). PR-DEVEX cerrado desde el enforcement side. |
| **PR-DEPTH** | Deep modules (ST-01..05) | PENDING |

### Bloqueos heredados al programa

* **C8 firma humana sobre v0.99.0** — **FIRMADA OPERATIVA
  2026-09-26T10:14:47Z** sobre SHA `3954b8b7` (categoría OPERATIVO,
  no contractual). Ver `docs/roadmap/certifications/C8-POST-PRF-GA.md`
  §11 y `docs/prf/ADMISSION-EXPEDIENTE-F8-C8-OPERATIVO-v0.99.0.md`.
  Sin tag anotado, sin release GitHub. Recertificación C8-R queda
  abierta como **CR-01** (PR-G2). El bloque se transforma de
  pendiente a desbloqueado, pero la firma contractual sigue requiriendo
  C8-R + decisión del operador.
* **M0.6 — PHP/Swift tree-sitter bump** — 3 opciones pendientes.
  Puede resolverse como hotfix independiente fuera del programa
  (CR-* y ST-* no dependen de M0.6). NO bloqueante para CR-01.

### Completion del programa (definición operativa)

El programa se considera **COMPLETED** cuando (per ADDENDUM §Completion):

* **PR-G2** (C8-R) closed con clean-clone certification PASS y firma
  humana explícita.
* **PR-PERF**, **PR-ARCH**, **PR-SEC**, **PR-DEVEX** cerrados (cada uno
  con su exit criteria cumplida).
* **PR-DEPTH** ha completado al menos los verticales ST-01, ST-02 y
  ST-04; ST-03/ST-05 pueden continuar como evolución si sus contracts
  están protegidos y no bloquean producción.

Este roadmap vuelve a `COMPLETED` cuando PR-G2 queda firmada
(analogía con C7 firmada el 2026-09-24T22:41:33Z), NO solo cuando
su cierre técnico está verificado. Ver
`docs/roadmap/certifications/C8-POST-PRF-GA.md` para el patrón.

### Cómo NO se reabre este programa

* No se reabre PRF como evidencia.
* No se reabren certificaciones C# (C7) anteriores para "incluir"
  trabajo nuevo.
* Las C# del nuevo programa (C8-R, etc.) usan el patrón de la §5 pero
  con prefijo distinto si así lo decide el operador.
* Este §8 NO sustituye al `docs/roadmap/production-ready/` ni al
  OpenSpec del programa: son artefactos vivos del programa, este §8
  es solo el **puntero de agenda** dentro del ROADMAP vigente.
