# CogniCode — Test Harness Evolutivo, Incremental y Reanudable

**Estado: núcleo implementado y gated; adopción como sustituto del gate, no.** El
paquete incluye un harness ejecutable (`bin/harness.py`, biblioteca estándar, sin
dependencias) con `plan`, `run`, `status`, `resume`, `explain` y `certify`, un
ledger de evidencia fuera del checkout y una suite de 127 tests que corre con el
runner de contratos del repositorio. También existe la **delegación opt-in** del
wrapper `scripts/test-fast.sh` (`TEST_FAST_ENGINE=harness`), con caída cerrada al
motor CR-08 cuando no puede cubrir la demanda. Lo que **no** está hecho: adopción
como motor por defecto del bucle local, sustitución de la selección del gate,
adaptador nextest y lanes de release. Está declarado más abajo, en
«Estado».

PipelineK sigue siendo la autoridad del gate (ADR-TH-003). El harness no
sustituye ninguna etapa: se ejecuta como un contrato más
(`scripts/ci/test_harness_contract.py`) dentro de la etapa de contratos del
merge-gate, de modo que no cambia qué bloquea un merge.

AAT sigue siendo especificación y no hay todavía mediciones de paridad con el
gate. La clase UAT se retiró del repositorio el 2026-10-15 por decisión del
operador (ADR-TH-006): este paquete ya no incluye un plan de UAT.

- **Fecha de auditoría**: 2026-10-15
- **Repositorio destino**: https://github.com/Rubentxu/CogniCode
- **Ubicación del paquete**: `test-harness/` en la raíz del worktree de evolución paralelo de CogniCode.

## Objetivo del diseño

El objetivo propuesto es reducir el tiempo de retroalimentación y evitar volver a ejecutar pruebas previamente superadas **solo si su evidencia continúa siendo válida** después de corregir una falla. La reducción de tiempo todavía no se ha medido.

El diseño busca preservar la certificación exhaustiva, la fiabilidad y la autoridad de **PipelineK**, y reunir la implementación futura del *harness* y su gobierno en un solo subárbol versionado.

## Propuesta: qué centralizar y qué no

| Bajo `test-harness/` | Permanece en ubicación contractual |
|---|---|
| Documentación, contratos, tests del harness y los componentes ya implementados (planificador, selector, ejecutores, adaptadores y ledger) | Tests Rust junto a sus crates; Cargo manifests; `AGENTS.md`; PipelineK como autoridad CI; pipelines y artefactos de release |
| Futuros adaptadores del sandbox y *drivers* de campañas | Manifiestos y fixtures históricos en `sandbox/` durante la migración, hasta probar compatibilidad |

En esta propuesta, centralizar significaría tener **un punto único de gobierno/ejecución**. No se moverían físicamente tests de crates por estética: Cargo los descubre según su modelo.

Las rutas antiguas solo se eliminarían cuando no quede consumidor y exista una prueba de migración.

## Navega por tarea

