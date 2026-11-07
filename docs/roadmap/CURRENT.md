# CURRENT — Snapshot operativo (deriva de `docs/roadmap/ROADMAP.md`)

> **Autoridad**: `docs/roadmap/ROADMAP.md` es la **única autoridad de agenda**
> de CogniCode. Este fichero es un **snapshot puntual**, no una segunda fuente
> de estado: donde este documento y el ROADMAP diverjan, manda el ROADMAP y
> este snapshot se considera obsoleto hasta su próxima regeneración.
>
> **Snapshot**: 2026-11-07, sobre `5c813212050cb7927119e910b0109e13fefa2be9` (la
> incrementalidad ya está integrada: el grafo se reconstruye sólo donde cambió,
> el servicio dice cuánto conservó, y el escaneo declara los ficheros que no
> puede fechar ni los que no puede leer; los recuentos que el producto
> publica están medidos: la línea base es el grafo, y lo que no se pudo medir
> se declara en vez de publicar un cero; un nombre que lleva varios
> símbolos ya no se resuelve en silencio: la ambigüedad se publica con sus
> candidatos; una relación que el almacén rechazó ya no se descarta: se
> nombra, y el número de relaciones que se publica es el que el grafo
> realmente tiene; y el catálogo publicado dice de su runtime la versión que
> ese runtime reporta: el `runtime_metadata` del maestro es el espejo de lo
> que el runtime publica, y las claves que el runtime no publica siguen
> curadas a mano; y la suite E2E de navegador de Explorer **ya no existe**: se
> retiró el 2026-11-04 junto con las puertas de merge y las lanes de CI
> —medido en `67c474e2`—; lo que queda son dos clases de test y solo dos:
> unitario, e integración con mocks (MSW); y las relaciones
> Kotlin↔Java ya no se publican por coincidencia de nombre: un corpus
> versionado declara los homónimos y los overloads que NO se pueden probar, y
> un contrato comprueba que de esos nombres ambiguos no nace ninguna arista,
> que las cuatro relaciones que sí se pueden probar sí se publican —de Kotlin
> a Java, de Java a Kotlin, de Kotlin a Kotlin y de fichero a fichero—, y que
> la forma de `TargetResolution` queda fijada: tres variantes, no cuatro, y
> con la razón escrita de por qué la cuarta no existe; y una referencia que el
> grafo no puede resolver ya no desaparece sin decirse: una llamada ambigua —
> el proyecto declara ese nombre varias veces y la llamada no dice cuál— se
> publica con su identidad —qué llamada, en qué fichero y en qué línea— y con
> su motivo —cuántos candidatos tiene el nombre—, y el build deja de declarar
> `Complete` cuando no pudo atarla: `Complete` pasa a significar «no quedó nada
> sin atar», ni un fichero sin leer ni una referencia sin atar a una
> declaración; las dos estrategias de build publican la misma pérdida, porque
> las dos resuelven contra el mismo índice del proyecto; y la forma del
> dominio no cambia: `TargetResolution` sigue con tres variantes y la negativa
> se modela donde ya había forma para ella —`PendingClass` y
> `PendingReference`—, con un valor de dominio nuevo que transporta la línea
> donde la llamada está escrita; y las dos superficies ya no se comparan por
> separado: la equivalencia entre la CLI y el MCP se mide sobre el corpus de
> Kotlin y se sostiene —son adaptadores sobre un mismo servicio, así que tienen
> que decir lo mismo del mismo proyecto—, y la medición es del CONTENIDO de la
> negativa y no de su recuento: el corpus de Terraform sólo pierde una
> referencia que nadie declara, así que un número basta, pero el de Kotlin
> pierde además las referencias AMBIGUAS —el nombre está declarado varias
> veces y la llamada no dice cuál—, y la negativa tiene una identidad —qué
> llamada, en qué fichero y en qué línea— y un motivo —cuántos candidatos
> llevan el nombre— que las dos superficies tienen que publicar IGUALES;
> medido: las dos publican las mismas dos negativas
> —`demo/gamma/Caller.kt:3 → run [calls, ambiguous, 3]` y
> `demo/gamma/UseHandler.kt:6 → invoke [calls, ambiguous, 2]`—, el mismo veredicto
> (`partial`) y el mismo grafo no vacío —29 símbolos y 4 relaciones—; la
> equivalencia SOSTENÍA, así que el residuo era la MEDICIÓN y no un defecto:
> esta unidad no toca el código de producción; y la respuesta que el MCP publica
> cuando sirve el grafo desde la caché ya no es un veredicto hueco: el manifiesto
> sólo cubre los ficheros de los que se construye el grafo, de modo que una
> entrada que el recorrido DECLINA —un enlace simbólico que no sigue— era
> invisible para la comprobación de frescura, y la caché respondía `complete`
> sobre un árbol que una construcción en frío del MISMO árbol declara `partial`;
> medido: el grafo era el mismo —3 símbolos y 0 relaciones— y lo que mentía era
> el veredicto; ahora la respuesta servida de la caché declara los huecos del
> árbol TAL COMO ESTÁ AHORA y lleva el veredicto que una construcción en frío
> daría —`partial`, con `src/link.rs` declarado como `symlink`—, y los huecos
> tienen un solo dueño: `SourceInventory::coverage_holes`); y el lazo de
> feedback ya no puede dar verde sin mirar el test que se acaba de escribir:
> el modo rápido que existía tenía DOS caminos —`changed`, que corría la
> biblioteca del crate y el target de integración que los paths tocaban, y
> `fallback`, al que se entraba cuando el selector no clasificaba el cambio—,
> y el segundo se saltaba el primero entero, de modo que corría
> `cargo test --workspace --lib`, que son SÓLO pruebas unitarias; ese modo
> rápido **ya no existe**: se retiró con las puertas de merge, y lo que queda
> es un gate de tres pasos —unitarios, integración con mocks y mutantes— que
> se corre entero y no por subconjunto;
> y la línea publicada es una: el grafo contaba desde cero y el producto desde
> uno, y cada superficie publicaba la suya; la conversión vive ahora en un
> sitio —`published_line()`— y el esquema lo dice en cada propiedad que publica
> una línea; el `navigate` de la CLI pedía al backend la línea de encima de la
> que la persona nombraba, y ahora la posición viaja tal como se escribió; y el
> core no imprime: devuelve el resultado y el presentador lo escribe, así que
> `--format json` produce un documento y nada más, y el doctor sale con 1 sin
> que la biblioteca mate el proceso;
> y el MCP contesta donde el contrato lo puso: una tool que publica un
> `outputSchema` entrega su respuesta en `structuredContent` y su resumen en el
> segundo bloque de texto, el protocolo declarado es `2025-11-25`, las tools
> publican sus pistas de comportamiento y el JSON ya no lleva saltos de línea;
> y el MCP dice por qué se reporta un hallazgo: `explain_finding` publica el
> mensaje, la cadena causal, el detector y el veredicto de grounding de cada
> hallazgo, y una lista vacía sin desenlace ya no es indistinguible de «no hay
> hallazgos»;
> y la paginación se compara con el catálogo maestro: el oráculo del recorrido
> es la declaración del producto —la que el servidor lleva dentro del binario— y
> no un catálogo calculado en otro build, que es lo que tenía el gate entero en
> rojo;
> y el análisis del grafo es reproducible bit a bit: la detección de
> comunidades, los ciclos, los god nodes, los llamantes, los caminos calientes,
> las dependencias entre módulos, los símbolos del grafo y las aristas del
> grafo publican el mismo orden en cada corrida del mismo proyecto; el sitio
> del orden se decidió con la cifra —si la lectura se repite en cada
> publicación, el orden va en la escritura (`BTreeSet`/`BTreeMap`)—, y hay un
> dato que sorprende: con claves largas (`fichero:nombre:línea`) el `BTreeMap`
> busca MÁS RÁPIDO que el `HashMap`, porque SipHash recorre la clave entera;
> y el grafo de un proyecto real termina: la complejidad y el conjunto de
> retroalimentación ya no se quedan sin acabar sobre código de verdad, y el
> arreglo —cada grupo entra una sola vez— se midió en 46,5 MB; y el lazo más
> largo mide el orden topológico, que era un agujero real del crate de
> algoritmos de grafo;
> y el valor del daemon es el hash de frescura: un daemon vivo dice que su
> índice está fresco y el cliente lo sirve sin releer el árbol; un árbol sin
> nada que leer tiene un estado, y las dos superficies lo dicen igual;
> y el paso de mutantes del gate mide de uno en uno, porque su veredicto es
> por mutante: en paralelo, siete mutantes de un mismo fichero parecían
> supervivientes y de uno en uno los diez son atrapados; el veredicto ya no
> confunde un mutante atrapado con un tiempo agotado; y toda corrida de
> mutación lleva techo de memoria, con un dueño del presupuesto y dos rutas
> que lo leen;
> y los tests del CLI ya no escriben la configuración real del IDE: la
> escritura atómica es la del core —temporal exclusivo— y cada test apunta el
> HOME y las variables del IDE a su propio directorio; la contención que
> cubría la carrera vieja se retiró con medición, y el gate no se abreviaba
> con ella: los binarios del CLI no están en su camino crítico;
> y el vocabulario publicado tiene dueño: el formato del diagrama lo declara
> el dominio, la estrategia del índice la declara la fábrica, el nivel que
> publica `project_overview` es el que entrega, y el diagrama de
> `export_callflow` dibuja las comunidades que dice;
> y los argumentos de una tool se declaran una vez —la forma se deriva del
> tipo— y la columna publicada es una, como la línea;
> y el gate del lote de diez pasó, medido en serie: 4701 unitarios, 6794 de
> integración y 627 mutantes —618 atrapados, 9 inviables, 0 supervivientes—;
> y desde entonces: la complejidad y el conteo de parámetros tienen una sola
> medición; `reparse_on_edit` distingue un fichero borrado de uno ilegible; y el
> servidor real exige el token en `/mcp` y `/watch`;
> y el snapshot operativo se regenera sobre la punta: el ratchet acepta cinco
> commits de deriva contra la base con `main`, y el SHA declarado se había
> quedado atrás; la cabecera declara la punta real, la prosa acumula lo que
> llegó con el commit que lo midió, y la tabla cuenta los commits de distancia
> con una fila por unidad;
> y la cobertura tiene un dueño: la decisión de qué estado se publica estaba
> escrita dos veces —la CLI y el MCP—, y el presenter del MCP la escribía una
> tercera: un outcome completo servido de la caché se publicaba como «unknown»
> mientras el handler vivo publicaba «complete» para esa misma respuesta; ahora
> la decide `coverage_verdict`, en la capa de aplicación, y las dos superficies
> la piden a la misma función;
> y las preguntas dicen la cobertura con la que se respondieron: `analyze_impact`
> y `graph impact` / `graph hierarchy` publican, junto a la respuesta, la
> palabra de la cobertura y los tres hechos que la sostienen —los ficheros que
> no se pudieron leer, las referencias que no se pudieron atar a una
> declaración y las relaciones que el repositorio rechazó—; una respuesta que
> no dice su cobertura dice que lo cubre todo, y un cero publicado sobre un
> grafo parcial se lee como un hallazgo; la cobertura se calcula una vez, al
> cargar el grafo, y el servicio la recuerda para que una segunda pregunta de
> la misma sesión publique la misma cobertura;
> y el reindex incremental dice lo que costó: `reparse_on_edit` publicaba
> cuántos ficheros tocó y cuántos símbolos cambiaron, y el trabajo se repartía
> entre cinco fases —leer el estado, establecer la identidad de cada fichero,
> reinterpretar, escribir en el índice, reconstruir el grafo— con una sola
> suma publicada; ahora publica las fases que corrieron y cuánto tardó cada
> una, los ficheros que tocó y los que consideró, la memoria alta y los
> percentiles de la sesión cuando hay observaciones suficientes; la palabra
> del rebuild la decide el producto y no una etiqueta escrita junto a la
> llamada —`cache_hit`, `full`, `partial`, `error`—, y con ella sale un
> defecto latente: el manifiesto decide por el contenido y el reloj es sólo
> una pista, pero el handler decidía por el reloj solo, así que un fichero
> cuyo contenido había cambiado bajo un reloj intacto se saltaba en
> silencio; y el benchmark que mide el coste no lo corre el gate —un
> benchmark se pide a propósito—, sino que se pide con sus tres formas y sus
> dos tamaños, y comprueba antes de medir que el rebuild que va a medir es el
> que dice medir;
> y el gate mide un radio, y el radio lo decide un script: el gate entero costaba
> 55 m 46 s en caliente y el paso de mutantes era el 98,8% de esa hora —medido
> en el recibo de TASK-69—, así que un gate cuyo coste no depende de lo que
> cambió es un gate que no se puede correr seguido; ahora `scripts/gate_scope.py`
> decide qué se mide y el gate lo corre: el radio de los tests es el cierre
> inverso de dependencias de los crates que el lote tocó, con el grafo de
> `cargo metadata`, y el radio de los mutantes son los crates que el lote tocó
> que son asequibles de mutar más el crate del calendario cuando le toca el
> turno; el radio falla cerrado —si no se puede determinar, el gate corre entero
> y lo dice— y el calendario acota la edad de un hueco de mutación a un número
> declarado de fusiones, porque el crate que lleva la lógica de grafo no se deja
> de medir: se mide en su turno;
> y el entorno de compilación está declarado, y su declaración vive en la máquina
> porque en el repositorio no puede: un `[profile.test]` en el workspace
> invalidaría la caché compartida de `/var/home/rubentxu/cargo-targets/`, que es
> de todos los proyectos de la máquina —regla 4 de «Estrategia de tests»—, así
> que el perfil, el linker y la caché de compilación viven en
> `~/.cargo/config.toml`; lo que el repositorio declara es el instrumento que
> los mide y el contrato que vigila que sigan declarados: el trabajo del
> compilador baja a la mitad con `debug = "line-tables-only"` en vez de
> `debug = 1` ("limited", que lleva **más** información de depuración), medido
> en el recibo de TASK-76 sobre series alternadas antes/después, con el tiempo
> de CPU —el trabajo, no la competencia por la máquina— como cifra robusta: el
> build en frío pasa de 746,75 s a 370,40 s y la reconstrucción del core de
> 482,15 s a 300,99 s; el linker del host es `mold`, declarado y no heredado
> (con `gcc` como driver porque rustc le pasa un `-m64` que `mold` no
> entiende), y `sccache` queda activo con su tasa de aciertos visible, aunque
> aquí su beneficio es limitado: no cachea los binarios, y su clave incluye el
> *fingerprint* de cargo, que cambia con el mtime de las fuentes;
> y el impacto nombra los tests que alcanza, y el comando que los corre: la
> pregunta «qué depende de este nombre» ya tenía un dueño y respondía con los
> símbolos que dependen y con el riesgo, y le faltaba la mitad que el
> consumidor pide —qué tests hay que correr antes de tocar el cambio—; ahora la
> respuesta trae la faceta `affected_tests` con el nombre del test, el
> fichero, la línea y el comando exacto que lo corre, en el MCP, en el CLI y en
> la sesión de workspace; el que decide qué es un test es el LECTOR, porque es
> el único que puede ver la marca —`#[test]`, `@Test`, `[Fact]`—, y una regla
> por nombre no serviría: aquí los tests de integración se llaman
> `the_answer_is_ordered` y los unitarios viven en `#[cfg(test)] mod tests`
> dentro de `src/**`; donde el lenguaje no decide su runner —Java, C#, Kotlin—
> no hay comando, y la respuesta lo dice en vez de inventar uno; y la recall de
> esa selección se mide con un comando que se pide a propósito, fuera de la
> suite y del gate, contra la ejecución completa del paquete: **70,0 %** y
> **28,6 %** en dos símbolos, con las faltas nombradas una a una, así que la
> selección **ordena** y no se promueve a filtro;
> y el ROADMAP ya no declara retirado lo que sigue en el árbol: la nota 2 de §11.1
> mandaba a leer el recibo del JOURNAL porque la poda documental se había llevado
> `docs/adr/ADR-D1-006-uat-retirement.md`, y ese fichero está en el árbol, con la
> decisión aceptada dentro; medido: `git log --all --diff-filter=D` no registra su
> baja, PR #399 (`97259133`) borró seis ficheros y ninguno era este, y los dos SHAs
> que la nota citaba no viven en `main` —el que vive es `4996246e`—, así que la nota
> mandaba a un sitio que ya no era el único y escondía el fichero que sí tenía la
> decisión; ahora la nota cita el ADR que existe y un contrato vigila la clase: una
> ruta que el ROADMAP declara retirada no puede seguir en el árbol, y el RED se midió
> sobre la nota vieja (1 rojo) y el GREEN sobre la nueva, con cinco controles que
> lo sostienen —el ruido del contrato medido antes de escribirlo: 31 casos a nivel de párrafo,
> 14 a nivel de frase con tablas, y **1** a nivel de frase sin ellas—;
> y el goal ya es un concepto con un dueño: el registro
> (`interface::goals::GoalSpec`) declara **una vez** el nombre, el título, la
> descripción con ejemplos, la autoridad, la fase mínima, el dueño y los
> argumentos de un goal, y de esa declaración salen las cuatro superficies por
> las que un consumidor lo ve —el `inputSchema` que el MCP publica, el comando
> clap, la entrada del maestro y la ayuda—; el primero montado es `status`,
> que dice qué ofrece el build y qué se ve del workspace, y los argumentos los
> declara el struct que los lee, así que el flag de la terminal y el campo del
> cable son el mismo argumento con dos nombres; medido: el catálogo pasa de 78
> a 79 tools **sin retirar ninguna**, y la captura del runtime —regenerada con
> el binario de tres features— coincide byte a byte con la que ya lo traía; y
> AC2 —los 78 nombres antiguos registrados como alias con aviso de
> deprecación— **queda cerrado**: los 78 nombres se publican en el perfil
> `legacy` con su aviso, y los 13 que un goal sustituye nombran al goal que
> contesta la misma pregunta —los otros 65 dicen la verdad: que la
> superficie de goals todavía no contesta esa pregunta y que la tool la
> calcula—;
> y la respuesta ya es un **sobre**: cada respuesta dice QUÉ encontró, SOBRE QUÉ
> se apoya, QUÉ dejó fuera y QUÉ se puede hacer ahora; el sobre tiene un solo
> dueño (`interface/hateoas.rs`) y las dos superficies lo adaptan sin decir
> cosas distintas —el MCP lo publica en `structuredContent`, con el resultado
> dentro, la base, el presupuesto y los enlaces, y el CLI lo imprime como
> órdenes copiables—; los enlaces se calculan del resultado (ambigüedad,
> recorte, fan-in alto, cobertura parcial, riesgo y plan de cambio), cada uno
> nombra una tool del catálogo con sus argumentos rellenos y su motivo, y los
> que llevan handle van firmados y anclados al árbol y a la instantánea que
> los acuñó, con un tag de integridad del payload: un handle cuyo payload se
> reescribe no vuelve a pasar, y el adaptador resuelve los handles ANTES del
> despacho, de modo que uno caducado devuelve un error tipado y la tool no
> corre; medido: la cláusula de compatibilidad se mantiene —`content[0]`
> sigue llevando el payload byte a byte—, el `output_schema` publicado pasa a
> ser el del sobre con el contrato de cada tool debajo de `result`, y el
> catálogo no pierde ninguna tool; y la terminal dice la verdad: el
> reindexado se imprime como `cognicode graph full --rebuild` —que reconstruye
> el grafo, no el índice de símbolos—, la dirección de la jerarquía se lee
> del enlace en vez de estar escrita, y el motivo de un enlace cuenta en
> palabras («1 file», no «1 files»);
> y los nueve goals ya son un vocabulario con un dueño: un goal es lo que un
> consumidor **pide** y una tool es lo que el producto **calcula**, así que el
> registro (`interface::goals::GoalSpec`) declara una vez cada goal —nombre,
> título, descripción con ejemplos, autoridad, fase mínima, dueño, argumentos
> y los nombres legados que sustituye— y de ahí salen las cuatro superficies;
> los nueve —`status`, `context`, `find`, `inspect`, `trace`, `impact`,
> `overview`, `audit` y `change`— se montan sobre los dueños que ya
> calculaban eso, sin recalcular nada; y la superficie que el servidor
> anuncia la decide un perfil —`default`, `full`, `legacy`—, con una regla
> que tiene un solo dueño: la fase mínima del goal, de modo que un goal
> nuevo que declara su fase sabe si sale por defecto; medido: el perfil por
> defecto anuncia **cinco goals** y la lista pesa **4.350 bytes** por el
> cable —el presupuesto es de 6 KB—, porque los goals no publican
> `outputSchema`: con el esquema del sobre bajo cada uno, la lista pesaría
> entre 26.420 y 65.890 bytes; el perfil `legacy` publica las 78 tools con
> el aviso que nombra al goal que las sustituye, y ese aviso lo declara el
> goal, no el perfil; y `cognicode` sin argumentos ya es el goal `status`: la
> raíz del producto; medido en vivo: el sobre que imprime la terminal y el
> que publica el cable son **idénticos** —las mismas seis claves con los
> mismos valores—, y un servidor de sólo lectura **no anuncia** `change`
> (83 entradas en vez de 87);
> y una receta es una pregunta compuesta con un dueño: el goal `recipe` —el
> décimo— corre una secuencia de goals en el orden de sus fases, memoiza cada
> paso por `(goal, args, hash del árbol)`, corre en paralelo lo que es
> independiente, y dice lo que costaría **antes** de correrlo; el hash del árbol
> es el mismo que decide si el snapshot está fresco —la autoridad es el
> contenido, no la mtime—, y la fase de cada paso la decide la fase mínima del
> goal, que ya era un hecho publicado; medido: la superficie por defecto **no
> cambia** —siguen siendo cinco goals y **4.350 bytes** por el cable, los
> mismos de la unidad anterior—, porque el goal `recipe` declara la fase
> `Analyze` y el perfil sólo anuncia los goals anteriores; el perfil `full`
> publica **88** entradas y el de sólo lectura **84**, y `change` no está entre
> ellas; el producto trae **cinco recetas** compiladas en el binario
> (`context`, `onboard`, `pre-change`, `post-change` y `review`) y un workspace
> puede añadir las suyas en `.cognicode/goals.toml`: sólo con goals de lectura,
> y una receta que invoque `change` se rechaza **entera** con su motivo; un
> paso que falla deja la receta en `partial`, con el motivo y un enlace
> `retry` —el séptimo caso del sobre— que lleva los argumentos que el reintento
> necesita; y en vivo: el plan de la receta `context` dice **2 pasos y 60 ms**
> sin correr nada, la primera corrida computa 2 pasos en 15 ms, la segunda los
> sirve de la memo en 0 ms, un cambio del árbol los vuelve a computar, y el
> sobre que imprime la terminal y el que publica el cable son **idénticos** —
> con `params` como **objeto** en los dos, que es donde se ve que las dos
> superficies leen lo mismo; y el harness de medición ya sabe contar qué
> enlaces de `_links` se siguen —un contador puro sobre una traza— y producirla
> desde una sesión real de `cognicode-mcp` encadenada, con una política de
> seguimiento declarada: mide el contrato de cable, no a un agente; y toda
> llamada a `subprocess` de los contratos de `scripts/ci/` y del harness de
> `scripts/measure/` lleva plazo (regla dura 7), vigilado por un contrato por
> `ast`, y el SKIP de mutantes del gate dice a cuántas fusiones llega el
> siguiente turno del calendario; y el plazo alcanza también a `scripts/` y
> `scripts/product/`, al recorrido recursivo de `sandbox/scripts` y a todo `curl`
> de los scripts de shell; y la release `v0.102.1` está publicada e instalada
> (la `v0.102.0` no pasó su suite, no tiene release y no se mueve).
> Versión del workspace: `0.102.1`. Última release publicada: `v0.102.1`
> (2026-11-07, 8 assets).
>
> **La historia vive en el Journal**; aquí sólo vive el estado.

