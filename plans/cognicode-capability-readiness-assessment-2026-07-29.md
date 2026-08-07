# Evaluación integral de capacidades de CogniCode

> **Fecha:** 2026-07-29  
> **Base analizada:** `cdf1d588`  
> **Ámbito:** ingestión, evolución temporal, persistencia y consulta de grafos,
> MoldQL, algoritmos, representaciones semánticas y `cognicode-explorer`  
> **Modalidad:** análisis de solo lectura; no se modificó código fuente  
> **Registro de interfaz:** Product  
> **Estrategia de color observada:** Restrained

## Resultado ejecutivo

CogniCode **todavía no cumple de extremo a extremo** el objetivo de analizar una
base de código de forma eficiente, repetir ese análisis a lo largo del tiempo y
explorarlo visualmente mediante un lenguaje, algoritmos y diagramas semánticos.

La conclusión necesita una precisión importante:

- La **base tecnológica** es adecuada y, en varios lugares, avanzada.
- El **modelo conceptual** es prometedor.
- Existen implementaciones valiosas de extracción, grafos, algoritmos, vistas,
  investigación y navegación.
- Sin embargo, las piezas críticas no están conectadas bajo un único contrato
  operativo.
- Varias capacidades marcadas como entregadas existen como tipos, adaptadores,
  pruebas aisladas o exportadores, pero no como una experiencia reproducible de
  extremo a extremo.

La madurez se divide en dos niveles muy distintos:

| Nivel | Evaluación | Motivo |
|---|---:|---|
| Sustrato técnico | 3/4 | Buen stack, modelos tipados, algoritmos y puertos útiles |
| Operabilidad de extremo a extremo | 1/4 | Ingestión, revisiones, ejecución y UI tienen bloqueos |
| Fidelidad semántica de diagramas | 1/4 | Call graph útil; C4 heurístico; UML/casos de uso incompletos |
| Preparación para producción | 1/4 | Faltan garantías de build, escala, cancelación y trazabilidad |

**Veredicto global:** CogniCode está en una fase de **fundación avanzada pero
producto integrado temprano**. El siguiente salto no consiste en añadir más
algoritmos o más tipos de diagrama. Consiste en hacer verdadera, reproducible y
visible la cadena que ya fue diseñada.

## Objetivo evaluado

El objetivo se descompuso en siete preguntas verificables:

1. ¿Puede CogniCode ingerir una base de código grande eficientemente?
2. ¿Puede actualizarla incrementalmente sin perder ni conservar datos erróneos?
3. ¿Puede consultar una revisión histórica exacta después de nuevas ingestas?
4. ¿Tiene una base de grafos potente, tipada y fácil de explotar?
5. ¿MoldQL y los algoritmos ejecutan consultas reproducibles y gobernadas?
6. ¿Explorer convierte esas capacidades en una interfaz clara y útil?
7. ¿Las vistas C4, casos de uso, clases, llamadas y secuencias preservan semántica
   real, en lugar de limitarse a dibujar cajas?

## Método y límites de la evaluación

La evaluación combinó:

- inspección directa de código Rust, TypeScript, SQL y pruebas;
- contraste entre `CONTEXT.md`, ADR, especificaciones y `ROADMAP.md`;
- auditorías separadas de ingestión, grafos, MoldQL, algoritmos, diagramas,
  operabilidad y Explorer;
- validación manual de los hallazgos críticos citados por los auditores;
- una ejecución de build/E2E de Explorer durante la auditoría visual;
- análisis heurístico de connascencia y profundidad de módulos.

No se ejecutó la matriz completa de `cargo test --workspace` ni una prueba de
carga real contra PostgreSQL. Los defectos clasificados como críticos en este
informe se apoyan en contratos y rutas de código inequívocas. Las estimaciones
de rendimiento que requieren medición se identifican como tales.

## Mapa de madurez actual

Escala utilizada:

- **0 — Ausente:** no hay modelo ni ruta ejecutable.
- **1 — Prototipo:** vocabulario, tipos o prueba aislada.
- **2 — Parcial:** implementación útil con brechas de integración o fidelidad.
- **3 — Utilizable:** flujo principal operativo con límites conocidos.
- **4 — Producción:** reproducible, observable, probado y gobernado.

| Capacidad | Madurez E2E | Situación actual |
|---|---:|---|
| Extracción sintáctica multilenguaje | 2 | 30 configuraciones; fidelidad desigual |
| Ingestión incremental | 1 | Tiene etapas, pero la ruta actual contiene bloqueos críticos |
| Historial temporal de grafos | 1 | Hay IDs de revisión, no estados históricos persistidos |
| Modelo de grafo tipado | 3 | Nodos, aristas, procedencia, confianza y propiedades sólidas |
| Persistencia canónica PostgreSQL | 2 | Elección correcta; migraciones y contratos inconsistentes |
| Navegación estructural | 2 | Call graph útil; puertos y stores aún fragmentados |
| MoldQL de selección | 2 | `FIND` funciona parcialmente; límites y escalabilidad pendientes |
| MoldQL gráfico | 1 | AST y planes existen; ejecución productiva está vacía o deshabilitada |
| Algoritmos de grafo | 2 | Buen catálogo puro; sin registro ni lineage de ejecución |
| Call graph visual | 3 | Es la representación más madura |
| C4 | 1 | Proyección heurística del filesystem y manifests |
| Casos de uso / vertical slices | 1 | Recorrido de llamadas etiquetado como caso de uso |
| UML de clases / jerarquía de tipos | 1 | Datos parciales; semántica se aplana a `References` |
| Secuencias UML | 0 | No existe modelo temporal de participantes y mensajes |
| ViewSpec moldable | 1 | Tipos, store y wizard; ejecución y rutas incompletas |
| Explorer UX | 1 | Buena dirección; el build actual no compila |
| Escala y operabilidad | 1 | Budgets y métricas parciales; sin evidencia a escala real |

