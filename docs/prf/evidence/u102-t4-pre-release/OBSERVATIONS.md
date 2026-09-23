# U102 — T4 pre-release evidence (ciclo §102, post-H-06 cycle)

## Alcance
Validación T4 (pre-integración-remota) ejecutada en HEAD post-H-06 cycle
sobre `origin/main` (214 commits ahead). Niveles T0 (build+clippy) + T1
(lib tests) + T2 (bin tests) + T3 (workspace --tests) ejecutados.

## Resultados observados

| Nivel | Comando | Resultado |
|-------|---------|-----------|
| T0 build | `cargo check --workspace` | ok (51.87s inicial; 3.05s incremental) |
| T0 clippy | `cargo clippy --workspace --all-targets -- -D warnings` | EXIT 0 |
| T1 lib | `cargo test --workspace --lib` | 4156 passed / 0 failed / 27 ignored |
| T2 bin (sample cogh) | `cargo test -p cognicode-cli --bin cogh` | 312 passed / 0 failed / 1 ignored (×5 verde estable) |
| T3 integration | `cargo test --workspace --tests -- --test-threads=2` | **5315 passed / 0 failed / 33 ignored** |

## Deuda técnica atacada en este ciclo

1. **Flaky `t_l2_commit_records_versions_layout_in_journal`** (cognicode-cli):
   - Causa: el test usaba `crate::lifecycle_journal::journal_path(VERSION)`
     que internamente releía `COGNICODE_HOME` desde env global, susceptible
     a interferencia entre tests paralelos.
   - Fix: usar `home.journal_version(VERSION)` (path determinista del
     `CognicodeHome` instanciado, sin tocar env).
   - Validación: 5/5 runs verde (criterion: "test result: ok. 312 passed;
     0 failed; 1 ignored" en cada una).

## Restricción de entorno (documentada, no resuelta)

* `t_e86_3_uninstall_without_ide_prints_helpful_message` flake en suite
  workspace-wide parallel, pasa 10/10 cuando se aísla. Causa: helpers de
  lifecycle.rs (`setup_temp_home`, `run_cogh`) usan `set_var`/`remove_var`
  sobre `HOME` y `COGNICODE_HOME` sin lock global entre bins. No es regresión
  nueva — el helper precede al ciclo H-06 y al fix U102.
* Mitigación operativa: `--test-threads=2` produce 0 failures. Pendiente:
  sustituir estos helpers por uno basado en `std::env::temp_dir().with_suffix`
  + `Mutex<()>` global (próximo WU técnico, fuera de scope de T4 pre-release).