## Qué cambió desde el snapshot anterior (`765ea29f` → `5c813212050cb7927119e910b0109e13fefa2be9`)

Entre una punta y otra entraron **6 commits** (`git rev-list --count 765ea29fc3b4eb9eae23b00982aea1603489263c..5c813212050cb7927119e910b0109e13fefa2be9`).
Lo que sigue no es una lista de intenciones: cada fila dice qué se movió y
en qué commits está; la medición de cada unidad está en su recibo del `JOURNAL`.

| Qué cambió | Commit | Lo que se midió |
|---|---|---|
| `scripts/` y `scripts/product` llevan plazo en todo subprocess | `cccb3bdf` (TASK-161) | El contrato por `ast` cubre los scripts raíz y los de producto; una llamada sin plazo es un rojo. |
| El contrato de plazos es recursivo y cubre `sandbox/scripts` | `040af78e` (TASK-162) | El recorrido ya no se detiene en el primer nivel. |
| Todo `curl` de los scripts shell lleva plazo | `dce24aab` (TASK-163) | Un remoto que no contesta es «sin respuesta», no un veredicto. |
| Recibo: la regla dura 7 en Rust no deja deuda | `b62f715b` (sin tarea) | Se midió que no hay llamada de Rust que justifique una tarea. |
| Corte `v0.102.0` (sin release) y `v0.102.1` | `8eec4c63`, `5c813212` (sin tarea) | `v0.102.0` no pasó su suite; `v0.102.1` lleva los dos arreglos y se publicó con 8 assets. |

