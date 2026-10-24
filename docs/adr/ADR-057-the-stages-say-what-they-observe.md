# ADR-057 — Una etapa que no puede hacer caer la puerta tiene que decir qué observó

**Estado**: ACEPTADO (decidido el 2026-10-24; pendiente de ejecución)
**Fecha**: 2026-10-24
**Decisores**: Maintainer
**Contexto**: H27 · las puertas y lo que no puede hacerlas caer (`integration.pipeline.kts`, `certification.pipeline.kts`), candidato `C37` de la cola de la misión

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

Diez etapas se ejecutan y **no pueden hacer caer la puerta**:

| Pipeline | Etapas |
|---|---|
| `integration.pipeline.kts` | `parser-adversarial`, `architecture-drift-e2e`, `findings-canonical-grounding`, `workspace-isolation`, `cogh-clean-home-install`, `benchmarks`, `version-sync` |
| `certification.pipeline.kts` | `sandbox-pull`, `sandbox-setup`, `sandbox-ci-probe` |

El mecanismo es `|| echo 'ADVISORY: …'`, porque PipelineK no tiene
`continue-on-error`. **Ningún contrato las declara**: la única vez que la palabra
`ADVISORY` aparece en `scripts/ci/` es en un probe y en un guard de aislamiento de
build. Una etapa que falla y no bloquea se declara en la prosa del pipeline y en
ningún sitio más, así que nadie puede decir qué vale.

Y el motivo que las justifica cita un fichero que **ya no está en el repositorio**:
`integration.pipeline.kts:10-13` dice que «tres jobs en `ci.yml` llevan
`continue-on-error: true`». No hay `ci.yml` en este repositorio —la casa migró de
GitHub Actions a PipelineK—; los únicos `ci.yml` que existen están en los
checkouts de terceros de `sandbox/repos/`, que el propio house excluye porque no
son superficies suyas. El motivo heredado describe un mundo que ya no existe.

### Y una etapa que no hace lo que su nombre dice

**MEDIDO 2026-10-24.** `cogh install --profile core` es un **dry-run**: imprime el
plan y dice «re-run with `--apply` to perform the install», con código de salida 0.
La etapa `cogh-clean-home-install` ejecuta exactamente eso y termina con el eco
«clean-HOME install attempted». La etapa que se llama «instalación en HOME limpio»
**no instala**: comprueba que el planificador sabe dónde instalaría.

Y no es que no pueda: `cogh install --profile core --apply` funciona sin red, con
código 0, `doctor_pass: true`, sha256 del artefacto y el binario real en
`$HOME/.local/share/cognicode/bin/`. La clase de pruebas UAT que ADR-D1-006 retiró
—«distribución desde bytes release»— sólo tiene hoy un sustituto: este script de
shell en la lane de release, y una etapa que.planifica.

## Decisión

1. **Una etapa que no puede hacer caer la puerta declara qué observa y por qué se
   queda así.** La declaración vive en un contrato, con su recuento comprobado, en la
   forma que el house ya usa para las superficies sin carril: una tabla, un motivo y
   un número que se comprueba.

2. **La declaración no cita ficheros que no están en el árbol.** El motivo de una
   etapa tiene que ser verificable donde vive: si cita un fichero, el fichero
   existe.

3. **Una etapa cuyo nombre dice que instala, instala.** La stage de HOME limpio
   instala de verdad, con `--apply`, y el contrato comprueba que la etapa que dice
   «install» dice «apply».

## Alternativas consideradas

**Convertir las diez etapas en puertas.** El argumento del house —«convertirlas en
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
pipeline, que nadie lee como una declaración y que ya cita un fichero inexistente.

## Consecuencias

**Positivas.**

- Las diez etapas que no bloquean quedan con un dueño declarado y un recuento
  comprobado: una etapa nueva que se marque advisory sin declararse pone el
  contrato en rojo.
- La stage de HOME limpio comprueba de verdad lo que su nombre dice, y con ella la
  garantía de «distribución desde bytes release» que ADR-D1-006 dejó sin suite.
- El motivo heredado que citaba `ci.yml` desaparece: la razón que se conserva es la
  que se puede comprobar en este árbol.

**Negativas.**

- El contrato tiene que mantener una tabla. Es el mismo precio que paga
  `test_every_test_surface_has_a_lane.py`, y es un precio que la casa ya ha
  aceptado por la misma razón.
- La stage de HOME limpio instala de verdad: escribe en el HOME temporal que ella
  misma declara, y su duración crece lo que cuesta un binario.

**Lo que esta unidad NO hace.**

- No convierte ninguna etapa en puerta.
- No toca los Playwright: ya están declarados como material de referencia.
- No reconstruye el circuito real: ya existe.

## Referencias

- `scripts/ci/test_every_test_surface_has_a_lane.py` — el precedente: una superficie
  sin carril se declara con su motivo y su recuento, y los recuentos se comprueban.
- `integration.pipeline.kts` y `certification.pipeline.kts` — las diez etapas.
- `scripts/ci/test_pipeline_stage_bodies.py` — el precedente: un contrato que vigila
  el cuerpo de una stage.
- `docs/adr/ADR-D1-006-uat-retirement.md` — la retirada que dejó sin suite la
  distribución desde bytes release.
- ADR-056 — la misma regla aplicada a las rutas del grafo: una garantía escrita sin
  mecanismo es un defecto con nombre.
