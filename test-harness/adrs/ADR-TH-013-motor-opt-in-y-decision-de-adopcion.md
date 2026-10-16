# ADR-TH-013 — Motor opt-in del wrapper `fast` y por qué no se adopta todavía

**Estado:** ACCEPTED. **Fecha:** 2026-10-16. **Contexto:** la fila W3 del
[roadmap](../roadmap/ROADMAP.md) listaba «Migrar `test-fast.sh`» como pendiente
y el mandato del operador pide valor entregado, no ceremonia. **Autoridad del
gate:** no cambia. Ninguna etapa de PipelineK se mueve aquí.

## Contexto

`scripts/test-fast.sh` es el bucle de feedback del evolutivo. Su autoridad es
`scripts/ci/select-suites.sh` (CR-08): el wrapper detecta los paths cambiados,
le pide la selección y ejecuta él mismo las suites que CR-08 emite. El harness
tiene un planificador con paridad medida contra ese mismo selector (ADR-TH-008),
así que la pregunta era directa: ¿puede el wrapper delegar en el harness?

Dos hechos deciden la respuesta, y los dos se midieron:

1. **La lane `change` del harness no es la lane del wrapper.** El harness exige
   la **clausura de dependientes** del paquete tocado más las etapas de alcance
   workspace (`cargo:build`, `cargo:clippy`, `cargo:fmt`). CR-08 emite, para el
   mismo cambio, las suites del paquete tocado. Para un cambio de una línea en
   `cognicode-ladybug`, CR-08 pide **1** suite y el plan del harness exige **26**.
   Es un mini-gate, más estricto a propósito.
2. **El wrapper y el harness leían bases distintas.** El wrapper mira working
   tree + staged contra `HEAD` y sólo cae a `HEAD~1..HEAD` cuando el árbol está
   limpio; el harness recibía `--base HEAD` fijo, así que con el árbol limpio
   planificaba un cambio **vacío** mientras el wrapper planificaba el último
   commit. Dos motores midiendo cosas distintas es exactamente lo que la paridad
   de este wrapper existe para evitar.

## Decisión

1. **La delegación es opt-in y el defecto no cambia.** `TEST_FAST_ENGINE=harness`
   usa el planificador y el runner del harness; sin esa variable el wrapper se
   comporta como siempre. Un valor desconocido es error de uso (rc=2), no una
   caída silenciosa al defecto.

2. **La delegación falla cerrada cuando no puede cubrir la demanda.** Antes de
   ejecutar, el wrapper pide el plan con `--oracle cr08` (el suelo que impide
   exigir menos que el selector) y **no delega** si:
   - el plan declara un scope `NOT_IN_CATALOG`: CR-08 pide algo que la lane
     `change` no ejecuta (hoy `explorer`, que sólo existe en integración), o
   - el usuario pasó `--paths`: es entrada explícita y el ChangeSet del harness
     sale de git, así que delegar no sería correr más o menos, sería correr
     **otro** cambio y devolver su veredicto como si fuera de este.

   En ambos casos sigue el motor CR-08 y se dice por qué. Un wrapper que ejecuta
   de menos en silencio es peor que uno lento.

3. **La base se alinea.** El wrapper devuelve la base que usó (`HEAD` o `HEAD~1`,
   la que corresponde al diff que leyó) y el harness planifica con ella.

4. **El camino de ejecución se puede inspeccionar sin ejecutarlo.** `harness run`
   acepta `--oracle` y `--plan-only`. `resume` acepta `--oracle` por la misma
   razón: reanudar no puede exigir menos que planificar.

5. **Cinco contratos** (`M12`–`M16` de `scripts/ci/test_fast_full_wrappers.py`):
   el defecto sigue siendo CR-08; un motor desconocido sale 2; la decisión de
   delegar se contrasta contra la paridad real del plan y no contra una lista
   escrita a mano; `--paths` nunca delega; y los códigos 1 (incumplido) y 3
   (indeterminado) del harness no se aplanan a verde.

## Lo que apareció al ejecutar de verdad

Delegar obligó a ejecutar un plan del harness completo por primera vez (hasta
ahora se usaba para planificar, comparar y observar). Salieron tres defectos de
integración que ningún test cubría, y los tres estaban en el camino de la
adopción:

