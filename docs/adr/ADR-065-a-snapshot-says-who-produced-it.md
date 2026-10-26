# ADR-065 — El snapshot dice quién lo produjo

**Estado**: EJECUTADO (decidido el 2026-10-25; ejecutado el 2026-10-25)
**Fecha**: 2026-10-25
**Decisores**: Maintainer
**Contexto**: H5 · la identidad de la caché incremental (`docs/roadmap/consumer-contracts/specs/SPEC-CU-005-SNAPSHOT-INCREMENTAL.md`) y el snapshot persistido del grafo (`crates/cognicode-core/src/interface/mcp/handlers/mod.rs`)

---

## Contexto

### Lo que faltaba

El producto persiste un snapshot del grafo y lo vuelve a servir cuando las
fuentes no han cambiado. Medido, en el árbol de `1b8f03df`, el snapshot
guardaba **dos** cosas:

- el grafo (`CallGraph`),
- el manifiesto de contenido (`FileManifest`): qué ficheros se leyeron y con
  qué hash.

Lo que **no** guardaba era quién lo había calculado. Ni la versión del
analizador, ni su commit, ni sus capacidades, ni el esquema con el que se
escribió.

La consecuencia no es teórica. La decisión de servir un snapshot se tomaba
comparando el manifiesto con el disco —`is_manifest_stale`, que compara la
marca de tiempo y luego el hash de cada fichero—, y **nada más**. Un binario
distinto —otra versión, otro commit, otro conjunto de capacidades— servía el
snapshot igual que si fuera suyo. El grafo que el consumidor recibía lo había
calculado otro analizador, con otra ontología y otras reglas, y nada en la
respuesta lo decía.

Lo que sí se publicaba era una `basis` —el workspace, el digest de la
configuración y el digest del manifiesto— con `complete: true`. Es decir: la
identidad del análisis se declaraba completa mientras le faltaba
justamente la parte que decide si el análisis sigue valiendo.

### Por qué era invisible a la suite entera

La suite probaba que el snapshot se guardaba, que se cargaba, que un snapshot
corrupto se rechazaba, y que un snapshot con las fuentes cambiadas se
rechazaba. No probaba **quién** había escrito el snapshot, porque el snapshot
no lo decía. La comprobación que faltaba era la que hace esta unidad: la
identidad del analizador tiene que estar en el artefacto, no en el
razonamiento de quien lo lee.

### Lo que no se rompía

El contenido seguía mandando: un fichero modificado invalidaba el snapshot,
un fichero nuevo lo invalidaba, uno borrado lo invalidaba, y un hash distinto
con la misma marca de tiempo también. El defecto era de **identidad**, no de
estabilidad: el producto servía un grafo viejo, pero solo cuando las fuentes
no habían cambiado, y eso lo hacía parecer correcto.

---

## Decisión

### La clave: tres componentes, y solo tres

La identidad de un análisis es la pregunta «¿esto sigue valiendo?» hecha
comparable. La clave vive en el dominio, en
`crates/cognicode-core/src/domain/value_objects/snapshot_key.rs`, y declara:

| Componente | Qué dice |
|---|---|
| `workspace` | Qué se miró: la raíz canónica del árbol analizado |
| `analyzer` |Quién lo miró: versión, commit y capacidades del analizador |
| `schema` | Con qué forma se escribió: la versión del snapshot |

`AnalyzerBuild` es la identidad del **proveedor**: el crate que calcula el
grafo. No es la del consumidor que pregunta, que no dice nada sobre cómo se
calculó la respuesta. La versión sale de `CARGO_PKG_VERSION` de ese crate, el
commit de un `build.rs` que hornea el `HEAD` del checkout —el mismo
mecanismo que ya usa el CLI para `cognicode identity`—, y las capacidades de
la lista de features con las que se compiló.

El constructor es **puro**: mismos datos, misma clave. No lee el disco, no
pregunta al reloj, no llama a la red. Quien trae los datos es quien construye
la clave.

### El snapshot guarda la clave, y se decide con ella

El formato del snapshot pasa de `cognicode.graph.cache/v3` a `v4`: la forma
del fichero cambia, porque ahora lleva la clave dentro. Un snapshot de la
versión anterior no se lee con la forma nueva, y su versión lo dice, de modo
que se reconstruye en vez de aceptarse como evidencia.

