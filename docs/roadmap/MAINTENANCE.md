# Maintenance backlog v0.98.x

> **Estado**: backlog activo de mantenimiento sobre la rama estable v0.98.x.
> Estas unidades son **mantenimiento**, no features. Se cierran con bump SEMVER patch (`v0.98.2`, `v0.98.3`, etc.) sin abrir nueva release mayor.
> Vive en `docs/roadmap/` junto al roadmap ejecutivo.

## M0.* — Mantenimiento v0.98.x

| ID | Descripción | Estado | Evidencia | Trigger |
|---|---|---|---|---|
| **M0.1** | `cogh rollback --to <same>` falla por journal nuevo no listado en `active_install_is_coherent()`. Test RED: `current=0.98.1, install coherent, old journal exists` → debe devolver 0 sin cambios. | PENDING (necesita info operador para reproducir) | Test ya pineado y verde: `t_e86_4_rollback_to_current_is_noop`. El no-op logic vive en `cmd_update` (línea 602), no en `cmd_rollback`. Sin pasos reproducibles del operador no se puede distinguir bug de comportamiento correcto. | Detectado por revisión operador 2026-09-25 |
| **M0.2** | `cargo fmt --all --check` falla con 104 archivos drift detectados por G0.1 al pasar por PR-CI. Trabajo mecánico: `cargo fmt --all` en bloque + commit atómico `chore(fmt): apply rustfmt over drifted files`. | CLOSED 2026-09-25 | PR #291 squash-merged como `26746a64`. 25 archivos formateados + 5 lints arreglados (M0.2.1+M0.2.2) + workflow chmod +x fix (M0.2.3) + 3 fixtures (M0.2.4) + assertion state13 (M0.2.5). CI run #36120627650 con 4/4 jobs PASS. | Detectado por G0.1 (run #36113397644) |
| **M0.3** | Auditoría clippy residual (`H-clippy-cli-residual D34-2`) + `moldql` panic test preexistente. | CLOSED 2026-09-25 | clippy strict: `cargo clippy --workspace --all-targets -- -D warnings` exit 0 (cubierto por M0.2.1+M0.2.2). moldql: `cargo test -p cognicode-explorer --lib` → 834 passed; 0 failed; 0 ignored; 0 measured. Tests de moldql_pattern_mcp/Rest/e28_3_runtime_wiring todos verdes (231+4+3+7+13 = 258 tests moldql pasan). Los `panic!` en `intent.rs:135,166,182,196` y `consolidated_handlers.rs:1177,1279,1359` son tests de contrato (pinean invariantes), NO regresiones. Sin panic test preexistente fallando. | Carry-over PRF (F7 §244, STATE §13, RELEASE-CANDIDATE §80–81) |
| ~~**M0.3.b**~~ | ~~CLI equivalente a `find_usages` MCP tool.~~ | ~~MOVIDO A E3~~ | ~~No es mantenimiento: es feature nuevo (analogous a otras tools). MAINTENANCE no admite features.~~ | ~~Carry-over PRF, ahora evolutivo~~ |

## E3 (evolutivo, fuera de MAINTENANCE)

**E3 — `find_usages` CLI wrapper sobre MCP tool**: feature nuevo que expone
la MCP tool `find_usages` (que ya existe) como subcomando del binario
`cognicode`. Forma parte de la evolución natural de "MCP tool → CLI
equivalente" para herramientas usadas frecuentemente.

- **Scope**: nuevo binario o subcomando en `crates/cognicode-cli/`.
- **Prereq**: definir contrato público (E0.W1 CapabilityDescriptor unificado).
- **Severidad**: baja (mejora UX).
- **Trigger**: carry-over PRF, ahora evolutivo.
- **Estado actual en ROADMAP**: a añadir.

## Criterios de cierre (M0.*)

1. Test RED de regresión o caracterización.
2. Fix mínimo.
3. Test verde.
4. Si toca binario: bump SEMVER patch (`v0.98.2` por cada cierre o agrupación, según §release del roadmap).
5. Gate `merge-gate` verde en el PR.
6. Si toca docs de release o README: actualizar.

## Cómo NO se hace mantenimiento

- No se mezcla con features (no se mezcla M0 con E0..E3). Por eso `find_usages` sale de aquí.
- No se reabre PRF ni se justifica con su roadmap.
- No se reabren certificaciones C# anteriores para "incluir" la corrección; las C# quedan como firma del estado en su fecha.

## Cómo se decide agrupar o separar releases

- Si dos M0.x tocan el mismo binario (cogh, cognicode-mcp) y no hay dependencias entre ellos → se pueden agrupar en un solo `v0.98.2`.
- Si hay dependencias (M0.2 fmt-fix toca todo el repo, M0.1 toca solo cogh) → se pueden hacer dos releases separados: `v0.98.2` con M0.2 y `v0.98.3` con M0.1, o ambos juntos.
- M0.3 NO requiere release por sí mismo (clippy ya estaba strict, moldql ya estaba testeado). M0.3 es "verificación de carry-over", no código.
- Decisión la toma el operador en cada cierre.