## Release: la última publicada es la v0.102.1

| Tag | Estado | Por qué |
|---|---|---|
| `v0.101.23` | publicada (2026-10-18) | la anterior |
| `v0.101.24` | tag, sin release | su gate de candidato encontró un defecto real del índice de símbolos |
| `v0.101.25` | tag, sin release | su gate de candidato murió en la última etapa por un comando de shell mal escrito |
| `v0.101.26` | tag, sin release | su gate de candidato murió en `clean-clone` por un defecto real del filtro de recorrido |
| `v0.101.27` | publicada (2026-10-22, 8 assets) | lleva los tres defectos que los gates anteriores encontraron |
| `v0.101.28` | publicada (2026-10-22, 8 assets) | lleva el orden del grafo filtrado y la lane de publicación reparada |
| `v0.101.29` | **publicada** (2026-10-22, 8 assets) | lleva las tres unidades nuevas del bloque B1 |
| `v0.101.30` | publicada (2026-10-23, 8 assets) | lleva la ontología: el campo que el consumidor lee |
| `v0.102.0` | tag, sin release | su suite de candidato encontró dos defectos reales (un cert y un test de snapshot) |
| `v0.102.1` | **publicada** (2026-11-07, 8 assets) | lleva los dos arreglos; instalada en local |

Un tag que no llegó a publicarse no se mueve. `v0.101.27` sí se publicó, tras
pasar su gate de candidato sobre el clon limpio —**6507 pruebas, 0 en rojo**—
y la lane de publicación, que tuvo que arreglar tres checks que no podían
pasar antes de llegar a publicar. Lo que se puede instalar es, por fin, el
corte que lleva el filtro de recorrido arreglado.

**`v0.101.30` es la vigente**, y no movió `v0.101.29`: un tag publicado no
se mueve. Lleva la última de la ontología: un recurso de Terraform y un
play de Ansible dejan de publicarse como una función y como un módulo, y el
campo que el consumidor lee dice qué son. La prueba no se hizo sobre el
árbol del repositorio: se bajaron los assets publicados, sus siete sumas
coincidieron con los bytes descargados, y el servidor MCP publicado
respondió **19 declaraciones y 0 funciones** a un directorio con un `main.tf`.

**`v0.101.29` sigue publicada** y no se movió: el instalador que no
termina el proceso, la señal de material Rust que mira donde mira el grafo y
el grafo vacío que ya no se presenta como un análisis completo.

## La regla que este ciclo destapó

Un gate que muere por cuota de disco **no** dice nada del código que quiere
probar, y confundirse cuesta un ciclo entero:

1. **Cuota de disco.** `/tmp` es un tmpfs con cuota de usuario. Cuando se
   llena, `cargo test` devuelve `os error 122` y **379 pruebas fallan a la vez**
   con un error que no menciona la cuota. El arreglo es mover los temporales a
   un volumen con espacio: `TMPDIR=/var/home/rubentxu/cognicode-tmp`. El
   volumen de compilación ya se movió; los temporales de las pruebas, no.
2. **Defecto real.** El filtro de recorrido excluía cualquier ruta que tuviera
   un directorio llamado `.cache` en cualquiera de sus componentes. El gate de
   candidato clona en `~/.cache/cognicode/gate/`, así que el clon se ciega a
   sí mismo y el proyecto sale vacío. **Arreglado y fusionado**: el filtro ya
   no ciega al clon.
3. **Carriles que se invalidan entre sí.** Dos gates de merge corriendo a la
   vez sobre el mismo repositorio comparten el puntero de la última corrida, y
   se invalidan mutuamente. El síntoma —«cookie file not created» con la
   etapaunderlying en verde— no dice nada del código. Los gates pesados van
   **en serie**, uno detrás de otro.
4. **La máquina decidía el veredicto.** Un repositorio de laboratorio que una
   prueba construye hereda la configuración de git de quien la ejecuta. En una
   máquina con una política de identidad, los commits del laboratorio se
   deniegan, el repositorio se queda sin historia, y las pruebas leen «0
   commits» donde había dos. Affected: nueve pruebas de Rust y cuatro
   contratos. Ninguna fallaba por el producto.
5. **`crates.io` retiró `tokio 1.54.0`** después de la última vez que `main`
   pasó su propio gate, así que `main` llevaba su comprobación en rojo sin que
   nadie lo mirase.

Y tres que encontró la propia lane de publicación, al publicar de verdad:

6. **Un gate que exige un directorio que nadie escribe.** La lane pedía un
   `dist/` que el candidato dejó de escribir en octubre. Pasaba en los
   worktrees viejos, porque el directorio viejo seguía ahí, y rechazaba
   cualquier árbol limpio con un candidato completo. Medía el worktree.
7. **El momento de la comparación es parte del contrato.** La comprobación del
   conjunto publicado corría *antes* de que se subieran los manifiestos, y
   comparaba contra un conjunto que los incluía. No era una comprobación más
   estricta: era otra comprobación, y siempre fallaba.
8. **Un 404 justo después de escribir suele ser tu propia escritura.** La
   prueba de consumidor descargaba los assets segundos después de sustituirlos;
   GitHub da id nuevo a cada asset re-subido, y el id que pedía la descarga
   ya no existía.

Y una que encontró el propio squash de este ciclo:

9. **La cabecera de una fusión la compone la forja, y no pasa por los hooks.**
   El squash crea un commit cuyo mensaje es el título del pull request más la
   referencia del pull request, y ese commit lo crea el servidor: nunca pasa
   por los hooks que juzgan los commits que hace esta máquina. La política lo
   veía por primera vez cuando el clon intentaba seguir a `main`, y lo que
   quedaba era un commit que **ningún** clon podía atravesar con un `git
   pull`. Ahora el título se juzga antes de la fusión, y el publicador del
   gate consulta el juez antes de gastar sus minutos.

Y otra que encontró la ontología, que es de la misma familia:

10. **Un contrato que no se ha visto rojo no mide nada.** Las seis pruebas
    del contrato de la ontología pasaron en verde el día que se escribieron,
    y eso no decía nada: un contrato que no puede fallar puede estar
    midiendo una cosa que no es la que dice medir. La demostración es un
    experimento reversible —revertir sólo los dos lectores y volver a
    leer— y lo que entonces se leyó fue `"function"` para un bucket de S3 y
    `"symbol.module"` para un play de Ansible. Es el mismo cuidado que se
    tuvo con la autoridad de mentira del juez de la cabecera, aplicado al
    otro lado: no basta con que el contrato pase, hay que haberlo visto
    fallar.

11. **Un gate que no empieza no es un gate en rojo.** El `merge-gate` de
    este ciclo se negó a arrancar con el código 2 y un motivo que sí dice
    algo: el volumen de compilación no tenía sitio. El volumen por defecto
    —compartido con los demás checkouts de la máquina— estaba al 99%, y la
    regla que lo vigila pide 30 GiB antes de empezar. La salida ya estaba
    escrita y no es liberar cachés ajenas: `PREFLIGHT_WORK_DIR` mueve el
    trabajo a un volumen con sitio, y este repositorio vive en otro
    volumen con espacio de sobra. Dos cosas se midieron para no elegir a
    ciegas: un fichero de 2 GiB escrito y borrado **no devolvió** el
    espacio, lo que distingue «no se libera» de «otro proceso lo está
    tomando»; y la papelera del sistema acumulaba casi 400 GiB de caché de
    build ya descartada de este proyecto, regenerable por construcción.
    La regla de `build-volume.sh` no es burocracia: es la que convierte una
    muerte por cuota en un motivo que dice la causa y la salida.

12. **Un guard que compara dos listas escritas a mano no vigila la
    verdad, vigila que las dos se parezcan.** La lista de lenguajes que
    el producto declara y la lista de un documento se comparaban entre
    sí, y por eso el guard daba verde mientras las dos mentían: el
    registro —lo único que decide qué puede leer el producto— no estaba
    en la comparación. La regla que sale de ahí es general: un guard
    tiene que medir contra **la autoridad**, no contra otra copia. Y
    cuando la autoridad ya existe y es viva, lo correcto es derivar de
    ella y dejar de mantener la copia.

Un rojo de gate se lee primero: qué etapa, qué error, y si el error es del
código o de la máquina.

13. **Un gate que depende de la red no es un gate.** La etapa `contracts` del
    gate se puso en rojo sin que cambiara una línea del código que certificaba:
    entre la corrida verde y la roja solo había un párrafo del Journal. La
    causa era que un contrato aísla el `HOME` para no heredar nada de quien lo
    ejecuta, y al aislarlo deja a `rustup` sin toolchain, que la descarga desde
    la red: 117,7 s por llamada, con el `timeout=120` del contrato al borde. La
    red decidía el veredicto, y dos corridas del gate de esa misma mañana
    habían fallado igual. La regla: una comprobación que necesita un dato de la
    máquina —el host, la toolchain, la versión— lo recibe como dato, y si no
    puede obtenerlo **falla cerrado y lo dice**, en vez de inventar un valor y
    sacar de ahí un veredicto sobre otra cosa.

14. **Lo que no cambió se conserva, y lo que decide qué se rehace es la
    identidad, no el nombre.** Rehacer el grafo entero por tocar un fichero
    no es lentitud: es trabajo que se tira. Y la pregunta de qué hay que
    rehacer tiene una respuesta que no es «el nombre de la función», sino
    «la identidad de la declaración»: un nombre puede seguir existiendo y
    ser otra cosa —movida de línea, con otra declaración al lado—, y en ese
    caso la respuesta que se guardó ya no vale aunque el nombre sea el
    mismo. La regla que sale de ahí: cuando se decide qué se invalida, se
    compara **lo que identifica**, no lo que se nombra; y lo que se conserva
    se escribe en el libro mayor, porque una respuesta conservada sigue
    siendo una respuesta —un libro que sólo guarda lo nuevo se vacía en
    cada construcción y rehace el proyecto entero sin querer—.

