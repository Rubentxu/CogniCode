# ADR-TH-008 — Paridad medida con el selector vigente, y qué significa el suelo

**Estado:** ACCEPTED. **Fecha:** 2026-10-15. **Contexto:** W3/W6 del roadmap del
harness, tras la implementación del núcleo (ADR-TH-007). **Autoridad del gate:**
no cambia; el oráculo sigue siendo `scripts/ci/select-suites.sh` (CR-08).

## Contexto

TH-S01 pide «preservar el selector CR-08 como oráculo de compatibilidad mientras
migra; comparar sus decisiones antes de cambiar la cobertura». Hasta ahora esa
comparación no existía, y sin ella cualquier recorte de obligaciones era una
afirmación sin respaldo. La primera medición encontró tres diferencias reales y
dos bugs del harness, que es exactamente para lo que sirve tener el oráculo
delante.

## Decisión

1. **Comparación mecánica por scope, con dirección explícita.** CR-08 habla de
   suites (`core`, `explorer`, `mcp`, `cli`, `ladybug`) o de `fallback` (= todo);
   el harness habla de obligaciones por suite. Cada scope se traduce 1:1 a su
   paquete (CR-08 emite un nombre por suite y, cuando acopla dos crates, emite
   los dos nombres). El resultado es uno de cinco veredictos:
   `AGREE`, `HARNESS_STRICTER`, `DECLARED_DEVIATION`, `HARNESS_WEAKER`,
   `ORACLE_UNKNOWN`. `HARNESS_WEAKER` sin desviación declarada es un fallo, no un
   matiz.

2. **Desviaciones declaradas, cortas y atribuidas al motivo del propio oráculo.**
   Se conceden por el `reason` que emite CR-08, no por una lista de paths
   inventada aquí:
   - `docs_changed`: CR-08 exige `ladybug` para cualquier cambio en `docs/**` como
     heurística conservadora; el harness exige solo los paquetes y scripts que
     citan textualmente el fichero.
   - `unmatched_slashed_rs` y `unknown_path`: el oráculo cae a ALL porque su lista
     de reglas no menciona el path. La desviación **solo** se concede si el
     harness tiene clasificación positiva de todos los paths cambiados (miembro
     del grafo de cargo, `docs/**`, o documentación de raíz de la lista
     verificada). Si no la tiene, la caída del oráculo se respeta y la diferencia
     se reporta como debilidad.

3. **Un scope que ninguna obligación de esta lane ejecuta no es una debilidad.**
   El vocabulario de CR-08 es anterior al reparto de lanes: nombra `explorer`, y
   las suites de explorer solo corren en la lane de integración. Cuando el oráculo
   pide un scope que el catálogo de la lane no puede cubrir con ninguna obligación
   de test, se declara `NOT_IN_CATALOG` con nota, en vez de inventar un hueco.

4. **Suelo del oráculo opcional en el plan.** `plan --oracle cr08` añade las
   obligaciones que CR-08 exige y la selección por impacto no cubría, con el
   motivo escrito («el oráculo CR-08 lo exige … y el harness no puede demostrar
   que sobre»). Si el oráculo no se puede leer, el plan exige todo y queda
   `UNKNOWN_INPUT`: sin oráculo legible no hay forma de demostrar que no se está
   por debajo.

5. **Acoplamientos del oráculo respetados en la selección.** CR-08 revalida
   `core` cuando cambian `mcp` o `explorer`, porque el contrato que leen esas
   suites vive en core. El harness lo copia como regla declarada
   (`ORACLE_COUPLINGS`) mientras no demuestre lo contrario; sin ella, un cambio en
   mcp habría dejado de ejecutar suites que el gate de hoy sí exige.

6. **Sin etapa de sombra en las pipelines.** El harness ya se ejecuta dentro del
   gate como contrato (`scripts/ci/test_harness_contract.py`), y ese contrato
   incluye ahora la comprobación de paridad. Añadir una etapa `harness plan
   --shadow` cambiaría la forma de la pipeline (y los contratos que la pinen) sin
   añadir evidencia nueva: la paridad se mide en cada merge, con corpus y con el
   selector de verdad. Cuando exista una razón para consumir el plan desde el
   gate (por ejemplo, sustituir una selección), la etapa nacerá con esa razón.

## Consecuencias

- El harness no puede reducir cobertura respecto al selector vigente sin que el
  gate lo note: la comprobación de paridad corre en la etapa de contratos.
- Medición del corpus (2026-10-15, repo real): **0 debilidades sin declarar** en
  las lanes `change`, `merge` e `integration`; 5 clases `HARNESS_STRICTER`
  (el harness exige más que el oráculo), 9 `AGREE` y 2 desviaciones declaradas.
- El corpus cuesta ~1,3 s y el contrato completo ~95 s (incluye la suite del
  harness), dentro del presupuesto de la etapa de contratos.
- Dos bugs del harness salieron a la luz por esta comparación, y ambos eran del
  tipo que produce falsos verdes: el presupuesto del barrido de citas se agotaba
  y la búsqueda devolvía «nadie cita» (falso negativo), y la atribución se
  quedaba con la primera cita, perdiendo paquetes. Los dos están corregidos y con
  test.

## Lo que este ADR no decide

- Sustituir CR-08. Sigue siendo el oráculo; el harness mide y no decide la
  cobertura de la lane de merge.
- Migrar `scripts/test-fast.sh` al harness (W7).
- La política de reutilización en lanes oficiales, que sigue en `merge` opt-in
  (ADR-TH-007).