Al cargar, la **primera** pregunta es sobre el productor, y no cuesta nada:
se responde sin leer el árbol. Si el snapshot lo produjo otro analizador, no
se sirve —el producto reconstruye y publica su propio análisis—. Solo si el
producto coincide se pasa a la comparación de contenido, que es la que ya
existe.

El motivo del rechazo se escribe en el log de observabilidad, no se esconde:
un snapshot que no se sirve y no lo dice es un fallo silencioso.

### La base publicada declara el analizador

La `basis` que el producto publica gana cuatro campos: `analyzer_version`,
`analyzer_commit`, `analyzer_features` y `snapshot_schema`. Vienen de la
**misma clave** con la que se escribe el snapshot, así que la base no puede
describir un analizador que el snapshot no nombre: hay una sola fuente de
verdad.

El `commit` es `null` cuando el binario se construyó fuera de un checkout. Es
un hecho sobre el binario, no un hueco que se rellene con un valor inventado.

El catálogo publicado (`product/tools.json`) declara los mismos cuatro
campos, y un contrato de CI lo comprueba: un documento que describe una forma
que el producto ya no tiene es una promesa que el producto no cumple.

### Lo que la clave NO lleva, y por qué

La especificación de la unidad enumera más componentes. Los que no entran en
esta clave, con su motivo:

- **El manifiesto de contenido.** El snapshot ya lo guarda y el producto ya
  lo compara fichero a fichero. Meter aquí el mismo digest sería una segunda
  fuente de verdad sobre los mismos bytes, y las dos podrían separarse: la
  clave diría una cosa y el manifiesto otra, sin que nada lo notara.
- **La revisión del workspace analizado y su digest de suciedad.** El
  análisis es una función de los bytes y del analizador. Con los bytes
  fijados, la revisión es redundante, y pedirla sería invalidar la caché en
  cada commit del proyecto analizado **sin cambiar ninguna respuesta**. El
  coste es real y el beneficio, ninguno.
- **El perfil de configuración.** La parte de la configuración que cambia lo
  que el analizador puede ver son las capacidades, y ya están en la clave. El
  resto de la configuración —el nivel de log— no cambia el grafo, y
  incluirlo invalidaría la caché cuando alguien cambia una variable de
  entorno.
- **El ámbito.** El snapshot cubre el árbol entero del workspace, y eso ya lo
  fija el par `workspace` + manifiesto.

### Las capacidades se declaran, y el contrato lo vigila

El lenguaje no deja preguntar en tiempo de ejecución qué features se
activaron, así que la lista es explícita. Eso es un sitio que se puede
quedar viejo, así que un contrato de CI la compara con lo que declara
`Cargo.toml`: si se **añade** una feature y no se nombra en la identidad, el
contrato se pone rojo —un analizador que no declara su capacidad no se puede
identificar—; si se **borra** una feature de `Cargo.toml`, el `cfg!` deja de
existir y el compilador lo dice.

---

## Alternativas consideradas

### (a) Subir la versión del snapshot y no hacer nada más

Descartada. La versión del formato solo cambia cuando cambia la forma. Un
analizador nuevo que escriba el mismo formato seguiría sirviendo el snapshot
del anterior: el defecto sobrevive a cada release en cuanto el formato no
cambia, que es casi siempre.

### (b) Poner el digest del manifiesto dentro de la clave y comparar la clave entera

Descartada por el motivo ya escrito: el manifiesto está en el snapshot y se
compara por fichero. Meter su digest en la clave crea una segunda verdad
sobre los mismos bytes, y la comparación por fichero es más barata cuando
algo ha cambiado —solo lee el fichero cuyo hash necesita—. Además, el digest
del manifiesto no distingue un snapshot de otro analizador, que es el defecto
que se quiere cerrar.

### (c) Reemplazar la comparación por manifiesto por la comparación de la clave

Descartada por el mismo motivo, en el otro orden: la comparación por fichero
existe, funciona, y su semántica está probada —un fichero ilegible invalida,
uno borrado invalida, uno nuevo invalida—. Reemplazarla por una comparación
de digest cambia el comportamiento —un `touch` sin cambio de contenido dejaría
de invalidar— sin que la unidad lo necesite. La clave se **añade** a la
decisión, no la sustituye.

### (d) Incluir la revisión del workspace en la clave