15. **El mensaje de una fusión lo compone la forja, y el juez que lo juzga
    llega tarde.** La lección 9 dijo que el squash no pasa por los hooks; esta
    ventana lo midió: el título del pull request se juzga antes de la fusión
    **si la fusión la pide el publicador del gate**, y no si la pide la CLI de
    GitHub. Medido el 2026-11-05: un squash cuya cabecera llevaba una línea de
    crédito entró en `main` sin que nadie la juzgara, y el primer juez que la
    vio fue el hook local, cuando el clon intentaba seguir a `main` —que es
    decir, cuando el daño ya estaba hecho y el commit ya era la historia. El
    arreglo no es del lado del hook: es que **la fusión se juzga antes de
    ocurrir**, en el mismo sitio que se pide. Y el arreglo del daño ya hecho no
    es reescribir `main` —que está protegido y no se reescribe—: es que el
    crédito que la política prohibe tenga una casa donde sí quepa. La política
    dice cuál: `AUTHORS.md` —que este repositorio aún no tiene— y, mientras
    no exista, el `JOURNAL`. El mensaje del commit no es esa casa: es el sitio
    donde el crédito no puede ir.

## Unidades abiertas

| Unidad | Qué es | Detalle |
|---|---|---|
| **B3** (`C23`) | ontología y arquitectura simplificadas | `docs/adr/ADR-056-the-routes-say-the-same.md` |

**El snapshot dice quién lo produjo, está cerrada.** El producto persiste un snapshot del grafo y lo vuelve a servir cuando las fuentes no han cambiado. Lo que guardaba era el grafo y el manifiesto de contenido; lo que no guardaba era quién lo había calculado. La decisión de servirlo comparaba el manifiesto con el disco y nada más, así que un binario distinto —otra versión, otro commit, otras capacidades— servía el snapshot como si fuera suyo: el consumidor recibía un grafo que había calculado otro analizador, con otra ontología y otras reglas, y nada en la respuesta lo decía. La base publicada declaraba el workspace, la configuración y las fuentes con `complete: true`: una identidad a medias.
La identidad vive ahora en el dominio, en un valor con constructor puro: `SnapshotKey` dice qué se miró (el workspace), quién lo miró (el analizador: su versión, su commit y sus capacidades) y con qué forma se escribió (el esquema). El snapshot la guarda y se decide con ella, y la primera pregunta al cargar es sobre el productor, que no cuesta nada porque se responde sin leer el árbol. Si lo produjo otro analizador, no se sirve: el producto reconstruye y el motivo queda en el log. La base publicada gana cuatro campos que salen de la misma clave con la que se escribe el snapshot, y el catálogo publicado declara los mismos.
El RED está medido sobre el camino malo: los tests nuevos fallaban 3 de 4 y el contrato de CI encontraba 16 hallazgos contra el código anterior. Con el binario real por stdio, un snapshot al que se le cambia la versión declarada no se sirve. Lo que queda de esta unidad —que un cambio no rehaga el grafo entero, y la medición de frío, tibio y edición— está medido y encolado como `C47`.

**Una fecha se publica solo si se midió, está cerrada.** El producto publicaba la marca de tiempo de un fichero como un número de segundos desde la época, y ese número decía siempre algo: cuando la fecha no se podía leer publicaba `0`, que dice «1 de enero de 1970»; y el grafo por fichero, que convertía la distancia a la época en vez del instante, publicaba 1980 para un fichero de 1960. Once sitios de producción medían la fecha por su cuenta. La caché nunca sirvió un grafo viejo —la invalidación compara también el hash del contenido—, así que el defecto no era de estabilidad: era que el producto publicaba una fecha que no había medido.
La causa eran tres hechos distintos —una fecha medida, la época exacta y la ausencia de fecha— vestidos del mismo número. Ahora el tipo publicado distingue los tres: `Some(0)` es un fichero escrito en la época, `Some(-60)` uno escrito antes, y `None` uno del que no se sabe cuándo se escribió. El signo no es decorativo: un fichero puede traer una fecha anterior a la época, y un contador sin signo no la puede representar. La medida tiene un dueño único en el dominio, y la caché sigue invalidando por contenido: un fichero sin reloj no invalida el snapshot por su cuenta.
El RED está medido sobre el camino malo: los tests nuevos fallaban 2 de 3 y el contrato de CI encontraba 24 hallazgos contra el código anterior. Con el binario real por stdio, un fichero de 1960 devuelve `modified: -60`.
**Un rechazo llega al cliente como un rechazo, está cerrada.** El servicio de ficheros puede rechazar una petición —un camino fuera del workspace, un patrón que no es una expresión regular— y eso es una decisión del producto, no una caída. Pero los cinco handlers que pasan por el contador de métricas reconstruían el error como fallo interno, de modo que el cliente recibía "application: Internal error: Invalid parameter: …" donde debía recibir "application: Invalid parameter: …".
La causa no era el aplanado sino su por qué: el contador exigía su propio tipo de error, así que el llamante tenía que convertir el suyo y reconstruirlo después, y esa reconstrucción perdía la categoría. Ahora el contador es genérico sobre el error y lo devuelve intacto; pide una etiqueta, que es lo único que la telemetría necesita saber. La etiqueta es la categoría del error, con un match exhaustivo que el compilador mantiene al día.
El defecto era invisible a la suite entera por una razón concreta: el servidor MCP inicializa el colector de métricas y los tests no, de modo que la suite probaba siempre el camino que ya funcionaba. El RED está medido sobre el camino malo: los dos tests nuevos fallaban y nombraban los cinco handlers, y el contrato de CI fallaba dos de cuatro contra el código anterior. Con el binario real por stdio, el servidor —que sí cuenta— entrega la categoría.

**El consumidor declara la versión sobre la que escribe, está cerrada.** La
escritura condicional existía en la capa de aplicación, pero el borde la perdía: el
campo se fijaba a `None` en la conversión, el esquema del protocolo no lo declaraba,
y `read_file` no publicaba ningún digest. Es decir, la capacidad existía y era
inaccesible: el consumidor no tenía forma de nombrar la versión que leyó. Ahora
`read_file` publica el digest del fichero entero, y sale de la misma lectura que
cuenta las líneas —sin una segunda pasada por el fichero, y con el orden seguro,
porque el digest se calcula antes de leer el contenido—. `write_file` y `edit_file`
aceptan el hash declarado y la conversión lo lleva. El documento publicado declara
el campo, y la captura del runtime se refrescó desde un `tools/list` real: su
`tool_version` decía `0.101.17` y el gate, que sólo comparaba el conjunto de
nombres, no se enteraba.
La demostración está hecha: seis de los siete tests nuevos se vieron fallar antes,
el contrato fallaba dos de seis, y el UAT con el binario real por stdio leyó,
escribió declarando la versión y fue rechazado cuando la versión ya no era la.

**La escritura se publica entera, o no se publica, está cerrada.** El servicio
de ficheros abría el temporal con `File::create`, que no exige creación exclusiva,
de modo que un temporal preexistente se trunca; la limpieza estaba sólo en el error
del cambio de nombre, así que un fallo de escritura dejaba el temporal en disco; y
no había comprobación de versión, de modo que una edición pisaba el trabajo de otro
editor entre la lectura y la escritura sin que nadie se enterara. Ahora hay un
adaptador único: el temporal se crea en exclusiva, el contenido se sincroniza antes
de que el destino exista, el directorio se sincroniza después del cambio de nombre,
y la limpieza está en `Drop`, de modo que lo que no se publica se borra porque el
escritor se suelta y no porque alguien se acuerde de borrar en cada camino de
error. El llamante puede declarar la versión que leyó, y la escritura se niega si
el destino ya no la tiene: la versión se comprueba antes de crear nada —para que
una petición rechazada no deje ni el fichero ni los directorios que pidió— y otra
vez justo antes de publicar. El digest es el del producto, no una tercera forma de
hashar. La demostración está hecha: tres de los cinco tests nuevos se vieron fallar
antes, y el contrato que vigila que no vuelva comprueba que cada detector nota un
texto que la viola.

**Un corte por byte no parte un caracter está cerrada.** El servicio de ficheros
recortaba el texto por índice de byte, y con texto multibyte un índice dentro
de un carácter abortaba el proceso: el desplazamiento lo elegía el llamante —
el token de continuación no está firmado— y el tamaño del trozo también. El
corte se pide ahora con la forma que el tipo hace segura: los tramos se toman
con `get`, que devuelve `None` cuando el rango no cae en fronteras, y el
tamaño del trozo se ajusta al último carácter entero que cabe. Un módulo
nuevo de fronteras sustituye a los recortes sueltos, y hay un contrato que
vigila que no vuelva: no lista los cortes prohibidos uno a uno, sino que mira
cualquier corte por índice sobre texto en los ficheros que recortan. La
demostración está hecha: los cuatro tests nuevos se vieron abortar antes, en
las líneas 986, 1003 (dos veces) y 1376.

**Lo que se rechaza no deja rastro, y las ediciones se aplican al original está
cerrada.** Dos controles llegaban tarde o no llegaban. El límite de tamaño de
`write_file` se comprobaba después de crear el árbol de directorios, así que una
escritura rechazada dejaba en el disco lo que había pedido: el límite es el único
control que depende sólo de la petición, así que es el único que no puede llegar
después del efecto. Y las ediciones se comprobaban sobre el contenido original pero
se aplicaban sobre el resultado de la anterior: sobre `A B`, `A→B` y `B→C`
producian `C C`, y sobre `abc`, `ab→X` y `bc→Y` la segunda edición desaparecía en
silencio con `applied: true` y sin motivo — una petición declarada aplicada cuando
la mitad de lo que pedía no se hizo. Ahora el tamaño se comprueba antes de crear
nada, y las ediciones se aplican a la vez sobre el original: las posiciones se
calculan sobre el original, los cambios se aplican desde el final, y dos ediciones
que se solapan se rechazan con el motivo `overlapping_edits` en vez de perder la
segunda en silencio. La garantía que el servicio ya publicaba —cada edición
encuentra exactamente una ocurrencia— pasa a ser lo que el código hace.

**Una etapa que no puede hacer caer la puerta dice qué observa, y la que dice
que instala instala.** Trece etapas se ejecutaban sin poder bloquear, y
ninguna declaraba qué valen: su motivo vivía en la prosa de un pipeline y
citaba un `ci.yml` que ya no está en el repositorio, porque la casa migró de
GitHub Actions a PipelineK y el nombre del workflow viejo se quedó en el
comentario. Ahora las trece que tragan un fallo están declaradas con su motivo
y su recuento comprobado, en un contrato que pone en rojo la etapa nueva que
se marca advisory sin declararse, y la etapa nueva que traga un fallo sin
decirlo. Y la que se llamaba instalación en HOME limpio sólo planificaba:
`cogh install` es un dry-run que imprime el destino y sale con cero. Ahora
instala de verdad, con la opción de aplicar, y fija la raíz dentro de su
propio HOME — porque sin esa fijación la raíz la decide el `CARGO_HOME` del
entorno, y en un runner de CI eso es fuera del sandbox que la etapa declara.

