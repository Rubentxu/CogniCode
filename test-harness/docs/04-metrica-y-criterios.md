# Benchmark y criterios de calidad del evolutivo

## Mediciones preimplementación

Medir *cold* y *warm* de: `scripts/test-fast.sh`, `scripts/test-full.sh`, `merge-gate.pipeline.kts`, `integration.pipeline.kts`, `certification.pipeline.kts` y campaña sandbox representativa. Capturar (sin alterar flags): wall time total, por stage, compilación vs test, duración/errores de cada test, reintentos, CPU, RSS, consumo de disco/log y contención CARGO_TARGET_DIR.

Guardar muestras y metodología en `$XDG_STATE_HOME/cognicode-test-harness/benchmarks/`, con commit y hora; publicar al repo **solo resumen anonimizado** y metodología revisada. Medir mínimo tres repeticiones por ruta comparable; la primera warm empieza tras compilar; sin cambiar build flags globales.

## Métricas a comparar

- TTR (*time to red*): tiempo desde cambio hasta test fallido identificable.
- TTF (*time to fix/green*): desde corrección hasta gate verde verificable.
- RRR (*re-execution ratio*): suites reejecutadas / suites requeridas, desglosado por valididad.
- Cache reuse rate: válidas reusadas / requeridas, contabilizando por suite y casos.
- Overhead plan/resume: segundos y porcentaje frente a gasto de tests.
- False-green count: debe ser **0** bajo mutaciones AAT.
- Coverage parity: conjuntos exactos de casos/targets/flags, no sólo suma de tests.
- Gate coverage debt: obligaciones sin prueba, debe ser 0 en modo oficial.
- Duración de release candidate y del smoke público: medición separada; no se atribuye toda mejora al harness.

## Objetivos de aceptación (propuestas, recalibrables tras baseline)

1. Mínimo funcional: dado un fallo en suite N y arreglo que sólo cambia sus inputs, se conservan recibos válidos previos y se reejecutan N + dependientes afectados. Debe ser verificable por `explain`.
2. Ninguna mejora de tiempo es válida si falsifica PASS, reduce cobertura o evita UAT finales.
3. *Target* inicial tras baseline: reducir tiempo de *fix-to-green* warm al menos un 30% en una suite independiente fallida; si no, registrar resultado y rediseñar, no proclamar velocidad ficticia.
4. Overhead del planificador objetivo <=2s warm en repositorio de tamaño actual, medido y excluyendo Cargo builds; si no se logra, optimizar sin debilitar fingerprint.
5. AAT y UAT obligatorias 100% PASS para permitir `harness` como autoridad delegada; informe de migración 1:1 de obligaciones.

## Interrupción y recurso

Time budget por lane para sesiones de agentes: plan/status ligeros; `run` por suite con timeout y checkpoints visibles; `resume` tras reinicio de agente. Un proceso con 6h no puede impedir avances de otras work units: gate caro reservado a integración/certificación y ejecutado fuera del bucle de TDD (pero antes de release).

## Informe final esperado

Tabla `antes/después`, condiciones de máquina, SHAs, configuración, pruebas/casos ejecutados, cache hits, explicación de *reused*, costes por gate, limitaciones y estimación de ahorro. Ahorro no medido = `UNKNOWN`.
