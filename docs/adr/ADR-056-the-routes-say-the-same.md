# ADR-056 — Las tres rutas que construyen el grafo tienen que decir lo mismo

**Estado**: EJECUTADO (decidido y aplicado el 2026-10-24; verificado en el código y en la suite)
**Fecha**: 2026-10-24
**Decisores**: Maintainer
**Contexto**: H24 · construcción del grafo del proyecto (`crates/cognicode-core/src/application/services/analysis_service.rs`), candidato `C23` de la cola de la misión

---

## Contexto

### Lo que dice el informe externo

La auditoría del 2026-10-24, en su hito H4 —«Un único motor de grafos»—, propone
compartir inventario, lectura, resolución y reporte entre las variantes síncrona,
filtrada y asíncrona. Su criterio de cierre es: «un mismo corpus produce relaciones y
cobertura equivalentes por las tres vías».

El roadmap ya tenía dueño para este defecto: la auditoría interna del 2026-10-21 lo
anotó como «los constructores que no comparten motor» y le dio los candidatos `C16` y
`C23`. `C16` está cerrado; `C23` es el que sigue vivo en la cola.

### Lo que muestra el árbol

**MEDIDO 2026-10-24**, por lectura estática del código.

- Hay **cinco** constructores vivos, no tres: `build_project_graph` (`:171`),
  `build_project_graph_filtered` (`:806`), `build_project_graph_async` (`:1083`),
  `build_graph_per_file_with_report` (`:687`) y `build_graph_lightweight` (`:632`).
  Las tres primeras construyen el grafo del proyecto y las tres son **de producción**:
  la completa desde la CLI, la filtrada desde `workspace_session.rs:1171`, y la
  asíncrona desde el handler MCP `aix_handlers.rs:1979`.
- **El lector de árbol ya está unificado**: las tres llaman a `read_file_facts`
  (`:358`, `:915`, `:1264`). La duplicación está aguas abajo del lector, no en él.
- **El inventario centralizado sólo llega a dos de las tres**: `SourceInventory`
  (`infrastructure/graph/source_files.rs:107`) lo usan la vía completa (`:220`) y la
  filtrada (`:831`). La asíncrona camina por su cuenta (`:1132-1168`).
- Resolución, caché y reporte están triplicados: 111 líneas idénticas entre la
  completa y la filtrada, 80 comunes a las tres.

### Las divergencias, medidas

**MEDIDO, y hay una afirmación que no se sostiene: el denominador de la
cobertura.** `GraphCoverageMetrics` declara que su denominador son «los ficheros de
los que se construyó el grafo» y que «un fichero sin lenguaje reconocido nunca fue
candidato, así que no cuenta» (`application/dto/analysis.rs:147-149`), y las tres
rutas lo cumplen: la completa cuenta `files_with_lang` (`:594`), la filtrada filtra
por lenguaje antes de contar (`:851`), y la asíncrona también. Medido sobre un
corpus con un manifiesto y un readme, las tres publican la misma cobertura. Esta
unidad no lo arregla porque no estaba roto: **lo fija**, porque nada lo fijaba.

Las divergencias que sí existen:

1. **Un symlink declinado es un hueco en dos rutas y silencio en la tercera.** El
   inventario publica lo declined, porque un symlink que no se sigue es una parte del
   árbol que el grafo no describe (`source_files.rs:199-206`). La vía asíncrona
   filtra los symlinks en silencio (`:1150`) y publica `Complete` sobre un árbol que
   no cubría.

2. **Un directorio que no existe se rechaza en una sola ruta.** La completa lo
   rechaza (`:186-191`); la filtrada y la asíncrona no, y contestan con un grafo
   vacío y `Complete`.

3. **El orden de los resultados no lo fija la asíncrona.** La completa ordena
   (`:434`) y la filtrada (`:947`); la asíncrona no, así que el índice global de
   símbolos hereda el orden del recorrido en lugar de un orden declarado.

4. **El criterio de cierre del informe no lo verifica ningún test.** El contrato
   `h16_walk_classification_coherence.rs` compara el *reporte* de las tres rutas —y es
   la forma correcta de comparar— pero su corpus no tiene symlinks, y ningún test
   compara aristas ni métricas de cobertura entre rutas. Lo que se compara entre
   vías es la lista de ficheros omitidos, nunca lo que el grafo dice.

## Decisión

1. **La ruta asíncrona deja de caminar por su cuenta.** Usa el inventario que ya
   usan las otras dos, y con él decide el conjunto de ficheros, lo declined, los
   errores y los huecos: un solo sitio decide qué es una fuente.

2. **Un directorio que no existe se rechaza en las tres rutas**, con el mismo motivo.

3. **El plegado se hace en orden de ruta, en las tres.** El índice de nombres
   repetidos guarda los candidatos en el orden en que los recibe, así que el orden
   del recorrido es parte de la respuesta.

4. **La equivalencia es un contrato, no una prosa.** Un test compara, sobre el mismo
   corpus, las relaciones, la cobertura, los huecos y el orden de las tres rutas. La
   forma es la del contrato `h16`: un corpus, una aserción que nombra la diferencia, y
   la dirección peligrosa fijada por el nombre del check.

## Alternativas consideradas

