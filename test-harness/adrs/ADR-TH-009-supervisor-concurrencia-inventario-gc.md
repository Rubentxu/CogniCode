# ADR-TH-009 — Supervisor: concurrencia con cerrojos, pin de inventario y GC

**Estado:** ACCEPTED. **Fecha:** 2026-10-15. **Contexto:** W4 del roadmap del
harness, tras el núcleo (ADR-TH-007) y la paridad con el oráculo (ADR-TH-008).
**Autoridad del gate:** no cambia.

## Contexto

El supervisor hacía lo mínimo: una suite detrás de otra, un recibo antes y otro
después, y reanudación. Faltaba lo que TH-S03 pide explícitamente y lo que el
README declaraba como no implementado: límite de concurrencia con locks, exclusión
de suites seriales, y las pruebas adversariales (dos intentos al mismo run,
SIGKILL a mitad, fallo del supervisor). Además, TH-S02 exige que una suite que
pierde casos por una feature no pase por verde (`expected_min_cases`/pin de
inventario) y una política de GC que separe evidencia de caché.

## Decisión

1. **Concurrencia explícita, nunca por defecto.** `--max-workers N` (tope 8) con
   `default=1`: sin pedirlo, el harness sigue siendo secuencial. La concurrencia
   efectiva y los cerrojos tomados quedan escritos en el estado del run
   (`concurrency`), para que nadie tenga que deducir cómo corrió.

2. **Cerrojo compartido implícito del target dir de Cargo.** Dos `cargo test` en
   el mismo workspace se serializan de todas formas (cargo bloquea su target dir),
   así que se declara como recurso. El paralelismo real, hoy, viene de las suites
   que no compilan (scripts). Preferimos decirlo a fingir un speedup.

3. **Serial = barrera de escritor, no solo exclusión entre seriales.** Una suite
   marcada `serial` no se solapa con **nadie**: la primera versión solo la
   excluía de otras seriales y dejaba a una suite normal corriendo en paralelo con
   una que muta el entorno. Es un fallo que encontró el test, no la revisión.

4. **Metadatos declarados donde el comando no dice nada.** `serial` y `locks` no
   se pueden derivar del argv, así que viven en
   `test-harness/config/suite-metadata.json` (versionado, revisable) y se aplican
   al catálogo. Un fichero ilegible es un error, no un aviso: seguir sin metadatos
   paralelizaría suites que el repositorio declaró seriales.

5. **Pin de inventario por suite.** `test-harness/inventory.json` guarda el
   mínimo de casos esperado por suite, derivado de ejecuciones reales
   (`harness inventory --write`) y revisable en el PR. El mínimo entra en la
   huella de entradas, invalida la reutilización de un recibo que ejecutó menos, y
   hace que el gate no acepte ese PASS: es un hueco con nombre, no un verde.
   Sin pin, el suelo sigue siendo 1 caso para Cargo y ninguno para los scripts.

6. **GC que conserva la evidencia.** `harness gc --keep N [--dry-run]` borra solo
   los logs de runs fuera de la ventana y **nunca** toca recibos. Un run en
   `RUNNING` se conserva siempre (puede ser una interrupción en curso) y el
   `--dry-run` es el modo con el que se mide antes de borrar. La operación deja
   evento en el ledger.

7. **Un fallo del supervisor es un hueco, no un crash.** `_execute_guarded`
   captura cualquier excepción de la ejecución, la registra como `INFRA_ERROR` con
   su motivo y sigue: el run termina con veredicto y huecos nombrados en vez de
   morir sin agregar nada.

8. **El marcador `RUNNING` es único por intento.** Incluye el instante de arranque
   en su id porque reanudar vuelve a lanzar la misma suite y los recibos son
   inmutables: reutilizar el id obligaba a reescribir evidencia (y el guardia de
   inmutabilidad abortaba el run, que es lo que hizo el test de SIGKILL).

9. **Lease con reconciliación de dueños muertos.** Un `kill -9` deja el fichero
   del lease. Si el PID del dueño ya no existe y el host es este, el lease está
   huérfano: se reclama y se deja evento (`lease_reclaimed`). Si el dueño vive, o
   el lease viene de otra máquina donde no se puede comprobar, se rechaza.

## Consecuencias

- El harness ya no rechaza `--max-workers > 1`: lo aplica con cerrojos declarados
  y lo registra. Sigue sin paralelizar por defecto.
- Un PASS con menos casos de los pinados no cierra el gate, y un recibo así no se
  reutiliza.
- El ledger se puede podar sin perder evidencia; `--dry-run` lo demuestra.
- 14 tests nuevos cubren cerrojos, solape real, exclusividad serial, SIGKILL +
  reanudación, dos runs con el mismo id, inventario y GC. Dos bugs del supervisor
  salieron de ahí (barrera serial incompleta y colisión del marcador `RUNNING`).

## Lo que este ADR no decide

- Paralelizar de verdad la compilación (target dirs por worker): exige medir si
  compensa y un ADR propio.
- Subir el tope de 8 workers o hacerlo configurable por lane.
- Retención del ledger por antigüedad (hoy: por número de runs).
- El adaptador nextest y las lanes de release (W5).