## Fortalezas que deben conservarse

### Stack tecnológico coherente

Rust, Tree-sitter, PostgreSQL, SQLx, Petgraph, React, Cytoscape, ELK, Mermaid,
MCP y REST son elecciones adecuadas. El análisis previo del proyecto llega a la
misma conclusión y señala correctamente que el riesgo está en los contratos, no
en la tecnología (`docs/analysis/cognicode-graph-stack-assessment.md:11-20`).

No existe evidencia que justifique sustituir PostgreSQL por Neo4j. ADR-014
mantiene PostgreSQL como fuente canónica y reserva Neo4j como posible oráculo de
CI (`docs/adr/ADR-014-moldql-pattern-graph-analytics-platform.md:28-41`,
`:153-159`). Esa decisión es correcta.

### Modelo rico de nodos y aristas

`GraphNode` y `GraphEdge` permiten propiedades JSON tipadas, procedencia y
confianza. Ésta es una base considerablemente mejor que un call graph plano.
El vocabulario incluye símbolos, decisiones, documentos, evidencia, issues y
entidades arquitectónicas.

El problema no es expresividad del dato, sino que las interfaces productivas no
explotan de manera uniforme ese modelo.

### Buen sustrato de planes

`MoldPlan`, `GraphPlan`, `PlanLimits`, `ResultSet`, `ExecutorError` y
`ProvenanceEnvelope` establecen una dirección arquitectónica correcta. ADR-014
define bien la separación entre lenguaje, plan normalizado y ejecutor
(`docs/adr/ADR-014-moldql-pattern-graph-analytics-platform.md:61-96`).

### Algoritmos puros reutilizables

`cognicode-graph-algos` concentra funciones puras para PageRank, caminos,
componentes, comunidades, reducción transitiva y conexiones sorprendentes
(`crates/cognicode-graph-algos/src/lib.rs:1-35`). El diseño mediante slices
planos evita acoplar los hot loops al dominio y permite reutilización nativa.

### Registro de vistas incorporadas

La distinción entre `ViewKind`, `RendererKind` y `HierarchyKind` es valiosa
(`crates/cognicode-explorer/src/dto.rs:1241-1428`). El patrón
`ViewDescriptor`/`ViewExecutor` y el registro de vistas incorporadas es uno de
los módulos más profundos y coherentes del sistema.

### Navegación de investigación

La pila de panes, Spotter, investigaciones, evidencia, narrativas y preservación
del recorrido de exploración expresan una visión de producto diferenciada. No
es un dashboard genérico: intenta convertir la exploración en una narrativa
técnica durable.

### Sistema visual de producto razonable

Explorer posee una estrategia visual restringida, tokens coherentes, foco
visible y reducción global de movimiento
(`apps/explorer-ui/src/tailwind.css:8-85`, `:106-150`). La dirección visual es
adecuada para una herramienta de ingeniería.

## Bloqueadores críticos

### P0-1 — Una base PostgreSQL nueva no puede aplicar correctamente la cadena de migraciones

`m0018` crea foreign keys hacia `(workspace_id, id)`
(`m0018_workspace_scoped_identity.sql:111-139`), pero el índice único que hace
válida esa referencia se crea recién en `m0019`
(`m0019_unique_index_workspace_id.sql:24-33`). La propia migración documenta que
sin ese índice una base nueva falla (`m0019_unique_index_workspace_id.sql:14-20`).

El runner ejecuta `m0018` y sólo después `m0019`
(`postgres_repository.rs:251-273`). Como cada paso propaga el error, `m0019` no
puede reparar una migración anterior que ya abortó.

Además:

- PgUpsert conserva `ON CONFLICT (id)` para nodos
  (`pg_upsert_stage.rs:136-165`), aunque la identidad ya es workspace-scoped.
- Conserva `ON CONFLICT (source_id, target_id, kind)` para aristas
  (`pg_upsert_stage.rs:197-222`), mientras el índice único incluye
  `workspace_id` (`m0018_workspace_scoped_identity.sql:90-98`).
- El trigger común accede a `NEW.source_path`
  (`m0010_pipeline_schema.sql:219-240`), pero `graph_edges` no tiene esa columna
  (`m0010_pipeline_schema.sql:34-44`).

**Consecuencia:** la persistencia canónica no tiene hoy un bootstrap confiable.
Todo análisis que dependa de una base nueva, CI o despliegue limpio queda bajo
sospecha hasta cerrar esta inconsistencia.

### P0-2 — La ingestión entra en deadlock con más de diez resultados

`run_scan` consume completamente el canal de extracción y acumula todos los
resultados en un `Vec` (`ingest/service.rs:78-110`). Después crea un segundo
canal con capacidad `BATCH_SIZE` y espera cada `send` antes de iniciar
`pg_upsert_streaming` (`ingest/service.rs:112-129`).

`BATCH_SIZE` vale diez (`pg_upsert_stage.rs:25-27`). El undécimo envío espera a
un consumidor que todavía no fue iniciado.

**Consecuencia:** la ruta principal no puede ingerir de manera fiable una base
de código real. Este defecto invalida cualquier afirmación actual de pipeline
streaming o de eficiencia a escala.

### P0-3 — Las revisiones no preservan el estado histórico del grafo

`graph_revisions` sólo almacena workspace, número, fecha y flag de head
(`m0017_graph_revisions.sql:16-22`). Los nodos y aristas no llevan
`revision_id`.