| Si necesitas... | Empieza aquí |
|---|---|
| **Ejecutar el harness o entender su implementación** | [Uso](#uso) y [ADR-TH-007](adrs/ADR-TH-007-implementacion-nucleo.md) (decisiones del núcleo: lenguaje, ledger, huellas, fail-closed). |
| **Comprobar que el harness no recorta cobertura** | [Paridad con el selector vigente](#paridad-con-el-selector-vigente) y [ADR-TH-008](adrs/ADR-TH-008-paridad-con-el-oraculo.md) (veredictos, desviaciones declaradas y medición). |
| **Concurrencia, inventario pinado y GC del ledger** | [Concurrencia, cerrojos y pin de inventario](#concurrencia-cerrojos-y-pin-de-inventario) y [ADR-TH-009](adrs/ADR-TH-009-supervisor-concurrencia-inventario-gc.md) (cerrojos, SIGKILL, lease huérfano). |
| **Ver qué mutaciones se matan de verdad** | [Ejecución del AAT](aat/AAT.md#ejecución-2026-10-15) y [ADR-TH-010](adrs/ADR-TH-010-aat-ejecutado.md) (estados, suelo de obligaciones, identidad e inestabilidad). |
| **Saber qué seleccionaría el harness en un PR real** | La etapa `harness-shadow-plan` del merge-gate (advisory, no decide) y [ADR-TH-012](adrs/ADR-TH-012-etapa-de-sombra.md): qué observa, qué no cambia y cómo se usan los datos. |
| Ver hallazgos y rastrear evidencia del repositorio | [Auditoría de código](docs/00-auditoria-codigo.md) y [rutas y referencias](docs/06-rutas-y-referencias.md). |
| Entender arquitectura, estados o migración | [Arquitectura](docs/01-arquitectura.md), [ontología](docs/02-ontologia-y-estados.md) y [compatibilidad](docs/03-migracion-y-compatibilidad.md). |
| Implementar selección, validez o reanudación | Specs [TH-S01](specs/TH-S01-seleccion.md), [TH-S02](specs/TH-S02-evidencia-invalidez.md) y [TH-S03](specs/TH-S03-ejecucion-resume.md). |
| Integrar adaptadores, gates u operación segura | Specs [TH-S04](specs/TH-S04-adaptadores.md), [TH-S05](specs/TH-S05-gates-release.md) y [TH-S06](specs/TH-S06-operacion-seguridad.md). |
| Consultar decisiones, contratos y entregas previstas | [ADRs](adrs/), [contratos JSON](contracts/README.md), [criterios](docs/04-metrica-y-criterios.md), [decisiones pendientes](docs/05-decision-pendientes.md) y [roadmap](roadmap/ROADMAP.md). |
| Usar el harness en el bucle local sin pagar el mini-gate | `--profile dev` (sólo lane `change`): ejecuta lo que cambió y lo que CR-08 exija, sin clausura de dependientes ni etapas de workspace, y con el suelo del oráculo intacto. Medido 2026-10-16: misma selección que CR-08 (1 suite), 29 s la primera pasada contra 28 s de CR-08 en caliente, y **3 s** la repetición (recibo reutilizado) contra 28 s. Decisión: [ADR-TH-015](adrs/ADR-TH-015-perfil-dev-del-bucle-local.md) |
| Entender el gate de merge y el reparto de obligaciones por lane | [Auditoría de `merge-gate`](docs/07-auditoria-merge-gate.md) (las 73 stages clasificadas y la medición antes/después de B1), [ADR-TH-005](adrs/ADR-TH-005-dueno-unico-por-suite.md) (una suite, un dueño) y [ADR-TH-006](adrs/ADR-TH-006-retirada-uat.md) (retirada de la clase UAT). |
| Aceptar el diseño o buscar falsos verdes | [AAT](aat/AAT.md): **21 de 30 mutaciones se matan hoy con test** y las 9 restantes están declaradas con motivo (sandbox, nextest, release). El plan de UAT que acompañaba a este paquete se retiró el 2026-10-15 (ADR-TH-006). |
| Preparar instrucciones para un agente implementador | [Mandato del agente](prompts/MANDATO-AGENTE.md). |

El README sirve de índice. Las reglas normativas viven en las specs y ADRs; este mapa no las reemplaza.

Diagrama: [arquitectura](diagrams/arquitectura.mmd).

Manifiesto del paquete: [`PACKAGE-MANIFEST.json`](PACKAGE-MANIFEST.json) declara
`path`, `sha256` y `bytes` de cada fichero, se excluye a sí mismo y queda
obsoleto en cuanto se edita un documento sin recalcularlo: es el inventario con
el que se comprueba que el paquete entregado es el que se describe. Los ADR
vigentes son TH-001..TH-014 y este paquete no incluye ningún plan de UAT
desde el 2026-10-15 (ADR-TH-006).

## Uso

Los comandos existen y se ejecutan con la biblioteca estándar de Python; no hay
nada que instalar. Todos aceptan `--repo`, `--state-dir` y `--format text|json`.

```shell
# Qué hay que ejecutar y por qué, sin ejecutar nada.
python3 test-harness/bin/harness.py plan --lane change --base origin/main
python3 test-harness/bin/harness.py plan --lane change --base origin/main --profile dev   # bucle local: sin clausura ni etapas de workspace

# Ejecuta el plan, escribe recibos y agrega el veredicto.
python3 test-harness/bin/harness.py run --lane change

# Estado del último run, y de uno concreto.
python3 test-harness/bin/harness.py status [--run-id <id>]

# Reanuda recalculando el plan con el snapshot de ahora.
python3 test-harness/bin/harness.py resume --run-id <id> [--mode continue|failed-only]

# Por qué una suite está en el plan, o por qué se queda fuera.
python3 test-harness/bin/harness.py explain --suite 'cognicode-core:lib#-'

# Certifica con evidencia fresca (no reutiliza nada, en ninguna lane).
python3 test-harness/bin/harness.py certify --lane certification --candidate <sha>

# Informe reproducible: perfil medido del gate y mapa de cobertura (W0).
python3 test-harness/bin/harness.py report --lane merge --pipelinek-db .pipelinek/merge-gate.db \
  --budget 420 --protect contracts --protect fmt --protect clippy

# Concurrencia explícita con cerrojos declarados (por defecto sigue siendo 1).
python3 test-harness/bin/harness.py run --lane merge --max-workers 4

# Pin del inventario de casos por suite, y recolección de logs del ledger.
python3 test-harness/bin/harness.py inventory --write     # deriva el pin de recibos reales
python3 test-harness/bin/harness.py inventory --check     # falla si alguna suite ejecuta menos
python3 test-harness/bin/harness.py gc --keep 20 --dry-run

# Compara la selección con el selector vigente (CR-08) sobre el cambio actual,
# o sobre el corpus de clases de cambio.
python3 test-harness/bin/harness.py parity --base origin/main
python3 test-harness/bin/harness.py parity --corpus --lane change

# Plan con el suelo del oráculo: nunca exige menos que el selector vigente.
python3 test-harness/bin/harness.py plan --lane merge --oracle cr08
```

Códigos de salida: `0` gate satisfecho, `1` obligaciones incumplidas, `2` error
de plan o de harness, `3` evidencia indeterminada o caducada, `130` interrupción.

`plan` imprime cada obligación con su motivo y cada exclusión con el suyo, y en
`--format json` devuelve `{"plan": …, "run_id": …, "excluded": [...], "warnings":
[…]}`: el documento `plan` cumple `contracts/plan.schema.json` tal cual, y los
extras de la CLI viajan al lado.

Política de reutilización, en un solo sitio (`src/harness/policy.py`):

| Lane | Reutiliza |
|---|---|
| `change` | sí, por defecto |
| `merge` | no por defecto; sí con `--allow-reuse` explícito |
| `integration`, `certification`, `release-candidate`, `release` | nunca |

`policy_version` entra en cada huella y en cada veredicto: cambiar la política
invalida la evidencia anterior por construcción.

## Ubicación del estado

Fuera del checkout, como propone la spec (TH-S02):
`$XDG_STATE_HOME/cognicode-test-harness/<repo-slug>-<repo-id>/` (por defecto
`~/.local/state/cognicode-test-harness/…`). El contrato del harness comprueba que
el repositorio no se toca.

```text
ledger.jsonl          eventos, solo se añade
receipts/<id>.json    recibos inmutables (un recibo RUNNING marca lo que se lanzó)
runs/<id>.json        estado del run: plan, decisiones, veredicto
logs/<run>/<suite>.log  salida completa, referenciada por su sha256
leases/<run>.lock     un solo escritor por run
index.json            vista derivada del directorio de recibos, reconstruible
```

Los recibos son inmutables: reescribir uno con contenido distinto es un error, no
una actualización. El índice se reconstruye solo si el directorio cambió, y la
verdad es el directorio. `gc` borra logs de runs fuera de la ventana conservada y
nunca toca recibos; un run en `RUNNING` se conserva siempre.

Un `kill -9` deja el fichero del lease detrás. Si el PID del dueño ya no existe y
el host es el mismo, el lease está huérfano y se reclama dejando evento
(`lease_reclaimed`); si el dueño vive, o viene de otra máquina, se rechaza.

## Concurrencia, cerrojos y pin de inventario

`--max-workers N` (tope 8) ejecuta suites en paralelo, con `default=1`: sin pedirlo
el harness es secuencial. Cada suite toma sus recursos declarados, y el directorio
de compilación de Cargo es un recurso compartido implícito (cargo lo bloquea de
todas formas; el paralelismo real hoy viene de las suites que no compilan).

Una suite marcada `serial` es una barrera de escritor: **nadie** corre mientras
ella corre. `serial` y `locks` no se pueden derivar del comando, así que se
declaran en [`config/suite-metadata.json`](config/suite-metadata.json) —versionado
y revisable— y el estado del run registra la concurrencia efectiva y los cerrojos
tomados.

`inventory.json` pina el número de casos esperado por suite, derivado de
ejecuciones reales (`inventory --write`). El mínimo entra en la huella de entradas,
invalida la reutilización de un recibo que ejecutó menos y hace que el gate no
acepte ese PASS: una suite que pierde pruebas por una feature apagada es un hueco
con nombre, no un verde. Sin pin, el suelo es 1 caso para Cargo y ninguno para los
scripts.

## Perfil del gate de merge y presupuesto

Medido en el ledger durable de PipelineK (run `1b37e37b`, 58 etapas, 1 630 s):
las diez etapas más caras suman el 94% y `rustdoc-gate` sola era 667 s (41%). Su
dueño es la lane que bloquea `main` —`merge-gate.pipeline.kts`, con stage nombrada y
contrato que exige dueño único— y volvió allí el 2026-10-16 tras medir el motivo de
ese coste: no era el chequeo sino el **directorio de target compartido**, que impedía
a `cargo doc` reutilizar su caché. Con un directorio propio cuesta 124 s en frío,
**1 s** en régimen estable y ~70 s tras tocar `cognicode-core`
([ADR-TH-014](adrs/ADR-TH-014-rustdoc-vuelve-y-el-coste-era-el-target.md), que
enmienda [ADR-TH-011](adrs/ADR-TH-011-dueno-unico-del-gate-rustdoc.md)). En el mismo
recorrido se corrigió un defecto que gravaba **cualquier** compilación en un
worktree: `crates/cognicode-cli/build.rs` declaraba
`rerun-if-changed=../../.git/HEAD`, ruta que no existe cuando `.git` es un fichero, y
cargo recompilaba el crate en cada invocación (`cargo doc` repetido: 163 s → 1 s, 0
unidades `Dirty`); lo vigila `scripts/ci/test_build_inputs_exist.py`.

El baseline medido queda versionado en
[`docs/08-baseline-tiempos.md`](docs/08-baseline-tiempos.md), generado con
`report --write-baseline` (lleva fecha, commit y el comando para regenerarlo). La
etapa de sombra deja además una observación `plan_observed` en el ledger por cada
merge, y `report` la agrega (cuántas obligaciones exige la selección de media),
para que la adopción se decida con PRs reales y no con el corpus solo.

`harness report` responde a esa pregunta con aritmética declarada sobre el ledger
real del orquestador: con `contracts`, `fmt`, `clippy` y la observación
protegidas, **dejar de ejecutar tres etapas de test** (`cli-unit-tests` 62 s,
`cli-integration-suites` 44 s, `core-unrestricted-evidence-kernel` 38 s) llevaría
el gate de 550 s a **406 s**, dentro del presupuesto de 420 s. El informe no
decide nada: identifica qué habría que mover y cuánto falta si no cabe.

## Etapa de sombra en el gate

`merge-gate.pipeline.kts` ejecuta `harness.py plan --lane merge --base origin/main
--oracle cr08 --shadow` en una etapa **advisory**: imprime el resumen (obligaciones
de la lane, cuántas exige el cambio, cuáles quedan bloqueadas por estado externo y
qué dijo el oráculo) y no puede fallar la lane, porque PipelineK no tiene
`continue-on-error` y el comando termina en `|| echo 'ADVISORY: …'`. No ejecuta
suites ni escribe en el repositorio. El contrato comprueba que existe, que es
advisory y que solo vive en la lane de merge.

## Suelo de obligaciones críticas

`obligations.json` lista el mínimo de obligaciones que la lane de merge (y las
demás) **no pueden perder**: si borrar una etapa del gate dejara el catálogo sin
`cargo clippy`, `--doc`, los contratos o las suites ancla, el plan pasa a
`UNKNOWN_INPUT` con el nombre de la que falta y el veredicto no puede ser
`SATISFIED`. No sustituye al catálogo —que se sigue leyendo de las pipelines— ni
añade obligaciones: solo impide que desaparezcan en silencio.

## Restricciones no negociables

- **Fail-closed:** missing, stale, partial, cancelled, quarantined y unknown **no equivalen** a PASS.
- Un PASS anterior es reutilizable únicamente si la función de validez devuelve `Reusable` y se emite una **justificación trazable**.
- `cargo test --workspace --all-targets` no ejecuta doctests; la matriz debe conservar `--doc` donde corresponda. `cargo test --workspace` (sin targets) sí considera doctests.
- Nextest `-R` rerun experimental **no invalida automáticamente** por cambios de código; nunca constituye por sí solo la prueba de vigencia.
- No sobrescribir la configuración global Cargo, no aplicar `CARGO_BUILD_JOBS` ni `[profile.test]` a todo el workspace sin autorización, y no destruir `CARGO_TARGET_DIR` compartido.
- No introducir GitHub Actions. PipelineK sigue siendo la autoridad vigente en `docs/adr/ADR-CI-ORCHESTRATOR-CUTOVER.md`.
- Preservar los recibos y los artefactos exactos del candidato release; publicar únicamente bytes verificados. Las instalaciones desde los tarballs finales requieren una instalación real, no una simulada.
- No reabrir PRF histórico; ubicar cada avance en el roadmap Post-PRF existente y conservar la disciplina SDDK de `AGENTS.md`.

## Qué decide la selección

La fuente de obligaciones son las pipelines: el catálogo se lee de los
`*.pipeline.kts` (los `cargo test`, los `cargo fmt/clippy/deny/build` y los
`bash scripts/…`), y los ids se derivan del comando observado. No hay una lista
paralela de suites que mantener a mano.

Clasificación de un cambio:

| El cambio toca… | Consecuencia |
|---|---|
| un manifest, `Cargo.lock`, toolchain, `.cargo/`, `scripts/ci/`, `sandbox/`, un `*.pipeline.kts`, un `build.rs` | **todo** es obligatorio: ninguna clausura fina lo demuestra |
| una ruta que no pertenece a ningún paquete del workspace | **todo** es obligatorio (fail-closed, con la ruta nombrada) |
| un fichero de test eliminado en la rama | **todo** es obligatorio: con él se fue su cobertura |
| código de un paquete | ese paquete y todos sus dependientes transitivos (según `cargo metadata`) |
| `docs/**` o documentación de raíz (`AGENTS.md`, `README.md`, …) | solo los paquetes y scripts que **citan** ese fichero; si nadie lo cita, no se exige nada |
| una suite que el gate marca `|| echo 'ADVISORY: …'` | se declara excluida: no bloquea, así que no exige prueba |
| una obligación con estado externo (sandbox, contenedores, `cargo deny`) | se declara `BLOCK`, y el plan queda indeterminado |

Sin ChangeSet fiable (sin base, sin merge-base, historia sin ancestro común) o sin
grafo de paquetes, el plan es `UNKNOWN_INPUT` con el motivo escrito y **todas** las
obligaciones requeridas: un plan vacío por accidente sería un gate que aprueba sin
mirar.

Medición de referencia (2026-10-15, clon del repositorio, cambio de una línea en
`crates/cognicode-ladybug/src/lib.rs`): `plan` tarda 2,5 s, exige 27 de las 39
obligaciones de la lane `change` y excluye 12 con motivo, incluidas las 8 suites
de `cognicode-core` y 4 scripts. Los 27 requeridos incluyen `cli`, `mcp` y
`runtime` porque dependen de `cognicode-ladybug`, no por precaución.

## Paridad con el selector vigente

CR-08 (`scripts/ci/select-suites.sh`) sigue siendo la autoridad mientras no se
demuestre paridad, así que el harness no lo sustituye: lo compara. `harness
parity` traduce cada scope del oráculo a su paquete y clasifica la diferencia:

| Veredicto | Significado |
|---|---|
| `AGREE` | El harness exige al menos lo mismo que el oráculo |
| `HARNESS_STRICTER` | El harness exige más; el oráculo iba corto |
| `DECLARED_DEVIATION` | El harness exige menos en una clase declarada y razonada |
| `HARNESS_WEAKER` | El harness exige menos sin justificación: es un fallo, no un matiz |
| `ORACLE_UNKNOWN` | El oráculo no se pudo leer: no se puede afirmar paridad |

Medición del corpus (2026-10-15, repositorio real, 16 clases de cambio):

| Lane | `AGREE` | `HARNESS_STRICTER` | `DECLARED_DEVIATION` | `HARNESS_WEAKER` |
|---|---|---|---|---|
| `change` | 9 | 5 | 2 | 0 |
| `merge` | 9 | 5 | 2 | 0 |
| `integration` | 9 | 5 | 2 | 0 |

Las dos desviaciones declaradas son `docs/**` (CR-08 exige `ladybug` para
cualquier documento; el harness exige los paquetes que lo citan) y la caída a
ALL del oráculo para rutas que su lista de reglas no menciona, aceptada solo
cuando el harness tiene clasificación propia de esos paths. La comparación vive
dentro del contrato del gate, así que un recorte de cobertura sin justificar hace
caer la etapa de contratos.

La primera medición de paridad encontró tres diferencias reales y **dos bugs del
harness**, los dos del tipo que produce falsos verdes: el presupuesto del barrido
de citas se agotaba y devolvía «nadie cita» en falso, y la atribución se quedaba
con la primera cita y perdía paquetes. Los dos están corregidos, con test.

Acoplamientos: CR-08 revalida `core` cuando cambian `mcp` o `explorer` (contrato
compartido). El harness copia esa regla mientras no demuestre lo contrario, y
`plan --oracle cr08` añade además cualquier obligación que el oráculo pida y la
selección por impacto no cubra.

## Pruebas del harness y cómo se gated

```shell
python3 scripts/run_contract_tests.py test-harness/tests/test_*.py   # 127 tests, ~70 s
python3 scripts/ci/test_harness_contract.py                          # contrato del gate
```

La suite del harness no usa pytest a propósito: el runner de CI no lo instala, y
una suite que solo corre en la máquina del mantenedor no la ejecuta nadie. Se
ejecuta con el mismo `scripts/run_contract_tests.py` que el resto de contratos y
se fabrica repositorios de verdad (`git init` + workspace de Cargo) en directorios
temporales.

El contrato del gate (`scripts/ci/test_harness_contract.py`, dentro de la etapa de
contratos de `merge-gate`) fija cuatro cosas: la suite corre entera y no pasa en
vacío; el plan valida contra `plan.schema.json` y la indeterminación se declara;
el harness no escribe dentro del repositorio; y un hueco declarado no se convierte
en PASS. Además comprueba que el harness importa solo biblioteca estándar, que es
lo que le permite correr en el gate sin instalar nada.

## Estado

**Implementado y gated**: plan/run/status/resume/explain/certify/parity/inventory/gc;
ChangeSet con merge-base, working tree y untracked con `-z`; catálogo derivado de
las pipelines con metadatos declarados (serial, locks); impacto por
`cargo metadata` con dependientes transitivos; huellas de entrada con
`policy_version` y mínimo de casos pinado; ledger append-only con recibos
inmutables, índice reconstruible, lease con reconciliación de dueños muertos y GC
que conserva evidencia; supervisor con concurrencia, cerrojos y SIGKILL probado;
clasificación de procesos que no convierte un 101 en verde; agregación de gate con
huecos nombrados; suelo de obligaciones críticas; AAT ejecutado (21/30 filas con
test, 9 declaradas); etapa de sombra advisory en el merge-gate que publica el plan
real de cada PR; delegación opt-in del wrapper `fast` con paridad contrastada
contra el plan real (ADR-TH-013); 127 tests dependency-free y el contrato del gate.

**No implementado, y no se afirma lo contrario**:

- **Sustituir a CR-08.** La paridad está medida sobre un corpus de 16 clases de
  cambio (0 debilidades sin declarar en `change`, `merge` e `integration`) y el
  contrato del gate la vuelve a medir en cada merge, pero el selector sigue siendo
  la autoridad y el harness no decide cobertura en la lane de merge.
- **Adopción de la selección en el gate.** La etapa de sombra ya existe
  ([ADR-TH-012](adrs/ADR-TH-012-etapa-de-sombra.md)): cada merge deja en su log el
  plan real (obligaciones de la lane, requeridas por impacto, bloqueadas). Lo que
  sigue sin decidirse es **qué etapas pasan a ejecutarse por impacto**; eso exige su
  propio ADR con esos datos delante.
- **Reutilización en lanes oficiales.** `merge` ejecuta todo por defecto;
  reutilizar exige `--allow-reuse`, y la paridad no está demostrada.
- **Paralelismo de compilación.** `--max-workers` reparte suites con cerrojos, pero
  los `cargo test` comparten target dir y se serializan en la práctica; paralelizar
  de verdad exige un target dir por worker y medir si compensa.
- **Adaptador nextest y lanes de release** (empaquetado, instalación,
  provenance): sin tocar.
- **AAT sin ejecutar del todo**: 21 de 30 filas se matan con test; 2 son `PARTIAL`
  (el mecanismo está probado, la fila pide además sandbox o servicio externo) y 7
  son `NOT_RUN` con motivo (sandbox, nextest, release, `known_failures`). No se
  cuenta como cobertura el AAT que no se ejecuta.

**Límites conocidos del método**: la clausura por citas textuales no ve una ruta
construida por concatenación de cadenas, y por eso la lista de ficheros que se
tratan como potencialmente inertes es corta y explícita. La clausura de una suite
se corta a 4 000 ficheros y el plan avisa cuando lo hace. Los tests con `#[cfg]`
que quedan a cero por una feature no se distinguen todavía de un filtro que no
casa: los dos acaban en «recibo no bien formado», que es un hueco, no un verde.