**El grafo deja de tener una segunda manera de responder está cerrada.** Una
instantánea de un grafo dice cómo lucía una vez; no puede decir que el grafo
*siga* al código, porque un grafo que conteste lo mismo antes y después de un
renombrado pasa todas las instantáneas tomadas antes y falla todas las
tomadas después. Ahora hay un solo escenario que cambia el código y hace dos
preguntas a la vez: qué se movió —la identidad renombrada ya no se publica, la
nueva sí, y la relación sigue al nombre nuevo— y qué se quedó —cada otra
identidad y cada otra relación donde estaba—. Y la caché de ficheros olvidó
el fichero que cambió, y sólo ese: se relee uno y se reutilizan los otros
dos. La identidad se compara por nombre y fichero **relativo**, porque lo que
se compara es el código y no dónde está el código. La demostración está
hecha: anulada la huella de la caché, el contrato se puso rojo diciendo que
la identidad renombrada ya no se publicaba, es decir, que el grafo
respondía lo mismo de todo.

**C16 está cerrado.** El grafo del proyecto tenía una segunda puerta, en las
estrategias, que nadie usaba pero que seguía abierta: sus constructores se
comían los errores del recorrido y su método por defecto declaraba
`Complete` sobre un grafo que no había medido. La puerta del índice era la
peor: el MCP ofrecía una estrategia que **no indexaba nada** y contestaba
"tu proyecto no declara ningún símbolo", con un mensaje de éxito y 0 ms.
Y la ruta que camina el proyecto completo declaraba `Complete` sobre un
proyecto con una fuente rota, mientras la ruta por fichero la declaraba
`Partial`. Ahora el trait de estrategias queda con lo que todas pueden
cumplir sin mentir —indexar— y el grafo se construye en un solo sitio: el
servicio. El lector examina el árbol antes de publicar sus símbolos, y como
el árbol se analiza una vez y se lee dos veces, el cuidado sale más barato
que antes. La ruta filtrada usa el mismo inventario y el mismo informe que
la completa. La demostración está hecha: revirtiendo sólo las dos
correcciones, cinco contratos se pusieron rojos.

**La lectura declara la identidad de lo que leyó está cerrada.** El control
plane respondía con el veredicto, las reglas y las violaciones, pero no
decía **sobre qué código** se emitieron: el producto publicaba el veredicto
sin publicar las fuentes que lo sostienen, y un consumidor no tenía con qué
comparar. La primera versión de esa identidad tenía un defecto que la
invalidaba entera —el resumen se calculaba sobre la ruta **absoluta**, así
que el mismo código leído en dos sitios daba dos identidades distintas, y
la identidad era del sitio y no del código—. Ahora la identidad se calcula
en la **misma pasada** que recorre el árbol, sobre el mismo conjunto de
ficheros que el evaluador examina, y sobre la ruta **relativa** a la raíz
leída: el mismo código tiene la misma identidad donde esté el árbol. El
estado es un tipo cerrado —hay identidad o no la hay— y cuando no la hay el
resumen no viaja, porque el resumen del conjunto vacío se parecería a «no
hay nada» y a «no lo sé». Las dos rutas publican el mismo campo con la misma
identidad, y la forma canónica está escrita en
`docs/roadmap/consumer-contracts/specs/SPEC-CU-010-CONTROL-PLANE-READ.md`.
La demostración está hecha: el contrato de portabilidad se vio fallar antes
de la corrección y otra vez al revertir sólo la elección de la ruta.

**C26 está cerrado.** El lector de un lenguaje contestaba lo mismo a tres
hechos distintos: que declaraba algo, que no aplicaba a aquel fichero, y que
fallaba — y el fallo real, que es un lector que revienta, se lo tragaba el
`catch_unwind` y acababa pareciendo un fichero que no declara nada. La
posición tampoco era real: las declaraciones se construían con
`Location::new(fichero, 0, 0)`, así que el producto publicaba la línea 1 de
cualquier fichero. Y un fichero con un error de sintaxis lo publicaba el
lector de Terraform mientras el lector genérico lo rechazaba: los dos
caminos no podían estar en desacuerdo sobre lo que es un fichero roto. Ahora
el desenlace está tipado —declaró, no aplica, o falló con su causa—, el
consumidor que recibe un fallo lo dice en vez de caer al lector genérico, y la
declaración se publica donde está escrita. La demostración está hecha:
revirtiendo el tratamiento del fallo, la posición o el rechazo del fichero
roto, los contratos correspondientes se pusieron rojos.

**C15 está cerrado.** El producto llevaba cuatro listas distintas de
extensiones y cada una contestaba a una pregunta distinta: el vigilante miraba
la extensión y nada más, el inventario clasificaba con una tabla propia y el
endpoint publicaba una lista que no describía lo que el servidor vigilaba. Lo
que el vigilante vigilaba no era el proyecto: en este repositorio miraba
**248.757 ficheros** frente a los **3.122** que recorre el inventario, y cada
instalación de dependencias era un lote de cambios que reconstruía el grafo
entero. Ahora hay una regla, en un sitio: el producto observa un fichero
cuando puede decir qué es y cuando vive donde el producto mira. La
clasificación sale del registro de lenguajes, el inventario y el vigilante
aplican las mismas reglas —los directorios ruidosos y lo que el repositorio
excluye—, y el endpoint publica la lista del núcleo en vez de la suya. El
vigilante vigila ahora **2.870 ficheros**, exactamente los que el inventario
recorre y que el producto puede nombrar, y el endpoint anuncia 60 extensiones
en vez de 21. La demostración está hecha: revirtiendo cada mitad de la regla
por separado, los contratos correspondientes se pusieron rojos.

**C10 está cerrado.** El producto publicaba un trozo de la identidad de una
cosa como si fuera su nombre. Para un símbolo pasaba por casualidad —el nombre
está en el medio de la identidad—, pero para una declaración que un lector de
infraestructura nombra, un recurso de Terraform o un play de Ansible, el trozo
que sobrevivía era el fichero donde la cosa vive: el producto llegaba a decir
que "main.tf" era el símbolo del que más depende el proyecto, y lo preguntaba
en voz alta en las preguntas sugeridas. En el camino de hechos, cada lector
respondía de otra manera: el hecho se descartaba, el fichero quedaba vacío, o
el nombre salía como el fichero. Ahora el dominio declara la gramática de la
identidad y la lee en un solo sitio, y los publicadores toman el nombre del
símbolo que lo lleva. La identidad de una declaración no cambia de bytes: el
recurso sigue siendo `tf:main.tf:aws_s3_bucket.app_bucket`. La demostración
está hecha: revirtiendo el lector, o revirtiendo los publicadores, los
contratos correspondientes se pusieron rojos.

**C9 está cerrado.** El dominio construía su propio adaptador: el puerto de
proyección montaba la implementación concreta de infraestructura en una
función libre. La costura se invirtió —el puerto declara la fábrica, la
infraestructura la implementa, la raíz de composición la inyecta y los
algoritmos la reciben hecha— y el guard arquitectónico aprendió a ver la
dependencia escrita donde se usa, no solo la importada. Ese era el agujero:
el guard daba verde sobre el defecto que dice impedir, porque solo leía
sentencias `use`. La demostración está hecha: reintroducir la construcción en
el dominio volvió a poner el guard en rojo con el motivo exacto.

**C30 está cerrado.** La lista de lenguajes que el producto declara sale
del registro de parsers, y el documento de la matriz ya no declara lista
propia. Antes esa lista la escribía a mano y se había separado de la
verdad en las dos direcciones: anunciaba tres extensiones como si fueran
lenguajes —`jsx`, `tsx`, `luau`— y callaba doce que el producto sí lee.
Un guard que comparaba dos listas escritas a mano daba verde sobre esa
mentira; ahora mide la derivación contra el catálogo publicado.

**C6, C31, C5, C21, C8, C7, C32, C14 y C19 están cerrados.** El lazo de feedback ejecutaba la
biblioteca del crate y nunca el test de integración que se acababa de
escribir: ahora pide el binario que produce ese fichero —y sólo ese— sin
convertir el modo rápido en la batería completa. La autoridad de una
herramienta dejó de derivarse del catálogo del proyecto analizado. La línea
base vacía dejó de obligar a reejecutar la suite entera. La configuración
ejecutable dejó de apuntar a documentos que no existen, el guard de
procedencia pasó a gobernar el contrato del consumidor, y cada superficie de
prueba tiene ahora un carril declarado o la razón escrita de por qué se
queda sin él. Y el gate da el mismo resultado en cualquier máquina: los
laboratorios de git ya no heredan las reglas de quien los ejecuta.

**C14 se cerró con `v0.101.27`**: una PR fusionada no es una release, y esta
sí se publicó, verificada desde sus propios bytes y con los binarios
instalados arrancados fuera de cualquier repositorio.

**La guarda del toolchain no depende de la red, está cerrada.** Una comprobación
del gate que necesita un dato de la máquina —el triple del host— lo pedía
justo donde no lo había: dentro de un entorno aislado que deja a `rustup` sin
toolchain, con lo que el shim la descarga desde la red. Medido: 117,7 s por
llamada, con el `timeout=120` del contrato al borde. El resultado era que la red
decidía el veredicto del gate, y dos corridas de esa misma mañana ya habían
fallado con la misma firma. El defecto de fondo era del mismo origen: cuando
`rustc -vV` fallaba, el host quedaba vacío y el target **nativo** se rechazaba
como si fuera cruzado, con el motivo de que faltaba un compilador que nadie
había preguntado.
Ahora la guarda recibe el host como dato y no ejecuta `rustc` cuando se le da;
cuando no lo recibe, lo resuelve como antes y **falla cerrado** si no puede,
nombrando esa causa en vez de sacar un veredicto sobre el target. El contrato
resuelve el host una sola vez, en el proceso padre y con el entorno real, y la
hermeticidad es estructural: la función que ejercita la guarda rechaza una
llamada sin host salvo que se pida explícitamente la ruta de resolución. El RED
está medido sobre el camino malo: dos de los tres tests nuevos fallaban con el
veredicto falso, siendo el target nativo. El GREEN está medido: el contrato
completo, 11 de 11, en 0,64 s —antes, 117,7 s por llamada—, y la suite entera
de contratos, 438 comprobaciones verdes y 0 rojas.

**El grafo se reconstruye sólo donde cambió, está cerrada.** Tocar un fichero
rehacía el grafo entero: la fase de resolución —la que mira 34 779 relaciones
en un proyecto de 400 ficheros— se repetía completa en cada construcción, y el
consumidor no tenía forma de saber cuánto de su grafo anterior seguía en pie.
Ahora un módulo nuevo, `infrastructure/graph/reuse.rs`, es el dueño único de
esa decisión: lleva un libro mayor entre construcciones con lo que la última
sabía de cada fichero —su hash y la identidad de cada declaración que hace— y
lo que resolvió para cada par (fichero llamante, nombre llamado). La regla es
la de la **identidad**: un nombre se invalida cuando algún fichero —cambiado,
nuevo o borrado— añadió su declaración, la eliminó, o le dio otra identidad;
y el fichero de quien llama invalida sus propias relaciones, porque las
llamadas que dice se leen otra vez en cada construcción. Lo que el cambio no
toca se conserva.
Antes de escribir la regla se midió que esa identidad es estable —sobre el
corpus de 400 ficheros, el conjunto completo de ids de la construcción en frío
y el de la caliente son idénticos: 18 011 ids, 0 aristas colgantes—, porque una
regla de identidad sobre identidades inestables no sería sound. El primer
sondeo decía lo contrario (1 299 pares con id distinto) y era un artefacto del
propio sondeo: el mapa colapsaba homónimos y guardaba el último según el
orden de iteración.
El GREEN está medido sobre el camino malo, con mutación focal: se rompieron
tres veces la regla a propósito y cada mutante murió sólo por los tests
escritos para él —la regla vieja por nombre, la regla vieja con guarda de
ámbito, y la regla sin la cláusula de identidad—, y los tres se revirtieron.
La suite de caracterización queda en 10 de 10, y la medición (CU-005.6) sobre
400 ficheros en 61 directorios: frío 3,371 s; tibio 766,7 ms de p50 y 771,3
ms de p95; edición al final 820,3 ms; edición en medio 861,3 ms de p50 y
2,465 s de p95. Con la regla anterior, sobre el mismo corpus, la edición
cuesta p50 2,391 s y p95 2,543 s: el endurecimiento la hace 2,8 veces más
rápida por un 5 % en la construcción tibia. La medición encontró además un
defecto de fondo —el libro mayor guardaba las respuestas nuevas y no las
conservadas, así que se vaciaba en cada construcción y una de cada cuatro se
rehacía entera— y el número dice lo que dice: cuántas relaciones se
resolvieron otra vez y cuántas se conservaron, no «el análisis fue
incremental». El análisis entero no lo es: analizar los ficheros sigue
asignando unos 199 MiB por construcción. El CLI y el MCP no llevan todavía el
número en su carga útil: publicarlo en esas superficies es un cambio de
contrato con su propio gate.

