# ADR-057 — Una etapa que no puede hacer caer la puerta tiene que decir qué observó

**Estado**: EJECUTADO (decidido y aplicado el 2026-10-24; verificado en el contrato y en la suite)
**Fecha**: 2026-10-24
**Decisores**: Maintainer
**Contexto**: H27 · las puertas y lo que no puede hacerlas caer (`integration.pipeline.kts`, `certification.pipeline.kts`, `release-candidate.pipeline.kts`, `release.pipeline.kts`), candidato `C37` de la cola de la misión

---

## Contexto

### Lo que el informe externo propone

La auditoría del 2026-10-24, en su hito H5 —«Certificación ligera y real»—, pide una
clasificación estricta: pruebas de dominio, contratos de interfaz, pruebas UI
simuladas y pruebas reales desde el binario hasta la respuesta, «cada garantía
tendría un único dueño». Su criterio de cierre es «dos o tres escenarios críticos
ejecutados contra binarios actuales o publicados; eliminación de los E2E
temporales al cerrar».

### Lo que el árbol ya tiene

**MEDIDO 2026-10-24.** La mayor parte de H5 **ya está hecha**, y el informe no lo vio
porque leyó el repositorio sin ejecutarlo:

- **La clasificación de las superficies existe.** `scripts/ci/test_every_test_surface_has_a_lane.py`
  declara cada superficie del repositorio en uno de dos mundos: tiene un carril que
  la corre de verdad, o no lo tiene y dice por qué y cuántos son. Los 41 specs de
  `apps/explorer-ui/e2e/` están en el segundo mundo, con el motivo escrito: «los
  specs quedan como material de referencia y no se presentan como una garantía». Los
  recuentos se comprueban, así que una superficie no puede crecer en silencio.
- **El circuito real existe.** `scripts/ci/release-install-smoke.sh` (instalar y
  operar el tarball publicado), `binary-smoke-$target`
  (`release-candidate.pipeline.kts:739`), `core-b6-w1-release-stdio-smoke` (`:851`,
  JSON-RPC `initialize` + `tools/list` por stdio contra el binario release),
  `cogh_cli.rs` (binario `cogh` real con `--home` aislado) y
  `cp1_control_plane_endpoint.rs:669` (HTTP real contra el router en un puerto
  efímero). El criterio del informe —«dos o tres escenarios críticos contra
  binarios actuales o publicados»— **se cumple hoy**.
- **«Eliminación de los E2E temporales» no tiene objeto.** Cero coincidencias de
  `temporal`, `temporary` o `wip` en `apps/explorer-ui/e2e/`.

### Lo que sí falta: el dueño de lo que no bloquea

**MEDIDO 2026-10-24.** Trece etapas se ejecutan y **no pueden hacer caer la puerta**.
Once de ellas se marcan a sí mismas como `ADVISORY`; dos se tragan un fallo con
`|| echo` sin declarar nada.

| Pipeline | Etapa | Cómo traga el fallo |
|---|---|---|
| `integration.pipeline.kts` | `parser-adversarial` | `\|\| echo 'ADVISORY: …'` |
| `integration.pipeline.kts` | `architecture-drift-e2e` | `\|\| echo 'ADVISORY: …'` |
| `integration.pipeline.kts` | `findings-canonical-grounding` | `\|\| echo 'ADVISORY: …'` |
| `integration.pipeline.kts` | `workspace-isolation` | `\|\| echo 'ADVISORY: …'` |
| `integration.pipeline.kts` | `cogh-clean-home-install` | `\|\| echo "ADVISORY: …"` |
| `integration.pipeline.kts` | `benchmarks` | `\|\| echo 'ADVISORY: …'` |
| `integration.pipeline.kts` | `version-sync` | `\|\| echo 'ADVISORY: …'` |
| `certification.pipeline.kts` | `sandbox-pull` | `\|\| echo 'ADVISORY: …'` |
| `certification.pipeline.kts` | `sandbox-setup` | `\|\| echo 'ADVISORY: …'` |
| `certification.pipeline.kts` | `sandbox-ci-probe` | `\|\| echo 'ADVISORY: …'` |
| `certification.pipeline.kts` | `perf-budget-verdict` | `set +e` y su propio `ADVISORY:` |
| `release-candidate.pipeline.kts` | `install-cargo-deny` | `\|\| echo` sin marcar |
| `release.pipeline.kts` | `create-draft` | `\|\| echo` sin marcar |

