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
| H3 — la resolución se decide por proximidad de ficheros | **`C13`** (`H4.4`), cerrada el 2026-10-29 (`d9bf2b45`); su residuo declarado —la pérdida de una relación no se publica— lo toma **`TASK-14`**, cerrada el 2026-10-29 (`00169311`); el residuo de ese cierre —la equivalencia CLI/MCP no se ha ejercido sobre este corpus— lo toma **`TASK-15`**, cerrada el 2026-10-29 (`9e1d36d7`); el residuo de ese cierre —la respuesta que el MCP sirve desde la caché es un veredicto hueco— lo toma **`TASK-16`**, cerrada el 2026-10-29 (`323abe72`); el residuo de ese cierre —el lazo de feedback da verde sin correr el test que se ha escrito— lo toma **`TASK-17`**, cerrada el 2026-10-29 (`6404a84b`) |
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

**El ciclo de TASK-17 está cerrado en el remoto.** El pull request `#499` —la
publicación de la rama `fix/fast-fallback-runs-touched-target`— se fusionó por
squash como `6404a84b`, con el `merge-gate` verde sobre `dcfdedb`. El lazo de
feedback del modo rápido ya no da verde sin correr el test de integración que el
cambio tocó.

**El residuo, con las palabras del cierre anterior.** El recibo de `TASK-16`
cerraba declarando, en su sección de gates, una advertencia que el gate mismo
enseñaba:

> el `fast` NO ejecuta el fichero de test nuevo, porque el selector CR-08 cae
> en su regla de catch-all —`product/tools.json` es un path que no conoce— y el
> camino de fallback corre `cargo test --workspace --lib`, que son SÓLO
> pruebas unitarias.

Eso no era una limitación del entorno: era la ruta que `fast` tomaba siempre
que el diff contenía un path que el selector no clasifica. Y
`product/tools.json` está VERSIONADO y no lo clasifica ninguna regla de CR-08,
así que el caso no era raro: era la norma de cualquier cambio que lo tocara. El
test nuevo se ejecutó a mano en aquel ciclo —`cargo test -p cognicode-core
--test cached_coverage_is_honest`, 5 pasados— y el `full` sí lo incluye, porque
el `full` corre `cargo test --workspace`. El defecto, entonces, era del LAZO DE
FEEDBACK, no del gate. Eso es lo que esta unidad mide y cierra.

**El mecanismo, leído en el código antes de tocarlo.** `scripts/test-fast.sh`
tiene DOS caminos de ejecución, y no eran equivalentes:

| camino | qué corre | quién lo llama |
|---|---|---|
| `changed` | `cargo test -p <crate> --lib` **más** el target de integración que los paths tocan | `run_suite()` |
| `fallback` | `cargo test --workspace --lib` y nada más | el bloque de fallback, que **se saltaba `run_suite()` entero** |

El camino `changed` es el que M13 arregló el 2026-10-22: `touched_targets()`
calcula el binario que cargo declara por cada fichero `.rs` directamente bajo
`crates/<crate>/tests/`. El `fallback` no lo tenía: se saltaba la función
entera, y por eso no veía ni la biblioteca por crate ni el target tocado. La
entrada al `fallback` es la regla 13 del selector: «cualquier path desconocido
-> fallback ALL».

**Lo que NO se hace, y por qué.** Tres caminos se descartaron con su coste
medido, y ninguno se vuelve a intentar aquí:

- **Derivar el crate del path tocado** (`crates/<dir>/tests/<fichero>.rs` ->
  `-p <dir>`). Es una SEGUNDA fuente de verdad del mapa suite->crate, que ya
  vive —y sólo vive— dentro del `case` de `run_suite()`, y que el contrato
  `test_the_selector_doc_and_the_runner_agree` pina ahí. Además da un ROJO
  FALSO: `crates/spike-ladybug/` está excluido del workspace, así que
  `cargo test -p spike-ladybug` responde «package ID specification
  `spike-ladybug` did not match any packages» y el lazo de feedback se rompe
  con un error que no habla del cambio.
- **Añadir reglas al selector para los crates que no mapea.** Amplía el radio
  de acción —cabecera del selector, `run_suite()` y el contrato que obliga a
  que ambos coincidan— para un hueco que el `full` ya cubre.
- **`cargo test --workspace --tests`.** Convierte `fast` en la batería de
  integración: los 99 binarios de `core` lo convierten en minutos, que es lo
  que `fast` no es.

**La decisión de forma.** El mapa suite->crate NO se mueve y NO se duplica: el
`fallback` llama a `run_suite()` con un segundo argumento, `touched-only`, que
corre los targets tocados de esa suite y no su biblioteca —el `fallback` ya
corre la biblioteca entera con `--workspace`, y repetirla cinco veces no compra
nada—. Para que esa llamada sea posible, el bloque de `fallback` se MUEVE debajo
de `run_suite()`: mientras vivía arriba, la función todavía no estaba
registrada en ese punto del script, y una llamada a una función que todavía no
existe no es una llamada, es un `command not found`. El movimiento es puro
reordenamiento: ninguna función cambia de nombre, ninguna suite cambia de
crate, y el `case` de `run_suite()` —lo que el contrato pina— sigue intacto.