**El escaneo declara los ficheros que no puede fechar, está cerrada.** La última excepción del contrato de la fecha medida era la familia de la ingestión, y su hueco era doble: una fecha anterior a la época se publicaba como 0 —que es la época— y un fichero cuya fecha no se puede leer desaparecía del escaneo. No era un fichero sin fecha: era un fichero que el escaneo ya no conocía, así que tampoco quedaba sitio donde decir que no se sabía cuándo se había escrito. La causa era que la medida y la decisión estaban en el mismo sitio, y ese sitio era un `filter_map`: cuando la fecha no se podía leer, el fichero entero se perdía.
Ahora la medida no filtra: cada fichero que el recorrido encuentra se mide con el dueño único —`file_time::measured_millis`— y se queda en el escaneo con su fecha ausente cuando el sistema de ficheros no la publica. La fecha es `Option<i64>` en milisegundos y con signo en todo el camino —`FileChange`, `ScanManifestEntry`, el puerto y la columna `mtime_millis INT64`—, y un NULL se lee como `None` y no como `0`. Sin fecha no se puede afirmar nada, así que el fichero se vuelve a mirar y decide el hash, que es lo que siempre decidió.
El RED está medido sobre el camino malo: la suite de caracterización falló contra el código viejo diciendo que el escaneo borró el fichero y con él su hueco. La reproducción es real y no falsifica el reloj: un directorio con modo `0444` se puede listar pero no atravesar. El GREEN se midió con mutación focal: cuatro veces se rompió el comportamiento a propósito, y cada mutante murió sólo por los tests escritos para él. La suite queda en 5 de 5, más dos tests del adaptador que demuestran que la ausencia y el signo negativo sobreviven al viaje de ida y vuelta a la base de datos. El contrato de la fecha medida ya no declara el hueco, y el registro de huecos —que ahora tiene que declarar que está vacío, porque un registro vacío es una afirmación y no un olvido— sigue vigilando el sitio: se metió una fabricación de vuelta en `scan.rs` y los dos detectores la cazaron.
La integración encontró después un defecto que los gates locales no veían, y no era del producto: el arnés de pruebas del adaptador agotaba el espacio de direcciones del proceso. El motor reserva ocho terabytes de espacio de direcciones —no de memoria— por cada base de datos que abre, y la suite abre una base por test: con sesenta y cuatro tests en paralelo no cabían. El arnés abre ahora sus bases con una configuración propia, recortada como lo hace la configuración de pruebas del propio motor, y un guardián abre veinte bases a la vez para que el límite no vuelva a decidirse por suerte. La producción abre igual que antes, con la configuración por defecto.

**Kotlin se navega sin JVM, está cerrada.** El producto declaraba `.kt` y `.kts` como extensiones conocidas y no soportadas: eso era honesto, pero dejaba al consumidor sin Kotlin. No era sólo un lenguaje que faltaba —el lector consumía un tipo de nodo por categoría, y un lenguaje que declara varios tipos en una categoría perdía todos menos el primero—, así que añadir el lenguaje sin cambiar el lector habría publicado un Kotlin cojo.
Ahora el vocabulario vive en un solo sitio: `Language::symbols()` declara los conjuntos de nodos que cada lenguaje usa, el lector los consulta, y los accesores de un solo tipo derivan de ellos en lugar de repetir la lista. Kotlin llega con una gramática —una, medida, sin Kotlin Compiler ni Gradle en el parser— y con las costuras que esa gramática pedía: el prefijo que envuelve al nombre, el nombre heredado del companion, el paquete como declaración propia. Los otros quince lenguajes publican exactamente lo que publicaban antes.
La identidad es `fichero:nombre:línea`, así que dos sobrecargas son dos símbolos y un constructor no es su clase. Un contrato permanente afirma el vocabulario completo —interface, data class, clase, clase anidada, companion con y sin nombre, enum, `object`, sealed, value class, constructor secundario, función top-level, método, extensión, sobrecargas, propiedades y paquete—, y dos contratos más comprueban que el código de un símbolo es exactamente su declaración y que el Unicode que declara se conserva. El caso negativo también está: un `.kt` que no parsea no se publica como leído, y un script `.kts` declara sus bindings de nivel superior y nada más —sus tareas y sus plugins son llamadas, no declaraciones.
El GREEN se midió con mutación focal: cinco veces se rompió el comportamiento a propósito, y cada mutante murió sólo por los tests escritos para él. La UAT efímera sobre los binarios release dio 30 de 30, con los símbolos pedidos antes de construir grafo. La batería encontró además dos contratos que se habían quedado sin precondición —usaban un `.kt` como hueco real, y un hueco así caduca en cuanto el producto soporta el lenguaje—; ahora los dos huecos son un fichero con sintaxis rota, que es un hueco para siempre. Quedan declarados tres residuos: la lectura de complejidad sigue con un tipo por categoría, la distinción de script que nombra la especificación no existe como tipo en el producto, y el destino de un `import` con alias es cosa de la unidad de relaciones.

**El escaneo declara los ficheros que no puede leer, está cerrada.** El escaneo
decide qué se vuelve a leer comparando el hash del fichero con el que el índice
guardó la vez anterior, así que deciding hay que leer. Cuando la lectura
fallaba, el escaneo no lo decía: el fichero desaparecía del mapa de hashes, de
modo que un fichero nuevo ilegible entraba en el índice con el hash vacío —una
afirmación sobre un contenido que nadie leyó— y un fichero existente ilegible
cayía en la misma rama que uno cuya fecha no cambió, y no se publicaba en
absoluto. El hueco era el mismo que el de la fecha, y por la misma razón: el
silencio del escaneo significa «no ha cambiado», y no significa «no sé».
La decisión de cambio deja de ser binaria y toma la forma que la fecha ya
tenía: dos ADTs, cada uno con una pregunta. `ChangeDecision` dice lo que dice
el contenido —`New`, `Changed`, `Unchanged`, `Unreadable { reason }`— y
`Freshness` dice lo que dice la fecha —`Unchanged`, `Changed`, `Unknown`—, que
no puede mezclarse con la anterior porque sin fecha no se puede afirmar nada:
el contenido se lee y decide. Un hash que no se pudo leer es una ausencia y no
un hash: `content_hash` pasa a ser `Option<String>` en todo el camino, el
índice guarda que nadie leyó el contenido, y como no hay hash con el que
comparar, la siguiente pasada vuelve a mirar el fichero. Y un fallo de lectura
se publica como fallo: `ChangeKind` gana `Unreadable`, el motivo viaja con el
cambio y la extracción publica un resultado fallido en vez de callarse.
El RED está medido sobre el camino malo: la suite de caracterización falló 3 de
3 contra el código viejo. Los otros dos comportamientos del contrato no se
podían ni expresar contra él —el `ScanEntry.content_hash` viejo era un
`String`, así que el índice no tenía forma de decir «nadie leyó esto», y el
`FileChange` viejo no tenía dónde poner una causa—, y eso es parte del
hallazgo. El GREEN se midió con mutación focal: cinco veces se rompió el
comportamiento a propósito y cada mutante murió sólo por los tests escritos
para él. La UAT efímera dice que el fichero sale sin hash y con su causa, que
el índice guarda la ausencia y que la segunda pasada vuelve a mirarlo.

**Los recuentos que el producto publica están medidos, está cerrada.** El
producto publicaba tres cifras que nadie midió: `symbols_added += 1` por cada
fichero que cambiaba, el `symbol_count` de un manifiesto que guardaba cero en
todos los ficheros, y `deleted_files.len() * 2` en el resultado incremental. Los
tres son el mismo defecto: un número que se publica como medido y no lo está. Un
cliente que compara el número con el grafo ve una diferencia que no sabe
explicar, y no tiene con qué comprobarla.
La línea base pasó a ser el grafo (ADR-001): es lo único poblado en las dos
rutas de publicación, y es lo que el cliente puede contrastar, porque lo
reconstruye y lo consulta. El manifiesto no puede servir —el que escribe la ruta
MCP pone `symbol_count: 0` en todos los ficheros, y además se toma antes de
construir el grafo—, y el índice semántico tampoco: en esa ruta se construye
vacío. De esa decisión salen tres cosas: la medida tiene un dueño
(`CallGraph::symbol_count_for_file`, que además dice si el número se midió), la
diferencia es una función pura del dominio (`symbol_delta`, en
`domain/value_objects/`), y lo que no se pudo medir se declara en
`symbols_unmeasured` en vez de publicar un cero que parece medido. Un grafo
vacío no midió nada: es la ausencia de una medida, no una medida de cero.
El RED está medido sobre el camino malo: cinco tests fallaron contra el código
anterior, y fallaron por el número y no por el campo — un fichero que crecía
publicaba 1 en vez de 4, uno que encogía 0 en vez de 4, uno que se borraba 0 en
vez de 4, un cambio no medible 0 en vez de declarar 1, y el resultado
incremental 2 en vez de 1—. El GREEN está medido: `test-fast` rc=0
(956+161+0+80+15+13+6) y `test-full` rc=0 (fmt, clippy con `-D warnings`, 6462
pruebas del workspace, 8 doc-tests, 439 contratos en 61 ficheros, `known_failures`
sin diferencias). La herramienta MCP y la sesión de workspace publican la misma
medida.

**Las relaciones Kotlin↔Java se publican sólo cuando se pueden probar, está
cerrada.** El grafo publicaba una arista cada vez que el nombre del callee
encontraba *alguna* declaración, y en un corpus donde ese nombre está declarado
varias veces eso es elegir un homónimo sin decirlo. La arista nacía, y el
consumidor recibía una relación que el producto no había probado: el nombre
`run` está declarado tres veces —dos en Kotlin y una en Java, en dos paquetes— y
el grafo no puede decir cuál sin inventarlo.
Ahora hay un corpus versionado, `sandbox/fixtures/kotlin-java/`, con doce
ficheros —ocho en Kotlin y cuatro en Java, en seis paquetes— que declara los
homónimos y los overloads que **no** se pueden probar, y un contrato
permanente que lo comprueba. MEDIDO: el extractor lee SEIS llamadas y el grafo
publica CUATRO relaciones; las dos que no publica son exactamente las dos que no
se pueden probar. Las otras cuatro sí se publican —Kotlin→Java, Java→Kotlin,
Kotlin→Kotlin y fichero a fichero—, y son las que hacen que la negativa no sea
un PASS vacío: sin ellas, «no sale ninguna arista» se cumpliría igual con un
corpus que no se lee.
La forma de la resolución queda fijada por el mismo contrato: `TargetResolution`
tiene TRES variantes, sólo `Resolved` nombra un destino, y el fichero dice por
qué la cuarta —`External`— no existe: no hay tabla de símbolos externos, y una
variante que nadie puede construir es una hipótesis que sobrevive más que el
código que debería haberla llenado. El `match` del contrato no tiene brazo `_`:
una cuarta variante rompe la COMPILACIÓN, que es un fallo que ni el CI puede
dejar pasar por alto. El RED está medido sobre el camino malo: con el
resolvedor mutado para que elija un candidato arbitrario, el contrato falla con
rc=101 (6 pasaron, 3 fallaron); restaurado, pasa con 9.
Lo que este corte NO cubre, y queda declarado: la pérdida de una relación no se
publica. MEDIDO: las dos referencias refusadas no aparecen ni como arista ni
como pendiente, y la ruta por fichero dice `Complete`. Eso contradice K-REL-02 y
K-REL-07, y está enrutado a su propia tarea —`TASK-14`, que vive en la bandeja
local de Backlog.md, fuera de Git por la política de documentos privados—,
fuera del alcance de esta unidad. El caso del overload que sí se podría elegir —el que ADR-K003 pide—
tampoco es implementable hoy: el pipeline sólo transporta el NOMBRE del callee
(`FileFacts.calls` es `Vec<(Symbol, String)>`), así que la aridad que
distinguiría los dos `invoke` no llega al resolvedor. Y la equivalencia
CLI/MCP medida hasta hoy cubre Terraform, no este corpus.

