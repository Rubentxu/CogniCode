# Goal — v1015 Consolidation & Promote

> **Estado**: ACTIVE 2026-10-10. Goal autorizado por el operador tras la
> auditoría técnica senior (rama `integrate/v1015` HEAD `3109f7f2`).
> **Opción C** del plan de repriorización: Tier 0 (quick wins) + Tier 1
> (connascence cleanup) + promoción a `main`.

## 0. Cómo se relaciona con el resto

| Artefacto | Rol |
|---|---|
| `docs/roadmap/ROADMAP.md` §9.1-9.4 | agenda activa. Cita el producer assurance-evidence como pendiente, no contradice este goal. |
| `docs/roadmap/CURRENT.md` | snapshot. **STALE** (SHA `15b5c68c9fde` en doc vs `3109f7f2` real; "80 commits ahead" vs 5 reales; "5650 tests" vs 4858). Regenerar en T0-4. |
| `odd/tasks/assurance-evidence-v1-exporter.md` | feature doc del trabajo YA COMMITEADO (5 commits en `integrate/v1015`). Este goal lo da por hecho y se centra en el siguiente movimiento. |
| `docs/prf/STATE.md`, `docs/prf/RECONCILIATION-MATRIX.md` | evidencia histórica. No se reabre. |

## 1. Objetivo

Cerrar la deuda de connascence detectada en la auditoría técnica senior
y promover `integrate/v1015` → `main` con `v0.101.10`, con el producer
`assurance-evidence/v1` ya integrado y la base hexagonal saneada para
los siguientes work units.

## 2. Problema

La auditoría (4 agentes paralelos: arquitectura, connascence, tests,
seams) identificó:

* **4 enums `SymbolKind` paralelos** en 4 archivos — CoN HIGH, shotgun
  surgery garantizada en cada variante nueva.
* **3 enums `RiskLevel`** con representaciones distintas (4 vs 5
  buckets, `serde` con discriminantes `= 0` vs snake_case vs lowercase).
* **2 `VerificationStatus` + 2 `ConfidenceTier`** idénticos en
  archivos distintos.
* **154 `println!` en `cognicode-core/src/interface/cli/commands.rs`** —
  el presenter vive en la librería, viola la separación
  library/adapter.
* **1 `println!` en `cognicode-mcp/src/mcp_client.rs:140`** — única
  violación REAL de AGENTS.md §17 ("logs nunca en stdout").
