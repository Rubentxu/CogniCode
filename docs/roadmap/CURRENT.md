# CURRENT — Snapshot operativo (deriva de `docs/roadmap/ROADMAP.md`)

> **Autoridad**: `docs/roadmap/ROADMAP.md` es la **única autoridad de agenda**
> de CogniCode. Este fichero es un **snapshot puntual**, no una segunda fuente
> de estado: donde este documento y el ROADMAP diverjan, manda el ROADMAP y
> este snapshot se considera obsoleto hasta su próxima regeneración.
>
> **Snapshot**: 2026-10-24, sobre `2ac87c02c2117003062145ce9ef5aa97697fc342` (el
> `main` ya fusionado que cierra el tramo: la lectura declara la identidad
> de lo que leyó —y esa identidad es del código y no del sitio donde está
> el código—, el grafo tiene un escenario que pregunta qué se movió y qué
> se quedó, y los checks del módulo compartido se ejecutan una vez y no
> veintiún).
> Versión del workspace: `0.101.31`. Última release publicada: `v0.101.30`
> (2026-10-23, 8 assets).
>
> **La historia vive en el Journal**; aquí sólo vive el estado.

## Release: la última publicada es la v0.101.30

| Tag | Estado | Por qué |
|---|---|---|
| `v0.101.23` | publicada (2026-10-18) | la anterior |
| `v0.101.24` | tag, sin release | su gate de candidato encontró un defecto real del índice de símbolos |
| `v0.101.25` | tag, sin release | su gate de candidato murió en la última etapa por un comando de shell mal escrito |
| `v0.101.26` | tag, sin release | su gate de candidato murió en `clean-clone` por un defecto real del filtro de recorrido |
| `v0.101.27` | publicada (2026-10-22, 8 assets) | lleva los tres defectos que los gates anteriores encontraron |
| `v0.101.28` | publicada (2026-10-22, 8 assets) | lleva el orden del grafo filtrado y la lane de publicación reparada |
| `v0.101.29` | **publicada** (2026-10-22, 8 assets) | lleva las tres unidades nuevas del bloque B1 |
| `v0.101.30` | **publicada** (2026-10-23, 8 assets) | lleva la ontología: el campo que el consumidor lee |

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

## Unidades abiertas

| Unidad | Qué es | Detalle |
|---|---|---|
| **B3** (`C23`) | ontología y arquitectura simplificadas | `ROADMAP.md` §11 |
| **H5** (`C11`) | `SnapshotKey` e incrementalidad honesta | `consumer-contracts/specs/SPEC-CU-005-SNAPSHOT-INCREMENTAL.md` |
| **H3** (`C12`), **H4.4** (`C13`) | navegación Kotlin sin JVM y sus relaciones con Java | `ROADMAP.md` §11 |
| **H26** (`C36`) | una capacidad que no está conectada no se ofrece; un lote se escribe entero o no se escribe | `docs/adr/ADR-055-the-batch-is-all-or-nothing.md` |

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

## La misión

Misión `cognicode` declarada (`goal-compass.json`), con 9 candidatos que
llevan evidencia —una ruta que existe— y un criterio de cierre observable. La
cola está escrita en el orden que fija la auditoría del 2026-10-21 (sus bloques
B1 a B6), de mayor a menor prioridad.

El primero que encadenaba era **C31**, y está cerrado: la autoridad con la que
una herramienta MCP se ejecutaba la leía del catálogo que había en el disco
del proyecto analizado. Un proyecto que declaraba `write_file` como
herramienta de solo lectura conseguía que el modo de solo lectura la
aceptase. La autoridad de una herramienta es del producto, y el producto no
es el proyecto que se analiza. Detrás de él vinieron **C5**, **C21**, **C8**
y **C7**, también cerrados.

**C25, C20, C24, C18, C28 y C29 están cerrados.** C17 salió en la PR #404.
C2, C3 y C4 salieron en las PR #400, #401 y #402.

## Gates

| Gate | Comando | Cuándo |
|---|---|---|
| fast | `bash scripts/test-fast.sh` | durante el evolutivo, con `--filter` mientras la unidad sea una |
| full | `bash scripts/test-full.sh` | antes de PR, antes de tag, antes de bump de SemVer |
| merge-gate | `bash scripts/ci/publish-merge-gate.sh` | publica el veredicto que `main` exige |

`full` incluye el gate de contratos. `merge-gate` corre sobre el árbol de
trabajo, así que **no se edita código mientras corre**: el resultado
correspondería a otro árbol. Y corre en un volumen con espacio: si los
temporales van a `/tmp`, la cuota puede matar la batería entera.

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
