# CogniCode — TODO de la revisión externa (reconciliado)

> **Este fichero no es una autoridad de agenda.** Es el slot donde cae el TODO
> que llega de una revisión externa, hasta que se reparte entre las unidades
> que ya existen.
>
> - **La única autoridad de agenda** es `docs/roadmap/ROADMAP.md` §11.
> - **El recibo append-only** de cada unidad está en `docs/roadmap/JOURNAL.md`.
> - **La cola de trabajo de la misión** está en `goal-compass.json`.
> - **Cómo se decide qué se toca** está en `AGENTS.md`.

## Qué había aquí

Hasta `e3832b87` este fichero contenía el TODO de la revisión de código remoto
de Kotlin, **byte a byte idéntico** a
`docs/roadmap/consumer-contracts/kotlin/KOTLIN-LINE-K0-K3.md` —la copia
canónica, que es la que manda y que sigue en el árbol. Nada se pierde al
sustituirlo: la línea Kotlin ya está integrada en el roadmap vivo. K0 y K1
están cerradas con veredicto propio, K2 es `C13` y ya está cerrada, y K3 no
está abierta.

## Qué hay ahora

La auditoría externa del 2026-10-26 —base `main@87aab47f`— está reconciliada,
cada hallazgo medido sobre `e3832b87`. Su borrador queda como documento local
no versionado, igual que el de 2026-10-20: lo que se versiona no es la
auditoría, es lo que se decide con ella. El enrutado:

