# K1 — Veredicto de navegación Kotlin sin JVM (línea Kotlin)

**Fecha:** 2026-10-26
**Unidad de trabajo:** C12 del roadmap vivo, que es **K1** de la línea Kotlin
(la misma unidad que el Journal seguía por el nombre de H-unit **H3**).
**Autoridad:** [`docs/roadmap/consumer-contracts/kotlin-evolution/specs/SPEC-KOTLIN-SYMBOLS.md`](../../consumer-contracts/kotlin-evolution/specs/SPEC-KOTLIN-SYMBOLS.md)
(K-SYM-01..07), la línea
[`docs/roadmap/consumer-contracts/kotlin/KOTLIN-LINE-K0-K3.md`](../../consumer-contracts/kotlin/KOTLIN-LINE-K0-K3.md)
y [`ADR-K002.md`](../../consumer-contracts/kotlin-evolution/adr/ADR-K002.md)
(**Estado: PROPOSED** — este veredicto no lo aprueba ni lo reescribe).
**Oráculo de aceptación:** `U-K1` en
[`uat/ACCEPTANCE.md`](../../consumer-contracts/kotlin-evolution/uat/ACCEPTANCE.md),
más el negativo que la propia línea exige («símbolo que no existe produce
`not found/unsupported` tipado; no un símbolo inventado por regex»).
**Árbol medido:** `4920517b477b55710930bb35f7c9b77768bdffa1` —el commit donde
el comportamiento de esta unidad entró— y su árbol
`d964c24b17c75997437d5be63e866963f5aeeb9f`, sobre `c87340f0` (la `main`
publicada tras el squash-merge de la PR #472). La puerta final se midió sobre
el árbol de cierre `50873ca6efe993cbdc40ef358bf97eee4388f3d9` (el commit
`c29e2e94`), que es el anterior más la declaración que la guarda de
contaminación exige; entre ambos árboles no cambia ni una línea de código de
producto.

## Qué es esta unidad

El puente que K0 dejó abierto y honesto: `kt` y `kts` estaban declarados como
extensiones conocidas pero **no soportadas**, y el grafo los contaba como
`KnownUnsupportedGap` en vez de analizarlos. Esta unidad es la que convierte
esa declaración en capacidad: Kotlin se parsea, sus símbolos se publican con
nombre y ubicación, y su código se devuelve como fragmento de la declaración.

El requisito que gobierna la unidad no es «que Kotlin funcione», sino **que el
consumidor sepa lo que se le publica**: qué es un símbolo, dónde está, y qué
código le corresponde. Por eso el criterio de cierre de la SPEC pide
explícitamente «diferencia fría/caliente determinista» y «un solo test
permanente discriminante para identity, otro para código de símbolo».

## La gramática: una, medida

De nueve gramáticas candidatas, **una** compila contra el `tree-sitter 0.27`
fijado en el workspace: `tree-sitter-kotlin-sg` 0.4.1, de la familia ast-grep
(MIT, con `LICENSE` en el crate). Se fijó en `Cargo.toml` y en `Cargo.lock`.

Dos propiedades de esa gramática han mandado en el diseño, y las dos se
midieron sobre sus nodos reales, no sobre su documentación:

- **No expone ningún campo `name`.** El nombre de una declaración es un
  *hijo* cuyo tipo lo dice: el `simple_identifier` de una función, el
  `type_identifier` de una clase, el `identifier` con la ruta con puntos de un
  `package` o de un `import`. Un lector que busque un campo `name` —que es lo
  que hacen todos los otros lenguajes— no encuentra nada, y uno que se quede
  con el primer identificador que ve publica un símbolo cuyo nombre es de
  otra cosa.
- **Declara `receiver` en sus tipos de nodo**, que ninguna de las otras
  expone; sin eso, una extensión (`fun String.shout()`) no tiene receptor y
  su código se publica como si fuera una función suelta.

Su recuperación de errores **reconstruye el fichero** en vez de colapsarlo a un
`ERROR` genérico: por eso un Kotlin roto degrada el grafo a `Partial` con el
fichero nombrado y su razón, en vez de desaparecer del escaneo.

## Lo que se añadió: cinco costuras de dominio, un lector

El dominio es donde un lenguaje declara lo que es, y hasta ahora lo declaraba
a medias: un tipo de nodo por categoría, en `Language::*_node_type`. Kotlin
no cabe en eso —declara varios tipos en tres categorías—, así que la unidad
añade el hueco y lo rellena **por lenguaje**, sin tocar lo que publican los
otros quince:

| Costura | Qué declara | Quién la lee |
|---|---|---|
| `Language::Kotlin` | el lenguaje: `.kt`/`.kts`, id `kotlin`, nombre `Kotlin` | parser, escáner, catálogo |
| `Language::symbols()` | los conjuntos de tipos por categoría | el extractor genérico |
| `Language::name_rule()` (`NameRule`) | qué tipos llevan nombre, qué tipos pueden precederlo, qué tipos heredan el nombre de su clase | **todos** los lectores de nombres |
| `Language::package_types()` | qué nodo declara el paquete | el lector de símbolos |
| `Language::reads_declared_symbol_sets()` | si el lector antiguo debe leer los conjuntos declarados en vez de un tipo por categoría | el parser antiguo y el lector de código |

`NameRule` es la pieza que evita la segunda fuente de verdad: el nombre de
una declaración se lee **siempre** por la misma función
(`infrastructure/parser/naming.rs::declaration_name`), así que el extractor
genérico y el parser antiguo no pueden discrepar sobre cómo se llama un
símbolo. La lección de `EvidenceStore` —no duplicar la semántica entre
interfaces— es exactamente lo que esta costura previene.

Dos reglas de `NameRule` describen lo que Kotlin hace y ningún otro lenguaje:
el **constructor secundario** no tiene nombre propio y toma el de su clase, y
el **`companion object` sin nombre** toma el de su clase con el sufijo
`.Companion`. Un `companion` con nombre propio lo conserva.

El lector de código (`infrastructure/semantic/symbol_code.rs`) tenía su
**propia lista de tipos de nodo por lenguaje** —una segunda tabla que
contradecía al lector— y por eso el fragmento de un símbolo Kotlin habría
sido el identificador suelto en vez de la declaración. Ahora consulta los
conjuntos declarados cuando el lenguaje los declara, y conserva la lista
histórica para los que no los declaran: cambiar lo que publican quince
lenguajes no es un cambio que esta unidad pueda hacer.

## El puente de K0, cerrado

`KNOWN_UNSUPPORTED_EXTENSIONS` queda **vacío**: `kt` y `kts` ya no son una
brecha anunciada, son código que se analiza. `WalkClassification::Analyze` es
lo que responde ahora por ellos, y el contrato que lo medía
(`h11_unified_classification_contract.rs`) se reescribió para exigirlo.

## Lo que ve el consumidor

`product/languages.json` registra `kotlin` como **`experimental`**, con
extensiones `kt`/`kts` y lector `generic`. `experimental` no es una cortesía:
el generador decide `supported`/`experimental` según si el lenguaje tiene un
escenario de aceptación en `m06_acceptance.rs`/`m10_acceptance.rs`, y Kotlin
**no lo tiene** — su UAT es efímera y su regresión permanente vive en el
contrato del core, no en esos escenarios. Marcarlo `supported` por el hecho de
parsear nodos es exactamente lo que K-SYM-07 prohíbe.
`product/product-manifest.json` se regeneró con su generador, no a mano.

## RED→GREEN: la regresión promovida

`crates/cognicode-core/tests/k1_kotlin_symbol_contract.rs` — 10 tests, y es
la **única** regresión permanente que esta unidad promueve. Cubre lo que la
SPEC pide y lo que la línea exige:

| Test | Qué discrimina |
|---|---|
| `the_two_readers_publish_the_same_kotlin_declarations` | el extractor genérico y el parser publican las mismas declaraciones Kotlin |
| `the_outline_reader_publishes_the_same_kotlin_declarations` | el tercer lector (el esquema jerárquico) tampoco discrepa |
| `the_kotlin_declaration_vocabulary_is_classified_explicitly` | el vocabulario completo, con su categoría y su línea: interface, data class, clase, clase anidada, companion con nombre, companion anónimo (`Plain.Companion`), enum, `object`, sealed, value class, constructor secundario, función top-level, método, extensión, las dos sobrecargas, las propiedades y el paquete |
| `two_overloads_are_two_symbols_and_the_constructor_is_not_its_class` | dos sobrecargas son dos símbolos; el constructor secundario no se funde con su clase |
| `the_kotlin_package_is_published_as_its_own_declaration` | el paquete es una declaración, con su línea y su columna |
| `the_code_of_a_kotlin_symbol_is_its_declaration` | el código de un símbolo es su declaración, no el primer identificador |
| `the_code_of_a_kotlin_symbol_keeps_the_unicode_it_declares` | el Unicode de la fuente sobrevive al viaje, sin desfase de columnas |
| `a_kotlin_file_that_does_not_parse_is_not_published_as_read` | un Kotlin roto no se publica como leído |
| `a_pipeline_script_is_read_as_source_and_does_not_invent_a_service` | un `.pipeline.kts` es fuente, y no inventa un servicio |
| `a_gradle_script_is_read_as_source_with_its_own_limits` | un `build.gradle.kts` es fuente, con sus límites |

**El RED de la capacidad** es que en `c87340f0` la capacidad no existe:
`Language::Kotlin` no está, la gramática no está, y el contrato nuevo ni
siquiera compila contra ese árbol.

**El RED de la regresión** está medido por mutación focal sobre el árbol
final: se rompió el comportamiento a propósito, una vez por costura, y cada
mutante murió sólo por los tests escritos para él. Los mutantes y su muerte
están en la tabla siguiente. Un test que no muere cuando se rompe su
comportamiento no es una garantía: es decorado.

| Mutante | Qué rompe | Muere en | Binarios que siguen verdes |
|---|---|---|---|
| M1 `is_symbol_node` ignora los conjuntos declarados (`if false {`) | el lector de código vuelve a su lista histórica de tipos, y el fragmento de un símbolo Kotlin deja de ser su declaración | `k1_kotlin_symbol_contract`: `the_code_of_a_kotlin_symbol_is_its_declaration`, `the_code_of_a_kotlin_symbol_keeps_the_unicode_it_declares` | 90 |
| M2 `NameRule.prefixes` vacía | se pierde el prefijo que envuelve al nombre (`modifiers`, `type_parameters`, `receiver_type`, `binding_pattern_kind`), y el nombre se corta donde está la palabra clave | `k1_kotlin_symbol_contract`: `the_kotlin_declaration_vocabulary_is_classified_explicitly`, `two_overloads_are_two_symbols_and_the_constructor_is_not_its_class` | 90 |
| M3 `NameRule.inherited` vacía | el companion sin nombre pierde el `.Companion` heredado y el constructor secundario pierde el nombre de su clase | `k1_kotlin_symbol_contract`: `the_kotlin_declaration_vocabulary_is_classified_explicitly`, `two_overloads_are_two_symbols_and_the_constructor_is_not_its_class` | 90 |
| M4 `package_types()` vacía | el paquete deja de ser una declaración del fichero | `k1_kotlin_symbol_contract`: `the_code_of_a_kotlin_symbol_is_its_declaration`, `the_kotlin_declaration_vocabulary_is_classified_explicitly`, `the_kotlin_package_is_published_as_its_own_declaration` | 90 |
| M5 la gramática de Kotlin se sustituye por la de JSON | no hay gramática Kotlin: el corpus deja de parsear y los símbolos desaparecen | `k1_kotlin_symbol_contract`: 4 tests —`the_two_readers_publish_the_same_kotlin_declarations`, `the_outline_reader_publishes_the_same_kotlin_declarations`, `the_code_of_a_kotlin_symbol_is_its_declaration`, `the_code_of_a_kotlin_symbol_keeps_the_unicode_it_declares`— y el test unitario `application::services::analysis_service::tests::test_u15_an_asset_is_not_a_gap_and_a_readable_source_is_analysed` | — (cargo se detiene en el primer binario rojo) |

M1–M4 mueren **sólo** en el contrato promovido: los otros 90 binarios de
test del core siguen verdes. M5 muere en el primer binario que encuentra, que
es el binario de la librería; ejecutado a propósito sólo sobre el contrato
promovido, mata 4 de sus 10 tests.

## Lo que la batería encontró por el camino: dos contratos sin precondición

La suite del core no estaba verde cuando se mide esta unidad, y el motivo no
era Kotlin: dos contratos usaban **un fichero `.kt` como hueco real** —el
hueco que produce `Partial`—, y un hueco así **caduza en cuanto el producto
soporta el lenguaje**. Al soportar `.kt`, los dos contratos dejaron de ver su
precondición y el producto pasó a publicar `Complete` donde ellos esperaban
`Partial`.

El hueco que queda es un fichero con **sintaxis rota**
(`broken_syntax.rs`), que produce un `SkipReason::Parse` para siempre y es
del mismo tipo que el que el recorrido por fichero ya publica. Los dos
contratos (`cu1_w2_service_analyzer_contract.rs` y
`h05_acceptance_laws.rs`) conservan sus aserciones —que `graph.complexity`
herede `Partial` y los **mismos** gaps de `graph.full` sobre el mismo
workspace— y su precondición ya no depende de la matriz de lenguajes.

La lección es general y queda escrita en el propio contrato: **un fixture
cuya verdad depende de que un lenguaje NO esté soportado es una bomba de
reloj**. La cobertura no se pierde al crecer el producto; se pierde el
contrato que la vigilaba.

## UAT efímero: 30/30 sobre los binarios release

El guion efímero (`/tmp/uat-k1-tools/uat_k1_release.py`, **fuera del repo**)
monta un corpus con el Harness Base versionado (`sandbox/fixtures/kotlin/`) y
añade **un** fichero roto, y habla con el `cognicode-mcp` y el `cognicode`
**release** por JSON-RPC de verdad. Resultado: **30 de 30 comprobaciones PASS,
rc=0**.

Lo que cubre, por el oráculo de `ACCEPTANCE.md`:

- **POSITIVO, símbolo:** clases, interfaces, `data class`, `enum`, `object`,
  `companion` con y sin nombre, constructor secundario, función top-level,
  extensión con receptor, dos sobrecargas y el paquete, cada uno con su
  línea y su columna.
- **POSITIVO, código:** el fragmento de una función top-level es su código; el
  de una extensión muestra su receptor; el de un `object` es su declaración y
  no la palabra clave; el de un `companion` es su declaración; el del paquete
  es `package …`.
- **POSITIVO, determinismo:** el fragmento y el conjunto de símbolos son
  **idénticos en frío y en caliente** —la diferencia fría/caliente que la
  SPEC exige.
- **POSITIVO, Unicode:** un símbolo con acentos, eñes y emoji se publica, la
  columna apunta al símbolo sin desplazamiento, y el fragmento conserva el
  Unicode de la fuente.
- **POSITIVO, scripts:** un `build.gradle.kts` no inventa declaraciones; un
  `.pipeline.kts` declara su binding y nada más.
- **NEGATIVO:** un símbolo que no existe devuelve **cero** ubicaciones en el
  índice, no un símbolo inventado; el CLI tampoco lo inventa.
- **AAT:** un fichero Kotlin roto degrada el build a `Partial` y se nombra con
  su razón de parseo.
- **CLI release:** `index build`, `index query` (con acierto y con el símbolo
  inexistente) e `index outline` sobre el fichero Kotlin.

**Veredicto de promoción: PROMOTE (una regresión) + RETIRE (el UAT).** El
guion efímero se borra al cerrar el ciclo —su evidencia vive en este
veredicto— y en su lugar queda el contrato permanente del core. El Harness
Base (`sandbox/fixtures/kotlin/`) **sí** se versiona: es reutilizable, y el
contrato permanente lo lee.

## Residuos declarados

1. **El índice de símbolos de la CLI publica ubicaciones `0:0` para el
   paquete**, y publica símbolos de un fichero que **no parsea**. El índice
   no distingue el fichero que no se pudo leer del que sí, y su coordenada de
   paquete es `0:0` en lugar de `1:1`. Es un defecto real, medido en la UAT, y
   **no lo arregla esta unidad**: el índice es de otra familia y el arreglo
   tocaría la proyección que lo escribe. Queda declarado, no escondido, y
   encaja en la unidad de la ontología (`C23`).
2. **K-SYM-01 pide un tipo `SourceKind` con `KotlinFile | Script | BuildDsl`.**
   En este árbol **no existe tal tipo**: no hay `SourceKind` en ninguna parte
   del código. Lo que Kotlin encuentra es la distinción por extensión (`.kt`
   frente a `.kts`) y, dentro de los scripts, por contenido (un
   `build.gradle.kts` y un `.pipeline.kts` se leen como fuente y no declaran
   servicios). El requisito, leído en sus términos, se cumple en lo que el
   producto puede cumplir; la forma que la SPEC propone no tiene a quién
   corresponda, y se dice aquí en vez de inventar un tipo para que el papel
   cuadre.
3. **`find_function_definitions` sigue leyendo un tipo por categoría**, así
   que en Kotlin no ve el constructor secundario. La lectura de símbolos —la
   de esta unidad— usa los conjuntos declarados; la de complejidad no. Es
   deuda de la costura, no de Kotlin, y la hereda cualquier lenguaje que
   declare más de un tipo por categoría.
4. **El destino textual de un `import` Kotlin con alias**
   (`import a.b.C as D`) no está afirmado por ningún contrato: la arista de
   importación es una relación, y las relaciones son de K2.
5. **Kotlin sigue siendo `experimental`**, y lo seguirá mientras no tenga un
   escenario de aceptación en los ficheros que el generador lee. Marcarlo
   `supported` es K2 y siguientes, si acaso.
6. **Las relaciones Kotlin↔Java (K2) y el proveedor LSP (K3) no se tocan.**
   Esta unidad es de símbolos.
7. **El `fast` no cubrió esta unidad.** El selector CR-08 no conoce
   `scripts/product/**`, así que una ruta ahí lo hace caer al subconjunto
   conservador, que no incluye los contratos de integración. Es un hueco
   **preexistente** del selector, no de esta unidad, y no se arregla aquí:
   tocar el selector de CI es otra unidad y otro gate. Queda declarado porque
   un `fast` verde que no ejecutó el contrato promovido no es evidencia de
   nada; la evidencia de esta unidad es el `full`.

## Batería

Los dos gates del ciclo, con su resultado medido. El `full` es la autoridad;
el `fast` es el lazo de iteración y se declara con su alcance real, no con el
que tendría en un árbol sin rutas desconocidas.

### `bash scripts/test-fast.sh` — rc=0

El selector CR-08 **cayó al subconjunto conservador** (`strategy=fallback`):
`scripts/product/test_product_support_matrix.py` no está en la tabla de
caminos del selector, así que la ruta no se pudo clasificar. El hueco es
**preexistente** —esta unidad no lo introduce— pero su consecuencia se
declara, porque no es cosmetics: este `fast` **no** ejecutó el contrato de
integración promovido.

| Medición | Valor |
|---|---|
| Subconjunto ejecutado | `cargo test --workspace --lib` + `cargo clippy --workspace --all-targets -- -D warnings` |
| Binarios | 10 |
| Pruebas | **4 434 verdes, 0 rojas**, 2 ignoradas |
| `k1_kotlin_symbol_contract` | **no ejecutado** — el `grep` del log da 0 coincidencias |

Por eso la evidencia de esta unidad es el `full`: el `fast` no la cubre.

### `bash scripts/test-full.sh` — rc=0

| Etapa | Resultado |
|---|---|
| `cargo fmt --all --check` | OK |
| `cargo clippy --workspace --all-targets -- -D warnings` | OK, 0 avisos |
| `cargo test --workspace` | **6 444 verdes en 237 binarios de prueba, 0 rojas**, 22 ignoradas |
| `cargo test --workspace --doc` | **8 verdes en 10 binarios de prueba, 0 rojas**, 12 ignoradas |
| `cargo test -p cognicode-explorer --features multimodal` | OK |
| `scripts/ci/run-all-contracts.sh` — 61 suites | **439 comprobaciones verdes, 0 rojas** |
| Comprobante de línea base (`known_failures.yaml`, core/lib) | OK — la línea base no declara fallos conocidos, así que no hay nada contra qué comparar y la suite no se repite |
| **rc global** | **0** |

Esta tabla es la medición del **árbol de cierre** `50873ca6` (commit
`c29e2e94`): la única que ve el árbol como lo verá el resto del mundo, con la
unidad ya versionada. Las cifras coinciden con las de la corrida anterior,
que se midió sobre el árbol de trabajo —y por eso el arreglo de formato que
se cuenta más abajo no cambió comportamiento—.

### Un `full` fue rojo, y el rojo era mío

Una de las corridas terminó en `rc=1` en la etapa `fmt --check`: un `assert!`
encadenado en `crates/cognicode-core/tests/cu1_w2_service_analyzer_contract.rs:194`
que rustfmt quería replegar. **No** era infraestructura, ni flakiness, ni el
ruido conocido: era un defecto de formato que esta unidad introdujo.

La corrección fue mínima y cosmética —`cargo fmt --all`, que tocó un solo
fichero y un solo fragmento— y la corrida siguiente dio `rc=0` con **las mismas
cifras** (6 444 / 0 rojas / 237 binarios, 439 contratos / 0 rojas). Eso es
justo lo que prueba que el arreglo no cambió comportamiento: si hubiera
cambiado algo, los recuentos se habrían movido.

### El `full` del árbol de cierre encontró un defecto que las corridas anteriores no podían ver

La corrida sobre el **árbol de cierre** —el que se entrega, con la unidad ya
versionada— salió en `rc=1`: de las 439 comprobaciones de contrato, **438
verdes y 1 roja**, con las otras 60 suites en verde. La roja era
`test_prose_script_contamination.py`, y la causa no fue un descuido de
ortografía sino una propiedad del propio guard: **enumera los ficheros
versionados** (`git ls-files`), así que mientras el contrato de Kotlin y su
fixture estaban sin commitear no podía verlos. Sus caracteres CJK le eran
invisibles, y las corridas verdes anteriores no lo contradecían: medían un
árbol que todavía no existía como commit.

Los caracteres no son contaminación de prosa: **son el sujeto del test**. La
garantía que `k1_kotlin_symbol_contract` afirma es que el código que se
publica no desplaza la columna de un identificador, y un identificador
latino con acentos no la pondría a prueba —un acento son dos bytes, un
ideograma son tres por carácter, y el desplazamiento es justo lo que se
mide—. Quitar el CJK para que la guarda callara habría debilitado la
garantía para satisfacer una guarda, y el propio contrato ofrece el
mecanismo para esto: **declarar el fichero con su razón**.

La corrección fueron dos entradas en `EXEMPTIONS` —el contrato de Kotlin y el
fixture Unicode— con la razón que explica por qué el CJK se queda. La guarda
sigue barriendo todo lo demás, y su comprobación de que toda exención siga
eximiendo algo impide que la declaración se vuelva una esquina muerta. La
corrida siguiente, sobre el árbol ya versionado, dio `rc=0` con las mismas
cifras.

**La lección es general, y por eso se queda escrita: un gate verde sobre el
árbol de trabajo no es un gate verde sobre el árbol que se entrega.** La
corrida de cierre no es un trámite: es la única que ve el árbol como lo verá
el resto del mundo.

## Estado al cierre

- Rama `feature/the-kotlin-declaration-is-named`, con el comportamiento en
  `4920517b`, este veredicto y la entrada del Journal en `ba55a695`, y la
  declaración que la guarda de contaminación exige en `c29e2e94`. La puerta
  final se midió sobre el árbol de `c29e2e94`.
- El veredicto del `merge-gate` no se publica aquí: lo publica la pull request
  cuando GitHub lo ejecute, y el Journal lo recoge cuando exista.
- El bump de SemVer, el tag y la publicación los decide el operador; el
  workspace queda en `0.101.31` y la última release publicada es `v0.101.30`.
- `ADR-K002.md` sigue **PROPOSED**: esta unidad no aprueba ADRs.

## Próximo trabajo ejecutable

La cola que deja esta unidad es la que dejó C46 menos C12: **C23** (50,
bloqueada por el operador —PR #464 abierto—), **C44** (15) y **C13** (10,
relaciones Kotlin↔Java, que ya no depende de esta unidad).