**Las referencias que el grafo no puede resolver se publican con su motivo,
está cerrada.** La pérdida que el cierre anterior dejó medida y con dueño: el
extractor lee SEIS llamadas sobre el corpus Kotlin/Java y el grafo publica
CUATRO relaciones; las dos que no publica son las dos que no se pueden probar,
y de esas dos no quedaba NADA publicado. La estrategia `full` publicaba un
número —`unresolved_edges`— y la estrategia `per_file` publicaba un
`BuildStatus::Complete` que afirmaba una cobertura que no era verdad. El
número dice cuántas son; no dice cuál es cada una, en qué fichero y línea está
escrita, ni por qué no se pudo atar. Eso contradecía K-REL-02 y K-REL-07 del
contrato del consumidor.
Ahora la pérdida se publica con su identidad y su motivo: qué llamada es, en
qué fichero y línea, y cuántos candidatos tenía el nombre que nombra.
`BuildStatus::Partial` lleva la lista —`pending`—, la CLI y el MCP ya tenían el
campo donde publicarla, y no se creó ningún campo nuevo en ningún contrato
público: lo que cambia es que se llena lo que ya existía. Y `Complete` deja
de significar sólo «no quedó ningún fichero sin leer»: pasa a ser «no quedó
NADA sin atar», ni un fichero sin leer ni una referencia sin atar a una
declaración.
La FORMA del dominio no cambia, y esa fue la decisión que se tomó antes de
tocar Rust: `TargetResolution` sigue TENIENDO TRES VARIANTES —una cuarta
rompería el `match` exhaustivo que el contrato de la unidad anterior fijó— y la
negativa se modela donde ya había una forma para ella: `PendingClass` y
`PendingReference`. Lo que faltaba era el TRANSPORTE, no la forma: la arista
descartada no dejaba sitio donde escribir su motivo porque la arista no
existe. El motivo lo produce la resolución y lo decide el dominio
(`PendingReference::of_call`), de modo que ninguna ruta lo decide por su
cuenta; y un valor de dominio nuevo transporta la línea donde la llamada está
escrita, que es lo que permite señalarla.
El alcance es una decisión, y se declara: se publica la negativa AMBIGUA —el
proyecto declara ese nombre varias veces— y no la negativa sin candidato —el
proyecto no declara ese nombre—, porque publicar todos los nombres de
biblioteca (`unwrap`, `len`, `expect`) llenaría la lista de huecos de ruido y
la volvería inútil. La pérdida sin candidato sigue contando, como antes.
El RED está medido sobre el camino malo: el contrato —el existente, sobre el
corpus que ya existe, sin un fixture nuevo— daba 9 pasados y 3 fallidos; ahora
da 12 y 0. Los tres tests nuevos fijan la pérdida en las DOS estrategias, y el
que compara ambas tiene una precondición que impide que pase por vacío. Y los
nueve ficheros de test que nombran `BuildStatus::Complete` se revisaron uno a
uno: el cambio de semántica está escrito en cada uno, y dos expectativas
dormidas resultaron estar equivocadas —no el producto—: una pedía que el hueco
nombrase un fichero que es una exclusión legítima, y otra afirmaba `Complete`
sobre un corpus cuyo hueco real son dos llamadas ambiguas. Lo que este corte
NO cubre, y queda declarado: la ruta semántica —la que usa un lector con
dominio, como Terraform o Ansible— sigue sin publicar sus llamadas no
resueltas, porque su lector no tiene índice del proyecto y no puede
distinguir una llamada ambigua de una que vive fuera del proyecto.

**Las dos superficies dicen lo mismo del mismo proyecto, está cerrada.** El
residuo que dejó el cierre anterior lo dijo con sus palabras: «la equivalencia
CLI/MCP no se ha ejercido sobre este corpus; la medición de equivalencia que
hoy existe cubre Terraform; este contrato llama a los servicios del dominio, no
a la CLI ni al MCP». Era una laguna de MEDICIÓN, y no de forma: la CLI y el MCP
son adaptadores sobre un mismo servicio, y las dos superficies leen la misma
pérdida del mismo sitio —el informe del build— y la proyectan con la misma
función —`PendingReferenceInfo::from_reference`—, así que la pregunta que
faltaba no era «¿qué.publish?» sino «¿publican lo mismo?».
La medición anterior, la de Terraform, comparaba un RECUENTO de referencias
pendientes. Un recuento basta cuando lo que se pierde es una referencia que
nadie declara, porque entonces lo único que hay que decir es que hay un hueco.
El corpus de Kotlin pierde además las referencias AMBIGUAS —`run` está
declarado tres veces y `invoke` dos, y ninguna de las dos llamadas dice cuál—
y esas tienen una IDENTIDAD —qué llamada, en qué fichero y en qué línea— y un
MOTIVO —cuántos candidatos llevan el nombre—, que son exactamente las dos cosas
que un consumidor necesita para poder actuar sobre el hueco. Comparar un
recuento no comparaba la negativa: comparaba su tamaño.
MEDIDO: las dos superficies publican las MISMAS dos negativas, campo por
campo —`demo/gamma/Caller.kt:3 → run [calls, ambiguous, 3]` y
`demo/gamma/UseHandler.kt:6 → invoke [calls, ambiguous, 2]`—, el mismo
veredicto (`partial`) y el mismo grafo no vacío —29 símbolos y 4 relaciones—.
La equivalencia SOSTENÍA: el residuo era la medición que faltaba, no un
defecto del producto, y por eso esta unidad no toca el código de producción.
Lo que añade es la PRUEBA que mide, y la prueba no puede pasar por vacío: dos
superficies que no publican nada son iguales en el vacío, y una equivalencia
medida sobre un grafo vacío es una equivalencia medida sobre nada. El RED se
midió con dos mutaciones focales, cada una revertida: sin la negativa en el MCP
—«the MCP publishes the negative the corpus declares: got []»— y sin la línea
en la proyección compartida. El inventario del harness antes y después es el
MISMO: 235 ficheros de test, 59 contratos de CI y 46 directorios de fixtures,
cero ficheros de test nuevos, cero contratos nuevos, cero fixtures nuevos —el
corpus `sandbox/fixtures/kotlin-java/` y el contrato de Kotlin ya existen, y la
medición vive en el fichero que ya sabía lanzar la CLI y el MCP sobre el mismo
árbol—.

**La respuesta que el MCP sirve desde la caché ya no es un veredicto hueco.**
El cierre de `TASK-15` declaró un residuo con sus palabras: «el camino de caché
descarta: el MCP tiene un camino de CACHÉ que responde `complete` con la lista
de pendientes VACÍA». Ese residuo hablaba de la lista de PENDIENTES, y la
superficie a medir era el VEREDICTO entero de la respuesta.
MEDIDO: la rama `loaded_from_cache` de `handle_build_graph` respondía
`(Some([]), Some([]), Some([]), "complete")` —la lista de omitidos, la de
pendientes y la de rechazadas vacías, y `complete`—. El GRAFO que acompañaba a
ese veredicto era real y no estaba vacío: 3 símbolos y 0 relaciones, los mismos
que daba una construcción en frío del mismo árbol. El hueco no estaba en el
grafo: estaba en el VEREDICTO, que afirmaba de un árbol algo que la caché no
podía saber.
Y no podía saberlo por una razón concreta: el manifiesto que decide si la
instantánea sigue siendo fresca cubre los FICHEROS de los que se construye el
grafo —los que el registro de lenguajes reconoce—, y una entrada que el
recorrido DECLINA —un enlace simbólico que no sigue— no es uno de esos
ficheros, así que no está en el manifiesto, así que la comprobación de frescura
no puede verlo. Un árbol al que se le añade un enlace simbólico después de la
instantánea es un árbol que una construcción en frío declara `partial` y que la
caché servía como `complete`.
MEDIDO, sobre el corpus de tres ficheros Rust que el repo ya versiona: con un
enlace simbólico declinado en `src/`, la respuesta EN FRÍO del árbol publicaba
`partial` con `src/link.rs` declarado como `symlink`, y la respuesta SERVIDA DE
LA CACHÉ publicaba `complete` con la lista de omitidos VACÍA. El mismo grafo —
3 símbolos, 0 relaciones— con dos veredictos distintos.
La corrección no cuesta un recorrido nuevo: la comprobación de frescura ya
recorría el árbol, así que el hueco del árbol sale de la MISMA pasada. Y los
huecos tienen ahora un solo dueño —`SourceInventory::coverage_holes`—, que es
la composición de lo que el recorrido declined, lo que no pudo hacer y lo que
todavía no sabe leer; las tres rutas que publicaban esa lista por su cuenta ahora
la piden. La respuesta servida de la caché publica los huecos del árbol tal
como está AHORA y lleva el veredicto que una construcción en frío daría. La CLI
no tiene camino de caché —siempre recorre y publica los huecos actuales—, así
que la corrección CIERRA una diferencia de paridad entre las dos superficies en
vez de abrirla.
La prueba vive en el fichero que ya medía la cobertura en caliente y en frío
(`crates/cognicode-core/tests/cached_coverage_is_honest.rs`): cero ficheros de
test nuevos, cero contratos nuevos, cero fixtures nuevos. Y no puede pasar por
vacío: exige que la respuesta EN CALIENTE venga de la caché —su mensaje lo
dice—, que el grafo no esté vacío y que el hueco sea el que el árbol declara —
`src/link.rs` como `symlink`—, porque dos respuestas vacías son iguales en el
vacío. El RED se midió con dos mutaciones focales, cada una revertida: sin el
veredicto honesto y sin el motivo del hueco.
El ciclo queda cerrado en el remoto: el pull request `#497` —la rama
`measure/mcp-cache-verdict-truth`— se fusionó por squash como `323abe72`, con
el `merge-gate` verde sobre `97fbef20`. `fast` rc=0 y `full` rc=0 —6504
pasados, 0 fallidos y 34 ignorados contando los doc-tests; 454 comprobaciones
de contrato en 0 fallos—, con una salvedad que el propio gate enseña y que
este recibo declara: el `fast` NO ejecuta el fichero de test nuevo, porque el
selector CR-08 cae en su regla de catch-all —`product/tools.json` es un path que
no conoce— y el camino de fallback corre `cargo test --workspace --lib`, que
son SÓLO pruebas unitarias; la prueba de esta unidad se ejecutó directamente —
`cargo test -p cognicode-core --test cached_coverage_is_honest`, 5 pasados— y el
`full` sí la incluye, porque corre `cargo test --workspace`.

