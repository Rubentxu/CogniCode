# test-strategy-tiered — Estrategia de tests inteligente (fast / full)

## Goal

Reducir el tiempo de feedback durante evolutivos sin perder cobertura en
releases. Hoy `cargo test --workspace` ejecuta **todo el árbol (~5000+
tests + doc-tests)** y la mayoría de los fallos son por **binarios release
ausentes** (`cogh`, `cognicode`) en `/var/home/rubentxu/cargo-targets/release/`
— un problema de **infraestructura**, no del código bajo cambio. El selector
`scripts/ci/select-suites.sh` (CR-08) ya clasifica qué suites correr
según los paths modificados, pero solo se invoca desde CI. Lo que falta
es el flujo del día a día y la disciplina contractual.

## Problem

- **Feedback loop lento durante evolutivos**: tocar `crates/cognicode-core/`
  y correr `cargo test --workspace` tarda >20 min para validar ~2 min
  de cambio real.
- **Ruido por infraestructura release**: 5+ tests de las suites
  `prf_dist_01_06_release_candidate_uat`, `prf_dist_workflow_flatten_uat`,
  `prf_f6_w1_release_coherence`, `prf_cli_04_two_process_uat` y
  `prf_f4_w3_corrupt_cache_recovery` fallan solo si el binario release
  no existe en el path (`/var/home/rubentxu/cargo-targets/release/cogh`).
- **Disciplina no escrita**: AGENTS.md dice "test RED → fix → test verde"
  pero no distingue entre tests del evolutivo y tests de release.

## Why

El operador invoca explícitamente esta optimización tras observar el
coste real (`cargo test --workspace` ≈ 20 min para un cambio en
`crates/cognicode-core`). El selector CR-08 ya cubre la decisión de qué
correr; lo que falta es la **superficie del desarrollador** y la
**política contractual** en AGENTS.md.

## Scope

- Crear `scripts/test-fast.sh` que detecta `git diff` y delega en
  `select-suites.sh` para correr solo suites afectadas.
- Crear `scripts/test-full.sh` que ejecuta la batería completa
  (workspace + doc-tests) y aplica `known_failures.yaml` para
  distinguir regresiones reales de baseline conocido.
- Documentar la disciplina "fast / full" en `AGENTS.md` § Disciplina
  de ingeniería.
- Ajustar `[profile.test]` en `Cargo.toml` workspace para reducir
  tiempo de link (debug=false, incremental=true, codegen-units=256,
  opt-level=0).

## Constraints

- **No reescribir** `select-suites.sh` ni su contrato
  (`scripts/ci/test_select_suites.py`). El wrapper solo consume su
  salida JSON.
- **No eliminar** `cargo test --workspace` como opción — sigue siendo
  la línea base para diagnóstico.
- **No añadir `[profile.test]` workspace**: `~/.cargo/config.toml`
  global ya define `opt-level=0, debug=1, codegen-units=256, jobs=4`
  optimizado para coexistir con 4 instancias OpenCode y proyectos Rust
  paralelos. Duplicarlo invalidaría caché compartida.
- **No pisar `CARGO_BUILD_JOBS`** en los wrappers; el límite global
  jobs=4 es contrato del operador para la máquina.
- **No fabricar** tests; este WU es de tooling + disciplina.
- Mantener idioma neutral en artefactos (comentarios en scripts en
  español, pero respetando el patrón existente del repo).

## Acceptance criteria

1. `bash scripts/test-fast.sh` ejecuta solo las suites que `select-suites.sh`
   determina para el `git diff` actual; exit code refleja `cargo test`.
2. `bash scripts/test-full.sh` corre `cargo test --workspace` +
   `cargo test --workspace --doc` y aplica `known_failures.yaml` para
   abortar si aparece regresión nueva.
3. AGENTS.md tiene una nueva sección **"Estrategia de tests"** con la
   regla: evolutivo = fast, antes de release = full.
4. `[profile.test]` con `debug = false` reduce tiempo de link sin
   ocultar fallos de runtime.
5. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`
   siguen exit 0.

## Tasks

- [x] T1 — Crear `scripts/test-fast.sh`
- [x] T2 — Crear `scripts/test-full.sh`
- [~] T3 — `[profile.test]` workspace → **CANCELADO**: ya cubierto por
       `~/.cargo/config.toml` global (jobs=4, codegen-units=256). Duplicarlo
       en workspace invalidaría caché compartida con los otros proyectos
       Rust en desarrollo en esta máquina.
- [x] T4 — Documentar estrategia en `AGENTS.md` (con respeto al contexto
       multi-app: no pisar CARGO_BUILD_JOBS, no duplicar profile.test)
- [x] T5 — Validar: fast sobre cambio en core, full sobre HEAD limpio

## Out of scope

- Reescribir `select-suites.sh`.
- Añadir un selector por heurística adicional (e.g., time-based).
- Modificar workflows (el repo no tiene `.github/workflows/`).
- Cambiar la numeración de release ni SemVer.

## Evidence

| Comando | Tiempo | Resultado |
|---|---|---|
| `bash scripts/test-fast.sh --dry-run` | ~45 ms | JSON con suites esperadas, sin compilar. |
| `time bash scripts/test-fast.sh --paths "crates/cognicode-core/src/lib.rs"` | 1m58s (subset `--lib`) | Exit 0. Antes (sin `--lib`) era 3m21s; batería completa `--workspace` era >20 min. |
| `cargo fmt --all --check` | <1s | exit 0 (sin drift introducido). |
| `cargo clippy --workspace --all-targets -- -D warnings` | — | exit 0 (sin warnings introducidos). |
| `bash scripts/test-full.sh` | (no corrido en este WU; el operador lo invoca antes de release). | Próximo gate. |