Guardar una revisión elimina el grafo actual del workspace y lo reemplaza
(`postgres_repository.rs:502-517`). Cargar una revisión valida que el ID haya
existido, pero consulta los nodos y aristas actuales sólo por workspace
(`postgres_repository.rs:1269-1315`, `:1346-1365`).

La ingestión incremental tampoco invoca el commit de revisión; hace upserts
directos y luego intenta refrescar (`ingest/service.rs:112-193`).

**Consecuencia:** no es posible garantizar que una consulta fijada a la revisión
N devuelva el grafo de N después de ingerir N+1. La funcionalidad central
“analizar a lo largo del tiempo” todavía no existe con fidelidad estructural.

### P0-4 — Explorer no compila actualmente

La auditoría ejecutó `npm run build` en `apps/explorer-ui` y obtuvo errores de
TypeScript/Vite. El bloqueo más directo está confirmado en el código:

- `z` se usa antes de importarse (`useInvestigations.ts:43`).
- `z` se importa en mitad del módulo (`useInvestigations.ts:160-162`).
- vuelve a importarse al final (`useInvestigations.ts:259-260`).

También se observaron contratos faltantes, imports inválidos y errores de tipos
en otros módulos.

**Consecuencia:** no existe un artefacto frontend de producción verificable. No
se puede aprobar visualmente la interfaz mientras la aplicación no alcance un
estado de runtime estable.

## Pilar 1 — Ingestión eficiente e incremental

### Lo que existe

El pipeline está modelado con etapas reconocibles:

```text
Scan → Extract → PgUpsert → Resolve → Cluster → Analyze → Report → Refresh
```

Hay hashing SHA-256, `scan_manifest`, extracción con Tree-sitter, batching,
resolución cross-file, comunidades, informes y refresh. El diseño conceptual es
el adecuado.

### Lo que impide considerarlo operativo

#### Las eliminaciones no llegan al grafo

Los cambios `Deleted` se eliminan del conjunto antes de extracción y persistencia
(`ingest/service.rs:78-82`). No existe un comando de borrado equivalente enviado
a PgUpsert.

El cleanup del manifest conserva `previous.keys()`
(`ingest/service.rs:172-176`) en vez del conjunto de archivos de la exploración
actual. En una primera ejecución ese conjunto puede estar vacío; en ejecuciones
posteriores puede conservar rutas eliminadas.

#### Se pierde el beneficio del mtime

PgUpsert persiste `mtime: 0.0` (`pg_upsert_stage.rs:226-239`), aunque el scanner
utiliza mtime como filtro previo al hash. Eso fuerza trabajo redundante y reduce
la eficiencia incremental.

#### Un fallo de extracción puede borrar el último estado válido

`upsert_one` elimina primero nodos y aristas del archivo
(`pg_upsert_stage.rs:123-134`). Si el resultado de extracción está vacío por un
fallo, el último estado conocido se pierde antes de disponer de un reemplazo
válido.

#### Cluster, Analyze y Report observan el cache anterior

Cluster y Analyze se ejecutan antes de Refresh
(`ingest/service.rs:157-193`). Ambos leen el `GraphCache`. El informe puede
describir el grafo previo a la ingestión o un grafo vacío.

#### El lock no tiene guard real

Se usa `pg_advisory_lock`, que es session-scoped, mediante el pool. El valor
`Some(())` no libera el lock al salir del scope (`ingest/service.rs:47-60`). La
conexión puede volver al pool conservando el lock.

### Estado del objetivo

**No cumplido.** La estructura del pipeline es prometedora, pero la ruta crítica
necesita una corrección de coherencia antes de medir rendimiento.

## Pilar 2 — Análisis a lo largo del tiempo

### Lo que existe

- IDs de revisión por workspace.
- Un head por workspace.
- `SnapshotProvider` y cache versionado en memoria.
- informes acumulados con fecha.
- herramientas llamadas `graph_diff` y `graph_timeline`.

### Brecha fundamental

Los IDs de revisión no identifican un conjunto inmutable de nodos y aristas.
Son metadatos sobre una tabla mutable.

Las herramientas temporales comparan principalmente conteos de símbolos,
aristas y health score almacenados en informes. No realizan un diff estructural
de nodos, aristas, propiedades o caminos entre dos revisiones.

### Capacidades mínimas que faltan

1. Persistencia temporal mediante snapshots por revisión o validez
   `valid_from`/`valid_to`.
2. Relación explícita `GraphReport → RevisionId`.
3. Retención y garbage collection de revisiones.
4. Diff estructural tipado:
   - nodos añadidos, eliminados y modificados;
   - aristas añadidas, eliminadas y modificadas;
   - cambios de procedencia, confianza y propiedades;
   - cambios de API, jerarquía, slice y arquitectura.
5. Consultas y algoritmos siempre fijados a `(workspace, revision)`.
6. Sesiones e investigaciones que preserven esa misma revisión.

### Estado del objetivo

**No cumplido.** Hay vocabulario y piezas iniciales, pero no historia estructural
reproducible.

## Pilar 3 — Base de información de grafos

### Fortalezas

- PostgreSQL es una fuente canónica adecuada.
- `GraphNode`/`GraphEdge` son extensibles.
- La procedencia y la confianza permiten distinguir hecho, inferencia y
  evidencia.
- La separación entre `SymbolRepository` y navegación estructural mediante
  `GraphQueryPort` es conceptualmente correcta.
- Los ejecutores PG y snapshot ofrecen una base para optimizar por backend sin
  filtrar esa elección al usuario.

### Brechas

#### Múltiples grafos no sincronizados

El runtime carga un `Arc<CallGraph>` al arrancar y construye sobre él
`SymbolRepository` y `GraphQueryPort` (`cognicode-runtime/src/lib.rs:39-80`,
`:100-121`). La ingestión actualiza otro `GraphCache`
(`cognicode-runtime/src/lib.rs:123-136`).

