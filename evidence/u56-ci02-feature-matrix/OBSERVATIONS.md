# PRF-CI-02 — Feature matrix (parte 1 de la campaña nightly)

Fecha: 2026-09-22

## Problema real encontrado

Ninguna combinación de features fuera de la default se había compilado
recientemente: `--no-default-features` y `--all-features` ROMPIAN en 3 crates.
Ejemplos: INTERPROC_SUMMARY sin gate, brazo `reparse_on_edit` sin gate,
`Decision { id: _ }` descartando el id usado bajo multimodal, imports de test
ausentes. La matriz no era verificable de hecho.

## Correcciones

- cognicode-core: gates persistence/program-analysis-server en adapter, tests y
  allowlist; stores in-memory des-gateados (no dependen de blake3).
- cognicode-runtime: bootstrap_ladybug gated (+2 tests de integración).
- cognicode-explorer: patrones de match, imports de test, mut schema.

## Resultados verificados (LOCAL, CARGO_TARGET_DIR compartido)

| Combo | Resultado |
|---|---|
| core no-default | 2104 passed / 0 failed |
| core default | 2145 / 0 |
| core all-features | 2960 / 0 |
| runtime no-default | 3 / 0 |
| runtime default | 6 / 0 |
| runtime all-features | 8 / 0 |
| mcp all-features | 8 / 0 |
| explorer default | 955 / 0 |
| explorer all-features | 1025 / 0 |
| graph-algos no-default | 1 / 0 |

fmt --check limpio; clippy workspace 0 errores.
Job `feature-matrix` añadido a ci.yml (8 combos, fail-fast: false).

## Estado

PARTIAL: matriz de features DONE. Pendientes en CI-02: adversariales y
benchmarks en la campaña nightly (comparación con baseline, desbloqueada
por §52).
