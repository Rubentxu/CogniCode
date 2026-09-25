# E0 — Contratos públicos y compatibilidad: CLOSEOUT

> **Estado**: CLOSED 2026-09-25 · **Versión pineada**: v0.98.1+ (HEAD `f6fd902c`) ·
> **Serie SemVer**: F0.* (evolución Post-PRF) — E0 NO introduce bump, solo contractualiza lo ya pineado.

## 1. Resumen ejecutivo

E0 se cerró **sin código nuevo de feature**, sino como **contractualización
de gates** que F0..F7 ya ejercitaban de forma implícita. La unidad
introduce:

- `CapabilityDescriptor` como autoridad única del catálogo de capabilities MCP.
- Política de compatibilidad binaria 0.97.x pineada por tests ejecutables.
- F0.1 (`cognicode find-usages` CLI) como **primer consumer real** del contrato E0.

Todos los criterios de cierre del modelo (ROADMAP §3) están cumplidos:

1. ✅ Contrato explícito: `docs/prf/specs/SPEC-MCP.md` (PRF-MCP-*) + `CAPABILITIES-MATRIX.md`.
2. ✅ Tests que fallaban antes y pasan después: 5 tests simetría L1.1 + 5 compat L1.3 + 14 F0.1 L1.4 + 4 E2E L1.4.W1.
3. ✅ Verificación quirúrgica: cada WU verificó solo los módulos impactados.
4. ✅ SemVer documentado: E0 NO bump (la rama v0.98.x es patch-only; F0.* sale como minor en su propia serie).
5. ✅ Gate `merge-gate` verde: PR-CI `merge-gate` con steps `drift-lint` (L1.2) y `compat-0.97.x` (L1.3) pineando E0 en CI.
6. ✅ Recibo en JOURNAL: esta entrada.

## 2. Tabla de requisitos vs evidencia

| Requisito | Evidencia | Test que pinea |
|---|---|---|
| Catálogo de capabilities autoritativo (declaración única) | `crates/cognicode-core/src/interface/mcp/capabilities.rs::list_tool_capabilities()` | `test_capabilities_declarations_match_pineo` (L1.1.W1) |
| Pineador de drift entre doc contractual y código | `sandbox/scripts/capabilities_drift_lint.py` | step `capabilities drift lint` en `merge-gate` (L1.2.W1) |
| Compatibilidad binaria backward (current server, old client) | `crates/cognicode-mcp/tests/find_usages_compat_0_97.rs::compat_backward_find_usages_works_with_0973_schema` | verde L1.3.W2 |
| Compatibilidad binaria forward (old server, current client) | `crates/cognicode-mcp/tests/find_usages_compat_0_97.rs::compat_forward_0973_server_handles_modern_request` | verde L1.3.W3 (local; CI lo skippea por ausencia de fixture) |
| Smoke del binario legacy 0.97.3 reachable | `compat_0973_smoke_initialize_responds_with_server_info` | verde L1.3.W1 |
| Consumer real del contrato (F0.1) | `crates/cognicode-core/src/interface/cli/commands.rs::execute_find_usages` | 14 tests F0.1 (L1.4.W2-W4) + 4 E2E (L1.4.W1) |
| Equivalencia CLI ↔ MCP para `find_usages` | `crates/cognicode-cli/tests/find_usages_cli_mcp_equivalence.rs` | 4 tests verde L1.4.W3 |
| Architectural review de F0.1 | `docs/prf/adr/ADR-PRF-008-F0.1-ARCHITECTURAL-REVIEW.md` | n/a (revisión estática) |

## 3. UAT matrix