No hay garantía de que Search, Views, MoldQL e ingestión lean la misma revisión.

#### Los nuevos ejecutores no están compuestos en runtime

No hay wiring productivo de `PgGraphExecutor`, `SnapshotGraphExecutor` ni un
`Arc<dyn GraphExecutor>` en `cognicode-runtime`. Las implementaciones existen,
pero el producto sigue usando rutas legacy.

#### Límites declarados pero no uniformemente aplicados

El contrato declara tiempo, cancelación, profundidad, nodos visitados, aristas
visitadas, filas, caminos y memoria. La auditoría encontró dimensiones
declaradas que no se hacen cumplir durante el recorrido y cancelación comprobada
después del trabajo costoso.

#### Caches y listeners no tienen lifecycle operativo completo

El provider creado en `run_scan` es local y efímero
(`ingest/service.rs:181-187`). No es el mismo provider compartido por los
lectores del runtime.

### Estado del objetivo

**Parcial.** El modelo de datos es potente. La explotación uniforme y segura no
lo es todavía.

## Pilar 4 — MoldQL

### Lo que existe

- selección de objetos con `FIND`;
- navegación `EXPLORE`;
- primitivas `PATH`, `NEIGHBORS`, `SUBGRAPH`, `CLUSTER`, `EXPLAIN`;
- composición booleana;
- parser de intención reducido;
- `MoldPlan` y `GraphPlan` normalizados;
- límites, errores y resultados tipados;
- dos ejecutores concretos.

### Lo que ejecuta realmente el producto

El executor productivo sigue llamando el compilador legacy y selecciona
Petgraph por defecto (`moldql/executor.rs:70-96`).

La ejecución Petgraph devuelve siempre un éxito vacío para las primitivas
gráficas (`moldql/compile.rs:581-592`). La ejecución PostgreSQL devuelve
`FeatureDisabled` (`moldql/compile.rs:560-570`). La composición booleana devuelve
`NotImplemented` (`moldql/compile.rs:594-605`).

`MoldQLServiceImpl` construye el contexto con `graph_query: None`
(`facades/moldql.rs:93-113`), aunque el runtime sí crea un `GraphQueryPort` para
otras fachadas. En consecuencia, `EXPLORE`, `fan_in` y `fan_out` no tienen
navegación productiva fiable.

El supuesto límite de 100 resultados está declarado pero no aplicado
(`facades/moldql.rs:19-20`).

### Defecto de reproducibilidad del plan

El lowerer asigna `PlanHash::compute(&0u32)` a todos los planes
(`moldql/lower_plan.rs:40-44`). Distintas consultas comparten hash.

La documentación de `PlanHash` afirma que `serde_json::to_vec` ordena claves
(`domain/plan/version.rs:127-143`), una garantía que no está implementada por ese
código de manera general.

### Estado del objetivo

**No cumplido para consultas gráficas.** El lenguaje tiene una buena dirección,
pero el camino normalizado todavía no gobierna la ejecución real.

## Pilar 5 — Algoritmos

### Implementados

El crate puro incluye, entre otros:

- PageRank;
- caminos simples acotados;
- componentes y condensación;
- comunidades;
- god nodes y conexiones sorprendentes;
- reducción transitiva;
- feedback arc set.

### Faltantes para una plataforma de analítica

ADR-014 requiere un contrato de admisión con versión, madurez, determinismo,
proyección, parámetros, esquema de salida, modos, complejidad, límites y fixtures
(`ADR-014:116-140`). No existe `AlgorithmRegistry`, `AlgorithmDescriptor` ni
`RunRecord` en el código productivo.

También faltan:

- WCC como capacidad productiva consistente;
- dominadores;
- articulation points;
- bridges;
- k-core;
- lineage persistido de cada ejecución;
- modos `stream`, `stats`, `annotate` y `persist`;
- composición de resultados analíticos en overlays de Explorer.

### Estado del objetivo

**Parcial.** Hay buenos algoritmos, no una plataforma reproducible y explotable.

## Pilar 6 — Representaciones visuales y semánticas

### Matriz de diagramas

| Representación | Estado real | Fidelidad |
|---|---|---|
| Call graph | Implementado | Buena para topología estática de llamadas |
| Dependency graph | Parcial | Mezcla o agrega dependencias; render E2E inconsistente |
| Impact radius | Parcial | BFS útil, pero algunas vistas muestran hotspots o aristas inferidas |
| C4 Context | Heurístico | Un system derivado del directorio; sin personas ni sistemas externos |
| C4 Container | Heurístico | Derivado de Cargo members y `apps/*` |
| C4 Component | Heurístico | Derivado de directorios de módulos |
| C4 Code | Heurístico | Símbolos renombrados a `code`, cap silencioso de 200 |
| Vertical slice | Prototipo | Profundidad de calls se etiqueta como capa arquitectónica |
| Casos de uso | Prototipo | Clasificación por nombre; sin modelo de dominio propio |
| Jerarquía de tipos | Catálogo | Datos parciales, sin proyección ejecutable |
| UML de clases | Ausente | No preserva miembros, roles ni relaciones específicas |
| UML de llamadas | Parcial | Flowchart de calls, no notación UML completa |
| UML de secuencia | Ausente | Sin participantes, mensajes ni orden temporal |
| Data flow | Etiqueta incorrecta | No existe grafo de reads/writes/productores/consumidores |
| State machine | Ausente | Sin estados, eventos ni transiciones |
| Mermaid | Exportación | Fuente visible; no render live en Explorer |
| Draw.io | Asistencia manual | Copia Mermaid y abre diagrams.net |
| SVG/PNG | Parcial | Depende de rutas y binarios no probados E2E |

