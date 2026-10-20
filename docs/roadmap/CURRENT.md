# CURRENT — Snapshot operativo (deriva de `docs/roadmap/ROADMAP.md`)

> **Autoridad**: `docs/roadmap/ROADMAP.md` es la **única autoridad de agenda**
> de CogniCode. Este fichero es un **snapshot puntual**, no una segunda fuente
> de estado: donde este documento y el ROADMAP diverjan, manda el ROADMAP y
> este snapshot se considera obsoleto hasta su próxima regeneración.
>
> **Snapshot**: 2026-10-20, sobre `4dbcb9944f88bb6e2e5a7a075c05a899cd0ee352`
> (la base de este ciclo). Versión del workspace: `0.101.23`. Última release
> publicada: `v0.101.23` (2026-10-18, 8 assets). `main` va **32** commits por
> delante de ese tag en cuanto este ciclo se funda: hay estabilizaciones
> mergeadas que todavía no están publicadas.
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

Misión `cognicode` declarada (`goal-compass.json`), con **23** candidatos que
llevan evidencia —una ruta que existe— y un criterio de cierre observable.
La cadena encadena sola el siguiente objetivo en cada cierre de turno.

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

El siguiente trabajo es **C18**: los metadatos reales de la arista y la
profundidad que le corresponde a la consulta. Después, el corte de release,
que es lo que convierte el trabajo en instalable.

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
- **Release pendiente**: `main` lleva 32 commits sin publicar (con este
  ciclo). Publicar exige gates sobre un candidato concreto, binarios
  instalados verificados y tag anotado.

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