El mecanismo es `|| echo 'ADVISORY: …'`, porque PipelineK no tiene
`continue-on-error`. **Ningún contrato las declara**: la única vez que la palabra
`ADVISORY` aparece en `scripts/ci/` es en un probe y en un guard de aislamiento de
build. Una etapa que falla y no bloquea se declara en la prosa del pipeline y en
ningún sitio más, así que nadie puede decir qué vale.

**Corrección de una cuenta propia.** La primera redacción de este ADR decía «diez
etapas» y las contaba por el idiom `|| echo`. La medición por cuerpo —el marcador
`ADVISORY` vive dentro del argumento de `sh(...)`, no en la línea donde empieza— da
**trece**: once se tragan el fallo y se declaran solas, y dos se lo tragan sin
decir nada. La undécima etapa que faltaba, `perf-budget-verdict`, no usa `|| echo`:
usa `set +e`, captura el código de salida y emite su propio `ADVISORY:`. Contar
por el idiom era contar por la forma; el contrato cuenta por el efecto.

Y el motivo que las justifica cita ficheros que **ya no están en el repositorio**:
`integration.pipeline.kts` dice que «tres jobs en `ci.yml` llevan
`continue-on-error: true»`, y `certification.pipeline.kts` razona sobre
`sandbox-nightly.yml`. No hay `ci.yml` ni `sandbox-nightly.yml` en este repositorio
—la casa migró de GitHub Actions a PipelineK—; los únicos `ci.yml` que existen están
en los checkouts de terceros de `sandbox/repos/`, que el propio house excluye porque
no son superficies suyas. El motivo heredado describe un mundo que ya no existe.

### Y una etapa que no hace lo que su nombre dice

**MEDIDO 2026-10-24.** `cogh install --profile core` es un **dry-run**: imprime el
plan y dice «re-run with `--apply` to perform the install», con código de salida 0.
La etapa `cogh-clean-home-install` ejecuta exactamente eso y termina con el eco
«clean-HOME install attempted». La etapa que se llama «instalación en HOME limpio»
**no instala**: comprueba que el planificador sabe dónde instalaría.

Y no es que no pueda. `cogh install --profile core --apply` funciona sin red, con
código 0, `doctor_pass: true`, sha256 del artefacto y el binario real en
`$HOME/.local/share/cognicode/bin/`. La clase de pruebas UAT que ADR-D1-006 retiró
—«distribución desde bytes release»— sólo tiene hoy un sustituto: este script de
shell en la lane de release, y una etapa que planifica.

**MEDIDO 2026-10-24, y es un segundo defecto.** La raíz de instalación no es la que
la etapa cree. `resolve_install_root` (`crates/cognicode-cli/src/lifecycle.rs:220`)
resuelve en este orden: `$COGNICODE_BIN_DIR`, luego `$CARGO_HOME`, luego
`$XDG_DATA_HOME/cognicode`, luego `~/.local/share/cognicode`. La etapa fija `HOME` y
`XDG_DATA_HOME` pero **no** fija `$CARGO_HOME`, y `CARGO_HOME` está puesto en un
entorno de CI. Medido con el binario release:

```
HOME limpio, sin CARGO_HOME   -> <HOME>/.local/share/cognicode/bin/cogh
HOME limpio, con CARGO_HOME   -> <CARGO_HOME>/bin/cogh
```

Es decir: con `--apply` y sin más, la etapa instala en el `CARGO_HOME` de la
máquina y **no** en el HOME limpio que su nombre promete. En un runner donde
`CARGO_HOME` es compartido, la etapa escribe fuera de su sandbox; en uno donde no
es escribible, falla con un error de permisos que `|| echo` se traga y que nadie
lee. La etapa declara «clean-HOME» y no lo cumple.

No se puede arreglar con `unset CARGO_HOME`: `scripts/ci/target-dir.sh` —el
resolutor que la etapa usa para encontrar el binario— consulta a `cargo metadata`,
y `cargo` lee su configuración de usuario de `$CARGO_HOME`. Sin `CARGO_HOME` y con
el `HOME` sustituido, el target dir resuelto sería otro y la etapa no encontraría
el binario que quiere instalar. La raíz se fija, no se unsetea.

## Decisión