Descartada por coste sin beneficio, y está razonada más arriba. Es la opción
que la especificación enumera, y el motivo para no tomarla es que el
contenido ya fija lo que la revisión fijaría.

### (e) Dejar la identidad en el interior, sin publicarla

Descartada. El arreglo interior evita el defecto, pero deja al consumidor sin
forma de saber quién calculó la respuesta que tiene delante. La `basis` ya
existe y ya dice `complete: true`: dejarlo así, sin el analizador, es decir
que la identidad del análisis está completa cuando no lo está.

### (f) Publicar la identidad solo en el log

Descartada por la misma razón que (e), y peor: el log no es un contrato. El
consumidor del protocolo no lee el log del servidor.

---

## Consecuencias

### Positivas

- Un snapshot producido por otro analizador no se sirve. El consumidor recibe
  un análisis queCalculó el binario que está en ejecución.
- La `basis` declara la identidad completa, y lo que dice sale de la misma
  clave con la que se escribe el snapshot.
- La clave es un valor del dominio, con constructor puro y sin dependencia del
  mundo: comparable, serializable y verificable.
- La lista de capacidades no se puede quedar vieja sin que el contrato lo diga.
- El esquema del snapshot queda en un único sitio, y el formato en disco lo
  nombra.

### Negativas

- **Cambio de contrato publicado.** La `basis` de `build_graph` gana cuatro
  campos. Son aditivos: un consumidor que lee campos conocidos sigue
  funcionando, y el catálogo publicado declara los cuatro. Un consumidor que
  valida el esquema con `additionalProperties: false` tiene que admitir los
  nuevos.
- **Cambio de formato en disco.** El snapshot pasa a `v4` y lleva la clave.
  Los snapshots anteriores se reconstruyen, no se sirven: es el comportamiento
  correcto, y es el mismo coste que cualquier cambio de forma.
- **El commit puede ser `null`.** Un binario construido fuera de un checkout
  no puede saber de qué commit salió. Se declara así, en vez de declarar una
  versión de commit inventada.
- **El commit identifica el checkout, no el árbol.** Un binario construido
  desde un árbol con cambios sin commitear lleva el commit base, que es
  exactamente lo que ya hace `cognicode identity` en el CLI. La identidad es
  exacta para un binario de release —árbol limpio, commit etiquetado—, que es
  lo que recibe el consumidor; para una construcción de desarrollo, dos
  binarios construidos desde el mismo commit base se identifican igual.
  Ninguna identidad horneada en tiempo de construcción puede distinguir dos
  construcciones del mismo árbol sucio: la única forma de hacerlo sería medir
  el árbol en tiempo de ejecución, y un binario distribuido no lleva su
  fuente. La limitación se declara en vez de disimularse.
- La incrementalidad en sí —que un cambio reconstruya solo lo que tiene que
  reconstruir— sigue sin ser la que pide la especificación de la unidad. Eso
  es otra unidad, medida y encolada aparte.

### Mitigaciones

- La prueba de frontera (`crates/cognicode-core/tests/a_snapshot_says_who_produced_it.rs`)
  mide las cuatro caras: la base declara el analizador, el snapshot persistido
  lo declara, un snapshot marcado como de otro analizador no se sirve, y un
  análisis en frío y el servido declaran la misma identidad.
- El contrato de CI (`scripts/ci/test_a_snapshot_says_who_produced_it.py`)
  vigila la existencia y la pureza de la clave, la identidad real del
  analizador, la correspondencia entre las capacidades declaradas y las que
  `Cargo.toml` declara, que el snapshot se guarde y se decida con la clave, y
  que el catálogo publicado declare lo mismo que el runtime.

---

## Referencias

- Contrato de la unidad: `SPEC-CU-005-SNAPSHOT-INCREMENTAL.md` (CU-005.1:
  identidad del análisis; CU-005.3: el acierto exige clave válida, esquema e
  integridad).
- ADR-064 — Una fecha se publica solo si se midió: el mismo criterio aplicado
  a la base temporal, con la misma consecuencia —la ausencia de una
  medición se declara, no se disfraza de un número—.
- ADR-056 — Las rutas dicen lo mismo: dos caminos que contestan distinto
  sobre el mismo árbol, y por qué el producto tiene que decir una sola cosa.
- Unit Candidate C11 en `goal-compass.json`.