**MEDIDO: el RED, y lo que el wrapper ejecutaba.** El contrato nuevo (`M17`, en
`scripts/ci/test_fast_full_wrappers.py`) no lee el código: ejecuta el wrapper
de verdad en un árbol git de laboratorio, con un `cargo` falso que anota los
argumentos que recibe, y mira la lista. El laboratorio se monta con dos paths:
`crates/cognicode-core/tests/prf_demo.rs` y `product/tools.json`. El primero es
un target de integración; el segundo es un path que el selector no conoce, y por
eso el caso cae en `fallback`. Antes de la corrección, el inventario era:

```
test --workspace --lib --quiet
clippy --workspace --all-targets --quiet -- -D warnings
```

y el fallo era, literal:

> AssertionError: FAIL: el camino de fallback no ejecuto el target de
> integracion que se toco. El target tocado es el unico que puede hablar del
> cambio, y el fallback lo es tambien: no tiene excusa para no correrlo.
> Lo que ejecuto: test --workspace --lib --quiet / clippy --workspace
> --all-targets --quiet -- -D warnings

`--test prf_demo` no estaba, porque nadie lo había pedido. Después de la
corrección, el inventario del mismo caso es:

```
test --workspace --lib --quiet
test -p cognicode-core --test prf_demo
clippy --workspace --all-targets --quiet -- -D warnings
```

Una invocación más, y es POR TARGET TOCADO, no por suite: un `fallback` que no
toca ningún test de integración corre exactamente lo que corría antes. El camino
`changed` no cambia: `test -p cognicode-core --lib`,
`test -p cognicode-core --test prf_demo`, clippy.

**Y el hueco del que el `fallback` no puede salir, declarado en voz alta.** El
selector no mapea todos los crates del workspace, así que un target de
integración de uno de ellos sigue sin correrse en `fast`. MEDIDO sobre
`cargo metadata --format-version 1 --no-deps`: 11 miembros, 213 targets de
integración; el selector mapea 197 —`core` (131), `cli` (33), `mcp` (17),
`explorer` (16) y `ladybug` (0)—, y quedan 16 sin mapear: `cognicode` (7),
`cognicode-runtime` (5), `cognicode-graph-algos` (2), `cognicode-graph-wasm`
(1), `cognicode-macros` (1) y `cognicode-sandbox` (0). El motivo es uno solo y no
es un descuido: el mapa suite->crate del selector no tiene caso para ellos, y
esta unidad no amplía el radio de acción del selector. El `full` sí los corre
—`cargo test --workspace` los incluye—, salvo `spike-ladybug`, que está
EXCLUIDO del workspace y por tanto no lo corre ningún lane de cargo: 9 targets
de integración sin carril en ninguna parte, que quedan aquí declarados. Y para
que el `fast` no pueda dar un verde vacío sin decirlo, el `fallback` cuenta los
targets que ha ejecutado y, si no ha corrido ninguno, lo dice: un wrapper que
no puede decir que no, aquí, no puede decir nada.

**Lo que este corte NO arregla.** El `merge-gate` nunca estuvo mal: corre
`test-full.sh`, que corre `cargo test --workspace`, que sí incluye los tests de
integración. El defecto era del lazo de feedback del developer, y por eso esta
unidad toca el wrapper y no el gate. Y los 16 targets de integración de los
crates que el selector no mapea siguen sin correrse en `fast`: los declara, no
los arregla.

**La cola.** `goal-compass.json` NO cambia en este ciclo, y hay que decirlo sin
rodeos: `TASK-17` nunca fue candidata de esa cola —nació como el residuo que el
cierre de `TASK-16` dejó medido y con dueño, en la bandeja local—, así que no
hay ficha que sacar. La cola viva sigue con `C23` (50, bloqueada por el
operador) y nada más.

**Gates.** `fast` rc=0; `full` rc=0 —6504 pasados, 0 fallidos y 34 ignorados
contando los doc-tests; `fmt --check` y `clippy -D warnings` en verde; 455
comprobaciones de contrato en 0 fallos. El contrato de wrappers pasa 20 de 20 —
19 antes de este cambio, más el `M17` nuevo—. Una nota de harness que este
recibo declara porque el cierre de `TASK-16` también la sufrió: el contrato se
ejecuta con `GIT_CONFIG_GLOBAL=/dev/null` y `GIT_CONFIG_SYSTEM=/dev/null`, que
es lo que hace el runner del gate (`scripts/run_contract_tests.py`). Con la
configuración global de esta máquina, el hook local rechaza el commit del
laboratorio por dos diagnósticos —identidad no permitida y mensaje sin tipo
Conventional Commit—, y el fallo es del entorno, no del contrato.

La puerta que queda abierta es la de la cola:

- **`C23`** — la unidad de mayor retorno de todo lo que propone la auditoría
  externa. Sigue bloqueada por el operador: su pull request `#464` es la
  única de las siete que aún lleva trabajo íntegro.

Y, con `TASK-17` cerrado, la cola viva se queda **sin ninguna unidad sin
bloqueo** otra vez: `TASK-17` era el residuo que el cierre de `TASK-16` dejó
medido y con dueño, y ya está hecha. Las vecinas del entorno siguen
igualmente atadas: `TASK-5` por una decisión de alcance del operador y
`TASK-4` por `C23`. La siguiente unidad, por tanto, **no la decide este
documento**: la desbloquea el operador, y hasta entonces el trabajo de este
ciclo es el que queda medido y enrutado.