1. **Una etapa que no puede hacer caer la puerta declara qué observa y por qué se
   queda así.** La declaración vive en un contrato, con su recuento comprobado, en la
   forma que el house ya usa para las superficies sin carril: una tabla, un motivo y
   un número que se comprueba. Trece etapas, trece motivos.

2. **Tragar un fallo obliga a declararse.** El marcador `ADVISORY` es el mecanismo de
   declaración de la casa, y el contrato lo exige de toda etapa que traga un fallo:
   si el cuerpo usa `|| echo`, la etapa se marca. Las dos etapas que se tragan un
   fallo sin decir nada dejan de hacerlo.

3. **La declaración no cita ficheros que no están en el árbol.** El motivo de una
   etapa tiene que ser verificable donde vive: si cita un fichero, el fichero
   existe.

4. **Una etapa cuyo nombre dice que instala, instala.** La stage de HOME limpio
   instala de verdad, con `--apply`, y fija la raíz de instalación dentro de su
   propio HOME, porque la raíz por defecto la desvía el entorno.

5. **Un contrato de CI es un test.** El T6 —la regla que exige que una
   corrección lleve un test— no veía la suite de contratos de la casa: sus
   patrones sólo conocían el test en Rust, el sandbox y el navegador. Una
   corrección de CI cuyo único test es un contrato no podía pasar la regla
   que dice que una corrección lleva un test. La puerta no era ciega al test:
   era ciega a una parte de la suite. Se corrige, y la corrección es de las
   reglas, no de los patrones: el T6 suma el nivel L5.

## Alternativas consideradas

**Convertir las trece etapas en puertas.** El argumento del house —«convertirlas en
puertas endurecería CI en vez de moverlo»— sigue siendo cierto para `benchmarks`
(miden tiempo) y para las tres del sandbox (necesitan red y clonar terceros), y
medir hoy no las hace más fiables: una suite adversarial que pasa hoy puede ser roja
mañana por el entorno, y una puerta que se pone roja por el entorno se apaga. Lo que
puede medirse es **qué observa cada una**, y eso es lo que se declara.

**Borrarlas.** Se pierde la observación: `parser-adversarial` es el corpus que
ataca al parser, y `workspace-isolation` es lo que garantiza que dos espacios de
trabajo no colisionan. Borrar un control para que el informe deje de verlo es
cambiar el producto para que la auditoría quede satisfecha.

**Dejarlo en prosa.** Es el estado actual: el motivo está en el comentario del
pipeline, que nadie lee como una declaración y que ya cita ficheros inexistentes.

**Declarar solo las once que se marcan.** Habría dejado dos etapas tragándose un
fallo en silencio, que es exactamente el hueco que este contrato cierra: la
declaración del fallo, no la forma del marcador.

## Consecuencias

**Positivas.**

- Las trece etapas que no bloquean quedan con un dueño declarado y un recuento
  comprobado: una etapa nueva que se marque advisory sin declararse pone el
  contrato en rojo, y una etapa que trague un fallo sin declararse también.
- La stage de HOME limpio comprueba de verdad lo que su nombre dice —instala, con
  `--apply`, y en su propio HOME— y con ella la garantía de «distribución desde
  bytes release» que ADR-D1-006 dejó sin suite.
- El motivo heredado que citaba `ci.yml` desaparece: la razón que se conserva es la
  que se puede comprobar en este árbol.
- El T6 puede ver el test de una corrección de CI. Antes, una corrección de CI
  cuyo único test es un contrato era indistinguible de una corrección sin test.

**Negativas.**

- El contrato tiene que mantener una tabla. Es el mismo precio que paga
  `test_every_test_surface_has_a_lane.py`, y es un precio que la casa ya ha
  aceptado por la misma razón.
- La stage de HOME limpio instala de verdad: escribe en el HOME temporal que ella
  misma declara, y su duración crece lo que cuesta un binario.
- La stage de crear el borrador deja de usar `|| echo` para el título de las notas y
  lo dice en una rama propia, para que el marcador no se mezcle con el valor que se
  publica.

**Lo que esta unidad NO hace.**

- No convierte ninguna etapa en puerta.
- No toca los Playwright: ya están declarados como material de referencia.
- No reconstruye el circuito real: ya existe.
- No reescribe la historia de los pipelines que cuentan de dónde salieron: la regla
  de citas comprobables alcanza al motivo de una etapa, no a la genealogía de una
  lane.

