# ADR-TH-007 — Implementación del núcleo del harness

**Estado:** ACCEPTED. **Fecha:** 2026-10-15. **Contexto:** W2/W3/W4 del roadmap
del harness, primer bloque ejecutable. **Autoridad del gate:** no cambia
(ADR-TH-003).

## Contexto

Las specs TH-S01..TH-S03 describen un harness incremental con evidencia
verificable, y hasta ahora el paquete era solo especificación. El primer bloque
implementable tenía que elegir entre dos caminos: construir el harness completo
(paralelismo, nextest, sandbox, lanes de release) y validarlo al final, o
construir el núcleo que decide y registra, congelarlo con tests, y dejar la
adopción en el gate para cuando exista paridad medida.

## Decisión

Se implementa el núcleo (`test-harness/{bin,src,adapters,tests}`) en Python con
biblioteca estándar y se conecta al gate **como contrato**, no como orquestador.
Decisiones concretas:

1. **Lenguaje y dependencias.** Python 3, biblioteca estándar, sin instalación.
   El gate instala `jsonschema` y `PyYAML` para otros contratos, pero no pytest;
   por eso la suite del harness se escribe al estilo de `scripts/run_contract_tests.py`
   (funciones `test_*`, sin fixtures ni parametrización) y el contrato del gate
   prohíbe imports de terceros en el harness. Un harness que necesitara `pip
   install` para correr localmente sería un harness que en CI no corre.

2. **Hexagonal, con el núcleo puro.** `domain`, `policy`, `validity` y `gate` no
   tocan el mundo: son datos con serialización canónica y funciones puras, y por
   eso se prueban sin repositorio. `planner`, `orchestrator` y `cli` orquestan;
   `adapters/` habla con git, cargo, las pipelines, el ledger y los procesos.

3. **Catálogo derivado, no declarado.** Las obligaciones se leen de los
   `*.pipeline.kts`: `cargo test`, `cargo fmt|clippy|deny|build` y
   `bash scripts/…`. La única tabla escrita a mano es qué pipelines alimentan
   cada lane, porque eso es intención de gate y no inventario. Una lista paralela
   de suites sería la segunda fuente de verdad que el repositorio ya pagó una vez.

4. **Fail-closed con motivo visible.** Sin merge-base, sin grafo de paquetes, con
   una ruta que ningún paquete posee, con un manifest o una pipeline tocados o con
   un fichero de test eliminado, el plan exige **todo** y escribe por qué. La
   indeterminación sale como `UNKNOWN_INPUT` con código 3; nunca como plan limpio.

5. **Documentación de raíz inerte solo si nadie la cita.** `README.md`,
   `AGENTS.md` y compañía no se declaran inocuos por su nombre: se busca su path
   en las fuentes de cada paquete y en `scripts/`. Si ninguna lo cita, no se exige
   nada; si alguna lo cita, esa unidad entra. La lista es corta y explícita, y el
   límite (una ruta construida por concatenación no se detecta) está declarado en
   el README.

6. **Recibo `RUNNING` separado del terminal.** Los recibos son inmutables y el
   ledger solo se añade. El marcador de «esto se lanzó» y el recibo final son dos
   hechos distintos con ids distintos (`...-running` / `...-terminal`), en vez de
   un registro mutable. Un `RUNNING` sin terminal no es PASS: es una interrupción
   que `resume` vuelve a ejecutar.

7. **Índice derivado, verdad en el directorio.** `index.json` acelera la consulta
   de «último recibo de una suite» y se reconstruye cuando cambia el número de
   recibos del directorio. Un índice rancio haría decir «no hay recibo previo» con
   el recibo delante, que es un falso negativo caro; el coste de comparar un
   recuento es despreciable.

8. **Política de reutilización en un módulo.** `change` reutiliza; `merge` ejecuta
   todo salvo `--allow-reuse` explícito; integración, certificación y release
   nunca reutilizan. `policy_version` entra en cada huella, así que cambiar la
   política invalida la evidencia anterior sin necesidad de recordarlo.

9. **Advisory del gate se copia, no se reinterpreta.** Las etapas que PipelineK
   deja fallar (`|| echo 'ADVISORY: …'`) se declaran excluidas con ese motivo. No
   se inventan obligaciones que el gate no exige, ni se convierten en huecos que
   nadie promete cubrir.

10. **El harness no cambia el gate.** Se ejecuta como contrato
    (`scripts/ci/test_harness_contract.py`) dentro de la etapa de contratos del
    merge-gate. No añade ni quita etapas, y la adopción en modo sombra queda para
    W6 con paridad medida.

## Consecuencias

- El harness es útil hoy para feedback local (`plan` en 2,5 s sobre el repo real,
  `run` reutilizando lo que no cambió) y verificable en el gate.
- La suite del harness (60 tests, ~35 s) y su contrato (~50 s, incluye la suite)
  suman ~50 s a la etapa de contratos. Es el precio de que el harness esté
  gated; medirlo es parte del bloque siguiente.
- No hay ejecución en paralelo: `--max-workers > 1` se rechaza con motivo. Menos
  capacidad, cero carreras por recursos compartidos.
- `certify` no reutiliza en ninguna lane, ni siquiera `change`: un recibo
  reutilizado no certifica.

## Lo que este ADR no decide

- La paridad con CR-08 (W3/W6), que sigue siendo el oráculo del selector.
- La adopción como etapa de observación en las pipelines (W6).
- El adaptador nextest y las lanes de release (W5).
- El formato final del ledger si se adopta a escala (JSONL + índice derivado es
  suficiente para un repositorio y verificable; SQLite queda como alternativa no
  descartada en TH-S02).
