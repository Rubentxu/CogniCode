# CogniCode — Test Harness Evolutivo, Incremental y Reanudable

**Estado: núcleo implementado y gated; adopción como sustituto del gate, no.** El
paquete incluye un harness ejecutable (`bin/harness.py`, biblioteca estándar, sin
dependencias) con `plan`, `run`, `status`, `resume`, `explain` y `certify`, un
ledger de evidencia fuera del checkout y una suite de 60 tests que corre con el
runner de contratos del repositorio. Lo que **no** está hecho: paridad medida
contra CR-08, adopción en modo sombra dentro de las pipelines, ejecución en
paralelo, adaptador nextest y lanes de release. Está declarado más abajo, en
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
| Ver hallazgos y rastrear evidencia del repositorio | [Auditoría de código](docs/00-auditoria-codigo.md) y [rutas y referencias](docs/06-rutas-y-referencias.md). |
| Entender arquitectura, estados o migración | [Arquitectura](docs/01-arquitectura.md), [ontología](docs/02-ontologia-y-estados.md) y [compatibilidad](docs/03-migracion-y-compatibilidad.md). |
| Implementar selección, validez o reanudación | Specs [TH-S01](specs/TH-S01-seleccion.md), [TH-S02](specs/TH-S02-evidencia-invalidez.md) y [TH-S03](specs/TH-S03-ejecucion-resume.md). |
| Integrar adaptadores, gates u operación segura | Specs [TH-S04](specs/TH-S04-adaptadores.md), [TH-S05](specs/TH-S05-gates-release.md) y [TH-S06](specs/TH-S06-operacion-seguridad.md). |
| Consultar decisiones, contratos y entregas previstas | [ADRs](adrs/), [contratos JSON](contracts/README.md), [criterios](docs/04-metrica-y-criterios.md), [decisiones pendientes](docs/05-decision-pendientes.md) y [roadmap](roadmap/ROADMAP.md). |
| Entender el gate de merge y el reparto de obligaciones por lane | [Auditoría de `merge-gate`](docs/07-auditoria-merge-gate.md) (las 73 stages clasificadas y la medición antes/después de B1), [ADR-TH-005](adrs/ADR-TH-005-dueno-unico-por-suite.md) (una suite, un dueño) y [ADR-TH-006](adrs/ADR-TH-006-retirada-uat.md) (retirada de la clase UAT). |
| Aceptar el diseño o buscar falsos verdes | [AAT](aat/AAT.md). Es un plan/especificación, no un resultado ejecutado. El plan de UAT que acompañaba a este paquete se retiró el 2026-10-15 (ADR-TH-006). |
| Preparar instrucciones para un agente implementador | [Mandato del agente](prompts/MANDATO-AGENTE.md). |

El README sirve de índice. Las reglas normativas viven en las specs y ADRs; este mapa no las reemplaza.

Diagrama: [arquitectura](diagrams/arquitectura.mmd).

Manifiesto del paquete: [`PACKAGE-MANIFEST.json`](PACKAGE-MANIFEST.json) declara
`path`, `sha256` y `bytes` de cada fichero, se excluye a sí mismo y queda
obsoleto en cuanto se edita un documento sin recalcularlo: es el inventario con
el que se comprueba que el paquete entregado es el que se describe. Los ADR
vigentes son TH-001..TH-008 y este paquete no incluye ningún plan de UAT
desde el 2026-10-15 (ADR-TH-006).

## Uso

Los comandos existen y se ejecutan con la biblioteca estándar de Python; no hay
nada que instalar. Todos aceptan `--repo`, `--state-dir` y `--format text|json`.

```shell
# Qué hay que ejecutar y por qué, sin ejecutar nada.
python3 test-harness/bin/harness.py plan --lane change --base origin/main

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
verdad es el directorio.

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
python3 scripts/run_contract_tests.py test-harness/tests/test_*.py   # 75 tests, ~45 s
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

**Implementado y gated**: plan/run/status/resume/explain/certify; ChangeSet con
merge-base, working tree y untracked con `-z`; catálogo derivado de las pipelines;
impacto por `cargo metadata` con dependientes transitivos; huellas de entrada con
`policy_version`; ledger append-only con recibos inmutables, índice reconstruible
y lease por run; clasificación de procesos que no convierte un 101 en verde;
agregación de gate con huecos nombrados; 60 tests dependency-free y el contrato
del gate.

**No implementado, y no se afirma lo contrario**:

- **Sustituir a CR-08.** La paridad está medida sobre un corpus de 16 clases de
  cambio (0 debilidades sin declarar en `change`, `merge` e `integration`) y el
  contrato del gate la vuelve a medir en cada merge, pero el selector sigue siendo
  la autoridad y el harness no decide cobertura en la lane de merge.
- **Etapa de sombra en las pipelines.** El harness no añade ni quita etapas: corre
  como contrato, y ahí dentro se mide la paridad. Una etapa `plan --shadow` no
  añadiría evidencia nueva (ADR-TH-008).
- **Reutilización en lanes oficiales.** `merge` ejecuta todo por defecto;
  reutilizar exige `--allow-reuse`, y la paridad no está demostrada.
- **Ejecución en paralelo.** `--max-workers > 1` se rechaza con un motivo: sin
  locks por recurso y separación de suites seriales sería compartir estado a
  ciegas.
- **Adaptador nextest y lanes de release** (empaquetado, instalación,
  provenance): sin tocar.
- **AAT** sigue siendo especificación: hay reglas adoptadas (PASS vacuo, recibo
  roto, recibo no terminal, hueco declarado, reanudación tras interrupción) con
  test propio, pero el documento no está ejecutado de punta a punta.

**Límites conocidos del método**: la clausura por citas textuales no ve una ruta
construida por concatenación de cadenas, y por eso la lista de ficheros que se
tratan como potencialmente inertes es corta y explícita. La clausura de una suite
se corta a 4 000 ficheros y el plan avisa cuando lo hace. Los tests con `#[cfg]`
que quedan a cero por una feature no se distinguen todavía de un filtro que no
casa: los dos acaban en «recibo no bien formado», que es un hueco, no un verde.