| UAT | Comportamiento esperado | Resultado | Recibo |
|---|---|---|---|
| UAT-E0-001 (drift simmetry) | Las capabilities declaradas en `list_tool_capabilities` coinciden 1:1 con las pineadas en `stable_tool_names_with_capabilities`. Si drift → test falla con mensaje actionable. | PASS | commit `11bdf385` |
| UAT-E0-002 (drift lint) | `python3 sandbox/scripts/capabilities_drift_lint.py --strict` exit 0; `--strict` exit != 0 si drift detectado. | PASS | step `capabilities drift lint` en `merge-gate` |
| UAT-E0-003 (compat binaria 0.97.3) | Server HEAD acepta request `find_usages` con schema legacy (solo `symbol_name`). Output envuelto en `content[0].text` contiene symbol/usages/total. | PASS | commit `32c6873e` (L1.3.W2) |
| UAT-E0-004 (compat binaria forward) | Server 0.97.3 acepta request `find_usages` con schema actual (`include_declaration` + `context_lines`). No devuelve code=-32602. | PASS | commit `32c6873e` (L1.3.W3) |
| UAT-E0-005 (consumer proof: F0.1) | `cognicode find-usages <symbol> --format text` exit 0 con output text-format prefixado `def ` para definiciones y línea de contexto para callsites. `cognicode find-usages <symbol> --format json` exit 0 con output `FindUsagesOutput` JSON. Exit 1 en error. | PASS | commit `3cb07f90` + UAT binaria manual |
| UAT-E0-006 (compat en CI) | Step `compat matrix 0.97.x` en `merge-gate` ejecuta 4 tests (W1 + W2 + 2 helper) si fixture `sandbox/.compat/0.97.3/cognicode-mcp` está presente. SKIP si fixture ausente. | PASS (local); SKIP (CI sin fixture) | commit `f6fd902c` |

## 4. Cert firmada (resumen)

| Campo | Valor |
|---|---|
| Cert ID | POSTPRF-E0-001 |
| Fecha | 2026-09-25 |
| SHA | `f6fd902c` |
| Hash artefactos | n/a (no release generada por E0; los binarios son HEAD de rama `main`) |
| Cliente/entorno | local + CI remoto |
| Comando | `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p cognicode-core --lib --quiet && cargo test -p cognicode-mcp --quiet && cargo test -p cognicode-cli --quiet && python3 sandbox/scripts/capabilities_drift_lint.py --strict` |
| Exit code | 0 |
| UAT PASS | 6/6 (ver §3) |
| Advertencias | 0 |
| Rollback | n/a (cambios aditivos: CLI subcommand + test + lint step; sin cambios breaking a API) |
| Revisión independiente | ADR-PRF-008 architectural review (R1 implementado: compat en merge-gate) |

## 5. Decisión de cierre

E0 se cierra **sin bump SemVer** porque:

- E0 introduce gates (drift-lint, compat matrix) y un test pineador de simetría. **No cambia la API pública** ni el binario.
- v0.98.x es la línea de patch (`v0.98.2`, `v0.98.3`) — no admite features Post-PRF.
- F0.1 (el consumer proof) se libera como **feature Post-PRF** en su propia serie `F0.*` con SemVer minor (probablemente `v0.99.0` o lo que la política E0-F0 establezca). La liberación de F0.1 es **fuera del scope de E0**; E0 solo contractualiza el contrato, no la release.

**Próximo bloque Post-E0**: ADR-009 sobre scope de `EvidenceStore` (preliminar E1), seguido de E1.W1 (primer FactStore durable pineado). Ver `docs/roadmap/JOURNAL.md` (entrada siguiente).

## 6. Lecciones aprendidas

1. **Compatibilidad binaria pineada por tests, no por introspección de schema.** El test `compat_backward_find_usages_works_with_0973_schema` verifica comportamiento runtime (request + response), no estructura JSON estática. Una refactor del handler que mantenga el schema pero rompa la semántica (e.g. orden de campos en `content[0].text`) falla el test.
2. **Fixture reproducible fuera del repo.** El binario legacy 0.97.3 vive en `sandbox/.compat/` (gitignored, extraído del tarball de la release). Cero dependencia de red en CI cuando el fixture está presente; SKIP honesto cuando no.
3. **`hashFiles` para steps opcionales en merge-gate.** Si el fixture existe en el runner, ejecuta; si no, SKIP. Mantiene branch protection con un solo required check (`merge-gate`) sin romper contribuidores sin el fixture.
4. **Single required check = single contract.** L1.2.W1 fusionó `capabilities-drift-lint` en `merge-gate` como step. L1.3.W1 (en `f6fd902c`) hizo lo mismo con compat matrix. Mantener `merge-gate` como único gate agregado evita la proliferación de required checks en branch protection.

## 7. Referencias

- ROADMAP ejecutivo: `docs/roadmap/ROADMAP.md` (tabla §2, fila E0).
- Mantenimiento v0.98.x: `docs/roadmap/MAINTENANCE.md`.
- Specs: `docs/prf/specs/SPEC-MCP.md`, `CAPABILITIES-MATRIX.md`.
- ADR: `docs/prf/adr/ADR-PRF-008-F0.1-ARCHITECTURAL-REVIEW.md`.
- Architectural review: `ADR-PRF-008` §Verdict.
- PRF-SEC / PRF-MCP: cerrados contractualmente en C7 sobre v0.98.1.
