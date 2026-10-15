# ADR-TH-006 — Retirada de la clase UAT

- **Estado:** ACCEPTED (2026-10-15). Decisión explícita del operador, tomada el
  mismo día que B1 y con independencia de él. Instrucción literal: «vamos a
  prescindir de los tests de UAT, elimínalos y sácalos de test harness» →
  «fuera UAT he dicho».
- **Contexto:** el repositorio mantenía dos clases de prueba con la misma
  ambición: los contratos que el gate de merge ejecuta en cada iteración, y una
  clase UAT (`*_uat.rs`) que repetía escenarios de aceptación contra binarios
  reales. Medido el 2026-10-15: 28 targets en tres crates, 68 tests, 4 804
  líneas. Sostenida contra el inventario de obligaciones, esa clase duplicaba
  cobertura que los contratos de lane ya pinean por otra vía
  (`a009`, `a016`, `prf_h06`, `b6_w1`, `cli_exit_code_propagation`), y su coste
  se pagaba en cada corrida de CI y en el baseline de tests del preflight.
- **Decisión:** la clase UAT deja de existir en este repositorio.
  1. Se eliminan los 28 targets `*_uat.rs` de `cognicode-cli`, `cognicode-core`
     y `cognicode-mcp`, con sus referencias en las dos pipelines, en los
     contratos que los citaban por nombre y en los comentarios que apuntaban a
     ficheros que dejan de existir.
  2. `test-harness/` deja de incluir un plan de UAT: se retira
     `uat/UAT.md` y su entrada en el índice, el roadmap y el manifiesto del
     paquete. El AAT (pruebas de mutación) **se mantiene**: es una clase
     distinta, la que sostiene la evidencia adversarial, y no es lo que el
     operador retiró.
  3. El baseline de `preflight-clean-clone.sh` se re-baseliniza
     (`passed` 5579 → 5512, `ignored` 37 → 36). El delta es **calculado**
     (67 tests que pasaban + 1 ignorado) y el primer clean-clone posterior es
     quien lo confirma; si el observado difiere más que la tolerancia, se
     re-baseliniza con el observado, que es lo que el propio fallo prescribe.
- **Lo que esta decisión acepta explícitamente:** deja de haber reproducción
  ejecutable en CI de las garantías que esa clase cubría — diferenciación
  read/write, allowlist de directorios, telemetría opt-in, shutdown limpio,
  namespace por digest, snapshots durables, concurrencia, determinismo de CLI,
  paridad del gate de clippy y distribución desde bytes release. Las garantías
  siguen descritas como evidencia histórica en `docs/prf/` y las que tienen
  contrato superviviente siguen verificadas por él; las demás pasan a ser
  afirmaciones documentadas, no verificadas en cada merge. Ese es el
  intercambio, y está escrito aquí para que no se descubra por sorpresa.
- **Consecuencias medidas:** `merge-gate.pipeline.kts` pasa de 73 a 58
  declaraciones de stage; `release-candidate.pipeline.kts` de 33 a 30; las
  suites de perfil release bajan de cinco a tres (CLI) y de una a cero (MCP);
  la enumeración CLI de 49 a 41 targets; el inventario MCP de 26 a 9 ficheros.
- **Coherencia con la gobernanza:** `AGENTS.md` exige una decisión explícita
  para reducir cobertura y prohíbe convertir un fallo en un aviso o reducir
  cobertura para poner un gate verde. Esta decisión no busca un gate verde: los
  contratos que sobreviven se actualizan en el mismo commit, y el propio ADR
  enumera lo que deja de verificarse.
- **Reversibilidad:** la retirada es un commit; revertirlo devuelve los 28
  ficheros y sus obligaciones. Lo que no se puede recuperar por revert es la
  decisión: reabrir la clase UAT exige una decisión nueva del operador.
- **Prueba que la sostiene:** `cli_gate_coverage_contract` (20/20),
  `a013_lifecycle_gate_contract` (12/12), `core_gate_coverage_contract` (11/11),
  `pipelinek validate` de las dos pipelines, y los contratos de texto de
  pipelines en verde tras la retirada.
