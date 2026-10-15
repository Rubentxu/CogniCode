# Rutas existentes y fuentes externas — tabla de trazabilidad

## Referencias de código específicas

Todas las rutas siguientes pertenecen al repositorio actual `Rubentxu/CogniCode`, salvo futuras refactorizaciones:

| Contrato nuevo | Fuente existente de contraste |
|---|---|
| TH-S01 selección | `scripts/ci/select-suites.sh`, `scripts/test-fast.sh`, `scripts/ci/test_select_suites.py`, `crates/cognicode-cli/tests/qw08_crate_selector.rs` |
| TH-S02 known failures | `scripts/check_known_failures.py`, `scripts/known_failures.yaml` |
| TH-S03 resume/CLI | `scripts/test-full.sh`, `scripts/run_contract_tests.py`, `AGENTS.md` |
| TH-S04 Cargo/binary | `scripts/ci/target-dir.sh`, `Cargo.toml`, `scripts/ci/test_serial_env_contract.py` |
| TH-S04 sandbox | `sandbox/scripts/run_campaign.sh`, `sandbox/scripts/run_scenario.sh`, `sandbox/justfile`, `crates/cognicode-sandbox/src/main.rs` |
| TH-S05 gate | `merge-gate.pipeline.kts`, `integration.pipeline.kts`, `certification.pipeline.kts`, `scripts/ci/pipeline_authority.py` |
| TH-S05 candidate/release | `release-candidate.pipeline.kts`, `release.pipeline.kts`, `docs/distribution/INSTALL.md`, `scripts/ci/release-install-smoke.sh` |
| TH-S06 profiles | `AGENTS.md`, `scripts/ci/target-dir.sh`, `scripts/ci/test_fast_full_wrappers.py` |

## Fuentes externas verificadas

- Cargo test: https://doc.rust-lang.org/cargo/commands/cargo-test.html
- Cargo metadata: https://doc.rust-lang.org/cargo/commands/cargo-metadata.html
- Nextest record/rerun (experimental): https://nexte.st/docs/features/record-replay-rerun/
- Nextest rerun explícito e invalidación no automática por código: https://nexte.st/docs/features/record-replay-rerun/rerun/

## Nota sobre el espacio de trabajo

Este evolutivo es del harness de **CogniCode**, NO de la skill `cognicode-diagrams` de `Rubentxu/agent-skill`, y no debe mezclarse su workspace (`~/.cognicode-diagrams/`) con `$XDG_STATE_HOME/cognicode-test-harness/`.