## Ejecución

**La declaración.** `scripts/ci/test_advisory_stages_are_declared.py` declara las trece
etapas con su motivo y su recuento por lane, y el recuento se comprueba contra el árbol:
siete en `integration.pipeline.kts`, cuatro en `certification.pipeline.kts`, una en
`release-candidate.pipeline.kts` y una en `release.pipeline.kts`. La regla que sostiene
no es «las etapas que están marcadas», sino **«las que tragan un fallo»**, y el criterio
es el efecto y no la forma: una etapa se traga el fallo si convierte un error en un
código de salida cero.

**El RED, medido antes del arreglo.** Cuatro comprobaciones fallaron, con su motivo
literal:

- `release-candidate.pipeline.kts: install-cargo-deny` y `release.pipeline.kts:
  create-draft` — «se tragan un fallo y no lo declaran».
- «la etapa que se llama «instalación en HOME limpio» no pidió la instalación: invocó al
  instalador con `['install', '--profile', 'core']`».
- «la etapa declara un HOME limpio pero la instalación caería fuera: la raíz elegida
  fue `` y el HOME de la etapa es `/tmp/cognicode-test-home-1000`».
- «la prosa que justifica estas etapas cita ficheros que no existen:
  `integration.pipeline.kts: parser-adversarial cita `ci.yml`».

Los controles del contrato son los que hacen que el verde signifique algo: el
inventario tiene que encontrar etapas, una etapa que traga un fallo y no se declara
tiene que salir en rojo, un motivo que cita un fichero ausente tiene que salir en rojo,
y la comprobación de la instalación tiene que distinguir una etapa que instala de una
que sólo planifica.

**La instalación.** La etapa de HOME limpio pide la instalación de verdad, con la
opción de aplicar, y fija la raíz dentro de su propio HOME. Medido con el binario
release, antes y después:

```
antes, con CARGO_HOME en el entorno   -> <CARGO_HOME>/bin/cogh
después, con la raíz fijada            -> <HOME>/.local/share/cognicode/bin/cogh
```

`doctor_pass: true`, sha256 del artefacto y el `CARGO_HOME` del entorno sin tocar.

**La prosa.** Siete citas a ficheros que no existen, eliminadas: cinco a `ci.yml` en
`integration.pipeline.kts` y dos a `sandbox-nightly.yml` en
`certification.pipeline.kts`. La genealogía de las lanes no se reescribe: es historia, y
la regla de citas comprobables alcanza al motivo de una etapa, no al relato de la
migración.

**El T6.** Sus patrones de fichero no veían los contratos de la casa, que son su suite
de pruebas de CI. Con los patrones originales, esta rama se rechaza con «fix(*)
commits found but no test files changed in the diff»; con el nivel L5 añadido, pasa. La
puerta no era ciega al test: era ciega a una parte de la suite.

**El GREEN.** 13/13. La puerta completa: 6.368 pruebas del workspace y 0 fallos, los 48
contratos de CI y de producto en verde, los cuatro pipelines tocados con
`VALIDATION SUCCESSFUL`.

**Lo que queda abierto.** Nada de esta unidad. `C37` sale del backlog de la misión: sus
cuatro decisiones están implementadas y el contrato las vigila. Lo que queda por
separado es el motor común de las tres rutas del grafo, que es otra unidad (`C23`).

## Referencias

- `scripts/ci/test_every_test_surface_has_a_lane.py` — el precedente: una superficie
  sin carril se declara con su motivo y su recuento, y los recuentos se comprueban.
- `integration.pipeline.kts`, `certification.pipeline.kts`,
  `release-candidate.pipeline.kts` y `release.pipeline.kts` — las trece etapas.
- `scripts/ci/test_pipeline_stage_bodies.py` — el precedente: un contrato que vigila
  el cuerpo de una stage.
- `scripts/ci/check_regression_test.sh` — el T6, cuya lista de patrones no veía los
  contratos de la casa.
- `crates/cognicode-cli/src/lifecycle.rs:220` — la precedencia de la raíz de
  instalación que desvía la stage.
- `docs/adr/ADR-D1-006-uat-retirement.md` — la retirada que dejó sin suite la
  distribución desde bytes release.
- ADR-056 — la misma regla aplicada a las rutas del grafo: una garantía escrita sin
  mecanismo es un defecto con nombre.