| Hallazgo de la auditoría | Dueño |
|---|---|
| H2 — el escaneo confunde un fallo de lectura con «no ha cambiado» | **`C49`**, cerrada el 2026-10-28 (`5c63e949`) |
| H4, H5, H6, H7a, H7b | `C23` (PR #464, bloqueada por el operador) |
| H3 — la resolución se decide por proximidad de ficheros | **`C13`** (`H4.4`), cerrada el 2026-10-29 (`d9bf2b45`); su residuo declarado —la pérdida de una relación no se publica— lo toma **`TASK-14`**, cerrada el 2026-10-29 (`00169311`); el residuo de ese cierre —la equivalencia CLI/MCP no se ha ejercido sobre este corpus— lo toma **`TASK-15`**, cerrada el 2026-10-29 (`9e1d36d7`); el residuo de ese cierre —la respuesta que el MCP sirve desde la caché es un veredicto hueco— lo toma **`TASK-16`**, cerrada el 2026-10-29 (`323abe72`) |
| H1 — ventana entre verificar y publicar | declarado, sin unidad: pide una decisión de alcance |
| H8 — el refactor construye grafos temporales | declarado, sin unidad: pide una decisión de alcance |
| H7c — el presenter reconstruye datos parseando texto | residuo dentro del bloque de `C23` |
| «Kotlin sigue declarado como no soportado» | **falso en este árbol**: `KNOWN_UNSUPPORTED_EXTENSIONS` está vacío desde K1 (`4920517b`) |

La reconciliación de las siete pull requests que seguían abiertas está medida
por SHA de blob contra `origin/main`: **#465–#470 ya están dentro de `main`** —
via el squash `#471` (`8689de6e`), y #470 además via `#472` (`c87340f0`)—, y
**#464 es `C23`**, sin ni un solo fichero en común con `main`. La causa está en
el historial: `#471` fue un squash de seis pull requests a la vez, así que sus
ramas quedaron viejas, no su trabajo pendiente.

## La auditoría del 2026-10-28

La segunda auditoría externa —base `main@2eef9d4f`— se reconcilió el mismo día,
cada hallazgo medido contra el árbol real (`main@5c63e949`). Su borrador queda
como documento local no versionado, igual que los anteriores. El enrutado:

| Hallazgo de la auditoría | Dueño |
|---|---|
| Lecturas silenciadas en el escaneo | **`C49`**, cerrada el 2026-10-28 (`5c63e949`) |
| Recuentos de símbolos publicados como medidos y no medidos (`symbols_removed = deleted_files.len() * 2`, `symbols_added += 1`, el `symbol_count` del manifiesto) | **`TASK-1`**, cerrada el 2026-10-28 (`ca378733`) |
| Resolución ambigua de símbolos en la capa de aplicación | **`TASK-2`**, cerrada el 2026-10-28 (`c7e5439c`) |
| Errores de arista descartados con `.ok()` | **`TASK-3`**, cerrada el 2026-10-28 (`b75d2028`) |
| La PR #464 sube la deuda CR-06 de producción de 28 a 35 pares | **`TASK-4`** — puerta medible de `C23` |
| Dos propietarios potenciales del estado del grafo | **`TASK-5`** |
| Esperas fijas en la suite E2E de Explorer (59 en 24 ficheros) | **`TASK-6`**, cerrada el 2026-10-29 (`14a67d7f`) |
| Casos desactivados en la suite E2E de Explorer (17 en 14 ficheros; la auditoría contó 16 en 13) | **`TASK-7`**, cerrada el 2026-10-29 (`9f8b2181`) |
| Catálogo público desactualizado | **`C44`**, cerrada el 2026-10-29 (`cf8ad557`) |
| Acoplamiento aplicación↔infraestructura, `views.rs` (9 669 líneas), `AnalysisService` (4 517) | `C23` |
| TOCTOU entre verificar y publicar (`file_operations.rs:1281-1287`) | declarado, sin unidad: pide una decisión de alcance |
| Duplicidad de las specs de perspectiva de Explorer | **no se reproduce**: `perspective-toggle.spec.ts` prueba P2.1–P2.6 y `perspective-toggle-full.spec.ts` tiene un test de visibilidad y otro desactivado con `[DEBT: …]`; el caso desactivado es justamente el que la auditoría llama duplicado |
| Plan de cinco fases | no entra: su bloque de mayor retorno es `C23`, que ya está en la cola |
| Propuestas laterales A–D | `DRAFT-1` a `DRAFT-4`: son propuestas, no trabajo hasta que alguien las mida |

`TASK-1` a `TASK-7` y `DRAFT-1` a `DRAFT-5` viven en la bandeja local de
Backlog.md (`.backlog/`, fuera de Git por la política de documentos privados).
Sus identificadores y su recibo están en `docs/roadmap/JOURNAL.md`. La bandeja
no es una segunda agenda: la cola viva sigue en `goal-compass.json` y la
autoridad de agenda en `docs/roadmap/ROADMAP.md` §11.

## Lo que este fichero NO decide

- **No abre un programa nuevo.** El bloque de mayor retorno que propone la
  auditoría —una autoridad única de inventario y snapshot— es `C23`, que ya
  está en la cola con prioridad 50. Ejecutarlo dos veces sería el segundo
  roadmap que este proyecto no admite.
- **No reabre Kotlin ni el programa PRF.** Sus cierres están escritos y sus
  mediciones son evidencia histórica.
- **No fija la prioridad de la cola.** La prioridad es del operador; la
  medición de este documento la justifica, pero no la decide.

## La puerta que queda abierta

**El ciclo de TASK-16 está cerrado en el remoto.** El pull request `#497` —la
publicación de la rama `measure/mcp-cache-verdict-truth`— se fusionó por squash
como `323abe72`, con el `merge-gate` verde sobre `97fbef20`. La respuesta que
el MCP sirve desde la caché ya no está AFIRMADA como honesta: está MEDIDA, y
medida sobre el hueco que la propia comprobación de frescura no puede ver.

**El residuo, con las palabras del cierre anterior.** El recibo de `TASK-15`
cerraba DESCARTANDO un camino de medición, y lo hacía nombrando la superficie
que esta unidad mide:

> **El peligro que se descartó antes de medir.** El MCP tiene un camino de
> CACHÉ que responde `complete` con la lista de pendientes VACÍA
> (`handlers/mod.rs:1491`, la rama `loaded_from_cache`): una equivalencia
> medida sobre un acierto de caché sería falsa por construcción —una superficie
> con la negativa y otra sin ella—.

El residuo hablaba de la lista de PENDIENTES; la superficie a medir es el
VEREDICTO entero de la respuesta. La pregunta abierta era si un consumidor del
MCP puede recibir un `complete` HUECO: un veredicto `complete` acompañado de
una lista de pendientes vacía mientras el grafo real NO estaba completo.

**El mecanismo, leído en el código antes de medir.** `handle_build_graph`
(`crates/cognicode-core/src/interface/mcp/handlers/mod.rs:1347`) tiene DOS
caminos de caché —la instantánea durable (`.cognicode/graph.cache`) y el
manifiesto de la misma sesión— y los dos marcan `loaded_from_cache = true`. En
los dos, la respuesta se armaba con tres listas vacías y `complete`. Lo que la
respuesta NO llevaba era el estado del árbol: el veredicto era el del BUILD que
persistió la instantánea, no el del árbol que el consumidor tenía delante.

**Por qué la caché no podía saberlo.** El manifiesto que decide si la
instantánea sigue siendo fresca describe los FICHEROS de los que se construye
el grafo —los que `SourceInventory::source_files()` deja pasar—. Una entrada
que el recorrido DECLINA —un enlace simbólico que no sigue— NO es uno de esos
ficheros, así que no está en el manifiesto, así que `is_manifest_stale` no
puede verla. La comprobación de frescura era ciega a una clase entera de
huecos: los que el recorrido deja a propósito, que son los que más se parecen
a una decisión y menos a una avería, y por eso son los que un consumidor no
puede ignorar.

**MEDIDO, sobre el corpus de tres ficheros Rust que el repo ya versiona
(`crates/cognicode-core/testdata/silent_errors_corpus`), con un enlace
simbólico declinado en `src/`:**

| | respuesta | veredicto | omitidos | símbolos | relaciones |
|---|---|---|---|---|---|
| en frío, sobre el árbol | freshly built | `partial` | `src/link.rs` [symlink] | 3 | 0 |
| servida de la caché | cached | `complete` | *(vacío)* | 3 | 0 |

El GRAFO era el mismo y no estaba vacío: 3 símbolos y 0 relaciones en las dos.
El hueco no estaba en el grafo: estaba en el VEREDICTO, que afirmaba de un
árbol algo que la caché no podía saber. Un consumidor que leyera `complete`
creería conocer un proyecto que sólo había visto a medias, y no tendría ni
siquiera un hueco enumerado que le permitiera sospechar.

**La forma del dominio no cambia, y por eso no es una preferencia.**
`BuildGraphOutput` sigue con los mismos cuatro campos y `"complete"`,
`"partial"`, `"unknown"` y `"unsupported"` siguen siendo su juego de valores.
No hay ADT nuevo, ni variante nueva, ni campo nuevo en ningún contrato
público. Lo que cambia es de dónde sale el CONTENIDO de las tres listas cuando
la respuesta viene de la caché: no del build que persistió la instantánea,
sino del ÁRBOL.

**La corrección no cuesta un recorrido nuevo.** La comprobación de frescura ya
recorría el árbol, así que el hueco del árbol sale de la MISMA pasada: el
inventario se calcula donde ya se calculaba y se pasa a `is_manifest_stale` en
vez de que aquél lo vuelva a recorrer. Y los huecos tienen ahora un SOLO
dueño: `SourceInventory::coverage_holes`
(`crates/cognicode-core/src/infrastructure/graph/source_files.rs:163`), que es
la composición de lo que el recorrido declined, lo que no pudo hacer y lo que
todavía no sabe leer. Antes los componían tres sitios por su cuenta —la ruta
de proyecto entero, la de un fichero y la de K0—, y tres composiciones propias
son tres respuestas esperando a separarse.

**MEDIDO, después de la corrección.** La respuesta servida de la caché
publica ahora los huecos del árbol TAL COMO ESTÁ AHORA y lleva el veredicto
que una construcción en frío daría: `partial` con `src/link.rs` declarado como
`symlink`, el mismo grafo —3 símbolos, 0 relaciones— y el mismo motivo. Y el
veredicto de la caché y el de una construcción en frío son IGUALES, campo por
campo.

**Y la paridad mejora.** La CLI no tiene camino de caché: `GraphCommand::Full`
(`crates/cognicode-core/src/interface/cli/commands.rs:1086`) va por
`ServiceAnalyzer` y `AnalysisOp::GraphFull`, que siempre construye y siempre
publica los huecos actuales. `loaded_from_cache` sólo existe en el handler del
MCP. La corrección, por tanto, CIERRA una diferencia de paridad entre las dos
superficies en vez de abrirla: hoy las dos dicen lo mismo del mismo árbol,
caliente o frío.

**La prueba no puede pasar por vacío, y eso es del contrato, no del estilo.**
Dos respuestas vacías son iguales en el vacío, y una equivalencia medida sobre
un grafo vacío es una equivalencia medida sobre nada. El test —en
`crates/cognicode-core/tests/cached_coverage_is_honest.rs`, el fichero que ya
medía la cobertura en caliente y en frío— exige, antes de comparar, que la
respuesta EN CALIENTE venga de la caché —su mensaje lo dice—, que el grafo no
esté vacío, y que el hueco sea EL que el árbol declara —`src/link.rs` como
`symlink`—, porque dos respuestas que comparten un error son iguales. La
comparación se hace sobre la ruta RELATIVA al proyecto, porque dos copias del
corpus viven en dos directorios y lo que se compara es el árbol.

**El RED, medido con mutación focal.** Dos mutaciones, cada una revertida, y
cada una muerta sólo por el test escrito para ella:

1. **El veredicto de la caché miente** (la rama de caché vuelve a decir
   `complete`): el test cae en la comparación de las dos respuestas —
   `left: Answer { status: "complete", … }` contra
   `right: Answer { status: "partial", … }`.
2. **El motivo del hueco se pierde** (el brazo del enlace simbólico deja de
   decir `symlink`): el test cae en la aserción ABSOLUTA del hueco —
   `left: [("src/link.rs", "other")]` contra
   `right: [("src/link.rs", "symlink")]`—, que es la que una comparación entre
   dos respuestas no puede ver.

Restaurados byte a byte, los dos dies y el test pasa. Un contrato que no se ha
visto rojo no mide nada.

**El inventario del harness, medido antes y después.** CERO delta: 235 ficheros
de test en `crates/` antes y **235** después; 62 contratos de CI
(`scripts/ci/*.py`) antes y **62** después; 985 ficheros versionados en
`sandbox/fixtures/` antes y **985** después. Ni un fichero de test nuevo, ni un
contrato nuevo, ni un fixture nuevo: el corpus ya versionado y el fichero que
ya medía la cobertura en caliente y en frío eran todo el harness que esta
unidad necesitaba. El catálogo publicado (`product/tools.json`) es MANUSCRITO
y no se regenera: se editó a mano, y sólo en las cuatro descripciones de
`build_graph` que el código publica —las que son el espejo de los comentarios
del doc—.

**Lo que este corte NO cubre, y queda declarado.** La respuesta servida de la
caché publica los huecos del árbol, pero sólo los que el recorrido puede
ENUMERAR: un directorio que el proceso no puede leer es un hueco que se
publica, y una entrada que desaparece entre la lectura y la medida también. Lo
que NO se publica es el hueco de un árbol al que el recorrido no puede ENTRAR
—un directorio sin permiso de ejecución, por ejemplo—, porque esa información
no está en el inventario: es la misma limitación que ya tenían las tres rutas
que lo componen, y esta unidad no la empeora ni la arregla. Y el camino de
caché de la INSTANTÁNEA DURABLE y el de la MISMA SESIÓN se comportan igual
porque comparten la rama, pero la verificación se ha hecho sobre el primero —el
mensaje lo dice, «durable snapshot»—; el segundo no se ha ejercitado aparte.

**Y dos derivas que este ciclo NO arregla, porque son de otra superficie.**
`product/tools.json` declara una descripción de `BasisDto.complete` que ya no
coincidía con la que el runtime publica, ANTES de este ciclo: es la superficie
de la PROCEDENCIA del análisis, no la del veredicto de cobertura, y arreglarla
aquí sería tocar un contrato que esta unidad no midió. Y el segundo mapa de
`SkipReason` a motivo —`SkippedReasonKind`, `handlers/mod.rs:3876`— pertenece
al contrato publicado de otra herramienta. Las dos quedan ENRUTADAS, no
arregladas.

**La cola.** `goal-compass.json` NO cambia en este ciclo, y hay que decirlo sin
rodeos: `TASK-16` nunca fue candidata de esa cola —nació como el residuo que el
cierre de `TASK-15` dejó medido y con dueño, en la bandeja local—, así que no
hay ficha que sacar. La cola viva sigue con `C23` (50, bloqueada por el
operador) y nada más.

**Gates.** `fast` rc=0; `full` rc=0 —6504 pasados, 0 fallidos y 34 ignorados
contando los doc-tests; 454 comprobaciones de contrato en 0 fallos—. Y una
advertencia que el gate mismo enseña y que este recibo declara: el `fast` NO
ejecuta el fichero de test nuevo, porque el selector CR-08 cae en su regla de
catch-all —`product/tools.json` es un path que no conoce— y el camino de
fallback corre `cargo test --workspace --lib`, que son SÓLO pruebas unitarias.
La prueba de esta unidad se ejecutó directamente —`cargo test -p cognicode-core
--test cached_coverage_is_honest`, 5 pasados— y el `full` sí la incluye,
porque corre `cargo test --workspace`. Un run filtrado no es evidencia de
gate, y aquí se dice cuál es cuál.

La puerta que queda abierta es la de la cola:

- **`C23`** — la unidad de mayor retorno de todo lo que propone la auditoría
  externa. Sigue bloqueada por el operador: su pull request `#464` es la
  única de las siete que aún lleva trabajo íntegro.

Y, con `TASK-16` cerrado, la cola viva se queda **sin ninguna unidad sin
bloqueo** otra vez: `TASK-16` era el residuo que el cierre de `TASK-15` dejó
medido y con dueño, y ya está hecha. Las vecinas del entorno siguen
igualmente atadas: `TASK-5` por una decisión de alcance del operador y
`TASK-4` por `C23`. La siguiente unidad, por tanto, **no la decide este
documento**: la desbloquea el operador, y hasta entonces el trabajo de este
ciclo es el que queda medido y enrutado.
