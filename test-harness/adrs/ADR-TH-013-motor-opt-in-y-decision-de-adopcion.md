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

Los cuatro llevan test que fallaba antes y pasa después (20 en
`test_selection.py`, 17 en `test_cli_e2e.py`, y el invariante de PASS en
`test_ledger_and_runner.py` con su caso negativo).

## Medición (2026-10-16)

Cambio medido: una línea añadida en `crates/cognicode-ladybug/src/lib.rs`, commit
que sólo toca ese fichero, árbol limpio. Clon del repositorio, caché de Cargo
compartida, ejecuciones secuenciales sin carga competidora. El ledger del harness
se aísla con `--state-dir` para poder medir el caso frío.

| Motor | Selección | Pared | Código |
|---|---|---|---|
| CR-08, primera invocación | 1 suite | **26 s** | 0 |
| CR-08, segunda invocación (mismo árbol) | 1 suite | **26 s** | 0 |
| Harness, ledger frío | 26 suites, 20 ejecutadas | **251 s** | 1 |
| Harness, ledger caliente (mismo árbol) | 26 suites, 3 ejecutadas y 23 reutilizadas | **11 s** | 0 |
| Harness, cambio nuevo en otro paquete (`cognicode-core`) | 27 suites, 7 ejecutadas | **206 s** | 1 |

El código de salida 1 de las dos corridas frías no es del camino de la delegación:
las dos se detienen en `cognicode-core:prf_h06_adversarial_e2e`, que exige el
binario `cognicode-mcp` y **no lo encuentra porque la obligación que lo construye
(`cargo build -p cognicode-mcp`) aparece después en el plan**. Es un defecto real
del orden de ejecución, está declarado abajo y no se disimula con un re-run: la
corrida caliente pasa porque el binario ya existía en el directorio de Cargo
compartido. La medición de tiempos sigue siendo válida (es pared de reloj de las
mismas suites), y el veredicto se reporta como es.

Lo que dicen los números, sin adornos:

- **La primera pasada es mucho más lenta** que CR-08: 26 suites contra 1, con
  compilación de release y `clippy` de workspace dentro. No es una regresión del
  harness —es su política, deliberadamente conservadora— pero sí la razón para
  **no** cambiar el motor por defecto: el wrapper existe para dar feedback rápido
  y el camino estricto tarda varias veces más.
- **La repetición sin cambios es donde el harness gana por goleada**: 22 s contra
  34 s del mismo CR-08, y sin volver a compilar nada. Ese es el caso real «no
  toqué nada, ¿sigue verde?», que CR-08 paga entero cada vez. La ventaja crece con
  el tamaño del plan: el coste de la repetición no depende de cuántas suites haya,
  sólo de cuántas cambiaron.
- **La primera pasada sigue siendo el precio de entrada**, y es el que decide.
  Con 251 s frente a 26 s del mismo CR-08, adoptar el harness como motor por
  defecto haría el bucle del evolutivo unas diez veces más lento en cada cambio
  nuevo, con la caché de Cargo ya caliente.
- **Un cambio incremental no es una primera pasada**: tocar otro paquete cuesta
  206 s, no 251 s, porque la huella sólo invalida lo que depende de ese paquete.
  Sigue siendo muy por encima de CR-08: la diferencia la marca el trabajo que el
  harness exige y el selector no.

## Defecto declarado y abierto: orden de ejecución

`cognicode-core:prf_h06_adversarial_e2e` necesita el binario `cognicode-mcp`
(construido por `cargo build -p cognicode-mcp`, que en merge-gate aparece en la
línea 324 del pipeline y la suite en la 444). El harness **no respeta todavía el
orden declarado de la pipeline**: el plan agrupa y ordena por paquete, así que la
suite puede caer antes que la obligación que la prepara. Con `fail-fast` en la
lane `change`, el run se detiene ahí y las suites siguientes ni se intentan.

No se tapa con un re-run ni con un `--fail-collect`, porque el defecto es de
orden, no de política. El arreglo es que el plan herede el orden en el que la lane
declara sus obligaciones (la misma autoridad que el catálogo ya lee), y va en el
bloque siguiente con su propia medición: es el único defecto de los cinco que
queda abierto, y es la razón por la que la corrida fría no llega a verde en un
clon sin binarios construidos. En una máquina con los binarios ya construidos el
mismo plan da `SATISFIED` (11 s), que es lo que mide la fila caliente.

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
- Los cuatro defectos corregidos valen para **cualquier** consumidor del harness,
  no sólo para este wrapper: la lane de merge o de integración habría ejecutado
  el comando de release-candidate y habría inventado obligaciones de comentarios
  exactamente igual.

## Lo que este ADR no decide

- Que el harness sustituya a CR-08 en ningún gate. Ninguna etapa de PipelineK
  cambia y la autoridad sigue donde estaba (ADR-TH-003).
- El perfil de desarrollo del punto anterior: existe como propuesta, no como
  código.
- Reutilización de recibos en lanes distintas de `change` (sigue opt-in por
  ADR-TH-007) y el orden de adopción por etapas (W6).
