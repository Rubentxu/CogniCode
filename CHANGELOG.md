# Changelog

Todos los cambios notables de CogniCode se documentan en este archivo.
Formato basado en [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

> **Nota de reconstrucción**: v0.50.0–v0.86.0 no tenían entradas individuales
> en este archivo. Se reconstruyeron en E31-B5-rollup (B5) con un resumen
> de alto nivel; el historial commit-por-commit está disponible vía
> `git log v0.50.0..v0.86.0`. Las versiones anteriores a v0.50.0 no
> tienen entradas aquí; el historial completo puede reconstruirse
> desde `docs/ROADMAP.md` (working doc local, no versionado).

## [v0.101.9] — 2026-10-04

**Por qué existe v0.101.9, y qué pasó con `v0.101.8`.** `v0.101.8` está
publicado y **no se mueve**. Su lane construyó el candidato entero y salió con
fallo. Esta release lleva el arreglo.

Lo importante del caso es que **el trabajo estaba bien y el veredicto no**:

    Pipeline finished with FAILURE: shell exited with code 64
    PASS: published-layout CLI + MCP + skills install/update/reshim/uninstall

Esas dos líneas, en ese orden, son la misma corrida. Antes del 64 estaba el
PASS del UAT de instalador. La lane había compilado los dos targets, generado
los SBOM, limpiado la raíz del staging —los seis tarballs de `0.101.4` y
`0.101.5` que la lane tenía que eliminar, eliminados— y producido un candidato
cuyos **once artefactos verifican contra su `SHA256SUMS`**. Después, la limpieza
no pudo borrar su directorio temporal y su código de salida sustituyó al del
script.

### Corregido

- **La limpieza del UAT de instalación podía ser su propio veredicto.** El trap
  era `rm -rf "$TMP"` sin salir del directorio que borraba, y sin fijar estado,
  de modo que el código de la envoltura de recuperación reemplazaba al del
  script. Es el mismo defecto que B2 resolvió en `preflight-clean-clone.sh`, en
  el hermano que se quedó atrás: la stage 1 pasaba porque el preflight ya
  estaba arreglado, y esta moría porque el otro no.

- **Cada corrida del UAT dejaba 70 MB huérfanos.** Con la primera mitad
  arreglada se veía la segunda: el script corre con `HOME` y `XDG_DATA_HOME`
  dentro del temporal, y la envoltura trastera lo que borra en
  `$XDG_DATA_HOME/Trash`. El temporal acababa conteniendo su propia papelera, y
  un árbol que contiene su papelera no se puede mover. Devolver las dos
  variables antes de borrar lo cierra.

- **La política de soporte nombraba una rama que no lleva la versión.**
  `SECURITY.md` decía `0.101.8 (current main, not yet released)` con
  `origin/main` en `0.101.0`, y lo decía igual desde `v0.101.7`. Sobrevivió dos
  cortes porque ningún contrato leía el fichero: la versión de la tabla se
  vigilaba, la rama no. El contrato de Release Truth ahora cubre esa cuarta
  superficie.

### La forma del arreglo

Es la misma que la del preflight, a propósito: la función vive en el script que
la usa, lee el estado de salida en la primera sentencia, sale del directorio
antes de borrar, y el borrado va dentro de un condicional cuyo fallo avisa en vez
de decidir. Los seis contratos nuevos están en `qw04_preflight_contract.rs`, que
ya era el dueño de esa propiedad, y no en un fichero nuevo.

Una diferencia deliberada: en el UAT el motivo del borrado se **imprime**, y en
el preflight se descarta. El preflight escribe en el log de una stage que ya ha
certificado; el UAT es lo que el operador lee cuando algo falla, y un aviso que
no dice por qué obliga a reproducir la corrida entera.

## [v0.101.8] — 2026-10-04

**Estado de su lane, medido después de publicarlo.** Construyó el candidato
entero —dos targets, SBOM, once artefactos que verifican contra `SHA256SUMS`— y
salió con `LANE_EXIT=1` porque el UAT de instalación perdió su veredicto en la
limpieza. El candidato de este corte es por tanto íntegro y reproducible, pero la
lane no lo certificó. El arreglo va en `v0.101.9`, porque un tag publicado no se
mueve.

**Por qué existe v0.101.8, y qué pasa con `v0.101.7`.** `v0.101.7` está
publicado y **no se mueve**: un tag identifica bytes. Se cortó con el árbol
completo, pero **su pipeline de candidate no compilaba**, así que su lane murió
en el minuto dos sin llegar a la primera stage. Esta release es la primera cuyo
pipeline está probado, y lleva dentro tanto el arreglo como el contrato que lo
vigila.

Es la cuarta vez que un corte pasa sus contratos y su pipeline no arranca. La
causa es la misma desde `v0.101.5` y es una sola: **la suite ejecutaba el
cuerpo de las stages y no compilaba el fichero que las contiene**.

### Corregido

- **El pipeline de candidate no compilaba.** Dos párrafos de comentario en la
  guarda del linker explicaban cómo escapar un signo de dólar escribiendo el
  signo de dólar, y Kotlin los leyó como plantillas:

  ```
  ERROR Unresolved reference 'host'.       (línea 254)
  ERROR Unresolved reference 'host'.       (línea 254)
  ERROR Unresolved reference 'key_LINKER'. (línea 263)
  VALIDATION FAILED
  ```

  Un `#` abre un comentario de shell, pero la línea sigue siendo contenido del
  raw string, y en un raw string un dólar abre plantilla se llame como se
  llame la línea. El comentario que explicaba el bug contenía el bug. El bloque
  se reescribió **sin un solo signo de dólar** en la prosa, porque la prosa no
  lo necesita: los ejemplos de escapado están en la stage de abajo, que es la
  especificación.

- **Un contrato mide el nivel que alcanza, y no puede ver por encima de sí.**
  Los 213 contratos anteriores ejecutaban el cuerpo de una stage; ninguno
  compilaba el script que la contiene. Renderizar un cuerpo y pasarlo por bash
  demuestra que el shell es correcto, no que el envoltorio sea un programa.
  `test_every_pipeline_compiles` valida ahora los seis `.kts` del repositorio
  con `pipelinek validate`, que compila y comprueba el grafo de stages sin
  ejecutar ninguna. Mutado —devolviendo un dólar a un comentario— falla con el
  mensaje exacto del fallo real.

### Excepciones que este corte registra

`v0.101.7` queda tagueado, publicado y **sin candidato**, por el motivo de
compilación de arriba. Se suma a `v0.99.2`, `v0.100.0` y `v0.101.6`.

Y un incidente de gobernanza que aquí no se repara: **`v0.101.5` se ha movido
dos veces en el remoto** tras publicarse. Empezó apuntando a `65dcdba3`, pasó a
`64370bbb` y ahora a `188c3a52`. Un tag publicado identifica bytes, y cambiarlo
rompe esa propiedad para quien ya lo haya clonado. Se anota; la salida de este
corte es construir desde la línea correcta, no reconstruir la identidad de una
release anterior.

## [v0.101.7] — 2026-10-04

**Corte sin candidato.** El árbol estaba completo —la reconciliación de las dos
líneas, los ocho cherry-picks, la limpieza de la raíz de staging y la
reparación del gate de coherencia—, pero **el pipeline de candidate no
compilaba**, por un signo de dólar escrito dentro de un comentario para
explicar cómo escapar un signo de dólar. Su lane murió en el minuto dos. El
arreglo y su contrato están en `v0.101.8`.

## [v0.101.6] — 2026-10-03

**Corte sin candidato.** Se cortó con la reconciliación de las dos líneas
completa, y antes de que llegaran la limpieza de la raíz de `staging/` y la
reparación del gate de coherencia. La ventana honesta es `c7dba40c..HEAD`: 15
commits, 8 `fix`, 4 `docs`, 3 `test`, 0 `BREAKING CHANGE`, de los cuales cinco
`fix` entraron por cherry-pick desde la línea del otro actor. La divergencia y
el mapa están en la entrada de `v0.101.7`, que es la que produce candidato.

## [v0.101.5] — 2026-10-03

Ventana `v0.101.4..HEAD`: **6 commits** — 1 `fix`, 2 `refactor`, 1 `test`,
1 `integrate`, 1 `docs`, y 0 marcadores `BREAKING CHANGE`. Medido con:

```
git rev-list --count v0.101.4..HEAD                      # 6
git log --format='%s' v0.101.4..HEAD | sed -E \
  's/^([a-z]+)(\(.*\))?!?:.*/\1/' | sort | uniq -c | sort -rn
git log --format='%s%n%b' v0.101.4..HEAD \
  | grep -cE '^BREAKING[ -]CHANGE'                       # 0
```

**Por qué existe v0.101.5 y por qué v0.101.4 no se publica.** El tag
`v0.101.4` existe y su candidato llegó a correr, pero se cortó en un árbol
que **no contiene ninguno** de los cinco commits de este ciclo. Publicarlo
habría publicado una release cuyo contrato de salida de la CLI seguía
tragando errores en once de los doce brazos de `CommandExecutor::execute`:
operaciones que no se realizaban devolviendo 0. El tag no se puede mover sin
mutar una identidad ya existente, así que la salida limpia es v0.101.5.

### Corregido

- **Ocho de los once brazos de la CLI tragan el error y salen con 0.**
  `Analyze`, `Graph` y `FindUsages` propagaban; `Refactor`, `Index`,
  `Navigate`, `Doctor`, `DocsIngest`, `IssuesIngest`, `Evidence` y
  `Capabilities` imprimían el fallo con `eprintln!` y dejaban que `execute`
  terminara en `Ok(())`. El caso caro no era hipotético:
  `cognicode navigate references <símbolo>` imprimía
  `Invalid position` y salía con **0**, así que un agente que seguía la
  skill `cognicode-pr-review` creía haber consultado las referencias y no
  había consultado ninguna. N+81 ya había corregido la skill; corregir la
  documentación sin corregir el brazo dejaba el fallo al alcance de
  cualquiera que tecleara el comando.

  MEDIDO sobre el binario v0.101.2, no leído: `navigate references`,
  `navigate definition`, `navigate hover`, `index build`, `index query`,
  `graph full` y `graph mermaid` salían todos con 0.

- **Una raíz de proyecto inexistente se reportaba como un grafo vacío.**
  `AnalysisService::build_project_graph` llegado a una ruta que no existe
  devolvía `Ok` con estado `Partial`: el `WalkBuilder` entrega una entrada
  `Err` por la raíz ausente y `.filter_map(|e| e.ok())` la descarta, que
  es exactamente la que había que conservar. El handler MCP `build_graph`
  ya rechazaba ese caso, así que las dos interfaces respondían distinto a la
  misma pregunta. Lo mismo en `LightweightIndex::build_index` con `WalkDir`.

- **Un UAT que pasaba sin ejecutar lo que decía medir.**
  `prf_cli_01_uat::graph_full_nonexistent_path_does_not_exit_zero` invocaba
  `graph full --path`, y `graph full` no tiene `--path`: su firma es
  `graph full [PATH]`, un positional. Clap rechazaba el flag con exit 2 y
  el proceso moría antes del dispatch, así que la aserción se cumplía sin
  que el motor de grafos llegara a ejecutarse. Medido: la forma antigua
  devolvía **exit 2 idéntico con el defecto presente y ausente**, o sea que
  no podía detectar el bug que decía detectar.

### Cambiado

- **Los brazos propagan con `?` en lugar de imprimir y devolver.**
  `main.rs` ya hacía `CommandExecutor::execute(cli).await?`, así que la
  frontera de propagación existía y lo que la cortaba era cada brazo. La
  forma destino es `Self::execute_navigate(command).await?`: −75 líneas
  netas, sin tipos nuevos, y el `Result` que ya existía es el que llega a
  `main`.

- **La limpieza del preflight vive en el script que la usa.** El trap era
  `rm -rf "$WORK_DIR"` y el script hace `cd "$WORK_DIR/clone"`, así que se
  ejecutaba desde dentro del directorio que borraba; el estado del trap
  sustituye al del script. MEDIDO: el preflight de v0.101.2 certificó
  `passed=5934 failed=0` con `PREFLIGHT PASS` y la lane salió igualmente
  con 64, dejando 34 GB de clones huérfanos. Ahora la función sale del
  directorio antes de borrar y la limpieza no puede cambiar el veredicto.

### Añadido

- **`cli_exit_code_propagation`** (21 tests). Siete casos de error y **tres
  gemelos de éxito**, porque un contrato que solo afirma "esto sale distinto
  de 0" pasa entero si el arreglo convierte todo en error, incluido el
  éxito. Y cuatro contratos que releen el `match` de `execute` y clasifican
  cada brazo, de modo que un brazo nuevo que trague el error es un RED y no
  una línea más de una lista escrita a mano.
- **Siete contratos de limpieza en `qw04_preflight_contract`** (11 → 18).
  El que faltaba es el que reproduce el incidente: un `rm` que se niega
  cuando el target es ancestro del cwd —la envoltura real del entorno— y
  tres aserciones a la vez, exit preservado, negativa que **no** llega a
  producirse, y clon eliminado.

### Medido, y no arreglado aquí

- **`execute_doctor` también llama `std::process::exit`** y **es alcanzable**
  desde el binario normal, a diferencia de `DocsIngest` e `IssuesIngest`.
  Es una tercera violación de la misma frontera, y no se cambia aquí porque
  su exit code es un contrato publicado —"1 = entorno poco sano"— y moverlo
  a `Err` cambia el diagnóstico.
- **`DocsIngest` e `IssuesIngest` son superficie muerta**: viven detrás de
  `#[cfg(feature = "multimodal")]`, y `multimodal` no es una feature
  declarada de `cognicode-cli`. Sus `return Err` están puestos y el
  contrato los verifica en el fuente, pero nadie puede invocarlos.
- **`ide::tests::claude_config_path_default` es inestable**: lee `$HOME`
  mientras corre en paralelo con hermanos `#[serial]` que la mutan. Causa
  raíz medida — 48 `set_var("HOME")` en el binario, todos serializados, y
  este test no— y tasa medida: **4 fallos de 10 ejecuciones**. Preexistente
  a este ciclo, y por tanto fuera de él.
- **La provenance de esta release no existe.** Sin cambios respecto a
  v0.101.2: `RELEASE_REQUIRE_PROVENANCE=0` y el hueco de R2 documentado en
  `docs/adr/ADR-RELEASE-PROVENANCE-PIPELINEK.md`, pendiente de la decisión
  del maintainer sobre custodia de la clave.

## [v0.101.4] — 2026-10-03

Ventana `v0.101.3..f840df0c`: **2 commits** — 1 `fix`, 1 `test`, y 0 marcadores
`BREAKING CHANGE`. Medido con:

```
git rev-list --count v0.101.3..f840df0c                       # 2
git log --format='%s' v0.101.3..f840df0c | sed -E \
  's/^([a-z]+)(\(.*\))?!?:.*/\1/' | sort | uniq -c | sort -rn # 1 test, 1 fix
git log --format='%s%n%b' v0.101.3..f840df0c \
  | grep -cE '^BREAKING[ -]CHANGE'                            # 0
```

**Por qué existe v0.101.4 y por qué v0.101.3 no se publica.** El candidato de
`v0.101.3` **falló**. Nunca hubo candidate que publicar, y su tag se queda sin
certificar:

```
package-x86_64-unknown-linux-gnu … Error: unknown component
  `cognicode-0.101.3-x86_64-unknown-linux-gnu.tar.gz`
cp: no se puede efectuar `stat' sobre '…/release/cognicode-0.101.3-…tar.gz'
tar (child): staging/payloads-linux-x86-64/dist/: Es un directorio
staging/payloads-linux-x86-64/dist:
total 8                                    <- vacío, y la stage pasó
→ (dos stages después) archive-standalone: cogh missing or not executable
```

### Corregido

- `release-candidate.pipeline.kts`, `package-$target`: la línea
  `component="${'$'}{filename%-${'$'}version-*}"` usaba una **variable de
  shell** `version` que la lane nunca exporta, donde la línea de al lado usa el
  valor de Kotlin. Vacía, el patrón `%--*` no casa con nada y `component` se
  quedaba con el nombre completo del archivo: `cp` buscaba un binario llamado
  `…tar.gz`, `name --component` recibía lo mismo y `tar` escribía sobre el
  directorio `dist/`. La misma notación estaba mal en los bundles de skills
  (`cognicode-.tar.gz`) y en el mensaje de la stage de SBOM (`the sbom- stage`).
- `release-candidate.pipeline.kts`, `package-$target`: la stage **no fallaba**.
  `cp` y `tar` no se comprobaban, así que salía 0 sin haber producido un solo
  payload, y el defecto aparecía dos stages más tarde con un mensaje que
  señalaba el archivo y no el empaquetado. Ahora un `plan` vacío, un nombre
  vacío, un binario ausente, o un planificado distinto de lo producido, son
  todos fallos — la misma guarda «An empty candidate is not a candidate» que la
  lane de release ya tenía, puesta donde ocurre.
- `release.pipeline.kts`, `provenance`: la stage verificaba atestaciones
  **incondicionalmente** mientras su propio header describía una política
  condicional por `RELEASE_REQUIRE_PROVENANCE`. Como nada en el repositorio
  genera una atestation —el generador era una GitHub Action y PipelineK no
  tiene runtime de actions— la lane moría antes de `publish`: no una release
  sin provenance, una release inalcanzable. La política vive ahora en un único
  script, `scripts/ci/verify-provenance.sh`, y una sola stage lo llama. Los
  artefactos se comprueban siempre y el resultado se imprime; sólo un run que
  exige provenance trata la ausencia como fatal.

### Añadido

- `scripts/ci/test_candidate_packaging.py` — ejecuta el cuerpo **real** de la
  stage: extrae el `sh(...)` del pipeline, resuelve las dos notaciones de
  interpolación y lo corre contra un `cognicode-release` de mentira. En rojo
  contra el pipeline publicado: sale 0 produciendo `[]`.
- `scripts/ci/test_provenance_gate.py` — corre el gate con un `gh` de mentira
  que reproduce el 404 medido. Seis mutantes, ninguno sobrevive.
- `test_no_kotlin_value_is_written_as_a_shell_variable` — cubre la clase
  entera en todos los lanes, no sólo estos dos sitios.

### Correcciones de contrato

- Los dos contratos nuevos se declaraban **verdes sin ejecutar nada**: su
  `main()` usaba `dir(globals())`, que devuelve los métodos del dict y no los
  nombres del módulo, así que la lista salía vacía. Lo detectó el fallo-cercado
  del runner («exposes no test_* function»). Corregido, y ambos se han
 comprobados en rojo contra los pipelines que `v0.101.3` publicó.
- `product/profiles.json` **no** se toca en este corte: su `source_commit` está
  fijado a propósito a un baseline (`73235889`) y `test_profiles.py` lo
  comprueba. Regenerarlo con el SHA de este commit rompe el contrato — un
  `source_commit` obsoleto no es drift cuando alguien lo {_pin} deliberadamente.

### Desconocido

- La ruta de empaquetado de `aarch64-unknown-linux-gnu` no se ha ejecutado
  nunca: la lane murió en `x86_64` antes de llegar a ella.

## [v0.101.3] — 2026-10-03

Ventana `v0.101.2..69186a1c`: **5 commits** — 2 `fix`, 2 `docs`, 1 `test`, y 0
marcadores `BREAKING CHANGE`. Medido con:

```
git rev-list --count v0.101.2..69186a1c                      # 5
git log --format='%s' v0.101.2..69186a1c | sed -E \
  's/^([a-z]+)(\(.*\))?!?:.*/\1/' | sort | uniq -c | sort -rn # 2 fix, 2 docs, 1 test
git log --format='%s%n%b' v0.101.2..69186a1c \
  | grep -cE '^BREAKING[ -]CHANGE'                            # 0
```

**Por qué existe v0.101.3 y por qué v0.101.2 tampoco se publica.** El
preflight de `v0.101.2` **certificó** —5934 passed, 0 failed, ratchet dentro de
tolerancia, recibo `"result": "PASS"`— y la lane reportado fallo igualmente:

```
→ PREFLIGHT PASS
mavis-trash: refusing to trash protected path '.../cognicode-preflight-O2ZYbs'
mavis-trash: '...' is the parent of the current working directory
Pipeline finished with FAILURE: shell exited with code 64
```

Todo el trabajo de certificar se había hecho bien. La lane falló por no haber
borrado una carpeta temporal.

### Corregido

- `scripts/ci/preflight-cleanup.sh` (nuevo), cargado por
  `preflight-clean-clone.sh`. El trap de salida borraba el directorio temporal
  **desde dentro de sí mismo** —el script hace `cd "$WORK_DIR/clone"` en el
  stage 5—, y en un entorno donde el borrado va envuelto en una comprobación de
  seguridad esa comprobación se niega y devuelve 64. El estado del trap sustituye
  al del script, y un PASS se convertía en exit 64. Ahora la limpieza sale del
  directorio antes de borrar y, sobre todo, **no puede cambiar el veredicto**: un
  PASS sigue siendo PASS aunque la limpieza falle, y un fallo real sigue siendo un
  fallo.
- `scripts/ci/test_preflight_cleanup.py` (nuevo). Reproduce la negativa con un
  borrador de mentira que rechaza un directorio ancestro del cwd, que es
  exactamente la regla que falló. Vive aparte porque el preflight entero cuesta
  veinte minutos y 4G, y esta propiedad se observa en milisegundos. Suite de
  contratos: 141 → 147.

### Medido, y no arreglado aquí

`a1f961f6` —de los 11 brazos `CliCommand`, solo `Analyze`, `Graph` y
`FindUsages` propagan el error— sigue abierto y lo está trabajando otro actor en
paralelo. Se deja constancia aquí porque es visible en este mismo corte.

## [v0.101.2] — 2026-10-03

Ventana `v0.101.1..089be899`: **1 commit** — 1 `fix`, y 0 marcadores
`BREAKING CHANGE`. Medido con:

```
git rev-list --count v0.101.1..089be899                      # 1
git log --format='%s' v0.101.1..089be899 | sed -E \
  's/^([a-z]+)(\(.*\))?!?:.*/\1/' | sort | uniq -c | sort -rn # 1 fix
git log --format='%s%n%b' v0.101.1..089be899 \
  | grep -cE '^BREAKING[ -]CHANGE'                            # 0
```

**Por qué existe v0.101.2 y por qué v0.101.1 tampoco se publica.** v0.101.1
está tagueado y empujado, y su candidato se paró durante el preflight, ya en la
compilación de release. La razón no fue un fallo de infraestructura: fue una
auditoría del propio contenido que la release empaqueta.

`release-candidate.pipeline.kts` empaqueta cada skill publicada en
`staging/<id>-<version>.tar.gz`, y la release publica **exactamente** el
candidato, sin recompilar. Es decir: un candidato cortado antes de corregir una
skill publica esa skill rota de forma permanente, y corregirla después obliga a
otro corte. La tabla `SKILL_BUNDLES` de `release_contract.rs` declara
publicadas solo `cognicode` y `cognicode-mcp`; las tres invocaciones rotas
estaban en el camino de publicación, dos de ellas en `cognicode-pr-review`, que
no se empaqueta pero que si se distribuye con el repositorio.

### Corregido

- `skills/cognicode-pr-review/SKILL.md` enseñaba `cognicode navigate references
  <symbol>` dos veces, siendo el paso central de la skill. La firma real es
  `NavigateCommand::References { position: String }` y `position` es
  `file:line:column`. El símbolo se enlaza a `position`, `parse_position()`
  falla, y el brazo `Navigate` imprime el error en stderr **sin** hacer
  `return Err`: el proceso sale con 0 y el agente cree que consultó las
  referencias. La corrección no es mecánica: la intención declarada de la skill
  es "find all usages of a symbol", que es exactamente lo que hace
  `cognicode find-usages <symbol>`, que además propaga su error. `navigate
  references` queda documentado con su firma real.
- `skills/cognicode/SKILL.md` pasaba `MySymbol` a `cognicode index symbol-code`,
  cuya firma es `SymbolCode { file: String, line: u32, column: u32 }`: tres
  positionals requeridos, uno dado, y clap sale con 2.

### Añadido

- `scripts/ci/test_skill_cli_invocations.py`: contrasta cada invocación de CLI
  de un bloque ```` ```bash ```` de las skills contra las firmas clap reales
  leídas de `commands.rs` en cada ejecución. No es una lista mantenida a mano,
  que es justamente lo que faltó: `validate_skills.py` ya contrastaba nombres de
  tool MCP contra el catálogo, y no podía ver esto porque nunca mira invocaciones
  de CLI ni aridad de argumentos. Se descubre por el glob de
  `run-all-contracts.sh`, así que el propio preflight del candidato lo ejecuta.
  Suite de contratos: 119 → 133.

### Medido, y no arreglado aquí

De los 11 brazos `CliCommand`, solo `Analyze`, `Graph` y `FindUsages` propagan
el error con `return Err`. `Refactor`, `Index`, `Navigate`, `Doctor`,
`DocsIngest`, `IssuesIngest`, `Evidence` y `Capabilities` imprimen el fallo y
continúan, con lo que salen con 0. Es un defecto real y distinto del corregido
aquí, que es el contenido publicado; queda registrado como
`a1f961f6-dcec-45a4-8689-9900b0319f91` y no se mezcla aquí.

## [v0.101.1] — 2026-10-03

Ventana `v0.101.0..14b7fea3`: **6 commits** — 3 `fix`, 3 `docs`, y 0
marcadores `BREAKING CHANGE`. Medido con:

```
git rev-list --count v0.101.0..14b7fea3
git log --format='%s' v0.101.0..14b7fea3 \
  | sed -E 's/^([a-z]+)(\(.*\))?!?:.*/\1/' | sort | uniq -c | sort -rn
git log --format='%s%n%b' v0.101.0..14b7fea3 | grep -cE '^BREAKING[ -]CHANGE'   # 0
```

**Por qué existe v0.101.1 y no se publica v0.101.0.** v0.101.0 está
tagueado pero nunca publicado, y su candidato **no podía certificarse**: los dos
defectos que esta versión arregla están en el propio camino de release, así que
el árbol del tag no los contiene. Publicar v0.101.0 exigiría una de dos cosas
que este repositorio no hace por su cuenta: saltarse el preflight, o re-baselinar
con un override que el propio script marca como explícito del operador. Ninguna
de las dos es automática. El tag tampoco se puede mover: la lane de publicación
crea el release sin `--target` **justamente** para no mover una release a donde
el nombre resuelva ahora.

Ninguno de los dos defectos toca el producto: no hay cambios en `cogh`,
`cognicode` ni `cognicode-mcp`, ni en la superficie de tools, ni en los
contratos publicados. Es un PATCH por la regla mecánica del repositorio.

### Fixed

- **El invariante que cerró el cutover de CI leía el disco, no el índice.**
  `test_no_actions_workflows.py` reportaba dos `action.yml` dentro de
  `sandbox/repos/`, que está gitignored y contiene checkouts de terceros que
  GitHub nunca parseó. El escaneo excluía directorios por nombre y `sandbox` no
  estaba en la lista. La consecuencia era peor que un gate rojo: **verde en CI y
  rojo en cualquier máquina que hubiera corrido una lane de sandbox**, porque
  esos checkouts no existen en un runner. Ahora resuelve el índice con
  `git ls-files` —que es lo que responde a "qué publica este repositorio"— y
  un índice ilegible es un `None` explícito, no una lista vacía. Seis tests, y el
  primero encontró un defecto en el segundo: la comprobación de autocableado
  honraba una *mención* del runner en lugar de una *ejecución*, y quedaba verde
  con la suite apagada.

- **Una release no se podía certificar desde un tag.**
  `preflight-clean-clone.sh` pasaba `--branch HEAD` a `git clone` en un checkout
  detached, porque `git rev-parse --abbrev-ref HEAD` imprime la cadena literal
  `HEAD` y sale con código **0**: el `|| echo main` de respaldo nunca se
  disparaba. La lane moría en 0.3 s con `Remote HEAD branch not encontrada`, y un
  tag es exactamente cómo se corta una release. Ahora se omite `--branch` cuando
  no hay rama que nombrar —el commit certificado lo fijan `git fetch` y
  `git checkout $TARGET_SHA`, no el nombre de la rama— con la regla en una
  función compartida y un contrato de 5 tests.

- **El preflight rechazaba el release por tener 355 tests más pasando.**
  `passed=5934 failed=0` contra un baseline de `passed=5579`. La condición era
  `if [ "${PASSED_DIFF#-}" -gt "$TOLERANCE" ]`, y `${PASSED_DIFF#-}` quita el
  signo: comparaba el **valor absoluto**, así que +355 se reportaba como
  `regresión de tests passed`. El guard y su propio mensaje se contradecían. Se
  conserva la lectura que el mensaje describe —una regresión es que los tests
  **bajen**— y el `failed > 0` ya cubría el otro caso. Contrato de 7 tests con el
  caso medido.

### Added

- `docs/adr/ADR-RELEASE-PROVENANCE-PIPELINEK.md` — la vía de provenance sin
  Actions, **medida**: cosign 3.1.3 firma y verifica un bundle SLSA v1 offline y
  el digest del subject coincide con el `sha256sum` del artefacto. Queda
  `proposed` porque lo que falta no es técnico: es dónde vive la clave privada.

### Not fixed, deliberately

- **La provenance de esta release no existe.** `actions/attest-build-provenance`
  murió con Actions y no hay sustituto instalado; el gate es opt-in
  (`RELEASE_REQUIRE_PROVENANCE=0`) y solo avisa. Medido además: `gh attestation`
  no tiene verbo de generación, y el keyless de SLSA es un *consumidor* de un
  token OIDC cuyo emisor era Actions — retirarlo quitó lo único del pipeline que
  podía producir una identidad verificable. Es el hueco de R2, documentado, no
  cerrado. Esta release se publica **sin attestation**, y el ADR dice por qué.
- **El baseline del preflight sigue obsoleto** (`5579` contra `5934` reales).
  Corregir el signo del ratchet no arregla eso, y con un baseline por debajo de
  la realidad el ratchet queda débil: una caída grande de la suite real seguiría
  dentro de tolerancia mientras el total no baje del baseline. Re-baselinar es
  `PREFLIGHT_BASELINE_*`, decisión explícita del operador, y aquí no se toma.
- **`v0.99.2` y `v0.100.0` quedan tagueados y sin publicar.** `v0.100.0` es
  irrecuperable por la vía del lane: en su commit (`edd023b3`, #316) solo existen
  `merge-gate` y `product-fast`, porque las lanes de release nacieron después
  (#338, #340). Publicar con las lanes de hoy sobre un árbol que nunca las
  contuvo produciría artefactos cuyo commit no incluye la herramienta que los
  construyó, que es la provenance falsa que R2 existe para impedir.

### Closed

- **R0 — cutover de CI.** Verificado sobre `main`: 0 workflows de Actions, 6/6
  lanes `pipelinek validate`, `check-release-matrix.sh` OK, layout del candidato
  PASS, y los 14 contratos re-anclados con cero lecturas vivas de workflow. El
  invariante que lo cierra es el primer "fixed" de esta entrada. Detalle en
  `docs/roadmap/JOURNAL.md` N+79.

## [v0.101.0] — 2026-10-03

Ventana `v0.100.0..44d0316a`: **34 commits** — 11 `docs`, 8 `fix`, 6 `test`,
6 `ci`, y 3 merges. Medido con:

```
git rev-list --count v0.100.0..44d0316a
git log --format='%s' v0.100.0..44d0316a \
  | sed -E 's/^([a-z]+)(\(.*\))?!?:.*/\1/' | sort | uniq -c | sort -rn
git log --format='%s%n%b' v0.100.0..44d0316a | grep -cE '^BREAKING[ -]CHANGE'   # 0
```

**Sobre el bump.** La regla mecánica de este repositorio (regla 6) leería
esta ventana como PATCH: no hay `feat` ni marcadores `BREAKING CHANGE`. Se
publica como MINOR de todas formas, y la razón es que **la regla no puede
ver el cambio**: la ventana estrecha la API pública, pero squash-mergeada bajo
un título `fix`, el marcador `BREAKING CHANGE` nunca se escribió. Preferimos
subprometer el bump antes que publicar como no-rompiendo algo que sí rompe:

- `HandlerContext` pasa de 21 campos públicos a 0, con dos accesores en su
  lugar. Cualquier construcción externa de ese contexto deja de compilar.
- `OnDemandGraphBuilder::with_parser` y `::query_call_hierarchy` se borran: no
  tenían un solo consumidor.

El squash de #341 contiene además 25 commits que la regla habría medido por
separado; el título del squash no los describe, y por eso la medición por tipo
de commit no es fiable en esta ventana.

### Corregido

- `OnDemandGraphBuilder::build_index_from_sources` entregaba su texto al
  índice pero no al builder, y el cache de parseo es la única fuente de
  aristas. Toda consulta que necesita aristas después de esa llamada devolvía
  vacío, dijera lo que dijera el fuente. Aislado comparando el mismo fixture en
  memoria (`[]`) y en disco (`["b"]`).
- `OnDemandGraphBuilder::build_for_symbol` emparejaba `Callers` con un brazo
  vacío y solo traversaba callers bajo `Both`, así que una consulta de callers
  devolvía siempre vacío. RED: `got [] right ["a"]`.

### Seguridad de los tests

- Las 16 aserciones `count >= 0` de `cognicode-core` eran ciertas por
  construcción. Cada una pasa a fijar el valor que su fixture implica, y los 9
  `allow(clippy::absurd_extreme_comparisons)` que las ocultaban —4 blankets y 5
  por ítem, todos etiquetados "documents intent"— se retiran. Clippy acepta las
  nueve eliminaciones, luego ninguna seguía guardando nada.
- Dos de esas aserciones ocultaban defectos: `analyze_impact` recibía un símbolo
  con un FQN que no coincidía con nada del grafo y devolvía cero dependientes
  bajo un comentario que afirmaba lo contrario; y el fixture de
  `suggest_context` no tenía ningún símbolo con `fan_in >= 2`, así que el
  handler no tenía hot path que devolver.
- `graph_benchmarks` cronometraba el builder con `let _ = result.entries.len()`
  bajo el comentario "usize is always >= 0, just verify the result exists". Un
  builder que no resolvía una sola arista daba un número limpio. Los tres sitios
  usan ahora `black_box` y el setup compartido fija las aristas del fixture
  fuera de todo bucle cronometrado.

### CI

- El orquestador pasa a ser PipelineK en solitario: `.github/workflows/` queda
  vacío, y `test_no_actions_workflows.py` es el invariante que lo dice. El
  merge-gate publica su propio veredicto porque PipelineK 0.46.0 no publica
  commit status, y el puente vive en `scripts/ci/publish-merge-gate.sh`.

## [v0.100.0] — 2026-10-01

**First tag since `v0.99.2`** (`37129dfd`). It publishes the whole
`v0.99.2..cee87547` window: **99 commits** — 56 `docs`, 13 `fix`, 8 `test`,
7 `feat`, 3 `ci`, 2 `chore`, the rest merges.

MINOR bump from v0.99.2 (rule 6). The window contains **7 `feat` commits and
no breaking change**, so the bump follows the `feat`s; `fix`, `test`, `ci`,
`chore` and `docs` accumulate inside it without triggering anything further.
Measured with:

```
git log --format='%s' v0.99.2..cee87547 \
  | sed -E 's/^([a-z]+)(\(.*\))?!?:.*/\1/' | sort | uniq -c | sort -rn
git log --format='%s%n%b' v0.99.2..cee87547 | grep -cE '^BREAKING[ -]CHANGE'   # 0
```

> **Correction to the earlier draft of this entry.** It was written at
> `ab931890` and justified the bump with `feat(cli): A-014 cognicode
> capabilities --format json` (commit `3adca737`), stating the window held
> "no feat commits besides the trigger above". Both claims were wrong for the
> released history: `3adca737` is not an ancestor of `cee87547` (it exists only
> on `docs/cp2-a012-closure`), and the window holds 7 `feat` commits, not 1.
> The MINOR conclusion was right; its stated reason was not, and a release
> entry must not cite a commit outside its own range. The lists below are
> rebuilt from the measured history rather than carried over.

### Added

The 7 `feat` commits, which are what make this MINOR. All are published
surface rather than internal CI work:

- **Generated product manifest contract** (`96d06854`).
- **Generated MCP tool catalog** (`85222f67`).
- **A-005 language and platform support matrices** (`73235889`).
- **A-006 public profile stability contract** (`f24609f0`).
- **OSS surface**: licence, community docs and templates (`95595af9`, CP1).
- **Read-only posture enforced rather than promised** (`b1d3e773`): the
  public profile's read-only claim is now enforced in core.
- **Derived output contracts for 15 MCP tools** (`dbd611e8`, A-012/CP2.3).

### Fixed

- **MCP fails closed on unknown tool authority** (`3e6c7fc2`, audit
  finding #5) — an unrecognised authority is now rejected rather than
  resolved. Pinned by `b70018cc` under a read-only profile.
- **`application_no_interface` now fires on the real boundary**
  (`bc62eba4`) — the architecture rule was passing vacuously.
- **CLI stderr silenced in capabilities JSON, and asserted**
  (`70132ae6`): the machine-readable document was carrying diagnostics.
- **Every published document is validated against its own schema**
  (`11eaa936`).
- **Two product `--check` modes no longer invalidate themselves**
  (`ae42b108` manifest, `ff60df36` generator) — the provenance stamp they
  wrote made the next run fail.
- **WASM shim `package.json` is versioned again** (`0eff2e6e`); a nested
  `.gitignore` had dropped it.
- **Contract suites no longer need pytest** (`c4c7d8e1`, `7d2eadf1`), and
  **the CR-08 suite selector can actually select** (`5de41cd8`).
- **Two CLI suites added by PR #309 were ungated** — no Kotlin stage
  covered them (`b5d8682c`).
- **Dead clippy allowlist removed** from `merge-gate.pipeline.kts`
  (`2a13925d`).
- **A roadmap claim that was false when written** (`11905210`).
- **PR #315** (`cee87547`): ST-01 closed, the rustdoc gate wired in, and
  the advisories gate left green. Details in its own commits below.

### Security and supply chain

- **`cargo deny check advisories` is green.** It had been failing on the
  yanked `yoke-derive 0.8.3` under `yanked = "deny"`, and it is a blocking
  step in `release.yml` and `release-validate.yml`. Bumped to 0.8.4. No
  policy waiver was added: `disable-yank-checking` had been considered under
  a wrong premise and was not needed.
- **`RUSTSEC-2023-0057` retired from the `deny.toml` ignore list.** It was
  dead — `cargo deny` reported `advisory-not-detected` because `libc`
  0.2.189 is past the affected range. Retired with a dated row rather than
  deleted silently.
- **Two advisory reasons that contradicted the advisory database**,
  including one describing bincode as a transitive 1.x dependency when the
  affected crate is the 2.0.1 the workspace uses directly.
- **`docs/debts/DEBT-SEC-001-advisory-ignores.md`** added, with one row per
  advisory ignore, plus `advisory_ignore_backing_contract`, which fails if
  an ignore has no row or a row exists for an ignore that is not listed.
  The previous `deny.toml` header pointed at a frozen PRF matrix containing
  none of those ids, so the file enforced a rule its own ignores had broken
  five times out of five.
- **Four advisory ignores remain active**, each with a per-ignore rationale
  inline in `deny.toml` and a backing row in the debt register:
  `RUSTSEC-2024-0384` (`instant` 0.1.13, unmaintained), `RUSTSEC-2026-0192`
  (`ttf-parser` 0.25.1, unmaintained), `RUSTSEC-2025-0141` (`bincode` 2.0.1,
  a direct dependency whose team ceased development) and `RUSTSEC-2024-0437`
  (`protobuf` 2.28.0). The last is a real vulnerability rather than an
  unmaintained crate; its fix requires an OpenTelemetry 0.27→0.28 migration
  and was deferred deliberately, with exposure limited to `/metrics`. It
  remains its own unit and is **not** fixed by this release.

### Fixed — regression caught by the newly wired gates

These existed at `v0.99.2` and were only found once the gates ran:

- **An intra-doc link broken by `4b7de348`**, which survived that commit
  and six more because nothing ran the gate that would have caught it.
- **`cargo fmt --check` failing on the branch**, introduced by two commits
  of the same branch, which would have failed the merge-gate.
- **`.gitignore` negations that granted nothing.** `docs/` was excluded as
  a directory, so every `!/docs/...` below it was inert, including the two
  lines that declared `docs/ROADMAP.md` a tracked entrypoint.

### Changed — CI and architecture

- **M0.11 rustdoc warning gate added** (`e4f7641a`) and wired into
  `pr-ci.yml` and `merge-gate.pipeline.kts`, closing a core parity gap
  (`1a87fbdb`, `1f38703c`). Its absence is why the broken intra-doc link
  above survived.
- **`cogh setup` gated** (`29a96c53`): it was already implemented and
  executed nothing.
- **clippy-gate parity pinned**, including the unbuilt `rig` surface
  (`50223fbc`).
- **Published tool authority pinned to the enforced one** (`57c21f87`).
- **OSS docs: every cited path must exist**, not just one (`00068420`).
- **A-013 lifecycle UAT in the merge gate** (`4ee9ca61`), 46 black-box MCP
  tests.
- **PipelineK raised to 0.43.0** (`95a75f8d`), the latest published, with
  the merge-gate expressed in Kotlin DSL against `origin/main`
  (`30047ec9`).
- **SDDK pack contract declares measured capabilities** (`f50072a7`).

### Verification

Measured on the release commit, not carried over from an earlier state:

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo deny check advisories` | ok |
| `cargo deny check licenses` | ok |
| `cargo build --workspace` | exit 0 |
| `cargo test --workspace --no-fail-fast` | **5843 passed / 0 failed / 30 ignored** |

The suite figure is a full-workspace run including doc-tests, counted after the
run exited. Doc-tests need a private `TMPDIR` on a shared machine: cargo clears
"orphaned" `rustdoctest*` directories under `TMPDIR` at startup, so concurrent
cargo runs delete each other's doctest argument files and produce spurious
`failed to load argument file: /tmp/rustdoctestXXXXXX/rustdoc-cfgs` failures.

## [v0.50.0 — v0.86.0] — 2026-07-22 → 2026-08-05 (reconstructed summary)

Period summary: 307 commits across 36 version tags (v0.50.0 → v0.86.0)
over ~2 weeks. Major program cycles: E12 (ViewKind Realization),
E13-wave2 (knowledge-layer ports), E14 (narrative runtime),
E22 (knowledge-layer), E28.x (conformance audit), E29 (PostgreSQL →
LadybugDB migration), and E30 (sandbox infra + release-gate framework).

Cycle-by-cycle highlights:

- **E12 ViewKind Realization** (v0.76.0): vertical_slice, seam_map,
  summary, source_view executors (#170)
- **E13-wave2 Knowledge-Layer Ports** (v0.86.0): AdrInspector + Ladybug
  stubs + e2e green (#226); Phase 1 of the knowledge-layer
- **E14 Narrative Runtime** (v0.83.0): EmbedResolver + wiring in 4
  narrative shapers (#217)
- **E14-C2** (v0.85.0): NarrativeStore port + LadybugDB adapter +
  runtime wiring (#225)
- **E22 Knowledge Layer** (v0.74.x): `feat(e22-knowledge-layer)` Phase 1
- **E28.x Conformance Audit** (v0.75.0): E28 program complete
- **E29 PostgreSQL → LadybugDB Migration** (v0.76.5 — v0.80.1):
  - 6/6 spike stages (S1 build, S2 schema-load, S3 concurrency,
    S4 crash-recovery, S5 latency, S6 cypher-compat)
  - Phase 0 (clean-ports + define-new-ports + refactor-call-sites)
  - Phase 1 (9/9 lbug ports in `cognicode-ladybug` crate)
  - Phase 1.5 (e29-7 full PostgreSQL removal: RuntimePorts DTO +
    bootstrap_with_backend canonical entry; zero `pg_repo` +
    zero `PostgresBackend`)
  - Phase 3 (e29-3 port abstraction audit + debt-e29-3-1)
  - ADR-029 (CallGraphProjectionPort seam) + ADR-030 (QualityStore
    lbug schema)
- **E30 Sandbox Infra** (v0.87.0 era, started v0.86): 6/6 containers
  with real digest pins + hardened quadlets + Maven wrapper

**Commit distribution** (v0.50.0..v0.86.0):
- 101 feat
- 52 docs
- 44 fix
- 41 refactor
- 27 Merge
- 14 test
- 9 chore
- 7 style

**ADRs authored** (17): 001, 002, 003, 004, 005, 006, 007, 008, 009, 010,
011, 012, 013, 014, 015 (×2 — see E31-C renumber), 016, 017, 018, 026,
027, 028, 029, 030, 031, 032, 033.

For granular commit history, see `git log v0.50.0..v0.86.0`.

## [v0.99.2] — 2026-09-27

> Release trail: see `3abdcc2a docs(release): v0.99.2 — human-readable
> release trail + SDDK-107 row` for the publish-time disclosure and the
> full set of authoritative commits in the v0.99.2 cycle.

Production-Ready programme closure (Post-PRF stabilisation,
operator directive 2026-09-27: "creamos release y archivado sddk").
SEMVER PATCH (rule 6): feat commits in the period are
CI/architecture-enforcement (QW-03, QW-04, CR-06, CR-08) per
the programme authorised 2026-09-26, not user-visible product
features, so 0.99.1 → 0.99.2.

### Fixed

- **M0.6 — `fix(parser): bump tree-sitter 0.24.7 → 0.27.0`**:
  resolves the `LanguageError(version)` mismatch that crashed
  `cognicode analyze` on PHP and Swift corpora.
  Commit `e2ee94ad`.
- **M0.10 — `fix(parser)` + `fix(walkers)`**: two-layer grammar
  drift in `tree_sitter_parser.rs` (function_node_type + identifier
  name) and `type_ref_walkers.rs` (PHP/Swift node names). Restores
  symbol extraction for PHP and Swift treesitter inputs.
  Commits `2becec6a` + `4eacab93`.
- **M0.13 — `test(release)` + `test(cli)`**: target-dir UAT
  mismatch fix (`~/.cargo/config.toml` overriding
  `target-dir = /var/home/rubentxu/cargo-targets` broke 7 UAT
  tests pinning `<repo_root>/target/release/`). Added
  `common::release_dir()` helper in CLI + MCP test harnesses to
  honour the resolved Cargo target-dir across all profiles.
  Commits `555ed54c` + `89fffd58`.

### Added

- **M0.12 — `test(graph)` + `test(analysis_service)`**:
  re-enabled 3 bounded `#[ignore]` tests with contract
  assertions (lightweight_index + on_demand_graph benchmarks +
  debug_call_relationships). Commits `b482a4ff` + `0132e260`.
- **QW-03 — `feat(ci)`**: bin-tracking guard becomes testable +
  integrated into PR-CI merge-gate. Commits `d781e846` +
  `47085b0d`.
- **QW-04 — `feat(ci)`**: contractual test wired into
  `release.yml` + `pr-ci.yml`. Commit `cc357018`.
- **CR-06 — `feat(architecture)`**: application_no_infrastructure
  + application_no_interface canonical constraints registered
  + historical drift frozen under `TemporaryException` mechanism.
  Commits `82c6d644` + `417f6c23`.
- **CR-08 — `feat(ci)`**: dorny/paths-filter wired into `pr-ci` +
  deterministic suite selector + 14 contractual tests.
  Commits `3a834ca5` + `cba2c6eb` + `cbb1d824`.
- **M0.10 acceptance — `test(core)`**: 6 end-to-end acceptance
  tests for PHP/Swift symbol extraction, exercising the public
  API path. Commit `a47cf419`.

### Performance

- **e91.W1 — `perf`**: honest `iterations`/`converged` semantics
  on graph_insights. Commits `6f40a08b` + `42a1ddcf`.
- **e91.W2 — `perf`**: PageRank characterisation. Commits
  `8b4bbe85` + `6b2738f3`.

### Documentation

- 25 commits across `docs/roadmap/JOURNAL.md`,
  `docs/roadmap/MAINTENANCE.md`, the production-ready
  `ROADMAP-ADDENDUM.md`, `EXECUTION-PLAN.md`, the three
  `PHASE-{1,2,3}-*.md`, the OpenSpec changes, and
  certification addenda. Highlights:
  - JOURNAL entries N+1..N+20 (recount of the
    production-ready programme work and post-M0 closures).
  - MAINTENANCE rows M0.1..M0.13 closed except M0.11
    (rustdoc audit, 3-5d, OPEN).
  - lessons 71..84 formalised.

### Pre-flight evidence

- `cargo test --workspace` = `5668 passed; 0 failed; 30 ignored`.
- `cargo fmt --check` exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings`
  exit 0.
- Release plan via `sddk release plan --tag v0.99.2 --route local`
  returns the 4-step route (push_main, verify_main_sha,
  create_annotated_tag, verify_remote_tag).
- `git push origin main` (61 commits fast-forward to
  `origin/main` = `37129dfd`).
- Annotated tag `v0.99.2` + `git push origin v0.99.2`
  (`origin/v0.99.2 = v0.99.2`).

### Known follow-ups (operator-gated)

1. **SDDK-107 — repair local ledger** — the local
   `~/.local/share/sddk/data/ledger.sqlite` is missing the
   `ledger_events` table; the active framework is 1.171.2 but
   the active binary is 1.145.1 and the local DB schema
   appears to be the older version. This blocks
   `sddk cycle start` (release apply chain itself ran via
   native git because the cycle record wasn't available for
   `sddk release vault` either).
2. **M0.11** — 147 `broken_intra_doc_links` warnings (3-5
   day estimation). Doc-only drift.
3. **F0.1** — already CLOSED (§42 ROADMAP) since 2026-09-25;
   appears PENDING in MAINTENANCE due to inventory drift
   between the two sources. Reconciliation note added.
4. **CR-01 firma humana contractual** — analog to C7. PENDING
   since 2026-09-25.

## [v1.0.0] — 2026-09-20

Operational cut closing the E31 program and INC-007. Tagged per ADR-031 §3
after 3 consecutive ALL-GREEN scorecards on Tier-1 read_source + search.

### INC-007 closure (R4 fix)

- **ground_truth.code.contains** (substring presence semantics): new optional
  field in `ExpectedCode` replacing Jaccard similarity for full-file reads.
  Modalities are mutually exclusive: `content` (legacy) vs `contains`
  (substring). Ambiguity and empty fragments are rejected explicitly with
  an `error` field on `CodeMatchResult`. Multi-file responses are checked
  per `expected.file`. Truncated responses without continuation do not
  satisfy the check.

- **Manifest switch**: 5 `tier1_h3_read_source.yaml` scenarios now use
  `contains:` (serde, ripgrep, anyhow, tokio, clap).

- **G4 GREEN** at 100.0 avg across 5/5 Tier-1 repos (30/30 candidates, 0
  unverified) — previously RED at 95.7 due to Jaccard vs full-file.

### Tier-1 closure + 5 new program_analysis tools (cycle 4, e65fbc8d)

- 4 new manifests (`e31b9..e31b12`): 181 unique scenarios, 52 tools,
  2274 LOC. Closes E31-B4 deferred item (47 partial Tier-1 tools) and
  the coverage_matrix 5-tool gap (now 73/73 covered).
- Tools added: `cfg_per_function`, `dominators_cfg`, `slice_forward`,
  `slice_backward`, `taint_flow`.

### LSI program foundation — M6 / M7.1 / M7.2 / M7.3 / M7.4 (cycles e36 → e65)

The Living Software Intelligence (LSI) program lands the evidence-kernel foundation
plus the reactive runtime boundary. Cycles e36-e65 close M0-M3, M5, M6 and M7.1-7.4
on the LSI roadmap (`docs/CogniCode_Living_Software_Intelligence/docs/roadmap/ROADMAP.md`).
M4 (semantic provider pipeline) was delivered in e39/e39.1 separately.

- **M0 / M1 — Baseline + Evidence Kernel Foundation** (e36): 47 tests on
  `domain::evidence_kernel` (snapshot-pinned reads, LLM-provenance rejection,
  per-`EventId` global sequence, fact round-trip, kernel idempotency).
- **M2 — Projection Bridge** (e37): `CallGraphProjection::from_facts`,
  `FactGenericGraphProjection` (multimodal-gated), 42/42 LSI goldens byte-identical,
  equivalence harness with multi-lang-types fixture (QUARANTINED 0.5969).
- **M3 — Stable Identity & Temporal Snapshots** (e38): continuity matcher
  (tier mechanics T0-T3, fail-closed ambiguity), rename-evidence adapter,
  identity-benchmark with 7 cases (line-shift 1.0000 / rename precision-recall 1.0000),
  workspace-isolation suite (collisions=0).
- **M5 — Program Analysis Core** (m5-*): 49 tests on `program_analysis::ast_lift`,
  6 algorithm IDs wired into MCP (`cfg_per_function`, `dominators_cfg`,
  `slice_forward`, `slice_backward`, `taint_flow`, `interproc_summary`),
  conformance harness, replay contract via SHA-256 digest.
- **M6 — Findings & Detector IR** (e52 → e62.4): Detector IR with fail-loud validator,
  3 paradigms (AST / Graph / Dataflow) sharing the same core, M6 contract hardening,
  authority verification, kernel snapshot correctness (U42 prerequisite), grounding refs
  and canonical grounding (U42 closed, 3 adversarial acceptance tests).
- **M7.1 — Intelligence Event Log** (e63, U50): causal log with namespaced kinds,
  bounded payloads (`Inline | Artifact{digest}`), no clock in the domain,
  cause-existence-before-reference invariant, all-or-nothing append,
  `InMemoryEventLog` with one global sequence + `EventId → WorkspaceId` index.
- **M7.2 / M7.3 — Execution identity + Behavior authority** (e64, U52):
  `ExecutionContext` unified (scope / actor / correlation / trigger_event);
  `DetectorExecutionRef.context`; `AnalysisInput.scope == context.scope` invariant;
  `BehaviorAuthorityPolicy` table; `BehaviorAdmission` → sealed `BehaviorPermit`;
  `AiGenerated` / `Imported` downgrade to `AgentBehavior` (a declaration never escalates);
  `policy.behavior_output_rejected` event on authority refusal.
- **M7.4 — Behavior budgets + observable exhaustion** (e65): `BudgetKind` enum
  (Time / EffectCount / FactVisits), `BudgetAuthorizer` runs AFTER authority
  (refusal does NOT decrement); `BehaviorRuntime` accepts `&dyn Clock` (Clock port
  in `application::behaviors`); `CausalRecorder::record_behavior_budget_exhausted`
  consumes `ExecutionContext` as a unit; `behavior.budget_exhausted` event with
  `caused_by = behavior.started`; FakeClock is local to the integration test
  (port is consumable from outside the crate). UAT-U52 acceptance: 6/6 green;
  deterministic counters vs temporal budget split; five-property atomicity
  contract (effect does NOT reach sink; `BehaviorOutcome::BudgetExhausted`;
  causal chain `[trigger, behavior.started, behavior.budget_exhausted]`;
  FactStore count unchanged). 79/79 scoped tests green; e64 / e63 / e62.4
  regressions clean.

### Pre-cut gates (per ADR-031 §3 + `docs/V1.0.0-PRE-CUT-CHECKLIST.md`)

- **Gate 1 (T7 stability cadence)**: pending — 5 consecutive nights CV < 10%
  required (`docs/TEST-PLAN.md` §6). Counter: TBD (requires nightly execution
  outside this PR cycle).
- **Gate 2 (E31-G scorecard streak)**: ✅ DONE — 3 consecutive ALL-GREEN
  scorecards on `tier1_h3_read_source.yaml` (campaigns 20260920T200759,
  20260920T201829, 20260920T201840 — 30/30 runs each, 100.0 avg G4 across
  5/5 Tier-1 repos, max CV 5.9%).
- **Gate 3 (G8 scalability)**: ✅ DONE — SCAL-001 / INC-004 documented
  (1G→4G container mitigation applied, per e30-metric-baseline).
- **Gate 4 (CHANGELOG)**: ✅ DONE — this entry.
- **Gate 5 (Roadmap reconciled)**: ✅ DONE — `docs/ROADMAP.md` lines 617-727
  reflect state as of 2026-08-16.
- **Gate 6 (Branch cleanup)**: ✅ DONE — 14 local + 15 remote stale branches
  pruned (per `sandbox/scripts/prune_stale_branches.sh`).
- **Gate 7 (Tag cut mechanics)**: this tag.

### Scorecard evidence

Three consecutive ALL-GREEN runs (post-INC-007 R4 fix):

- `sandbox/results/scorecard_run_20260920T200759.json` — 11 GREEN, 2 AMBER, 0 RED
- `sandbox/results/scorecard_run_20260920T201829.json` — 11 GREEN, 2 AMBER, 0 RED
- `sandbox/results/scorecard_run_20260920T201840.json` — 11 GREEN, 2 AMBER, 0 RED

The 2 AMBER per run are G5 (no call-graph/analytics/navigation data in
nightly cadence) and G8 (no g8-probe results) — both whitelisted in
`sandbox/scripts/scorecard_streak.py` per documented waivers
(v1.0.0-PRE-CUT-CHECKLIST §3).

## [v0.93.0] — 2026-08-11

Checkpoint release between E31 close (v0.92.0) and the operational v1.0.0
cut. MINOR bump justified by DEFECT-1's parameter-alias BC layer (public
surface change with backward-compat aliases) across 18 MCP tools.

### E31 program rollup closure

- **E31-E2** (`#254`): `retrieve_and_verify` CV 0.105 — ACCEPT (closes B1 deferred).
- **E31-B2-rollup** (`#255`): 178 Tier-3 scenarios quarantined via
  `known_quarantined.yaml` (closes B2); remote CI triggers
  (`push`/`pull_request`/`schedule`) disabled per E31-B5 user directive —
  `workflow_dispatch:` retained (closes B3).
- **E31-B4-rollup** (`#256`): Tier-1 closure round 3 — 8 more tools
  promoted to 5/5; T3 closure ~46.6%.
- **E31-B5-rollup** (commit `4d5f8bb6`): CHANGELOG.md v0.50–v0.86
  reconstruction (closes B5).
- **E31-B6-rollup** (`#258`): INC-001..004 closure as ACCEPT (closes B6).

### E32 distribution program (asdf-vm-inspired)

- **E32-A** (`#262`): `cogh` CLI binary core (install / list / current /
  latest / update / uninstall / plugin / reshim / doctor / where).
- **E32-B** (`#261`): plugin manifest + registry client + bundled plugins
  (mcp-server, skills-cognicode-core, sandbox-templates).
- **E32-C** (`#260`): portable skill bundles + `cogh skill validate`.
- **E32-D** (`#259`): opencode IDE adapter.
- **E32-E**: zcode IDE adapter.
- **E32-F**: claude IDE adapter.
- **E32-G** (`#264`): codex IDE adapter (TOML config).
- **E32-H** (`#265`): lifecycle integration tests.

### UAT defect closure (5 blockers flagged for v1.0.0)

- **DEFECT-1** (`#271`): `feat(mcp)` — parameter-alias BC layer
  (canonical → legacy naming) across 18 MCP tools. MINOR surface.
- **DEFECT-2** (`#268`): `test(mcp)` — build_graph `directory=.` and
  absolute path coverage.
- **DEFECT-3** (`#269`): `fix(mcp)` — `handle_smart_search` sub-handlers
  capped at 60s with graceful degradation.
- **DEFECT-4** (`#267`): `fix(uat)` — TC-1.3 path for requests fixture
  aligned to actual src-layout.
- **DEFECT-5** (`#270`): `fix(core)` — tree-sitter extractor honors
  `variable_types`; broader Rust variable shapes.

### Distribution deployment

- `chore(cogh)` (`#272`): bundled mcp-server bumped to v0.93.0 with real
  SHA256.
- `chore(cogh)` (`#273`): mcp-server manifest pointed at
  `Rubentxu/CogniCode/releases` (single-repo distribution).

## [v0.92.0] — 2026-08-10

- E30.5 release-gate carry-forwards: `score_smoke_matchers()` helper extracted in `sandbox-core/scoring.rs` (W-1 closed); `assert_family_consistency()` startup guard in `release_scorecard.py` (W-2a closed). Resolves the E30 program's 2 carry-forwards. Direct merge PR #236, merge commit `42fcdb14`.

## [v0.91.1] — 2026-08-07

- E30 release-gate: 12/12 nightly scorecards executed; G1-G12 baseline frozen; G5/G6/G8 measured; carry-forwards W-1 (smoke matchers in scoring) + W-2 (assert_family_consistency guard) trackeados. Note: tag downshifted from user-spec v0.91.0 to v0.91.1 due to existing v0.91.0 on origin. Direct merge PR #233.

## [v0.91.0] — 2026-08-06

- E30.4 conformance evidence: `openspec_conformance.py` harness with `--validate-paths` and `--evidence-map` (`sandbox/reports/evidence_map.yaml` 61 entries); G10 wired (verified 100.0% / triaged 100.0% on 433 reqs after legacy_obsolete exclusion per ADR-031 §4); 6/6 Tier-C specs marked OBSOLETE; SPEC delta sync (`openspec-conformance` + `release-readiness-gate`). PR #232 direct merge, v0.91.0 MINOR taggeado out-of-band.

## [v0.90.0] — 2026-08-06

- E30 conformance-audit: openspec conformance harness (`openspec_conformance.py` 133 LOC, 431 reqs / 67 specs / 4 phantom dirs), `evidence_map.yaml` (33 entries), `conformance_matrix.{yaml,md}` derived; scorecard gates G10/G11/G12 wired; CHANGELOG.md canonical (Keep a Changelog v0.87.0→v0.89.0 + Unreleased); branch pruning `prune_stale_branches.sh`; ADR-031 + ADR-032 → ACEPTADO. PR #231 direct merge, v0.90.0 MINOR taggeado.

## [v0.89.0] — 2026-08-06

- E30 Fase 3: `e30-metric-baseline` — primer Release Readiness Scorecard de 12 gates (6 GREEN / 3 AMBER / 3 RED), baseline de rendimiento congelado, 3 campañas full, stability.json (G6 CV < 5%), G8 probe (typescript tier-3 timeout → SCAL-001), límites de contenedor a 4G.

## [v0.88.1] — 2026-08-06

- Hotfix: `js_repos.yaml` / `ts_repos.yaml` con `pinned_sha` a SHAs exactos (deuda C3.3).

## [v0.88.0] — 2026-08-06

- E30 Fase 2: `e30-corpus-expansion` — G2 tool coverage 68/68 (denominador runtime real vía probe paginado), corpus +5 repos (tokio, clap Tier-1; rust-analyzer, TypeScript, react Tier-3), SHA-pinning 27 repos, coverage generator + scorecard, MCP-TOOLS.md regenerado, matchers count-only.

## [v0.87.1] — 2026-08-06

- `e30.1-clippy-baseline-reset` — 490 clippy errors → 0 (baseline reset por archivo), match arms duplicados eliminados (-1557 LOC), deuda sandbox cerrada, CI Format & Lint GREEN por primera vez.

## [v0.87.0] — 2026-08-06

- E30 Fase 0: `e30-sandbox-infra` — 6/6 quadlets activos con digests reales, hardening go.container, migración Maven (mvnw), workflow nightly, smoke lane exit 0.