1. **El catálogo era ciego a la lane.** Un mismo `suite_id` se declaraba en
   varias pipelines con comandos distintos, y el lector se quedaba con uno:
   `cognicode-cli:cargo:build#-` es `cargo build -p cognicode-cli --bins` en
   merge-gate e `cargo build --release --target $target -p cognicode-cli --bin
   cogh` en release-candidate, y la lane `change` ejecutaba **el de
   release-candidate**. `origin` además apuntaba al fichero equivocado: `explain`
   decía merge-gate de un comando que vive en release-candidate. Ahora
   `CatalogEntry.argv_by_lane` guarda el comando de cada lane y `argv_for(lane)`
   devuelve el suyo.
2. **Obligaciones inventadas desde comentarios.** `release.pipeline.kts` explica
   en un comentario que el candidato construye `cargo build --release --bin
   cognicode-release`, y el lector lo tomó por una obligación: la lane `release`
   quedó con un argv que incluía la prosa (`… appearing in all three of its`).
   Una obligación inventada es peor que una perdida: se planifica y corre. Los
   comandos dentro de comentarios ya no se leen.
3. **Variables de pipeline sin expandir.** PipelineK expande `$target` con su
   propio intérprete; el harness pasaba el token literal, así que cargo recibía
   `--target $target` y respondía «could not find specification for target
   "$target"». El harness no inventa el valor: sustituye `$target` por el triple
   de la máquina **sólo si las pipelines declaran construir ese target**
   (`val targets`), y cualquier otra variable sin resolver deja la obligación en
   `UNKNOWN`: no se ejecuta, no se reutiliza y el plan queda indeterminado con el
   nombre de lo que falta. Un argv con una expansión viva no es un comando.

Y uno más, del lado del veredicto, que apareció al llegar al final del plan:

4. **Un `--doc` sin doc-tests no es un vacío.** `cargo test --doc` recorre los
   ejemplos de la documentación y un paquete sin ejemplos devuelve 0 casos con
   exit 0. El invariante de PASS exigía ≥1 caso a todo runner de Cargo, y el pin
   de inventario también, así que `cognicode-cli:doc#-` era un hueco permanente:
   18 suites ejecutadas y veredicto `UNSATISFIED` por esa única suite. La regla
   ahora distingue los doc-tests (0 casos es legítimo, y un pin explícito puede
   exigir más) y sigue exigiendo ≥1 para el resto: un filtro que no casa o una
   feature que apaga pruebas sigue siendo un verde vacío.

5. **El plan no respetaba el orden de la pipeline.** `select` y `_apply_oracle`
   ordenaban por `suite_id` (alfabético por paquete) en vez de por el orden en el
   que la lane declara sus obligaciones. `merge-gate.pipeline.kts` declara
   `cargo build -p cognicode-mcp` (línea 324) **antes** que
   `prf_h06_adversarial_e2e` (línea 444) precisamente porque la suite necesita ese
   binario; el plan ponía la suite primero y, con fail-fast, el run se detenía ahí
   sin ejecutar el resto. El orden es ahora el declarado, con `suite_id` como
   desempate para que el plan siga siendo reproducible.
6. **Las suites no recibían el entorno de la lane.** El pipeline exporta
   `CARGO_TARGET_DIR="$(scripts/ci/target-dir.sh)"` para las etapas que buscan
   binarios construidos (líneas 441-442 y 476-477) y el harness ejecutaba con el
   entorno ambiente: `prf_h06` buscaba `cognicode-mcp` en `$CARGO_TARGET_DIR` y no
   lo encontraba mientras el gate sí. El harness resuelve ahora el target con **el
   script del repositorio** (la misma autoridad que el gate) y lo deja escrito en
   el estado del run como evidencia.
7. **La huella de una suite no incluía sus dependencias.** Sólo miraba el subárbol
   de su propio paquete, así que un cambio en `cognicode-core` **no** invalidaba
   los recibos de `cognicode-cli`: la suite se seleccionaba por impacto (bien) y se
   reutilizaba un resultado de antes del cambio (mal). Es el peor modo de fallo del
   harness —un verde falso por construcción— y estaba escondido detrás de los dos
   anteriores: parecía «reutilización eficiente» (23 suites reutilizadas tras un
   cambio en `core`) cuando era un resultado que ya no valía. La clausura incluye
   ya las dependencias transitivas, y esa corrida pasó de «23 reutilizadas» a «0
   reutilizadas, 34 ejecutadas», que es lo correcto.

