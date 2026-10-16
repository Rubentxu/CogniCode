# Plan físico de centralización y compatibilidad

**Principio:** primero extraer; después cambiar autoridad; por último retirar rutas antiguas. Sin refactor masivo en un commit y sin tocar PRF histórico.

## Tabla de migración

| Desde (existe hoy) | Destino lógico | Estrategia de transición |
|---|---|---|
| `scripts/test-fast.sh` | `test-harness/bin` + planner | Wrapper de compatibilidad: delegación **opt-in** (`TEST_FAST_ENGINE=harness`) con el defecto intacto, caída cerrada al motor CR-08 cuando el harness no cubre la demanda y medida de coste antes/después (ADR-TH-013) |
| `scripts/test-full.sh` | `test-harness/bin` + lane full | Wrapper mínimo, preservar comportamiento full hasta política nueva |
| `scripts/test-all.sh` | Alias protegido o mensaje de deprecación explícito | Primero test de consumidor y destino; retirar referencias a crates parked |
| `scripts/ci/select-suites.sh` | `test-harness/src/planner` | Fijar tabla contractual CR-08; tests differential/characterization |
| `scripts/ci/run-all-contracts.sh` | `test-harness/adapters/contracts` | Conservar glob de CI `scripts/ci/test_*.py` por shim sin vacío silencioso |
| `scripts/run_contract_tests.py` | `test-harness/adapters/contracts` | Un runner real, preservar exit y nombres de pruebas |
| `scripts/check_known_failures.py`, `scripts/known_failures.yaml` | `test-harness/adapters/baseline` y `test-harness/contracts/` | Consumir resultados existentes; migrar baseline con versioning y parity |
| `sandbox/scripts/run_campaign.sh` | `test-harness/adapters/sandbox` | Mantener manifiestos y fixtures existentes, propagar rc; luego alias |
| `sandbox/scripts/run_scenario.sh` | `test-harness/adapters/sandbox` | Inventariar consumidores antes del traslado |
| `sandbox/justfile` | `test-harness/` como front de pruebas; `sandbox/` fixtures | Mantener `just --justfile sandbox/justfile` hasta cortes verificados |
| `crates/cognicode-sandbox/src/main.rs` | Sigue donde está como runner Rust | Invocado por adapter; migrar sólo scripts y control, no crate |
| `scripts/ci/test_*.py` | Sigue temporalmente en `scripts/ci` | No mover glob sin actualizar `run-all-contracts` y test contra vacuidad |
| `merge-gate.pipeline.kts`, `integration.pipeline.kts`, `certification.pipeline.kts` | PipelineK raíz -> invocan harness central | Shadow + paridad, migración stage by stage; si se decide mover, redirigir `pipeline_authority.py` y reparar cobertura |
| `release-candidate.pipeline.kts`, `release.pipeline.kts` | Permanecen publicación/candidato | Invocar `harness` para obligaciones de test cuando se certifique, no mover la fábrica de release |

## Reglas de un único lugar

- Bajo `test-harness/` viven versiones normativas de contratos nuevos; los wrappers fuera delegan **sin lógica adicional**.
- Para catálogo: generación determinista desde comandos y metadata y metadatos no derivables. No duplicar listas en shell, YAML, Kotlin y Python.
- No consolidar test suites de crate en scripts si implica perder cargo discovery o test target parity.
- Los ADR anteriores con `accepted` son inmutables; este paquete propone un ADR nuevo y reconciliación en roadmap activo.

## Trabajo en paralelo a main

1. `git fetch origin && git worktree add ../CogniCode-test-harness -b feat/test-harness-incremental origin/main` si nombre/ruta disponibles; usar un worktree fuera de checkout original.
2. Copiar **solo** el directorio `test-harness/` del ZIP en ese worktree. No sobrescribir contenido existente sin inspeccionar.
3. Registrar SHAs de base y estado de SDDK Post-PRF en el worktree. No modificar `AGENTS.md` ni rutas de CI al inicio.
4. W1-W2 preferentemente bajo subtree, con PRs pequeños de correcciones seguras sólo si main las necesita y green gate.
5. Mover *seams* hacia wrappers en PR acotados, uno por superficie. Rebase/integración frecuente para que la evolución paralela no diverja de runtime/release.
6. Habilitar rutas experimentales por flag/config *opt-in*, no por sustitución silenciosa de `test-full`.
7. W6 sólo integra el resultado en gates oficiales una vez completado el AAT y con aprobación de política.

## Criterio de anti-contaminación

`git status --porcelain` del checkout principal no debe cambiar por usar otro worktree. En el worktree del harness sólo cambia código versionado deliberadamente; datos de runs, `--db` y logs siempre externos. Si PipelineK actual deja `release/`/`dist/` en raíz durante build, documentar como artefactos de pipeline existentes, no mover sin cambiar la fábrica de release; no pretender que su aislamiento está resuelto por este trabajo.
