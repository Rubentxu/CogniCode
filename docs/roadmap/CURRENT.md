# CURRENT — Snapshot operativo (deriva de `docs/roadmap/ROADMAP.md`)

> **Autoridad**: `docs/roadmap/ROADMAP.md` es la **única autoridad de agenda**
> de CogniCode. Este fichero es un **snapshot puntual**, no una segunda fuente
> de estado: donde este documento y el ROADMAP diverjan, manda el ROADMAP y
> este snapshot se considera obsoleto hasta su próxima regeneración.
>
> **Snapshot**: 2026-10-21, sobre `933a3e431cb6cb33d436380f1d6970407a47b0ae`
> (la unidad C29 con su recibo, **fusionada en `main`** por la PR #405), rama
> `release/v0.101.24`, `origin/main` = `933a3e43`. Versión del workspace:
> `0.101.24`. Última release publicada: `v0.101.23` (2026-10-18, 8 assets).
> `main` va **33** commits por delante de ese tag: hay estabilizaciones
> mergeadas que todavía no están publicadas, y el corte `v0.101.24` está en
> curso sobre esta rama.
>
> Este fichero se regeneró desde 519 líneas a esta. La versión anterior mezclaba
> un snapshot con la historia de ocho ciclos: narrativas de programas cerrados,
> una lista de "próximo trabajo" que ya estaba hecha, y un estado de release
> que describía `v0.101.16`. **La historia vive en el Journal**; aquí sólo
> vive el estado. Reconstruirlo cuesta una lectura del Journal, no 519 líneas.

## Unidades abiertas

| Unidad | Qué es | Detalle |
|---|---|---|
| **H3** (K1) | gramática tree-sitter de Kotlin: `Language::Kotlin`, `.kt`/`.kts` | `ROADMAP.md` §11 |
| **H4.4** (K2) | relaciones Kotlin↔Java, sólo cuando se pueden probar | `ROADMAP.md` §11 |
| **H5** | `SnapshotKey` e incrementalidad honesta (CU-005) | `consumer-contracts/specs/SPEC-CU-005-SNAPSHOT-INCREMENTAL.md` |
| **H23** | poda por obligaciones y coste de la fase de tests | Journal, entrada del 2026-10-20 |
| **H24** | el producto dice la verdad: manifiesto, CLI=MCP, ontología | Journal, entrada del 2026-10-20 |

## La misión

Misión `cognicode` declarada (`goal-compass.json`), con **19** candidatos que
llevan evidencia —una ruta que existe— y un criterio de cierre observable.
La cadena encadena sola el siguiente objetivo en cada cierre de turno.

La cola está escrita en el orden que fija el informe de la auditoría del
2026-10-21 (sus bloques B1 a B5), de mayor a menor prioridad, que es como la
ordena el selector. **C25, C20, C24, C18, C28 y C29 están cerrados**; el
primero que encadenaría ahora es **C14**, la release con las estabilizaciones
mergeadas, que es el último punto del bloque B1. C2, C3 y C4 no están en la
cola porque su trabajo salió en las PR #400, #401 y #402.

**La declaración estaba inservible y ya no lo está.** Escribía la evidencia
en un campo que el cargador no lee, tenía la cola escrita al revés y una
referencia rota; el resultado eran 26 candidatos descartados y una cadena sin
nada que encadenar. Lo vigila `scripts/ci/test_mission_declaration.py`, que
usa las mismas reglas que el plugin declara. Con el código real del
supervisor, la cadena compone 47 candidatos, no descarta ninguno y elige
C25.

**C2 está resuelto**: el manifiesto de invalidación del grafo se deriva del
registro de lenguajes, y el manifiesto se toma antes del grafo para que no
pueda describir un árbol que el grafo no vio. Antes cubría 9 de las 51
extensiones que el grafo lee, así que `build_graph` podía responder
`status="complete"` con un grafo viejo.

**C3 está resuelto**: las siete órdenes de grafo de la CLI piden el grafo
al mismo servicio que usa el MCP. Antes la CLI nombraba los atributos de
un bloque Terraform y el MCP los recursos; ahora dicen lo mismo.

**C4 está resuelto**: `iac_query` publica el nombre del dominio. El lector
declaraba el tipo y el kind de cada bloque, la proyección al símbolo lo
tiraba, y el manejador publicaba el nombre interno de la variante de Rust:
un bucket respondía `Function`. Ahora responde `aws_s3_bucket`, y una
declaración sin tipo responde con su kind (`variable`, `module`, `play`).

**C17 está resuelto**: una referencia que el grafo no puede atar a una
declaración se publica con su fichero, su línea, su motivo y su clase, en
vez de descartarse en silencio. El estado del grafo distingue las dos
coberturas —la de ficheros y la de relaciones— y `complete` sólo se dice
cuando las dos están completas. La referencia pendiente se intenta atar
entre ficheros con la misma regla con que el lector lee un nombre: la
lectura más larga que nombra una declaración gana, y sólo se miran las
declaraciones del propio módulo que escribe la referencia.

El cambio se revisó dos veces contra el diff, y las dos revisiones
encontraron fallos que el arreglo inicial no cubría. Los cinco se midieron
en rojo antes de arreglarlos: la CLI decía `complete` con una referencia
pendiente —el MCP y la CLI respondían cosas distintas sobre el mismo
proyecto—; la resolución ataba una referencia con una declaración de otro
módulo —una arista que nada prueba—; el snapshot durable seguía con la
versión anterior, que decía `complete` sobre grafos que tenían huecos; el
ámbito subía a los ancestros, y en un módulo hijo eso cablea contra una
declaración que el hijo no puede ver; y la promesa de `complete` era más
grande que lo que el producto mide.

Los cinco están arreglados: las dos superficies dicen `partial` y publican
las referencias pendientes, la resolución sólo cablea lo que el ámbito del
módulo prueba, el snapshot lleva su versión, y la promesa se acotó a lo que
se mide. Con ellos quedaron resueltas cuatro obligaciones de contrato —el
catálogo de producto publicado, el CHANGELOG, la ruta asíncrona del build y
el veredicto cuando no se midió nada— y un test que pregunta a la CLI y al
MCP sobre el mismo árbol. Queda un residual anotado como `C24`: las
superficies por fichero no dicen todavía qué referencias no ataron.

**El gate de cadena de suministro encontró dos avisos de seguridad.** No
venían de este ciclo: `Cargo.lock` estaba intacto respecto a la base, y lo
que los sacó es que el aviso se publicó después de la última vez que `main`
pasó su propio gate —`main` llevaba su check en rojo sin que nadie lo
mirase—. Uno tenía arreglo directo y se aplicó. El otro no lo tenía, y en
vez de silenciarlo salió la dependencia que lo arrastraba: una que la CLI
declaraba y no usaba. El árbol quedó más pequeño y el gate volvió a estar en
verde.

## Los contratos que fija la auditoría del 2026-10-20

Tres specs nuevas, en `docs/roadmap/consumer-contracts/specs/`, escriben lo
que el producto tenía que decir y no decía:

- **SPEC-CU-007** — la cobertura de archivos y la resolución de relaciones
  son dos hechos. Una referencia sin resolver es un hecho con origen y
  motivo, no un silencio; `complete` no se dice si queda alguna pendiente.
- **SPEC-CU-008** — la consulta de infraestructura contesta con el tipo, la
  arista y la profundidad que le corresponden: los metadatos reales de la
  arista, `depth` como profundidad y no como tope, y la ambigüedad
  declarada.
- **SPEC-CU-009** — un recurso de infraestructura no es un símbolo de
  programación. La categoría del extractor genérico no puede contradecir la
  semántica del lenguaje.

Los nueve hallazgos de la auditoría se verificaron uno por uno contra el
código antes de escribirlos: siete son candidatos nuevos de la backlog
(C17–C23) y dos ya estaban (el vigilante y la puerta del dominio). La
revisión del ciclo C17 añadió uno más: `C24`, las superficies por fichero
que no dicen qué referencias no ataron.

## La auditoría del 2026-10-21 reordena el trabajo

No abre unidades nuevas: usa las mismas tres especificaciones y la misma
backlog. Lo que cambia es el orden, y sale de lo que el informe midió en el
código, no de una opinión.

| Bloque | Qué entrega | Candidatos |
|---|---|---|
| B1 | Grafo fiable y MCP seguro; termina publicando las estabilizaciones pendientes | `C18`, `C28`, `C29`, `C14` |
| B2 | Testing rápido y sin duplicidades | `C6`, `C5`, `C7`, `C21`, `C8` |
| B3 | Ontología y arquitectura simplificadas | `C19`, `C30`, `C9`, `C16`, `C10`, `C26`, `C15`, `C27`, `C22`, `C23` |
| B4 | Caché incremental fiable | `C11` |
| B5 | Navegación Kotlin sin JVM | `C12` |
| B6 | Relaciones Kotlin ↔ Java | `C13` |

Es el orden del **roadmap de ejecución del 2026-10-21**, que vive en
`ROADMAP.md` §11.1 con la autoridad de cada bloque. La línea de Kotlin
(`C12`, `C13`) queda al final: es la unidad más grande del roadmap y ningún
bloque la necesita. El corte de release (`C14`) va **dentro** de B1 y **al
final**: publicar las correcciones mergeadas y las de B1 no tiene por qué
esperar a la reescritura del testing ni a la de la arquitectura.

Tres candidatos nacen de la auditoría del 2026-10-20: **C25**, la cobertura
en caché —un fichero recuperado de la caché no cuenta como analizado, y una
construcción en caliente puede informar de un 0 % sobre un grafo completo—;
**C26**, el fallo silencioso del lector especializado, que se degrada a
resultado genérico en vez de conservar el diagnóstico; y **C27**, la prueba
metamórfica, que comprueba identidad, caché, cobertura y relaciones en un
solo escenario. Dos candidatos que ya existían recibieron la evidencia
nueva: **C19** con el caso de Bash, y **C10** con el ejemplo de una
identidad semántica que `SymbolFqn::parse` no entiende.

El informe del 2026-10-21 sobre `4dbcb99` añade **tres candidatos más**:
**C28**, un handler MCP que devuelve `Ok` con un `error` dentro —el
transporte comunica un éxito donde hubo un fallo—; **C29**, la cancelación
MCP, un `AtomicBool` compartido que nunca se restablece y no pertenece a su
petición; y **C30**, el catálogo de capacidades, que declara 21 lenguajes
mientras el registro de parsers tiene más.

**C25 está cerrado**: la cobertura publica los ficheros cubiertos, los
interpretados de nuevo y los reutilizados de la caché, y un build en
caliente informa lo mismo que uno en frío sobre el mismo árbol.

**C20 está cerrado**: un error del recorrido se cuenta como error y una
exclusión legítima como exclusión. El recorrido que decide de qué ficheros
se construye el grafo descartaba sus propios errores, y las tres rutas que
caminan el repo publicaban `Complete` sobre un árbol al que no habían
recorrido entero. El inventario publica ahora las tres cosas por separado
—el hueco real, la exclusión legítima y el error— y ninguna de las tres se
confunde con otra.

**C24 está cerrado**: las superficies por fichero dicen lo que no ataron. El
grafo por fichero sabe qué referencias no puede atar —la caché las guarda y
el grafo del proyecto las publica—, pero el manejador del MCP y la orden de
la CLI respondían como si el fichero no enunciara ninguna. Ahora las dos
publican las referencias pendientes con su origen y su motivo, y dicen lo
mismo sobre el mismo fichero.

**C18 está cerrado**: la consulta de infraestructura contesta con la arista
que tiene. Los dependientes se publicaban siempre como `references` con una
confianza de `1.0` sin mirar la arista que los une, y la profundidad era un
tope de resultados: con `depth: 1` y con `depth: 2` salía lo mismo. Ahora
cada relación lleva el tipo, la procedencia y la confianza de su propia
arista —una arista sin confianza se dice desconocida, no se invierte en `1.0`—,
`depth` es el número de saltos que se caminan, y un nombre que responde por
más de una declaración se declara ambiguo y pide la identidad canónica. El
grafo gana la contracara de las aristas inversas con sus metadatos, y no
cambia de forma en disco.

**C28 está cerrado**: un fallo de MCP llega como fallo. Las herramientas de
analítica del grafo respondían `{"error": "No call graph available. Run
build_graph first."}` como resultado de éxito: el transporte lo pasaba con
`isError: false`, el agente leía que funcionaba donde la herramienta había
fallado y las métricas lo contaban como un éxito. Los doce handlers de
analítica salían de `analytics_graph(ctx)?` —una sola fuente de verdad para
«no hay grafo» y «el store no se pudo leer»—, el símbolo y la comunidad que
no existen son `not found` y el modo que no existe es `invalid input`, y un
fallo de handler nunca llega como un resultado con `isError: false`. El
siguiente trabajo es **C29**, la cancelación MCP: que pertenezca a su
petición y se limpia al terminar.

**C29 está cerrado**: una cancelación pertenece a su petición. El servidor
guardaba una sola bandera de cancelación para toda la sesión y la ponía a
`true` para siempre, así que una petición cancelada envenenaba la sesión:
la siguiente se anunciaba como `Cancelled` aunque nadie la hubiera cancelado.
Medido contra el binario real, una sola notificación bastaba. Ahora cada
petición lleva su propia bandera, en un registro indexado por el
identificador de la petición que se borra cuando la petición termina: la
notificación marca a la que la pidió y a nadie más, y una notificación
tardía no marca a nadie. El protocolo ya traía la solución —rmcp indexa la
cancelación por `request_id`— y el adaptador la pisaba. El siguiente
trabajo es **C14**, la release con las estabilizaciones mergeadas.

Con C29 queda cerrado el bloque B1 de la auditoría del 2026-10-21 salvo la
publicación: C18, C28, C29 y C14 son sus cuatro candidatos, y los tres
primeros ya están.

Detalle de la auditoría que abrió la backlog: `docs/roadmap/auditoria-2026-10-20.md`
(copia local) y la entrada del Journal del 2026-10-20.

## Gates

| Gate | Comando | Cuándo |
|---|---|---|
| fast | `bash scripts/test-fast.sh` | durante el evolutivo, con `--filter` mientras la unidad sea una |
| full | `bash scripts/test-full.sh` | antes de PR, antes de tag, antes de bump de SemVer |
| merge-gate | `bash scripts/ci/publish-merge-gate.sh` | publica el veredicto que `main` exige |

`full` incluye el gate de contratos. `merge-gate` corre sobre el árbol de
trabajo, así que **no se edita código mientras corre**: el resultado
correspondería a otro árbol.

## Bloqueos abiertos

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
- **`merge-gate` verde y publicado**: la corrida del árbol de C29
  (`62c2da66`, `85a7fb7c-0bd9-4345-8e44-9cf50773aef3`) da **56 de 56
  etapas** `success`, y el veredicto **sí se publicó** en GitHub: `merge-gate=success
  published; the gate's own exit code was 0`. Las corridas anteriores no
  publicaban porque el SHA no estaba en el remoto (`HTTP 422`); con la rama
  subida y la PR #404 fusionada, ese bloqueo desaparece. La PR #404 quedó
  mergeada por squash en `e6820982` y la PR #405 —el recibo de C29— en
  `933a3e43`.
- **Release en curso**: `main` lleva **33** commits sin publicar y el corte
  `v0.101.24` está abierto sobre la rama `release/v0.101.24`. Publicar
  exige gates sobre un candidato concreto, binarios instalados verificados y
  tag anotado.

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