**El lazo de feedback ya no da verde sin correr el test que se ha escrito.**
La salvedad del cierre anterior no era una limitación del entorno: era la ruta
que el modo rápido tomaba siempre que el diff contenía un path que el selector
no clasifica. `scripts/test-fast.sh` tiene dos caminos y no eran equivalentes:
`changed` —que corre la biblioteca del crate y el target de integración que los
paths tocan, desde el 2026-10-22— y `fallback`, que se saltaba `run_suite()`
entero y corría `cargo test --workspace --lib`, que son SÓLO pruebas unitarias.
Como `product/tools.json` está versionado y ninguna regla de CR-08 lo clasifica,
el `fallback` no era un caso raro: era la norma de cualquier cambio que lo
tocara. El resultado era un lazo de feedback que daba verde sin haber mirado
nunca el test que el developer acababa de escribir.
El `merge-gate` nunca estuvo mal —corre `test-full.sh`, que corre
`cargo test --workspace`—: el defecto era del lazo, y por eso la corrección
toca el wrapper y no el gate.
La corrección NO deriva el crate del path (sería una segunda fuente de verdad
del mapa suite→crate, que vive y sólo vive dentro del `case` de `run_suite()`,
y además daría un rojo falso sobre `crates/spike-ladybug/`, que no es miembro
del workspace). El `fallback` llama a `run_suite()` con un modo nuevo,
`touched-only`, que corre los targets tocados de esa suite sin repetir su
biblioteca —el `fallback` ya corre la biblioteca entera—; para que esa llamada
sea posible, el bloque de `fallback` se mueve debajo de `run_suite()`: mientras
vivía arriba, la función todavía no estaba registrada en ese punto del script.
MEDIDO, con un contrato que ejecuta el wrapper de verdad en un árbol de
laboratorio y mira la lista de invocaciones: antes, el `fallback` ejecutaba
`test --workspace --lib --quiet` y clippy, y `--test prf_demo` no estaba; ahora
ejecuta `test --workspace --lib --quiet`, `test -p cognicode-core --test
prf_demo` y clippy. El camino `changed` no cambia.
Y el hueco del que el `fallback` no puede salir se DECLARA, porque un wrapper que
no puede decir que no, aquí, no puede decir nada: si no se ha corrido ningún
target de integración, el `fallback` lo dice. MEDIDO sobre `cargo metadata`: de
213 targets de integración en 11 miembros, el selector mapea 197 —`core` (131),
`cli` (33), `mcp` (17), `explorer` (16), `ladybug` (0)— y quedan 16 sin mapear
—`cognicode` (7), `cognicode-runtime` (5), `cognicode-graph-algos` (2),
`cognicode-graph-wasm` (1), `cognicode-macros` (1), `cognicode-sandbox` (0)—; el
`full` los corre, salvo `spike-ladybug`, que está excluido del workspace y por
tanto no lo corre ningún carril de cargo: 9 targets sin carril en ninguna parte.
El ciclo queda cerrado en el remoto: el pull request `#499` —la rama
`fix/fast-fallback-runs-touched-target`— se fusionó por squash como `6404a84b`,
con el `merge-gate` verde sobre `dcfdedb` —58 etapas, ninguna en rojo, y el
estado `merge-gate=success` publicado en el pull request—. La corrida es
comprobable después del hecho: queda en la base de eventos del gate,
`.pipelinek/merge-gate.db`, con su identificador de corrida
`1045a1a3-1669-4e5c-9f6f-aa3d59bfb073`, los 58 `StageFinished` en `success` —el
primero `build-isolation`, el último `merge-gate`— y el `RunFinished` en
`success` (`outcome: success`, `diagnostics: []`).

**El reindex incremental dice lo que costó, está cerrada.** `reparse_on_edit`
publicaba cuántos ficheros tocó y cuántos símbolos cambiaron, y el trabajo que hay
detrás se reparte en cinco fases —leer el estado que la operación necesita,
establecer la identidad de cada fichero candidato, reinterpretar los que
cambiaron, escribir lo que sale de ellos en el índice, y reconstruir el grafo—.
El producto publicaba una sola suma, así que un consumidor que quisiera saber si
el rebuild incremental merece la pena no tenía con qué responder: la fase cara y
la fase barata se veían iguales.
La causa era que la medición no existía donde el trabajo ocurre: el handler
contaba el tiempo total, y el total no distingue un acierto de caché de un
rebuild completo. Ahora cada fase se mide donde se hace —el `scan` lee el
manifiesto, el `hash` establece la identidad de cada candidato, el `parse` y el
`index` separan el tiempo de leer del tiempo de escribir, y el `graph` mide el
rebuild—, y la palabra del rebuild la decide el producto y no una etiqueta
escrita junto a la llamada: `rebuild_kind_of` la deriva de los números de la
cobertura, que son los del producto.
Con la fase `hash` sale un defecto latente: el manifiesto decide por el
contenido y el reloj es sólo una pista —la misma regla que sigue la frescura
del snapshot—, pero el handler decidía por el reloj solo, de modo que un
fichero cuyo contenido había cambiado bajo un reloj intacto se saltaba en
silencio. La identidad se establece ahora con el hash del contenido, y el reloj
queda como lo que es: una pista.
El RED está medido sobre el camino malo: 7 de 7 rojo en
`crates/cognicode-core/tests/it/the_reparse_says_what_it_cost.rs` —el documento
publicado no tenía clave `cost`—, y 7 de 7 verde después. Las aserciones van
por el JSON que ve el cliente y no por el campo tipado, para que el test falle
por el motivo correcto y no porque el campo todavía no exista.
El benchmark (`crates/cognicode-core/benches/reparse_benchmarks.rs`) NO lo corre
el gate —un benchmark se pide a propósito—: mide tres formas sobre dos
tamaños, y comprueba antes de medir que el rebuild que va a medir es el que dice
medir. MEDIDO, sobre el árbol de 120 ficheros: un acierto de caché cuesta
88,0 µs, un cambio de un fichero 4,95 ms y un cambio de 30 ficheros 7,69 ms; el
rebuild de un SOLO fichero ya cuesta cinco veces más en el árbol grande que en el
de doce —4,95 ms contra 1,01 ms—, y 29 ficheros más sólo suman 2,74 ms. El
coste que crece con el árbol es el del rebuild, no el del parseo: eso es lo que
la medición deja dicho, y es lo que hace falta saber antes de autorizar una
optimización sin umbral.

## La misión

Misión `cognicode` declarada (`goal-compass.json`), con 3 candidatos que
llevan evidencia —una ruta que existe— y un criterio de cierre observable. La
cola está escrita en el orden que fija la auditoría del 2026-10-21 (sus bloques
B1 a B6), de mayor a menor prioridad. **C46, C12 y C49 salen de la cola al cerrarse.**

El primero que encadenaba era **C31**, y está cerrado: la autoridad con la que
una herramienta MCP se ejecutaba la leía del catálogo que había en el disco
del proyecto analizado. Un proyecto que declaraba `write_file` como
herramienta de solo lectura conseguía que el modo de solo lectura la
aceptase. La autoridad de una herramienta es del producto, y el producto no
es el proyecto que se analiza. Detrás de él vinieron **C5**, **C21**, **C8**
y **C7**, también cerrados.

**C38 está cerrado.** Lo que se rechaza no deja rastro: una escritura
rechazada por su tamaño ya no deja el árbol de directorios que pidió, y las
ediciones múltiples se aplican sobre el contenido original en vez de sobre el
resultado de la anterior. El motivo del solapamiento se dice en la respuesta,
en vez de perder media edición en silencio.

**C25, C20, C24, C18, C28 y C29 están cerrados.** C17 salió en la PR #404.
C2, C3 y C4 salieron en las PR #400, #401 y #402.

## Gates

| Gate | Comando | Cuándo |
|---|---|---|
| gate | `bash scripts/test-gate.sh` | al cerrar un lote de diez tareas, y antes de pedir review |
| contratos | `bash scripts/ci/run-all-contracts.sh` | aparte del gate: los contratos de producto (manifiestos, catálogos, esquemas) |
| explorer-ui | `npm test --prefix apps/explorer-ui` | aparte del gate: los tests de la UI (vitest: unitarios e integración con MSW) |

El gate son **tres pasos, y los tres verdes**: unitarios, integración con mocks
y mutantes. Los corre `cargo nextest`, una invocación por paso. No hay `fast`,
ni `full`, ni `merge-gate`: se retiraron el 2026-11-04 (`67c474e2`) junto con
las puertas de merge, las lanes de CI y la suite E2E de navegador, porque
medían superficies que ya no existen. Lo que queda detrás del gate es la suite
en verde más una persona que lee el diff.

Los otros dos comandos **no los corre el gate**: los contratos y la UI son
gates aparte, y también tienen que estar verdes antes de pedir review —medido
el 2026-11-05: el gate sobre `14f71609` salió rc=0 con los contratos en rojo,
porque el ratchet del snapshot es un contrato y no un paso del gate—. Ese rojo
era deriva de ritmo —el snapshot declaraba un árbol 56 commits por detrás de la
punta— y lo cerró TASK-126; el gate del lote de diez, medido en serie el
2026-11-06 sobre `a3b8b862`, dio rc=0 con los tres pasos: 4701 unitarios, 6794 de
integración y 627 mutantes —618 atrapados, 9 inviables, 0 supervivientes—. Ese
certificado es del árbol del lote, no de la punta: después llegaron TASK-123,
TASK-127, TASK-140 y TASK-141, y el árbol de hoy mide 4706 unitarios y 6809 de
integración.

El gate corre sobre el árbol de trabajo, así que **no se edita código mientras
corre**: el resultado correspondería a otro árbol. Y corre en un volumen con
espacio: si los temporales van a `/tmp`, la cuota puede matar la batería
entera.

## Bloqueos abiertos

- **La lane de publicación ya no es un bloqueo, pero casi lo fue**: le pedía
  al árbol un directorio que el candidato no escribe, comparaba el conjunto
  publicado contra los manifiestos que la etapa siguiente sube, y descargaba
  los assets justo después de sustituirlos. Los tres están arreglados y
  medidos; la lane llegó a publicar y se ejecutó dos veces sobre el mismo
  tag. Este corte la vuelve a ejercitar.
- **C8-R (CR-01)**: la firma *contractual* del operador sobre un clon limpio.
  La firma operativa está hecha; la contractual no. Las citas al dossier del
  programa cerrado (`docs/prf/`) son históricas por contrato y no se
  reescriben, pero ese dossier ya no está en el árbol: la re-verificación
  tiene que apoyarse en la certificación vigente y en el Journal.
- **SDDK storage**: `~/.local/share/sddk/data/ledger.sqlite` está vacío y el
  ciclo no arranca. Es operator-gated, no un bug de CogniCode.
- **Binarios release**: algunas suites de CLI requieren
  `$CARGO_TARGET_DIR/release`. Si el binario falta, el fallo es de
  infraestructura local, no una regresión: `cargo build --release --bin cogh`.

## Dónde está cada cosa

| Qué | Dónde |
|---|---|
| Agenda (la única autoridad) | `docs/roadmap/ROADMAP.md`, §11 |
| Recibo de cada unidad, append-only | `docs/roadmap/JOURNAL.md` |
| Decisiones | `docs/adr/` y `docs/roadmap/consumer-contracts/**/adr/` |
| Contratos publicados del consumidor | `docs/roadmap/consumer-contracts/specs/` |
| Certificaciones | `docs/roadmap/certifications/` |
| Guía de distribución | `docs/distribution/INSTALL.md` |
| Identidad de acciones de la comunidad | `docs/cognicode-community-productization/` |
| Backlog de la misión | `goal-compass.json` |
| Cómo se decide qué se toca | `AGENTS.md` |
