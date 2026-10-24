# ADR-056 — Las tres rutas que construyen el grafo tienen que decir lo mismo

**Estado**: ACEPTADO (decidido el 2026-10-24; pendiente de ejecución)
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

1. **El denominador de la cobertura no es el mismo.** `GraphCoverageMetrics` declara
   que su denominador son «los ficheros de los que se construyó el grafo» y que «un
   fichero sin lenguaje reconocido nunca fue candidato, así que no cuenta»
   (`application/dto/analysis.rs:147-149`). La vía filtrada (`:851`) y la asíncrona
   (`:1182`) filtran por lenguaje. La completa calcula `total_files` **antes** de
   filtrar (`:246`, frente al filtro de `:302`), de modo que cuenta también el
   `Cargo.toml` y el `README.md`. Sobre un repo Rust normal, las tres rutas publican
   la misma cobertura con denominadores distintos, y `coverage_percent` sale distinto
   en cada una.

2. **Un symlink declinado es un hueco en dos rutas y silencio en la tercera.** El
   inventario publica lo declined, porque un symlink que no se sigue es una parte del
   árbol que el grafo no describe (`source_files.rs:199-206`). La vía asíncrona filtra
   los symlinks en silencio (`:1150`) y puede publicar `Complete` sobre un árbol que
   no cubría.

3. **Un directorio que no existe se rechaza en una sola ruta.** La completa lo
   rechaza (`:186-191`); la asíncrona no, y devuelve un grafo vacío.

4. **El orden de los resultados no lo fija la asíncrona.** La completa ordena (`:434`)
   y la filtrada (`:947`); la asíncrona no, así que el índice global de símbolos hereda
   el orden del recorrido en lugar de un orden declarado.

5. **El criterio de cierre del informe no lo verifica ningún test.** El contrato
   `h16_walk_classification_coherence.rs` compara el *reporte* de las tres rutas —y es
   la forma correcta de comparar— pero su corpus no tiene symlinks, y ningún test
   compara aristas ni métricas de cobertura entre rutas. Lo que se compara entre
   vías es la lista de ficheros omitidos, nunca lo que el grafo dice.

## Decisión

1. **El denominador de la cobertura cuenta los ficheros de los que se construyó el
   grafo**, en las tres rutas. La vía completa deja de contar ficheros que el grafo
   nunca consideró.

2. **La vía asíncrona deja de caminar por su cuenta.** Usa el inventario que ya usan
   las otras dos, y con él decide el conjunto de ficheros, lo declined, los errores y
   los huecos: un solo sitio decide qué es una fuente.

3. **Un directorio que no existe se rechaza en las tres rutas**, con el mismo motivo.

4. **La equivalencia es un contrato, no una prosa.** Un test compara, sobre el mismo
   corpus, las relaciones, la cobertura y los huecos de las tres rutas. La forma es
   la del contrato `h16`: un corpus, una aserción que nombra la diferencia, y la
   dirección peligrosa fijada por el nombre del check.

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
- La cobertura deja de depender de qué ruta la construyó: el porcentaje que publica
  el MCP es el mismo que el que ve la CLI.
- Un symlink que el recorrido no sigue es un hueco declarado en las tres rutas, no un
  silencio en una.
- La ruta asíncrona deja de mantener su propio recorrido: una regla de recorrido
  menos que pueda derivar de la otra.

**Negativas.**

- La cobertura de la vía completa baja en los repositorios con ficheros que no son
  fuente (un `Cargo.toml`, un `README.md`). Ese número era demasiado alto: el
  denominador incluía ficheros que el grafo nunca miró.
- La vía asíncrona cambia de origen de ficheros. Es un cambio real en un camino que
  la producción usa desde el MCP, y por eso lleva su propio contrato.

**Lo que esta unidad NO hace.**

- No extrae el motor común. `C23` sigue abierto por su primer criterio de cierre.
- No toca las otras dos vías (`per_file` y `lightweight`): construyen otros grafos y
  tienen su propio contrato (`h16`). Su cobertura, además, la publica otra.

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
