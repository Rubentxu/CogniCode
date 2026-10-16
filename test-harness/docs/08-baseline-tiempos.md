# Baseline de tiempos y cobertura del gate de merge

Documento generado, no escrito a mano: se regenera con el comando de abajo.

- **Fecha**: 2026-10-16T06:13:54+00:00
- **Commit del árbol medido**: `e9416628096a9d721d45765985b073d7dc579e1f`
- **Lane**: `merge`
- **Run de PipelineK**: `4d431256-6a5` (.pipelinek/merge-gate-final.db)
- **Etapas medidas**: 58
- **Total medido**: 549.9 s

Regenerar:

```shell
python3 test-harness/bin/harness.py report --lane merge \
  --pipelinek-db .pipelinek/merge-gate.db --budget 420 \
  --protect contracts --protect fmt --protect clippy \
  --write-baseline test-harness/docs/08-baseline-tiempos.md
```

## Etapas medidas (mayor a menor)

| Etapa | Segundos |
|---|---:|
| `contracts` | 263.8 |
| `cli-unit-tests` | 61.6 |
| `cli-integration-suites` | 44.2 |
| `core-unrestricted-evidence-kernel` | 38.5 |
| `cognicode-ladybug` | 29.7 |
| `core-lib` | 11.8 |
| `clippy` | 11.2 |
| `cli-a015-licenses-gate` | 6.5 |
| `build-cognicode-cli-debug` | 6.3 |
| `cli-qw03-bin-tracking` | 6.2 |
| `cli-a015-onboarding-gate` | 5.9 |
| `cli-action-register-identity-contract` | 5.6 |
| `cli-qw04-preflight` | 5.5 |
| `cli-gate-coverage-contract` | 5.5 |
| `cli-a014-capabilities-json` | 5.4 |
| `cli-a013-lifecycle-gate-contract` | 5.2 |
| `qw08-crate-selector` | 4.7 |
| `fmt` | 4.4 |
| `contract-toolchain` | 3.9 |
| `harness-shadow-plan` | 3.6 |
| `skills-bundles` | 2.3 |
| `advisories` | 1.5 |
| `cli-doctests` | 1.5 |
| `licenses` | 1.3 |
| `core-gate-coverage-contract` | 1.3 |
| `e91-w7-regression-budget` | 1.1 |
| `e91-w8-stage-profile` | 1.0 |
| `cr07-metrics-exposition` | 0.8 |
| `mcp-a012-structured-output` | 0.8 |
| `core-adversarial-e2e` | 0.7 |
| `mcp-prf-f5-w3-signal-cancel` | 0.7 |
| `mcp-a009-agent-safe-profile` | 0.6 |
| `mcp-a010-tool-authority-audit` | 0.6 |
| `mcp-prf-f4-w3-corrupt-cache-recovery` | 0.6 |
| `build-cognicode-mcp-debug` | 0.5 |
| `build-control-plane-debug` | 0.5 |
| `core-adversarial-campaign` | 0.5 |
| `core-prf-mcp-05` | 0.5 |
| `core-a014-product-asset-resolution` | 0.5 |
| `mcp-a016-tools-runtime-consistency` | 0.5 |
| `bin-tracking` | 0.4 |
| `verify-debug-binaries` | 0.3 |
| `toolchain` | 0.2 |
| `skills-validate` | 0.2 |
| `repo-contract` | 0.1 |
| `install-cargo-deny` | 0.1 |
| `install-rust-analyzer` | 0.1 |
| `rust-analyzer-answers` | 0.1 |
| `select-suites-script` | 0.1 |
| `capabilities-drift-lint` | 0.1 |
| `compat-matrix-0-97-x` | 0.1 |
| `check` | 0.0 |
| `supply-chain` | 0.0 |
| `lsp-toolchain` | 0.0 |
| `build-binary` | 0.0 |
| `selector` | 0.0 |
| `test-pr` | 0.0 |
| `merge-gate` | 0.0 |

## Cobertura del catálogo

- Obligaciones de la lane: 39
- Por tipo: {'test': 29, 'check': 6, 'script': 4}
- Por pipeline de origen: {'product-fast.pipeline.kts': 6, 'merge-gate.pipeline.kts': 33}

## Presupuesto

- Presupuesto declarado: 420.0 s
- Actual: 549.9 s → 405.6 s dejando de ejecutar 3 etapa(s)
- Cabe: sí (faltan 0.0 s)
- Protegidas: contracts, fmt, clippy, harness-shadow-plan