### C4 no es todavía un modelo C4 canónico

`build_architecture_impl` infiere:

- System desde el nombre del directorio;
- Containers desde `Cargo.toml` y `apps/*`;
- Components desde directorios de módulos;
- Code desde símbolos, con cap 200
  (`facades/graph.rs:326-619`).

La respuesta marca `truncated: false` aunque el cap se alcance
(`facades/graph.rs:567-618`). Container y Component usan el mismo filtro visual
(`apps/explorer-ui/src/state/c4Levels.ts:26-30`).

Esto sirve como **mapa estructural aproximado**, pero no como C4 completo. Faltan
responsabilidades, tecnologías, personas, sistemas externos, relaciones entre
niveles, procedencia de la inferencia y baseline persistida.

### El export C4 está roto por contrato

La UI envía niveles `c4-context`, `c4-container` y `c4-component`
(`C4Toolbar.tsx:15-30`; `api/client.ts:668-677`). El backend acepta únicamente
`context`, `container` y `component` (`c4_mermaid.rs:34-42`;
`api.rs:1062-1075`).

### Vertical slices e impacto inventan aristas

`CallEntry` conserva profundidad pero no padre ni arista. El export de impacto
conecta cada nodo de una profundidad con todos los de la siguiente
(`trace_mermaid.rs:229-272`). El vertical slice conecta con el primer nodo del
nivel anterior y etiqueta las profundidades como use case, domain, repository y
DB (`trace_mermaid.rs:319-395`).

El dibujo puede mostrar una relación que no existió en el grafo.

### UML de clases y jerarquía de tipos

Los walkers reconocen `extends`, `implements` y supertipos
(`type_ref_walkers.rs:114-129`, `:168-184`), pero el extractor emite todas esas
relaciones como `DependencyType::References`
(`ingest/extractor.rs:357-380`). Se pierde la distinción necesaria para herencia,
realización, composición y asociación.

`TypeHierarchy` existe en el catálogo (`dto.rs:1381-1428`), no como flujo
ejecutable completo.

### Secuencia UML

No existe un modelo temporal con:

- participantes/lifelines;
- mensajes ordenados;
- llamadas síncronas o asíncronas;
- retornos;
- activaciones;
- concurrencia;
- evidencia de runtime.

`MermaidRenderer` sólo reconoce texto que empieza por `sequenceDiagram` y lo
muestra en un `<pre><code>`; no genera ni ejecuta una secuencia
(`MermaidRenderer.tsx:1-9`, `:87-165`).

### Estado del objetivo

**Sólo el call graph está cerca de ser una vista semántica utilizable.** El resto
debe presentarse honestamente como prototipo, proyección o exportación.

## Pilar 7 — ViewSpec y runtime moldable

### Lo que existe

- DTO y validación de `ViewSpec`;
- store PostgreSQL;
- wizard en Explorer;
- catálogo de `RendererKind`;
- renderer registry extensible;
- búsqueda y descriptores de vistas incorporadas.

### Lo que falta

El composition root pasa `None` como `view_spec_store` a persistencia, registro y
búsqueda (`cognicode-runtime/src/lib.rs:148-159`, `:206-212`).

`execute_view_spec` devuelve explícitamente `FeatureDisabled`
(`facades/view.rs:452-459`). Las rutas REST CRUD que consume la UI no aparecen en
el router productivo (`api.rs:659-755`).

`PaneInspector` decide el render mediante una lista hard-coded de ViewKinds y
después usa `GraphView` o `Blocks` (`PaneInspector.tsx:32-43`, `:388-407`). No
delega en `renderer_kind` ni en `RendererRegistry`.

Vega-Lite continúa como placeholder (`rendererRegistry.tsx:203-224`) y un
renderer desconocido cae silenciosamente a JSON (`rendererRegistry.tsx:121-142`).

### Estado del objetivo

**Diseñado, no operable de extremo a extremo.** El runtime moldable es todavía
una promesa arquitectónica.

## Pilar 8 — Explorer: utilidad, UX y accesibilidad

### Evaluación técnica de interfaz

| Dimensión | Nota | Evidencia principal |
|---|---:|---|
| Accesibilidad | 2/4 | Buen foco/teclado; contraste deshabilitado en una suite |
| Rendimiento | 1/4 | Sin runtime estable ni pruebas de grafos grandes |
| Theming | 3/4 | Tokens consistentes y estrategia restringida |
| Responsive | 1/4 | 320 px marcado `fixme` |
| Anti-patrones visuales | 3/4 | Interfaz de producto sobria; identidad poco diferenciada |
| Total | 10/20 | Dirección aceptable, release bloqueado |

### Fortalezas de UX

- Spotter como entrada universal.
- Pila de panes que conserva la narrativa.
- tabs y shortcuts con intención de navegación por teclado.
- `LoadingTier` para loading/error/validating.
- empty states y affordances en varias superficies.
- tokens densos apropiados para una herramienta técnica.
- `prefers-reduced-motion` global.

### Brechas verificadas

#### Build y runtime

La aplicación no compila; por lo tanto no hay evidencia visual confiable del
estado actual.

#### Responsive

El test de 320 × 568 está marcado como deuda conocida y omite la aserción de
overflow (`e2e/responsive-full.spec.ts:18-45`).

#### Accesibilidad

La suite deshabilita `color-contrast` para el inspector y documenta una relación
4.17:1 (`e2e/a11y.spec.ts:75-82`). Para texto normal, eso queda debajo de WCAG
AA.

#### Targets táctiles

Los controles C4 usan padding `6px 12px` y fuente 12 px
(`C4Toolbar.tsx:81-164`). Es probable que queden debajo de 44 × 44; debe medirse
cuando el runtime vuelva a ser estable.