**Extraer ya el motor común**, que es lo que el informe propone y lo que el primer
criterio de cierre de `C23` describe («el archivo se parte por responsabilidad»).
Es un cambio de más de ochocientas líneas sobre un fichero de 4.369, y sin una
garantía de equivalencia que lo ate, puede congelar la divergencia con otro nombre:
extraer un motor común a partir de tres copias que no coinciden es un motor común
incorrecto, y el coste de descubrirlo después es el del grafo entero. Se hace
después, con la equivalencia verificada como red.

**Un test por ruta**, uno para cada constructor. Ya es lo que había, y es la razón de
que la divergencia sobreviviera: cada ruta pasaba su propio test mientras contestaba
distintas a la misma pregunta.

**Que la vía asíncrona sea la que cambia** en todo: es la que menos se usa desde la
CLI y la que más se usa desde el MCP, así que su respuesta es la que más consumidores
ven. Su archivo declinado no lo cuenta nadie.

## Consecuencias

**Positivas.**

- Las tres rutas contestan lo mismo sobre el mismo árbol, y el contrato lo dice.
- Un symlink que el recorrido no sigue es un hueco declarado en las tres rutas, no un
  silencio en una.
- Un directorio que no existe se rechaza en las tres, con el mismo motivo.
- La ruta asíncrona deja de mantener su propio recorrido: una regla de recorrido
  menos que pueda derivar de la otra.
- La cobertura no cambia, porque las tres ya contaban lo mismo. Lo que cambia es que
  ahora un test lo dice, y que el hueco de un symlink ya no desaparece.

**Negativas.**

- La ruta asíncrona cambia de origen de ficheros. Es un cambio real en un camino que
  la producción usa desde el MCP, y por eso lleva su propio contrato.

**Lo que esta unidad NO hace.**

- No extrae el motor común. `C23` sigue abierto por su primer criterio de cierre.
- No toca las otras dos vías (`per_file` y `lightweight`): construyen otros grafos y
  tienen su propio contrato (`h16`). Su cobertura, además, la publica otra.

## Ejecución

**El recorrido.** `build_project_graph_async` ya no camina por su cuenta. Usa
`SourceInventory::inventory`, el mismo inventario que usan la vía completa y la
filtrada, y con él decide el conjunto de ficheros, lo declined, los errores y los
huecos. Un symlink que el recorrido declina es ahora un hueco declarado en el
reporte, no un silencio: la ruta podía publicar `Complete` sobre un árbol que no
describía, y `Complete` es la dirección cara.

**El directorio que no existe.** La vía filtrada y la asíncrona lo rechazan ahora,
con el mismo motivo y el mismo tipo de error que la vía completa. Antes contestaban
con un grafo vacío y `Complete`.

**El orden.** La ruta asíncrona pliega los resultados en orden de ruta, como las
otras dos. El índice de nombres repetidos guarda los candidatos en el orden en que
los recibe, así que el orden del recorrido era parte de la respuesta.

**El contrato.** `crates/cognicode-core/tests/h24_the_routes_say_the_same.rs`
compara, sobre el mismo corpus, la cobertura, las relaciones, la resolución de un
nombre repetido, el orden de lo reportado, los huecos y la respuesta ante un
directorio inexistente. Siete checks: cuatro discriminantes y tres guardas.

**El RED, medido antes del arreglo.** Tres de los cuatro discriminantes fellaron,
con su motivo literal:

- `the filtered route built a graph over a directory that does not exist: Ok(…)` —
  la vía completa ya lo rechazaba; la filtrada y la asíncrona no.
- `the route async declined a symlink and reported Complete: the graph does not
  describe what is behind the link, and a coverage that says otherwise is a claim
  nothing backs`.
- `the route async reported the symbols in the order the machine listed the files
  in, not the order their paths say: ["…/d2.rs", "…/d2/x.rs", "…/d1.rs", …]`.

Los otros tres checks —la cobertura, las relaciones y que el grafo siga siendo un
grafo— pasaban **antes** y pasan **después**: son las guardas que impiden que la
equalización se convierta en un grafo vacío.

**Lo que la medición corrigió.** Este ADR afirmaba, al escribirse, que el
denominador de la cobertura divergía entre rutas. El test lo desmintió en la
primera ejecución: las tres contaban `files_with_lang`, y ya coincidían. El ADR lo
dice ahora con esas palabras. Una afirmación en prosa que nadie midió es una
hipótesis, y la casa la trata como una hipótesis hasta que el árbol la confirma o
la desmiente.

**El GREEN.** 7/7. La suite del core entera: 3.116 pruebas y 0 fallos.

**Lo que queda abierto.** El motor común no está extraído: `C23` sigue vivo por su
primer criterio de cierre. Lo que esta ejecución deja es la red que hace segura la
extracción.

## Referencias

- `crates/cognicode-core/src/application/services/analysis_service.rs` — las cinco
  vías y sus números de línea.
- `crates/cognicode-core/src/infrastructure/graph/source_files.rs` — el inventario
  compartido y lo que publica.
- `crates/cognicode-core/src/application/dto/analysis.rs` — el contrato del
  denominador de la cobertura.
- `crates/cognicode-core/tests/h16_walk_classification_coherence.rs` — el precedente:
  comparar rutas, no fijar cada ruta.
- ADR-055 — el mismo criterio aplicado al puerto de rutas: una promesa falsa se quita
  del tipo, no de la prosa.