Los siete llevan test que fallaba antes y pasa después: 20 tests nuevos (8 en
`test_selection.py`, 7 en `test_cli_e2e.py`, 5 contratos M12–M16 en
`scripts/ci/test_fast_full_wrappers.py`) más el invariante de PASS ampliado en
`test_ledger_and_runner.py` con su caso negativo. Dos de ellos —el orden de
ejecución y las dependencias en la huella— se comprobaron además **revirtiendo el
arreglo** y viendo caer el test, para que no pasaran por casualidad.

## Medición (2026-10-16)

Cambio medido: una línea añadida en un crate, commit que sólo toca ese fichero,
árbol limpio. Clon del repositorio, caché de Cargo compartida, ejecuciones
secuenciales sin carga competidora. El ledger del harness se aísla con
`--state-dir` para poder medir el caso frío.

| Motor | Cambio | Selección | Pared | Código |
|---|---|---|---|---|
| CR-08, invocación 1 | ladybug | 1 suite | **26 s** | 0 |
| CR-08, invocación 2 (mismo árbol) | ladybug | 1 suite | **29 s** | 0 |
| Harness, ledger **frío** | ladybug | 26 suites, 26 ejecutadas | **337 s** | 0 (SATISFIED) |
| Harness, repetición sin cambios | ladybug | 26 suites, 3 ejecutadas / 23 reutilizadas | **11 s** | 0 |
| Harness, repetición sin cambios (vía wrapper) | ladybug | 26 suites, 4 ejecutadas / 22 reutilizadas | **42 s** | 0 |
| Harness, cambio en otro paquete | `core` | 34 suites, 34 ejecutadas | **533 s** | 0 (SATISFIED) |
| CR-08 sobre el mismo cambio | `core` | 1 suite | **89 s** | 0 |

Lo que dicen los números, sin adornos:

- **La primera pasada es mucho más lenta** que CR-08: 26 suites contra 1, con
  `clippy` de workspace y compilaciones dentro. No es una regresión del harness
  —es su política, deliberadamente conservadora— pero es la razón para **no**
  cambiar el motor por defecto: el wrapper existe para dar feedback rápido y el
  camino estricto tarda más de diez veces más con la caché de Cargo ya caliente.
- **La repetición sin cambios es donde el harness gana**: 11-42 s contra 26-29 s
  del mismo CR-08 y sin recompilar. El caso real «no toqué nada, ¿sigue verde?»
  cuesta una fracción, y el coste no crece con el tamaño del plan: sólo se ejecuta
  lo que cambió.
- **Un cambio en un paquete del que todos dependen se paga entero.** Tocar
  `cognicode-core` invalida 34 suites: la huella es honesta y no hay atajo. Es el
  precio de no reutilizar resultados que ya no valen, y se declara como lo que es,
  no como una regresión de rendimiento.
- **La adopción como motor por defecto queda descartada con datos**: 337 s frente
  a 26 s en el caso más común. Lo que este ADR entrega es el mecanismo, la paridad,
  los siete defectos corregidos y el número con el que decidir.

## Consecuencias

- El mecanismo existe, está gated y es reversible: quitar la variable de entorno
  devuelve el comportamiento anterior, y el contrato lo fija por si alguien lo
  cambia sin querer.
- La adopción como motor por defecto **no se toma aquí**. Adoptarla sin más
  cambiaría velocidad por rigor en el bucle del evolutivo, y el rigor ya lo da el
  gate; el bucle necesita lo contrario.
- El requisito para adoptarla está identificado y **no** se ha construido: un
  perfil del harness para el bucle de desarrollo (paquetes tocados y sus tests,
  sin clausura de dependientes y sin etapas de alcance workspace) con su propia
  medición antes/después. Decidirlo es del operador (fila W6 del roadmap), no de
  este ADR.
- Mientras tanto la puerta es barata: `TEST_FAST_ENGINE=harness` produce planes y
  recibos reales por cada corrida, y esa es la evidencia con la que se decide.
- Los siete defectos corregidos valen para **cualquier** consumidor del harness.
  El más grave —la huella sin dependencias— habría reutilizado en la lane de merge
  recibos de suites cuyo código compilado cambió, no sólo aquí.

## Lo que este ADR no decide

- Que el harness sustituya a CR-08 en ningún gate. Ninguna etapa de PipelineK
  cambia y la autoridad sigue donde estaba (ADR-TH-003).
- El perfil de desarrollo del punto anterior: existe como propuesta, no como
  código.
- Reutilización de recibos en lanes distintas de `change` (sigue opt-in por
  ADR-TH-007) y el orden de adopción por etapas (W6).