#### Motion y rendimiento de layout

El detector de Impeccable encontró una transición de `width` en
`apps/explorer-ui/src/components/ScanBar.tsx:83`. Animar ancho fuerza layout y
puede degradar la fluidez durante el progreso de ingestión. Se registra como
warning; no se corrigió porque esta evaluación es de solo lectura.

#### Descubribilidad

La amplitud del backend supera lo que la landing explica. C4, drift,
investigaciones, ViewSpec y analítica requieren conocimiento previo del modelo
de perspectivas o aparecen sólo después de entrar en un objeto.

### Slop test

#### Primer orden

**Pasa con reservas.** La interfaz no cae en cards decorativas, gradients o
marketing SaaS. Se reconoce como herramienta técnica sobria.

#### Segundo orden

**Pasa parcialmente.** La paleta y varios patrones se acercan a “GitHub dark
developer tool”. La identidad de CogniCode debe emerger de semántica visual
estable, navegación narrativa y vocabulario, no de decoración.

## Pilar 9 — Rendimiento, observabilidad y operación

### Lo que existe

- `perf-budget.toml` para operaciones pequeñas;
- Criterion y scripts de budget;
- OpenTelemetry/Prometheus para herramientas MCP;
- pool PostgreSQL;
- bounded channels en partes de la extracción;
- jobs de scan con polling;
- Dockerfile y entrypoint;
- límites parciales en API y planes.

### Brechas

#### Los budgets no validan el objetivo del producto

Los presupuestos cubren operaciones como BFS de 100 nodos y subgrafos de 50
nodos (`perf-budget.toml:6-29`). No miden:

- ingestión de 100 MB, 1 GB o 100 000 archivos;
- tiempo incremental con 1 %, 10 % y 50 % de cambio;
- historial con cientos de revisiones;
- consultas C4/MoldQL sobre millones de aristas;
- render interactivo con 1 000–5 000 nodos;
- memoria y latencia bajo concurrencia.

El workflow CI no ejecuta el perf budget ni el frontend
(`.github/workflows/ci.yml:71-156`).

#### Jobs incompletos

`start_scan` llama `run_scan(..., None)`, por lo que desactiva el callback de
progreso, y siempre marca el job `Completed`
(`ingest/controller.rs:211-248`). `JobState::Failed` existe pero no se utiliza en
esa ruta.

No hay cancelación HTTP ni estado durable de job.

#### La apertura de workspace no registra el resolver de ingestión

`open_workspace` reconoce explícitamente el gap y no registra la ruta
(`api.rs:774-788`). Un scan iniciado por el workspace abierto puede fallar como
“workspace not registered”.

#### API sin gobernanza global

El router aplica CORS permisivo y tracing, pero no body limit, timeout global,
concurrency limit ni graceful shutdown (`api.rs:650-760`).

### Estado del objetivo

**No demostrado.** Antes de optimizar, deben cerrarse los defectos de corrección.
Después se necesita una suite de escala que represente repositorios reales.

## Pilar 10 — Documentación y verdad del producto

La documentación pública está desalineada con el estado actual:

- `README.md` anuncia `RedbGraphStore`, pero la arquitectura canónica actual es
  PostgreSQL.
- anuncia 6 lenguajes, mientras el registro contiene alrededor de 30.
- anuncia 32+ herramientas, mientras existen superficies MCP mucho más amplias.
- la tabla de crates omite `cognicode-runtime`, `cognicode-explorer`,
  `cognicode-graph-algos`, `cognicode-graph-wasm` y otros.
- `Cargo.toml` mantiene versión de workspace `0.5.0`, mientras el roadmap usa
  tags `v0.61.0`–`v0.70.0`.

Más importante: `ROADMAP.md` usa “DONE” para slices internas aunque no estén
integradas en producto. Eso puede ser correcto para una cadena de PR, pero no
debe interpretarse como capacidad utilizable.

Se recomienda adoptar cuatro estados explícitos:

| Estado | Definición |
|---|---|
| Foundation shipped | Tipos/puertos/adaptadores presentes |
| Integrated | Compuesto en runtime productivo |
| User-visible | Tiene entrada, resultado y estados UX |
| Production-proven | Pasa escala, errores, truncación y operación |

## Análisis de profundidad y entropía

### Método

**Método:** heurístico basado en lectura de código.  
**Confianza:** estimada, media.  
No se utilizaron métricas runtime de CogniCode porque la cadena de ingestión y
revisiones auditada no es todavía una fuente cuantitativa confiable.

### Paisaje de connascencia

| Par de módulos | Tipo | I estimada | Estado | Motivo |
|---|---|---:|---|---|
| Ingest ↔ schema PG ↔ manifest | Meaning/Algorithm | 4.0 bits | Crítico | Orden y claves deben coincidir en varios sitios |
| Ingest ↔ cache ↔ runtime readers | Identity/Timing | 3.6 bits | Alto | Múltiples instancias representan “el grafo actual” |
| MoldQL AST ↔ PetgraphPlan ↔ GraphPlan | Type/Algorithm | 3.2 bits | Alto | Tres algebras para la misma intención |
| ViewKind ↔ PaneInspector ↔ RendererRegistry | Name/Meaning | 2.6 bits | Medio-alto | El renderer real depende de listas duplicadas |
| C4 facade ↔ export ↔ filtros UI | Meaning | 3.0 bits | Alto | Cada capa interpreta los niveles por separado |

Umbral aplicado: más de 3 bits requiere refactor antes de ampliar la
funcionalidad.

### Design Quality Score estimado

**DQS estimado:** 0.38/1.0 — aceptable como fundación, insuficiente como
plataforma integrada.

