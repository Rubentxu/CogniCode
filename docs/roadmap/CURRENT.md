# CURRENT — Snapshot operativo (deriva de `docs/roadmap/ROADMAP.md`)

> **Autoridad**: `docs/roadmap/ROADMAP.md` es la **única autoridad de agenda**
> de CogniCode. Este fichero es un **snapshot puntual**, no una segunda fuente
> de estado: donde este documento y el ROADMAP diverjan, manda el ROADMAP y
> este snapshot se considera obsoleto hasta su próxima regeneración.
>
> **Snapshot**: 2026-10-28, sobre `082ba34c557f0e47ee9e083131a7ad9e26a584e6` (la
> incrementalidad ya está integrada: el grafo se reconstruye sólo donde cambió,
> el servicio dice cuánto conservó, y el escaneo declara los ficheros que no
> puede fechar ni los que no puede leer).
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

## Unidades abiertas

| Unidad | Qué es | Detalle |
|---|---|---|
| **B3** (`C23`) | ontología y arquitectura simplificadas | `docs/adr/ADR-056-the-routes-say-the-same.md` |
| **H4.4** (`C13`) | relaciones Kotlin↔Java, sólo cuando se pueden probar | `ROADMAP.md` §11 |

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