* **10 `unimplemented!()` en `rmcp_adapter.rs:2558-2701`** — tests
  `#[ignore]` con motivo "requires rmcp internals" que nunca correrán
  (rmcp's API interno es `pub(crate)`).
* **5 `handle_*` 80% idénticos** en `file_ops_handlers.rs` esperando
  una macro.
* **2 enums `EvidenceStore` + 2 enums `CallGraphStore`** con traits
  duplicados — AGENTS.md §20 lo nombra como caso de estudio.

Más la **contradicción documental**:
`CURRENT.md` y `ROADMAP §9.1` describen un SHA y un contador de
commits que no coinciden con `git rev-parse HEAD` ni con
`git log main..integrate/v1015`. Esto bloquea el ratchet
`test_roadmap_version_ratchet.py` y desinforma al operador.

## 3. Por qué ahora

El producer `assurance-evidence/v1` está listo y PR #345 está
MERGEABLE. Pero la rama `integrate/v1015` no puede promover a `main`
sin que el merge-gate (pipelinek) cierre el gap de la auditoría
documental. Hacer el cleanup de connascence en la misma serie v0.101.x
es preferible a:

* (a) promover con la deuda → más PR en serie v0.102.x cerrando
  enums, scope-creep visible.
* (b) cerrar enums en serie v0.102.x → un minor semver (v0.102.0) por
  lo que realmente es hygiene.

Por eso este goal los agrupa en v0.101.10 patch.

## 4. Scope

### 4.1 In-scope

* Tier 0: 5 quick wins sobre `integrate/v1015`
* Tier 1: consolidación de 4 enums + mover presenter
* Regenerar `docs/roadmap/CURRENT.md` con SHA real
* Disparar merge-gate sobre la rama consolidada
* Promover PR #345 → `main`
* Bump `v0.101.9 → v0.101.10` y tag anotado

### 4.2 Out-of-scope (queda para work units futuros)

* Consolidação de `EvidenceStore` legacy + kernel (Tier 2 — 2-4 semanas)
* Consolidação de `CallGraphStore` ports + traits (Tier 2)
* Extraer Tarjan SCC a port trait (cierra leak domain→petgraph)
* Reemplazar 80-arm match con HashMap (Tier 2)
* Implementar 6 ports "Phase 1 stub" de `LadybugStore` (Tier 3)
* `cognicode-core-mock` orphan crate (decidir: integrar o eliminar)
* PR-ARCH CR-A (control-plane) y CR-B (graph-algos) (Tier 3)
* PR-DEPTH ST-01..05 cierre formal (Tier 3)
* A-015 onboarding gate contractual (Tier 3)
* CR-01 firma C8-R contractual (operator-gated, no avanza sin acción)

## 5. Constraints

* **Sin breaking change público**: el contrato de `assurance-evidence/v1`
  se mantiene byte-identical para el consumer pipelinek-assurance. El
  SHA-256 del golden export (`45fa782147e2fc25ba6cc7e564ccaf326aae8ecaaef7ae37104133769405ee35`)
  no cambia.
* **TDD red → green → refactor**: cada item RED primero.
* **Sin nuevos `unimplemented!()`** en código que ya está testeado.
* **`cargo fmt --check` + `cargo clippy -D warnings`** verde después
  de cada WU commit.
* **No se reabre PRF** ni se justifican items contra su roadmap.
* **Cada commit con tests y docs** per AGENTS.md.
* **Work-unit commits** (no se acumula la rama entera para un solo PR;
  cada WU cierra con su commit + recibo).

## 6. Work items

> Convencion: `T0-*` = Tier 0 (quick wins), `T1-*` = Tier 1
> (connascence cleanup), `TF-*` = Tier final (promote + tag).

### Tier 0 — Quick wins (1-2 días)

| ID | Tarea | Evidencia antes | Verificación |
|----|-------|-----------------|--------------|
| **T0-1** | ~~Cambiar `println!` a `eprintln!` en `cognicode-mcp/src/mcp_client.rs:140`~~ | **REJECTED 2026-10-10** — falso positivo del audit. `mcp_client.rs` es el binario `mcp-client` (CLI test client), header del fichero: `//! CogniCode MCP Client — E2E test client using rmcp SDK`. El `println!("{}", result)` en :140 es la salida intencional de un CLI client (con comentario `// Print result to stdout`); AGENTS.md §17 ("MCP JSON-RPC por stdout, observabilidad por stderr") aplica al **server** MCP, no al client. La regla unix "stdout = data, stderr = logs" se respeta: el bin imprime su resultado a stdout y los diagnósticos a `eprintln!` (verificado: 7 `eprintln!` en el mismo fichero, todos los de error/warn). No se modifica. | n/a |
| **T0-2** | Eliminar `assert!(true)` en `cognicode-core/src/infrastructure/refactor/change_signature_strategy.rs:999` y `_suppress_unused_with_declared_digest` en `cognicode-core/src/assurance_export/workspace_extractor.rs:488-491` | los 2 patrones existen | `grep` retorna 0 para ambos; `cargo test -p cognicode-core --lib` verde |
| **T0-3** | Eliminar 10 `unimplemented!()` en `cognicode-core/src/interface/mcp/rmcp_adapter.rs:2558-2701` (todos con `#[ignore = "requires rmcp internals"]`) | `grep -c "unimplemented!" rmcp_adapter.rs` ≥ 10 en esa sección | `grep` retorna 0 en ese rango; el test que cite `requires rmcp internals` se elimina (no se reescribe) |
| **T0-4** | Regenerar `docs/roadmap/CURRENT.md` con `git rev-parse HEAD` real, `git log main..integrate/v1015 --oneline` real, `cargo test --workspace` real | doc actual con SHA `15b5c68c9fde` y "80 commits ahead" | ratchet `test_roadmap_version_ratchet.py` pasa |
| **T0-5** | Disparar merge-gate sobre el SHA actual (push a un commit vacío o rebase para que el bot lo recoja) | `mergeStateStatus: BLOCKED` en PR #345, sin checks reportados | checks reportados en PR; `merge-gate` SUCCESS o BLOCKED por causa identificable |

### Tier 1 — Connascence cleanup (1-2 semanas)

| ID | Tarea | Evidencia antes | Verificación |
|----|-------|-----------------|--------------|
| **T1-1** | Consolidar 4 enums `SymbolKind` en uno canónico en `domain/value_objects/symbol_kind.rs` con `Other(String)`; agregar `From` impls en los 3 wrappers restantes | 4 archivos declaran `SymbolKind` | `grep -rn "pub enum SymbolKind" crates/` retorna 1; suite de tests que ejercite los 4 paths con fixture canónico |
| **T1-2** | Consolidar 3 enums `RiskLevel` en uno canónico | 3 archivos declaran `RiskLevel` con representaciones distintas | `grep -rn "pub enum RiskLevel" crates/` retorna 1; `cargo test --workspace` verde; `serde`/`Rename` canónico |
| **T1-3** | Consolidar 2 enums `VerificationStatus` (en `application/dto/file_ops.rs:230` y `interface/mcp/schemas.rs:1979`) | 2 archivos declaran `VerificationStatus` idénticos | `grep` retorna 1; `From<>` innecesario |
| **T1-4** | Consolidar 2 enums `ConfidenceTier` (en `extraction/issues_confidence_rules.rs:32` y `extraction/docs_confidence_rules.rs:34`) | 2 archivos declaran `ConfidenceTier` | `grep` retorna 1 |
| **T1-5** | Mover el presenter de `cognicode-core/src/interface/cli/commands.rs` (154 `println!`) a `cognicode-cli`; el core expone solo `CommandExecutor::execute(command) -> Output` y un `Output` type | `grep -c "println!" cognicode-core/src/interface/cli/commands.rs` ≥ 154 | `grep -c "println!" cognicode-core/src/` baja a 0; el bin `cognicode` sigue funcionando (`cognicode --version` exit 0); `cognicode export assurance` exit 0 |

### Tier final — Promote (≤ 1 día)

| ID | Tarea | Verificación |
|----|-------|--------------|
| **TF-1** | `merge-gate` verde en PR #345 con todo Tier 0 + Tier 1 incluido | `gh pr view 345 --json mergeStateStatus` retorna `CLEAN` |
| **TF-2** | Merge squash de PR #345 → `main` | `git log main -1` muestra el merge; `git log main..integrate/v1015` retorna 0 commits |
| **TF-3** | Bump `v0.101.9 → v0.101.10` en `Cargo.toml [workspace.package].version` + 13 manifests + changelog; `cargo build --release` exit 0 | `git tag -l v0.101.*` lista `v0.101.10` |
| **TF-4** | Tag anotado `v0.101.10` con mensaje de release; `git push origin main v0.101.10` | `git ls-remote --tags origin | grep v0.101.10` muestra el SHA |

## 7. Acceptance criteria

El goal se cierra SOLO si:

1. Todos los items T0-*, T1-* y TF-* están en DONE con su verificación pasada.
2. `cargo fmt --check` exit 0.
3. `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
4. `cargo test --workspace` exit 0; conteo no regresivo vs HEAD actual
   (4858 unit + integration; 0 failed).
5. `cargo doc --workspace --no-deps` exit 0 con 0 warnings.
6. `cargo deny check licenses` exit 0.
7. `python3 scripts/ci/test_assurance_export_golden.py` exit 0
   (golden SHA `45fa7821...` sigue pineado; producer is deterministic).
8. `python3 scripts/ci/test_roadmap_version_ratchet.py` exit 0.
9. `merge-gate` PR #345 verde → mergeado a `main` → tag `v0.101.10`
   pusheado a `origin`.
10. `odd/tasks/v1015-consolidation-and-promote.md` actualizado con
    recibos append-only de cada WU cerrado.

## 8. Risks

* **R-TF-1**: el `merge-gate` puede seguir bloqueado por infra
  pipelinek (no código). Si persiste después de T0-5, escalación
  operator-gated.
* **R-T1-1**: consolidar 4 enums `SymbolKind` requiere auditar cada
  call-site para no perder variantes de negocio (`Impl`, `Const`,
  `Static`, `Macro`, `Variant`, `Property`, `Other(String)` en
  `ground_truth.rs`). Estimación: 2-3 días solo para el walk
  call-site. Riesgo de regresión si se hace con prisa.
* **R-T1-5**: mover 154 `println!` a `cognicode-cli` puede romper el
  output del bin si los `CommandExecutor::execute` no se han
  diseñado para ser output-agnostic. Necesita test de snapshot del
  output del bin `cognicode` antes y después.

## 9. Recibo (append-only)

| Fecha | WU | Commit | Notas |
|-------|----|--------|-------|
| 2026-10-10 | goal creado | (none) | autorizado por operador tras auditoría |
| 2026-10-10 | T0-1 | (none) | **REJECTED** — falso positivo del audit. `mcp_client.rs` es CLI client, no server. AGENTS.md §17 aplica al server, no al client. |
| 2026-10-10 | T0-2 | `a77ae90` | chore(cleanup): dead code + tautological assert!(true). −9 LOC. 13/0 + 14/0 tests verde. |
| 2026-10-10 | T0-3 | `443a540` | chore(tests): 10 unimplemented!() tests eliminados. −77 +19 LOC. 21/0 rmcp_adapter tests verde (era 31 con 10 ignored). |
| 2026-10-10 | T0-4 | (this commit) | docs(roadmap): regenera CURRENT.md con SHA real + 7 commits ahead + 2252/2 test counts + clippy pre-existente documentado. Ratchet `test_roadmap_version_ratchet.py` pasa. |
| _pendiente_ | T0-5 | _pendiente_ | disparar merge-gate |
| _pendiente_ | T1-1..T1-5 | _pendiente_ | connascence cleanup |
| _pendiente_ | TF-1..TF-4 | _pendiente_ | promote + tag |

## 10. Estado del SDDK

Este goal se intentó registrar como cycle en SDDK vía
`sddk cycle start --name "v1015-consolidation-and-promote" --branch integrate/v1015 --path a-min`
y falló con `LedgerFactory: database error: Invalid parameter name: cycle_leases, cycle_leases`.

Diagnóstico: el binario `sddk 2.14.0` instalado choca con el schema
de la DB local (`~/.local/share/sddk/data/ledger.sqlite`). Mismo
patrón que SDDK-107 (2026-09-27) pero con un síntoma distinto (no
"no such table" sino "Invalid parameter name" — rusqlite rechazando
un parámetro duplicado en la query).

Workaround: el goal se mantiene como artefacto versionado en
`odd/tasks/`. Cuando el storage de SDDK se repare, este doc se
materializa como cycle sin pérdida de información.

Owner de la reparación SDDK: operator-gated (lesson 85 de
MAINTENANCE.md: bypass implícito del required-check `merge-gate` es
aceptable para release, pero la decisión sobre regenerar el DB local
vs downgrade vs `sddk dev install` requiere al operador).

## 11. References

* Auditoría: conversación 2026-10-10, 4 agentes paralelos (architecture, connascence, tests, seams)
* `odd/tasks/assurance-evidence-v1-exporter.md` — feature doc del work ya commiteado
* `crates/cognicode-core/src/assurance_export/` — producer del envelope (5 archivos, 269+ tests)
* `tests/fixtures/assurance_export_golden.cbor` — golden export pinneado, SHA `45fa7821...`
* `scripts/ci/test_assurance_export_golden.py` — 7 contract assertions
* `merge-gate.pipeline.kts` — pipelinek gate authority
* AGENTS.md §17 (observability), §20 (no dos fuentes de verdad), §19 (seguridad)
* MAINTENANCE.md pivot 2026-09-26 + lesson 85