| Componente | Evaluación |
|---|---|
| Acoplamiento | Alto en ingestión, runtime y visualización semántica |
| Cohesión | Buena en planes, algoritmos puros y ViewRegistry |
| LSP/contratos | Riesgo alto por empty success, features deshabilitadas y drift |
| Connascencia | Alta entre schema, adapters, facades y UI |

La puntuación no debe usarse como métrica contractual. Sirve para ordenar el
trabajo: profundizar seams antes de añadir superficie.

## Módulos que deben profundizarse

### 1. Módulo de grafo canónico revisionado

- **Interface:** `commit(workspace, changes) -> RevisionId` y
  `snapshot(workspace, revision) -> GraphSnapshot`.
- **Implementation:** facts inmutables o temporales en PG, head atómico,
  retención y eliminación.
- **Seam:** entre escritores de ingestión y lectores.
- **Leverage:** reproducibilidad, diff, concurrencia y cache seguro.
- **Locality:** todas las invariantes temporales viven en un único módulo.

### 2. Módulo transaccional de ingestión incremental

- **Interface:** `ingest(workspace, change_stream, progress) -> IngestOutcome`.
- **Implementation:** canal acotado real, deletes explícitos, resolución, commit,
  publicación y analítica posterior.
- **Seam:** entran cambios de filesystem; sale una revisión comprometida.
- **Leverage:** análisis incremental eficiente y testeable.
- **Locality:** orden, rollback, progreso y cancelación dejan de repartirse.

### 3. Módulo de ejecución MoldQL/analytics fijado a revisión

- **Interface:** `execute(MoldPlan, GraphPin, Policy) -> ResultSet`.
- **Implementation:** lowering único, executor validation, límites,
  cancelación, equivalencia y lineage.
- **Seam:** entra intención; sale un resultado tipado y reproducible.
- **Leverage:** REST, MCP, ViewSpec y Explorer comparten semántica.

### 4. Módulo de proyecciones semánticas

- **Interface:**
  `project(ViewKind, GraphPin, Focus, Policy) -> SemanticProjection`.
- **Implementation:** proyecciones C4, jerarquía, calls, use cases, data flow y
  flow traces sobre facts canónicos.
- **Seam:** entra grafo; sale significado renderer-neutral.
- **Leverage:** Cytoscape, Mermaid, tablas y MCP dejan de inventar relaciones.

### 5. Módulo de runtime de vistas moldables

- **Interface:** descubrir, cargar, ejecutar y renderizar `ViewSpec`.
- **Implementation:** store PG, MoldQL, JSONata acotado y dispatch por
  `RendererKind`.
- **Seam:** entra especificación declarativa; sale contenido de pane.
- **Leverage:** nuevas vistas mediante datos, no cirugía en backend y React.

## Hoja de ruta recomendada

### Fase 0 — Recuperar una línea base verificable

**Prioridad:** inmediata.  
**Objetivo:** impedir que CI y roadmap reporten verde sobre rutas no ejecutables.

1. Corregir el orden/forma de las migraciones `m0018`/`m0019`.
2. Alinear los conflict targets de PgUpsert con workspace identity.
3. Separar los triggers de nodes y edges.
4. Corregir el deadlock de más de diez resultados.
5. Hacer que Explorer compile.
6. Añadir smoke E2E: abrir workspace → scan → job → graph stats → landing.
7. Eliminar el skip silencioso de tests PG por fallo de migración.

**Criterio de salida:** una base vacía migra; un repo con más de diez archivos se
ingiere; Explorer compila y muestra el grafo resultante.

### Fase 1 — Hacer verdadera la historia del grafo

**Objetivo:** que una revisión sea un estado, no un contador.

1. Elegir snapshot por revisión o temporal validity.
2. Hacer atómico `ingest → revision → head`.
3. Persistir deletes y conservar el último estado válido ante error de parsing.
4. Asociar reports y sessions a `RevisionId`.
5. Definir retención y garbage collection.
6. Construir diff estructural tipado.
7. Usar una sola instancia de `SnapshotProvider` en runtime.

**Criterio de salida:** tras N y N+1, ambas revisiones devuelven grafos distintos
y correctos; el diff explica exactamente qué cambió.

### Fase 2 — Poner `MoldPlan` y `GraphExecutor` a cargo

**Objetivo:** eliminar el compilador/executor legacy como ruta productiva.

1. Ejecutar `compile_to_plan()` desde MoldQL productivo.
2. Inyectar `GraphExecutor` y `GraphQueryPort` desde composition root.
3. Rechazar unsupported antes de ejecutar; prohibir empty success sintético.
4. Corregir hash canónico y hash por contenido.
5. Aplicar todos los límites durante el recorrido.
6. Crear conformance diferencial PG ↔ snapshot.
7. Publicar matriz de sintaxis realmente soportada.

**Criterio de salida:** PATH, NEIGHBORS, SUBGRAPH, CLUSTER, EXPLAIN y composición
producen resultados equivalentes por backend para la misma revisión.

### Fase 3 — Admitir analítica y semántica visual

**Objetivo:** construir significado antes de añadir notación.

1. Implementar `AlgorithmRegistry` y `AlgorithmDescriptor`.
2. Registrar lineage, parámetros, seed, límites y truncación.
3. Admitir primero PageRank, SCC, WCC y shortest paths.
4. Introducir `GraphTopology` y `FlowTrace` equivalentes.
5. Preservar edge kind, parent edge y orden en recorridos.
6. Enriquecer extracción de type ownership, implements/inherits, members,
   routes, actors y runtime traces.
7. Reconstruir C4, class/type, calls, dependency y use-case views sobre esas
   proyecciones.

**Criterio de salida:** cada vista declara qué facts requiere, qué confianza
tiene, cuándo trunca y qué no soporta.

### Fase 4 — Completar Explorer como workbench moldable

**Objetivo:** hacer visible y reutilizable todo lo anterior.

1. Wirear `PostgresViewSpecStore` en persistence, registry, search y execution.
2. Añadir CRUD/execute REST para ViewSpecs.
3. Hacer que `renderer_kind` gobierne todos los panes.
4. Completar graph, table, tree, code, markdown, Vega-Lite, Mermaid y composite.
5. Reparar responsive 320/768/1280 y contraste AA.
6. Diseñar entradas por intención: “Trace a request”, “Explore architecture”,
   “Compare revisions”, “Find drift”, “Inspect type hierarchy”.
7. Añadir pruebas E2E de happy, empty, error, truncation y revision change.

**Criterio de salida:** un usuario puede ingerir, fijar una revisión, consultar,
aplicar un algoritmo, abrir una vista, guardarla y reproducirla más tarde.

## Priorización por impacto

| Orden | Trabajo | Impacto | Esfuerzo | Riesgo |
|---:|---|---|---|---|
| 1 | Recuperar migraciones PG limpias | Crítico | M | Alto |
| 2 | Corregir deadlock de ingestión | Crítico | S | Bajo |
| 3 | Recuperar build de Explorer | Crítico | M | Medio |
| 4 | Corregir deletes/manifest/errores | Crítico | M | Alto |
| 5 | Implementar revisiones inmutables | Crítico | L | Alto |
| 6 | Unificar cache y lectores del runtime | Alto | L | Alto |
| 7 | Ejecutar MoldPlan/GraphExecutor | Alto | L | Alto |
| 8 | Conformance PG ↔ snapshot | Alto | M | Bajo |
| 9 | Completar jobs, cancelación y progreso | Alto | M | Medio |
| 10 | Crear proyecciones semánticas | Alto | L | Alto |
| 11 | Completar ViewSpec/RendererRegistry | Alto | L | Medio |
| 12 | Suite de escala y observabilidad | Alto | M/L | Bajo |

## Qué no conviene hacer ahora

1. **No añadir Neo4j como segunda fuente de verdad.** Multiplicaría el problema
   de sincronización que todavía no está resuelto dentro de PostgreSQL.
2. **No ampliar la gramática MoldQL** hasta que las primitivas actuales ejecuten
   y tengan conformance.
3. **No añadir más renderers UML** sin modelos semánticos que preserven relaciones
   y orden.
4. **No presentar C4 actual como arquitectura canónica.** Debe etiquetarse como
   inferencia básica con confianza y procedencia.
5. **No marcar una capacidad como DONE sólo porque el tipo o adaptador exista.**
   Exigir integración y visibilidad.
6. **No optimizar micro-operaciones** antes de corregir deadlocks, transacciones y
   verdad temporal.
7. **No trasladar lógica canónica al navegador.** El frontend debe renderizar
   proyecciones; no redefinir semántica de grafo.

## Criterios finales de aceptación del objetivo

CogniCode alcanzará el objetivo cuando pueda demostrar, con fixtures y una base
realista, el siguiente recorrido:

1. Ingerir un repositorio de al menos 100 MB sin deadlocks y con progreso.
2. Reingerir 1 % de cambios con coste proporcional al delta.
3. Consultar las revisiones N y N+1 y obtener resultados distintos y exactos.
4. Ejecutar la misma consulta MoldQL en PG y snapshot con resultado equivalente.
5. Aplicar un algoritmo con versión, parámetros, límites, lineage y truncación.
6. Abrir el resultado en Explorer desde una intención comprensible.
7. Navegar nodos preservando la historia de panes.
8. Visualizar call graph, C4 y type hierarchy con semántica verificable.
9. Generar una secuencia sólo cuando exista evidencia ordenada de ejecución.
10. Guardar una ViewSpec y reproducirla en la misma revisión.
11. Pasar build, E2E, WCAG AA, 320/768/1280 y budgets de grafos grandes.
12. Recuperarse de fallo, cancelación, restart y PostgreSQL temporalmente caído.

## Conclusión

CogniCode tiene una visión fuerte y varias piezas que justifican continuar:
modelo de grafo rico, PostgreSQL, planes tipados, algoritmos puros, vistas
moldables, pane stack e investigaciones. No hace falta reemplazar la base ni
reinventar el producto.

Pero el sistema todavía no puede sostener la promesa principal con evidencia:

```text
código → ingestión incremental → revisión inmutable → consulta reproducible
      → proyección semántica → visualización interactiva → investigación durable
```

La recomendación es frenar expansión de superficie durante dos ciclos y cerrar
primero **verdad del dato**, **ejecución normalizada** e **integración del
runtime**. Después de eso, C4, UML, casos de uso y analítica dejarán de ser capas
de presentación frágiles y pasarán a convertirse en capacidades reales del
producto.

## Metadatos de verificación de interfaz

```yaml
register: product
color_strategy: Restrained
type_scale: fixed-rem
motions_used: []
anti_patterns_checked:
  - gradient text
  - glassmorphism
  - hero metrics
  - identical card grids
  - excessive rounding
  - bounce or elastic motion
detector_result: findings: 1
slop_test:
  first_order: pass-with-reservations
  second_order: pass-with-reservations
risks:
  - Explorer does not currently compile
  - PRODUCT.md and DESIGN.md are absent
  - Runtime browser verification is incomplete
next_recommended: establish Phase 0 verification baseline
```

El detector de Impeccable se ejecutó sobre `apps/explorer-ui/src` y encontró un
warning de animación de layout en `ScanBar.tsx:83`. La ausencia de
`PRODUCT.md`/`DESIGN.md` y el build inestable impiden completar una aprobación
visual de producción, pero no alteran los defectos técnicos confirmados.
