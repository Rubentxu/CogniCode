#!/usr/bin/env python3
"""ROADMAP: coherencia de version y no retirar lo que sigue en el arbol.

Dos contratos sobre `docs/roadmap/ROADMAP.md`, en un solo fichero. No comparten
parser: uno lee la version, el snapshot y el Journal; el otro, las frases de
retirada. Se juntan por tema, y cada uno conserva su docstring completa.

# Parte 1 — La version y el snapshot

El ROADMAP no puede quedarse atras de la rama activa sin que el contrato lo diga.

## El hueco

`docs/roadmap/ROADMAP.md` declara ser la unica autoridad de agenda
(`AGENTS.md` § "Como distinguir el contexto"). `docs/roadmap/CURRENT.md`
es snapshot puntual y el propio fichero dice "regenerar antes de citar".
Pero nada pineaba la coherencia entre:

  1. la version que declara `Cargo.toml [workspace.package].version`, y
  2. una entrada reciente (ROADMAP o JOURNAL) que cite esa misma version, y
  3. el SHA del snapshot en `CURRENT.md` que coincide con el HEAD real.

El resultado MEDIDO 2026-10-09: workspace `0.101.9`, ROADMAP documenta
hasta `v0.99.2`, 9 releases patch consumidos sin que el ROADMAP los
refleje, `CURRENT.md` declarando su propia obsolescencia en cabecera.
Eso es exactamente "dos fuentes de verdad", el anti-patron §6 del
ROADMAP. Un PR que toque la version puede arrastrar al workspace
lejos del ROADMAP sin que nada se entere.

## Lo que se anade

Cuatro mutaciones, todas vistas caer sobre ficheros reales:

  - la version del workspace no aparece en ROADMAP ni JOURNAL reciente -> 1 rojo
    (es un solo caso pineado por la guarda "O ROADMAP O JOURNAL":
    MEDIDO, el JOURNAL de 615K cita `0.101.9` 121 veces, asi que el
    camino real para que esto falle es bumpear el workspace a una
    version no documentada en absoluto).
  - el SHA del snapshot en CURRENT.md no es el HEAD real                -> 1 rojo
  - CURRENT.md lleva mas de 14 dias sin regenerarse                    -> 1 rojo
  - la declaracion de snapshot en CURRENT.md falta o esta mal formada  -> 1 rojo
  - (control, sin mutacion)                                            -> todo verde

La ventana de 14 dias es el contrato: el snapshot es puntual, no
eterno; tras dos semanas se considera stale aunque el SHA coincida.
Una discusion razonable es mover esa ventana (30 dias?), pero
cambiarla aqui exige re-pasar todas las mutaciones vistas caer y
documentar la razon en el commit.

El contrato NO decide si la version documentada es la correcta: eso
es decision del operador. Lo que decide es que la declaracion y el
codigo no se contradigan en silencio, que es la pieza que faltaba.

## Limites del contrato

Solo conoce los nombres de su tabla. Si la version cambia de nombre
(e.g. el operador pasa de `0.101.9` a `1.0.0-rc.1`), el contrato
seguira pineando "version X aparece en ROADMAP o JOURNAL" — no sabe
que X es la ultima. La autoridad de que X es la version correcta
sigue siendo humana. El contrato es una red minima: detecta la
divergencia silenciosa, no la corrige.

# Parte 2 — Lo que el ROADMAP declara retirado

El ROADMAP no puede declarar retirado lo que sigue en el árbol.

## El hueco

`docs/roadmap/ROADMAP.md` es la agenda vigente: lo que dice ahí se cita
como si fuera cierto, y quien lo ejecuta va a buscar lo que la nota le
manda. Una nota que afirma que un documento fue retirado lo manda a otro
sitio —el recibo del `JOURNAL`— cuando el documento sigue en el árbol, con
la decisión dentro.

Eso es lo que pasó con la nota 2 de §11.1, MEDIDO en TASK-129: la nota
afirmaba que la poda documental —PR #399— se había llevado
`docs/adr/ADR-D1-006-uat-retirement.md`. El fichero existe, y
`git log --all --diff-filter=D` no registra su baja. La nota mandaba a un
sitio que ya no era el único, y hacía invisible el fichero que sí tenía la
decisión.

El daño que este contrato evita no es que la nota se equivoque. Es que la
nota **no se puede equivocar sin que nada lo note**: un documento que se
declara retirado y sigue presente es una contradicción que el árbol
denuncia y la prosa esconde.

## Lo que se comprueba

Para cada ruta de `docs/` que el ROADMAP declara retirada en una frase, el
documento no puede estar en el árbol.

  - una ruta declarada retirada que sigue en el árbol      -> 1 rojo
  - (control) una prosa sintética que retira un fichero que está -> 1 rojo
  - (control) una prosa sintética que retira un fichero que no está -> 0 rojos
  - (control) una negación no es una afirmación: «no fue retirado» no
    declara nada                                              -> 0 rojos
  - (control) el extractor ve las afirmaciones que dice ver  -> 1 afirmación

Los controles importan, y el último es el que sostiene a los demás. En el
árbol ya no queda ninguna afirmación de retirada: la única que había era
el defecto. Sin el control sintético, un extractor que dejó de ver
afirmaciones pasaría siempre y el contrato no vigilaría nada.

## Lo que NO se comprueba

  - **Que la retirada haya ocurrido de verdad.** El contrato mira el árbol,
    no la historia: una ruta que se declara retirada y que ya no está pasa,
    aunque la baja no figure en el historial de git. Que la baja figure es
    otra unidad.
  - **Que el documento citado sea el mejor.** Solo que el ROADMAP no afirme
    que está retirado algo que está.
  - **Las filas de tabla.** Una celda puede declarar una retirada mientras
    otra celda de la MISMA fila cita rutas, y una fila no es una
    afirmación sobre una ruta. Se miran las frases de la prosa.
  - **Los SHAs citados.** No se comprueba que el commit que se cita viva en
    `main`: hay varios que no, y verificarlos todos es otra unidad.

## La redacción que el contrato pide

El contrato lee frase a frase. Una frase que declara una retirada y cita
una ruta no puede distinguirse de una que declara la retirada de esa ruta:
la redacción tiene que separar las dos cosas. «El fichero `X` existe» y
«la poda se llevó seis ficheros» son dos frases, no una.

## Parte 3. ROADMAP §11: la autoridad que cita y el cierre que declara

Dos contratos sobre el mismo bloque de `docs/roadmap/ROADMAP.md`, en un
solo fichero. Cada uno conserva su propio extractor del bloque a proposito:
`open_units_block` (la autoridad: parrafo en blockquote, techo de 80 lineas)
y `section11_open_units` (el cierre: ids en negrita del trailer). Son dos
preguntas con dos delimitaciones, no una duplicacion que unificar sin medir.

# Parte 1 — La autoridad que §11 cita

La autoridad normativa que §11 cita tiene que existir y cubrir la unidad.

## El hueco

`docs/roadmap/ROADMAP.md` §11 dice cuales son las unidades abiertas y de
donde sale su especificacion. Es la puerta de entrada del trabajo
pendiente, asi que sus citas no son adorno: son lo primero que abre quien
llega.

Las tres estaban mal, y por el mismo motivo:

  - **H3 y H4 (K1 y K2) compartian un unico ADR**, `ADR-K003.md`, cuya
    linea 3 dice literalmente "**Aplica a:** K2". K1 se quedaba sin
    autoridad correcta y K2 recibia una cita que no podia desmentir.
  - **H4.4 y H5 se remitian a un HITOS del programa delivery-first** que
    vive bajo `docs/*` en `.gitignore`: `git ls-files` devuelve **0**
    ficheros. Existia en el disco de quien lo escribio y en ninguno mas.

Un clon limpio no resolvia ninguna de las tres citas. No es una opinion
sobre si el ADR elegido es el mejor: es que el puntero no llevaba a nada.

## Lo que se anade

Una regla derivada, no una segunda lista. La autoridad de cada ADR ya esta
declarada en su propia linea 3 ("Aplica a:"); aqui solo se comprueba que
§11 no la contradiga:

  - una ruta citada en §11 que no este VERSIONADA  -> 1 rojo
  - un ADR citado cuya "Aplica a:" no declara la unidad que §11 le
    atribuye                                           -> 1 rojo
  - (control, sin mutacion)                            -> todo verde

El control importa mas que los otros dos: una regla que no puede llegar a
fallar es indistinguible de una que nunca falla.

## Lo que NO se comprueba

  - **Que la autoridad citada sea la mejor eleccion.** Solo que exista, este
    versionada y declare la unidad. Elegir bien un ADR es juicio humano.
  - **El estado de la unidad.** Que H3 siga abierta no lo decide este
    contrato; lo hace la Parte 2 de este fichero, que es
    complementario y no solapado: aquel vigila el CIERRE, este la AUTORIDAD.

# Parte 2 — El cierre

La lista de trabajo abierto no puede sobrevivir a su propio cierre.

## El hueco

`docs/roadmap/ROADMAP.md` §11 termina con un trailer que enumera "las
unidades realmente abiertas" del programa vivo: **H3**, **H4.1**, **H4.2**,
**H4.4**, **H5** y **H0.1**.

Es una segunda fuente de verdad. El Journal es el registro append-only que
declara cuando una unidad cierra; el trailer del ROADMAP *repite* ese estado
en prosa, y nada pineaba que coincidieran.

Divergieron, y en la misma dirección que §10 hace un mes:

  - el programa **H0** completo (H0.1..H0.5) cerro en el Journal Entry con
    `bc3733a`, con su RED ya en verde y 267/267 en la suite fast;
  - §11 seguia nombrando **H0.1** entre las unidades abiertas, y de paso lo
    describia como "con RED ya presentado" — un RED que ya es verde.

Un agente que siga esa instruccion literal reconstruye unaCache que ya esta
corregida y probada.

## Lo que se anade

Una regla derivada, no una segunda lista: **una unidad que el Journal da por
cerrada no puede figurar como abierta en §11**. La autoridad es el Journal
(append-only, con SHA y recuento de suite); el trailer deriva de ella.

  - una unidad del trailer de §11 con cierre en el Journal  -> 1 rojo
  - el trailer no lista ninguna unidad                        -> 1 rojo
  - el Journal no tiene ningun cierre (regla muerta)          -> 1 rojo
  - (control, sin mutacion)                                   -> todo verde

El tercer caso es el guard anti-vacuidad: sin el, un Journal vacio o un
cambio de formato dejaria la regla mirando sin comprobar nada — el mismo
modo de fallo que `test_every_released_block_has_a_goal_doc` busca en el
contrato hermano de §10.

## Los ids de dos digitos

La lista de abiertas se extraia con `[A-Z]\\d(?:\\.\\d)?`, que casa con
`H5` y `H4.4` y **no** con `H23`: un digito y punto opcional. Una unidad
con dos digitos no era visible para la regla, de modo que el trailer
podia declararla abierta y el Journal cerrarla sin que nadie se enterara
-- que es exactamente el fallo que este contrato existe para cazar, en
una version mas pequena. La regexp ahora acepta el grupo de digitos
entero, y la regla vigila tambien las unidades nuevas.

## Lo que NO se comprueba, y por que

  - **Si una unidad sin cierre en el Journal esta de verdad abierta.** El
    Journal registra cierres; que no haya cierre es *ausencia de evidencia*,
    no evidencia de que siga abierta. La autoridad sigue siendo humana: este
    contrato solo quita del trailer lo que el Journal ya cerro, nunca
    declara abierta una unidad por su cuenta.
  - **El estado de H3, H4.x, H5.** siguen abiertos y aqui no hay evidencia
    en ningun sentido. Que el contrato no los mencione es correcto: no esta
   anotarlo lo que hace.
  - **Que el trabajo exista.** Mismo limite que el contrato hermano: esto
    comprueba la coherencia del libro, no la verdad del trabajo. Lo que caza
    eso es el gate de publicacion mas la suite de la unidad.

## Limites del contrato

Al ser una regla de *cierre* y no de *avance*, es monotona: no puede
detectar que se haya olvidado anadir una unidad nueva. Esa asimetria es
deliberada — el fallo caro es reconstruir trabajo vendido (lo que ocurrio),
no tardar un ciclo mas en notar una unidad nueva (que se nota en el primer
`git log`).

## Parte 4. Libro mayor y declaracion de la mision

Dos contratos de lectura de prosa/JSON declarativos, sin git ni procesos, en
un solo fichero. La parte A vigila que los goal docs y la tabla del work-loop
coincidan con ROADMAP §10; la B, que `goal-compass.json` hable los campos que
el cargador del plugin lee. Cada parte conserva su docstring original y sus
comprobaciones; los dos controles `test_control_all_green` se llaman ahora
`test_control_ledger_all_green` y `test_control_mission_all_green`.

Ejecutar: python3 scripts/ci/test_roadmap_coherence.py

# PARTE A — coherencia del libro mayor de bloques

El bloque no puede decir una cosa mientras la agenda dice otra.

## El hueco

`docs/roadmap/ROADMAP.md` §10 es, desde el cierre de B1, el unico sitio que
lista cada bloque con su tag y su veredicto. Los goal docs de
`odd/tasks/b[1-6]-*.md` son prosa que *repiten* ese estado en su cabecera.

Nada pineaba que las dos cosas coincidieran, y divergieron:

  - `b5-safe-refactoring.md` decia `PENDING` con el recibo entero en
    `_pendiente_`, cuando `v0.101.15` estaba publicado, habia veredicto
    escrito, y ROADMAP §10 lo listaba cerrado desde 2026-10-12. Un agente que
    siguió la instruccion literal —"avanza con el siguiente WU no
    bloqueado"— habria reconstruido ~2.500 lineas de codigo ya publicado.
  - La tabla "Estado actual del programa" de
    `odd/tasks/2026-Q4-work-loop.md` seguia diciendo `B1 PENDING` y
    `B2..B6 BLOCKED (B1 prerequisite)` cuatro dias despues del cierre.

El coste no fue teorico: el primer caso pide rehacer trabajo ya vendido.

## Lo que se anade

Una regla derivada, no una segunda lista: **la cabecera de cada goal doc se
comprueba contra §10**, que es la autoridad. Asi el goal doc no vuelve a ser
una fuente de verdad —deriva de otra— y la deriva se detecta sola.

Dos invariantes, ambas vistas caer sobre el estado real:

  - un bloque con tag publicado en §10 no puede declarar un estado distinto
    de `CLOSED` en su goal doc                                  -> 1 rojo
  - un bloque con tag publicado en §10 no puede figurar como `PENDING` o
    `BLOCKED` en la tabla del work-loop                         -> 1 rojo
  - (control, sin mutacion)                                      -> todo verde

## Lo que NO se comprueba, y por que

  - **Casillas `- [ ]` en un bloque `CLOSED`.** Es cosmético: quien lee el
    bloque ve `CLOSED` y el recibo con SHA debajo. Una casilla sin marcar no
    le dice a nadie que rehaga el trabajo; el que si lo hace es una cabecera
    que dice `PENDING`. Distinguir las dos cosas es el punto de la regla.
    (B4 y el doc detallado de B5 tienen las casillas sin marcar; es deuda
    de higiene declarada, no un defecto que este contrato deba bloquear.)
  - **`odd/tasks/b5-safe-refactoring-authorized-changes.md`.** Es una
    especificacion archivada y protegida: no se reescribe. Su cabecera y su
    recibo ya coinciden, asi que la dejariamos pasar igual; se excluye por
    proteccion, no porque se haya perdonado.
  - **Si el SHA del recibo resuelve en git.** Seria un buen contrato, pero
    `git cat-file -e` sobre SHAs de ramas ya_consumidas depende de que el
    objeto siga presente en el clon, y eso lo decidio el GC, no el
    documento. Un recibo puede ser honesto y su SHA pode no estar aqui.
    (Lo que si se comprobara —alcanzabilidad desde un ref, no mera
    existencia— habria caido sobre `b5-safe-refactoring-authorized-changes.md`,
    que acredita las cuatro WU de B5 al SHA `0e5708d`,commit hermano
    descartado que no esta en la historia del proyecto. Ver alli.)

## Limites del contrato

Este contrato comprueba **coherencia del libro mayor**, no la verdad del
trabajo que el libro mayor afirma. Un `CLOSED` acompanado de una edicion
consistente en ROADMAP §10 pasaria, y no deberia pasar: lo que caza eso es
el gate de publicacion (`merge-gate`) mas las suites UAT/AAT del bloque
(`b5_w1..b5_w4`), que es donde vive la prueba de que el trabajo existe.

La division es deliberada: el libro es barato de comprobar y caro que se
MENTIRA; el trabajo es caro de comprobar y lo que hay que proteger. Que cada
cosa la compruebe la evidencia que le corresponde.


# PARTE B — declaracion de la mision

La declaracion de la mision tiene que hablar el idioma que el cargador lee.

## El hueco

`goal-compass.json` es lo que el plugin `goal-compass` encadena: su `backlog`
es la cola de trabajo de la mision y su `evidenceRefs` es lo que se comprueba
antes de convertir un candidato en objetivo.

El cargador lee unos campos y **solo** esos (`adapters/mission.js`):

    candidateId, objective, whyNow, evidenceRefs, doneWhen, scope, risk,
    priority, prerequisites, source, tokenBudget

Un campo con otro nombre no se pierde con un aviso: se pierde **en silencio**.
La auditoria del plugin lo dice asi: *"el peor caso es una declaracion que
parece llevar la evidencia --porque el campo se llama `evidence` y su valor es
una ruta que existe-- y que en realidad la pierde entera: todos sus candidatos
quedan descartados y la cadena no tiene nada que encadenar"*.

MEDIDO 2026-10-21 en este repositorio: la declaracion escribia la evidencia
en `evidence` (una cadena) y no en `evidenceRefs` (una lista). Los 26
candidatos quedaron descartados por `sin-evidencia` y la cadena no encadenaba
nada. El `doctor` del plugin lo dira con su propia cuenta: 26 candidatos, 0
usables. Dos defectos mas en la misma declaracion: la cola estaba escrita al
reves --el selector ordena de mayor a menor y el primer candidato llevaba el
numero mas bajo--, y una referencia apuntaba a un fichero que no existe.

Nada en el repositorio lo vigilaba. La declaracion podia quedar inservible y
el roadmap seguia pareciendo completo.

## Lo que se anade

Un contrato que **no reimplementa** al cargador: usa las mismas reglas que el
plugin declara, sobre el mismo fichero, y falla si la declaracion no encadena.

  - un candidato sin `evidenceRefs` utilizable            -> 1 rojo
  - un candidato sin criterio de cierre observable         -> 1 rojo
  - la cola escrita al reves (prioridad no decreciente)   -> 1 rojo
  - una referencia que no apunta a un fichero real         -> 1 rojo
  - un candidato con un campo que el cargador no lee       -> 1 rojo
  - (control, sin mutacion)                                -> todo verde

El quinto caso es el guard anti-vacuidad: sin el, un campo mal escrito
--`evidence` en vez de `evidenceRefs`-- pasaria inadvertido, porque el valor
parece correcto y la ruta existe. Es el fallo que se midio.

## Lo que NO se comprueba, y por que

  - **Que el orden de las prioridades sea el que el roadmap quiere.** El
    contrato vigila que sea coherente y decreciente, no que sea el correcto:
    el orden es una decision del operador.
  - **Que un candidato este terminado.** El plugin lleva su propio registro
    de lo que la cadena encadenó y cumplio; este contrato no lo duplica.
  - **Que la mision exista fuera del repositorio.** El estado durable vive
    en el hogar del usuario, no aqui. Aqui solo se vigila la declaracion.
"""

from __future__ import annotations

import datetime as _dt
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

# --- Parte 1: version y snapshot ----------------------------------------

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CARGO_TOML = REPO_ROOT / "Cargo.toml"
ROADMAP = REPO_ROOT / "docs" / "roadmap" / "ROADMAP.md"
CURRENT = REPO_ROOT / "docs" / "roadmap" / "CURRENT.md"
JOURNAL = REPO_ROOT / "docs" / "roadmap" / "JOURNAL.md"
FRESHNESS_DAYS = 14


def _read(path: Path) -> str:
    if not path.is_file():
        return ""
    return path.read_text(encoding="utf-8")


def workspace_version() -> str:
    """Parse `[workspace.package].version` from Cargo.toml."""
    text = _read(CARGO_TOML)
    m = re.search(r'^\[workspace\.package\].*?^version\s*=\s*"([^"]+)"',
                  text, flags=re.MULTILINE | re.DOTALL)
    if not m:
        raise SystemExit(f"FAIL: no se encontro [workspace.package].version en {CARGO_TOML}")
    return m.group(1)


def head_sha() -> str:
    """Long SHA of HEAD."""
    return subprocess.check_output(
        ["git", "-C", str(REPO_ROOT), "rev-parse", "HEAD"],
        text=True,
    ).strip()


def _mentions(text: str, version: str) -> bool:
    """A document mentions a version if `vX.Y.Z` or `X.Y.Z` appears as a token."""
    escaped = re.escape(version)
    # accept both `v0.101.9` and bare `0.101.9`, but not substrings like `1.0`
    pat = re.compile(rf"(?:^|\W)v?{escaped}(?=\W|$)")
    return bool(pat.search(text))


def roadmap_or_journal_cites(version: str) -> tuple[bool, str]:
    """Returns (mentioned, source_label). Either ROADMAP or JOURNAL must cite."""
    rm = _read(ROADMAP)
    jn = _read(JOURNAL)
    if _mentions(rm, version):
        return True, "ROADMAP"
    if _mentions(jn, version):
        return True, "JOURNAL"
    return False, "none"


def current_snapshot_sha() -> str | None:
    """Extract the long SHA the CURRENT.md snapshot header declares.

    Anchored to the same `Snapshot**:` declaration that carries the date,
    not to the first 40-hex token in the file. CURRENT.md also quotes SHAs
    in its historical body — the previous snapshot, the "SHA funcional"
    line, per-release receipts — and reading the first match anywhere meant
    a stale SHA in that body could satisfy the ratchet while the header
    declared a different tree, or the reverse: a header update could be
    masked by an unrelated historical line. A ratchet that measures a
    different SHA than the one it claims to measure is not a ratchet.
    """
    text = _read(CURRENT)
    m = re.search(r"Snapshot\*\*:[^\n]*?\b([0-9a-f]{40})\b", text)
    return m.group(1) if m else None


def current_snapshot_date() -> _dt.date | None:
    """Extract the date quoted in CURRENT.md snapshot declaration (YYYY-MM-DD)."""
    text = _read(CURRENT)
    m = re.search(r"Snapshot\*\*:\s*(\d{4}-\d{2}-\d{2})", text)
    if not m:
        return None
    return _dt.date.fromisoformat(m.group(1))


def days_since(date: _dt.date) -> int:
    return (_dt.date.today() - date).days


# ---------- assertions (each is one mutation seen to fail) ----------

def test_workspace_version_exists() -> None:
    """Sanity: workspace version parses; without this nothing else is meaningful."""
    v = workspace_version()
    assert re.match(r"^\d+\.\d+\.\d+", v), f"version no es semver: {v!r}"


def test_roadmap_or_journal_cites_the_workspace_version() -> None:
    """M1: cargo.toml version must appear in ROADMAP.md or JOURNAL.md recent entries.

    Sin esta guarda, el operador puede bumpear `Cargo.toml` y olvidar el ROADMAP;
    el contrato pinea que no.
    """
    v = workspace_version()
    mentioned, source = roadmap_or_journal_cites(v)
    assert mentioned, (
        f"FAIL: workspace version {v!r} no aparece en {ROADMAP.name} "
        f"ni en {JOURNAL.name}. Anota el delta en uno antes de promover."
    )


SNAPSHOT_MAX_DRIFT = 5  # commits: a snapshot describing HEAD~5 is still honest


def test_current_sha_matches_head() -> None:
    """M2: CURRENT.md snapshot must reference a recent ancestor of HEAD.

    Sin esta guarda, CURRENT.md puede citar un SHA obsoleto y el lector lo
    toma como cierto porque el documento dice "regenerar antes de citar".
    El ratchet pinea que la cabecera no puede mentir sin que el contrato
    lo detecte.

    Se acepta cualquier ancestro dentro de SNAPSHOT_MAX_DRIFT commits **de
    la base donde esta historia se encontró con `main`**. La distancia se
    mide ahí y no contra HEAD: los commits propios de una rama son trabajo
    sin publicar, que ningun lector puede depender aun, y medirlos hacia
    atras hacia que el guard disparase en casi cualquier rama real — y la
    unica salida era citar la cabeza de la rama, que un squash-merge
    invalida enseguida. En `main` los dos refs coinciden y la ventana
    sigue mordiendo igual. Ver `_is_recent_ancestor`.
    """
    declared = current_snapshot_sha()
    actual = head_sha()
    assert declared is not None, (
        f"FAIL: {CURRENT.name} no contiene un SHA de 40 chars. "
        f"Regenera el snapshot."
    )
    base = _merge_base(actual, "main") or actual
    assert _is_recent_ancestor(declared, actual, SNAPSHOT_MAX_DRIFT, base), (
        f"FAIL: {CURRENT.name} declara SHA {declared[:12]}, "
        f"HEAD es {actual[:12]} y la base con main es {base[:12]}. El SHA "
        f"declarado debe ser alcanzable desde HEAD y estar a mas de "
        f"{SNAPSHOT_MAX_DRIFT} commits de esa base. Regenera el snapshot."
    )


def _merge_base(a: str, b: str) -> str | None:
    """Where the two histories last met, or None if they are unrelated."""
    out = subprocess.run(
        ["git", "-C", str(REPO_ROOT), "merge-base", a, b],
        capture_output=True, text=True,
    )
    return out.stdout.strip() or None


def _is_recent_ancestor(
    sha: str, ancestor: str, max_drift: int, distance_to: str | None = None
) -> bool:
    """True if `sha` is reachable from `ancestor` and near `distance_to`.

    Two independent questions, and the old code answered only the first
    badly:

    * **Is it on this history at all?** `rev-list --count A..B` alone
      counts the commits B has that A does not, which a SHA from a
      squash-merged side branch satisfies with 1 — so a snapshot could
      cite a tree that was never on this branch. Reachability is checked
      first, because a guard that cannot detect what its docstring names
      is not a guard.

    * **How far behind is it?** Measured to `distance_to`, which is
      where this line of history last met `main` — not to `ancestor`.
      On a feature branch the commits in between are unreleased work
      that no reader can depend on yet, so charging the snapshot for
      them made this fire on almost every real branch. And the only way
      to silence it was to cite the branch head, which a squash-merge
      then invalidates — the two halves of the old rule could not both
      be satisfied by any value once a branch passed five commits.

      On `main` the two refs coincide, so the window bites exactly as
      before: a `main` commit that nobody regenerated against is still
      a `main` commit nobody regenerated against.
    """
    if sha == ancestor:
        return True
    try:
        reachable = subprocess.run(
            ["git", "-C", str(REPO_ROOT), "merge-base", "--is-ancestor", sha, ancestor],
            capture_output=True, text=True,
        )
        if reachable.returncode != 0:
            return False
        out = subprocess.check_output(
            ["git", "-C", str(REPO_ROOT), "rev-list", "--count", f"{sha}..{distance_to or ancestor}"],
            text=True, stderr=subprocess.DEVNULL,
        ).strip()
        distance = int(out)
        return 0 <= distance <= max_drift
    except (subprocess.CalledProcessError, ValueError):
        return False


def test_distance_is_measured_to_the_merge_base_not_the_branch_head() -> None:
    """The window follows the base, not the tip — on any ref.

    Pinned with the repository's own root commit so it holds on a
    branch and on `main` alike: the root is reachable from every tip,
    and it is certainly more than `SNAPSHOT_MAX_DRIFT` commits back
    from one.
    """
    head = head_sha()
    root = subprocess.check_output(
        ["git", "-C", str(REPO_ROOT), "rev-list", "--max-parents=0", head],
        text=True,
    ).strip().splitlines()[-1]

    assert not _is_recent_ancestor(root, head, SNAPSHOT_MAX_DRIFT, head), (
        "measured against the tip, a commit far behind must be refused"
    )
    assert _is_recent_ancestor(root, head, SNAPSHOT_MAX_DRIFT, root), (
        "measured against a base at that commit, the distance is zero and it "
        "must be accepted — this is what lets a long feature branch keep a "
        "snapshot that names the last merge"
    )


def test_current_snapshot_is_fresh() -> None:
    """M3: CURRENT.md must be regenerated within FRESHNESS_DAYS days.

    El snapshot es puntual. Aunque el SHA coincida por casualidad, una
    declaracion de mas de 14 dias sin regenerar es opaca al estado real.
    """
    d = current_snapshot_date()
    assert d is not None, (
        f"FAIL: {CURRENT.name} no tiene cabecera 'Snapshot**: YYYY-MM-DD'. "
        f"Regenera."
    )
    age = days_since(d)
    assert age <= FRESHNESS_DAYS, (
        f"FAIL: {CURRENT.name} lleva {age} dias sin regenerarse "
        f"(snapshot {d.isoformat()}, hoy {_dt.date.today().isoformat()}). "
        f"Limite contractual: {FRESHNESS_DAYS} dias."
    )


def test_control_all_green() -> None:
    """Sanity: en estado nominal, los tres assertions anteriores pasan.

    Si esto falla en HEAD limpio, hay un bug en el contrato, no en el repo.
    """
    test_workspace_version_exists()
    test_roadmap_or_journal_cites_the_workspace_version()
    test_current_sha_matches_head()
    test_current_snapshot_is_fresh()


# --- Parte 2: retiradas -----------------------------------------------


# Una cita de ruta dentro de acentos graves: es la forma en que el ROADMAP
# nombra un documento del árbol.
CITED = re.compile(r"`(docs/[A-Za-z0-9._/-]+)`")

# Una afirmación de retirada. La lista cubre las formas que el ROADMAP
# usa: el verbo de retirada (retirar, eliminar, borrar, suprimir), la poda
# (podar, llevarse), la desaparición, y el pasado de existir —«existió» es
# decir «ya no está»—.
#
# `pod` lleva su forma propia porque `pod\w+` también casaría con «podemos»
# y «podría», que no retiran nada.
RETIREMENT = re.compile(
    r"\b(?:retir\w+|elimin\w+|borr\w+|suprimid\w+|desapareci\w+|llev\w+"
    r"|pod(?:ar|a|ó|aron|ando|aba|aban)"
    r"|exist(?:ió|ieron|ía|íamos|ían)"
    r"|ya no existe|ya no está)\b",
    re.IGNORECASE,
)

# Una negación cancela la afirmación si está cerca del verbo: «no fue
# retirado» no declara una retirada, la desmiente.
NEGATION = re.compile(
    r"\b(?:no|nunca|jamás|sin|ningún|ninguna|nadie)\b", re.IGNORECASE
)

# La ventana, en caracteres, en la que la negación sigue alcanzando al
# verbo. Es corta a propósito: una negación lejana no es la negación de
# ese verbo, y una afirmación que se salva de una negación que no era suya
# es una afirmación que el contrato no vigila.
NEGATION_WINDOW = 60


def prose_sentences(text: str) -> list[str]:
    """Las frases de la prosa del documento.

    Las filas de tabla no se miran, por lo que dice el contrato: una fila
    puede mezclar en la misma línea una celda que declara una retirada y
    otra que cita rutas.
    """
    prose = [line for line in text.splitlines() if not line.strip().startswith("|")]
    return re.split(r"(?<=[.!?:])\s+|\n", " ".join(prose))


def mask_citations(sentence: str) -> str:
    """La frase con las rutas citadas tapadas.

    Una ruta es un nombre, no una afirmación: el fichero
    `docs/adr/ADR-D1-006-uat-retirement.md` contiene la palabra
    «retirement», que no es un verbo de retirada. Sin taparla, el contrato
    leería el nombre del fichero como una afirmación de que se retiró.
    """
    return CITED.sub(lambda match: " " * len(match.group(0)), sentence)


def asserts_retirement(sentence: str) -> bool:
    """Si la frase declara una retirada.

    Declara una retirada si tiene un verbo de retirada SIN negación cerca.
    """
    masked = mask_citations(sentence)
    for marker in RETIREMENT.finditer(masked):
        window = masked[max(0, marker.start() - NEGATION_WINDOW) : marker.start()]
        if not NEGATION.search(window):
            return True
    return False


def retirement_claims(text: str) -> list[tuple[str, str]]:
    """Las (ruta, frase) en las que el documento declara una retirada."""
    claims: list[tuple[str, str]] = []
    for sentence in prose_sentences(text):
        if not asserts_retirement(sentence):
            continue
        for path in sorted(set(CITED.findall(sentence))):
            claims.append((path, " ".join(sentence.split())))
    return claims


def still_there(
    claims: list[tuple[str, str]], root: Path = REPO_ROOT
) -> list[tuple[str, str]]:
    """Las afirmaciones que el árbol contradice.

    Una ruta está si el árbol tiene ese fichero, o ese fichero con
    extensión `.md`: el ROADMAP cita los documentos por su nombre de
    fichero y a veces sin la extensión.
    """
    contradicted: list[tuple[str, str]] = []
    for path, sentence in claims:
        if (root / path).exists() or (root / f"{path}.md").exists():
            contradicted.append((path, sentence))
    return contradicted


def test_the_roadmap_does_not_retire_a_document_that_is_there() -> None:
    """Ninguna ruta que el ROADMAP declara retirada sigue en el árbol."""
    text = ROADMAP.read_text(encoding="utf-8")
    claims = retirement_claims(text)
    contradicted = still_there(claims)

    assert not contradicted, (
        "FAIL: el ROADMAP declara retirados documentos que siguen en el "
        "árbol:\n  "
        + "\n  ".join(f"{path} — «{sentence}»" for path, sentence in contradicted)
        + "\nUna nota que declara retirado un documento que existe manda a "
        "quien la lee a un sitio que ya no es el único, y esconde el "
        "fichero que sí tiene la decisión."
    )


def test_a_synthetic_retirement_of_a_present_document_is_caught() -> None:
    """Control: una retirada inventada de un fichero que está sale en rojo.

    Sin este control, un contrato que no mira nada y un contrato que mira
    y no encuentra nada serían lo mismo.
    """
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        present = root / "docs" / "adr" / "ADR-X.md"
        present.parent.mkdir(parents=True, exist_ok=True)
        present.write_text("decisión\n", encoding="utf-8")

        prose = (
            "Ese fichero existió (`docs/adr/ADR-X.md`, commit `abc1234`) y lo "
            "retiró la poda documental —PR #399—."
        )
        claims = retirement_claims(prose)
        assert claims, (
            "FAIL: el extractor no ha visto la afirmación de retirada de una "
            "prosa que la contiene"
        )
        contradicted = still_there(claims, root=root)
        assert contradicted, (
            "FAIL: una afirmación de retirada de un documento que está en el "
            "árbol no sale en rojo: el contrato no vigila"
        )


def test_a_retirement_of_a_document_that_is_gone_passes() -> None:
    """Control: una retirada verdadera no sale en rojo.

    El contrato tiene que poder decir «aquí está bien»: si todo fuera
    rojo, nadie distinguiría entre un defecto y un contrato ruidoso.
    """
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        prose = (
            "Ese fichero existió (`docs/adr/ADR-Y.md`, commit `abc1234`) y lo "
            "retiró la poda documental —PR #399—."
        )
        claims = retirement_claims(prose)
        assert claims, (
            "FAIL: el extractor no ha visto la afirmación de retirada de una "
            "prosa que la contiene"
        )
        assert not still_there(claims, root=root), (
            "FAIL: una afirmación de retirada de un documento que NO está en el "
            "árbol sale en rojo: el contrato es ruidoso"
        )


def test_a_negated_retirement_is_not_a_retirement() -> None:
    """Control: una negación no es una afirmación.

    Corregir una nota sobre una retirada exige decir que NO hubo retirada.
    Si la negación contara como afirmación, el contrato no dejaría
    corregir la nota que señala.
    """
    prose = (
        "Ese fichero no fue retirado: `docs/adr/ADR-Z.md` sigue en el árbol "
        "y la decisión vive dentro."
    )
    claims = retirement_claims(prose)
    assert not claims, (
        "FAIL: una frase que NIEGA una retirada sale como si la declarara: "
        f"{claims}"
    )


def test_a_document_name_is_not_an_assertion() -> None:
    """Control: el nombre de un fichero no es una afirmación.

    `docs/adr/ADR-D1-006-uat-retirement.md` lleva la palabra «retirement»
    dentro del nombre. Si el contrato leyera el nombre como una
    afirmación, no podría corregir la nota que señalaba: la nota
    corregida tiene que poder NOMBRAR el fichero.
    """
    prose = (
        "El fichero `docs/adr/ADR-D1-006-uat-retirement.md` está en el árbol, "
        "con la decisión aceptada."
    )
    claims = retirement_claims(prose)
    assert not claims, (
        "FAIL: el nombre de un fichero sale como si fuera una afirmación de "
        f"retirada: {claims}"
    )


def test_the_extractor_sees_the_claims_it_claims_to_see() -> None:
    """Control: el extractor ve las afirmaciones de retirada.

    En el árbol ya no queda ninguna afirmación que retirar —la que había
    era el defecto—, así que el control es sintético: una prosa con una
    retirada tiene que producir una afirmación. Si el extractor dejó de
    verlas, este contrato pasaría siempre y no vigilaría nada.
    """
    prose = (
        "La poda documental se llevó `docs/adr/ADR-W.md`, y con ella se fue "
        "la decisión que llevaba."
    )
    claims = retirement_claims(prose)
    assert claims, (
        "FAIL: el extractor no ve una afirmación de retirada en una prosa "
        "que la contiene. Si dejó de verlas, este contrato pasaría siempre."
    )


# --- Parte 3: ROADMAP §11 (las unidades abiertas): autoridad y cierre ---

# --- Parte 3, bloque 3, bloque 1: la autoridad que §11 cita ------------------------------

OPEN_UNITS_ANCHOR = "Las unidades realmente abiertas son"

# Techo del parrafo de §11: es un parrafo, no una seccion.
MAX_BLOCK_LINES = 80

# Rutas de repo citadas entre acentos graves dentro de esa zona.
CITED_PATH = re.compile(r"`(docs/[A-Za-z0-9._/-]+\.(?:md|json|kts))`")

# Una cita de ADR con la unidad a la que se le atribuye, escritas juntas
# para que la atribucion sea legible y parseable a la vez:
#   `.../ADR-K002.md` (K1)
# El separador entre la cita y su unidad admite `>` del blockquote: la
# atribucion suele caer en la linea siguiente. MEDIDO: con `\s*` el par
# ADR-K003/(K2) no casaba, la lista de pares salia vacia y el contrato
# daba verde SIN comprobar esa atribucion — precisamente la que existe
# para cazar. Un contrato que pasa mirando menos de lo que dice mirar es
# peor que no tener contrato.
#
# Se captura la RUTA completa, no el nombre de fichero. MEDIDO: hay dos
# `ADR-K002.md` bajo `docs/` — uno versionado en consumer-contracts y otro
# en `docs/cognicode-kotlin-roadmap/`, que no lo esta. Resolver por nombre
# seria elegir entre dos sin saber cual; resolver por la ruta que §11 cita
# no deja esa decision abierta.
ADR_WITH_UNIT = re.compile(
    r"`(docs/[A-Za-z0-9._/-]*ADR-[A-Za-z0-9-]+\.md)`[\s>]*"
    r"\((K[0-9][0-9.A-Za-z–-]*)\)"
)

# Lo que el propio ADR declara. Es la autoridad: aqui no se reinterpreta.
ADR_SCOPE = re.compile(r"\*\*Aplica a:\*\*\s*([^\n.]+)")


def open_units_block(text: str) -> str:
    """El parrafo de §11 que enumera las unidades realmente abiertas.

    Acotado a proposito. MEDIDO: sin techo, el recorrido se seguia por el
    resto del documento entero, porque §11 abre un blockquote largo y todo
    lo que viene despues sigue siendo cita — tomaba 1575 lineas en vez del
    parrafo. Un contrato que escanea el documento entero no esta mirando lo
    que dice mirar. El techo no recorta por comodidad: al superarlo, el
    formato cambio y el contrato debe dejar de poder afirmar nada, en vez
    de siguiendo citas sueltas.

    Si el ancla desaparece, lo mismo: un contrato que se apaga cuando
    cambia el formato inspecciona cero y sale verde.
    """
    start = text.find(OPEN_UNITS_ANCHOR)
    assert start != -1, (
        f"FAIL: {ROADMAP.name} ya no contiene el ancla "
        f"{OPEN_UNITS_ANCHOR!r}. Este contrato no puede declararse verde "
        "sobre texto que no leyo."
    )
    tail = text[start:].splitlines()
    block = [tail[0]]
    for line in tail[1:]:
        if not line.startswith(">"):
            break
        block.append(line)
        if len(block) > MAX_BLOCK_LINES:
            raise AssertionError(
                f"FAIL: el bloque de unidades abiertas de {ROADMAP.name} pasa "
                f"de {MAX_BLOCK_LINES} lineas de cita. El formato cambio y "
                "este contrato no puede afirmar que las citas que encuentra "
                "son las que corresponden."
            )
    return "\n".join(block)


def tracked(paths: list[str]) -> set[str]:
    """Ficheros versionados. `git ls-files` es la unica verdad de eso."""
    out = subprocess.run(
        ["git", "ls-files", "--", *paths],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return {line for line in out.stdout.splitlines() if line}


def test_every_cited_authority_is_versioned() -> None:
    """Una cita que no esta versionada no es una cita: es una nota local."""
    block = open_units_block(ROADMAP.read_text(encoding="utf-8"))
    cited = CITED_PATH.findall(block)
    assert cited, (
        f"FAIL: no se encontro ninguna ruta citada en el bloque de unidades "
        f"abiertas de {ROADMAP.name}. Sin citas, este contrato no comprueba "
        "nada y solo puede pasar."
    )
    present = tracked(cited)
    missing = sorted(set(cited) - present)
    assert not missing, (
        f"FAIL: {ROADMAP.name} cita como autoridad rutas que git no versiona: "
        f"{missing}. Un clon limpio no las resuelve, asi que el puntero no "
        "lleva a nada."
    )


def test_every_cited_adr_claims_the_unit_it_is_attributed() -> None:
    """El ADR declara su alcance en su linea 3; §11 no puede contradecirlo."""
    block = open_units_block(ROADMAP.read_text(encoding="utf-8"))
    pairs = ADR_WITH_UNIT.findall(block)
    assert pairs, (
        f"FAIL: no se encontro ninguna cita de ADR con su unidad en "
        f"{ROADMAP.name}. El formato esperado es `.../ADR-XXX.md` (K<n>), "
        "que es lo que hace falta para comprobar la atribucion."
    )

    versioned = tracked([path for path, _ in pairs])
    problems: list[str] = []
    for adr_path, unit in pairs:
        if adr_path not in versioned:
            problems.append(f"{unit} se atribuye a `{adr_path}`, sin versionar")
            continue
        adr = REPO_ROOT / adr_path
        scope = ADR_SCOPE.search(adr.read_text(encoding="utf-8"))
        assert scope is not None, (
            f"FAIL: {adr_path} no declara '**Aplica a:**'. Un ADR que no "
            "declara su alcance no puede desmentir una atribucion, asi que "
            "este contrato no tendria nada que comprobar."
        )
        if unit not in scope.group(1):
            problems.append(
                f"{adr_path} se atribuye a {unit}, pero declara "
                f"'Aplica a: {scope.group(1).strip()}'"
            )
    assert not problems, (
        "FAIL: atribucion de autoridad que el propio ADR desmiente: "
        + "; ".join(problems)
    )


# --- Parte 3, bloque 2: el cierre ---------------------------------------------

# El Journal declara cierres con estas formas. Se listan porque son las que
# el registro usa de verdad, no porque el formato sea estable.
CLOSURE = re.compile(r"\b(?:closed|acceptance:\s*PASS)\b", re.IGNORECASE)
INLINE_CODE = re.compile(r"`[^`\n]*`")


def _asserted(line: str) -> str:
    """La linea sin sus citas en codigo en linea: citar no es afirmar."""
    return INLINE_CODE.sub(" ", line)


def _journal_closure_lines(unit: str) -> list[str]:
    """Lineas del Journal que declaran cerrada `unit`.

    `\\b` alrededor del id evita que `H4.1` case con `H4.10` y que `H5`
    case con un token mayor. La coincidencia es por *linea*: el cierre de un
    ciclo se cuenta en su linea, no repartido en varias.
    """
    pat = re.compile(rf"\b{re.escape(unit)}\b")
    out = []
    for line in JOURNAL.read_text(encoding="utf-8").splitlines():
        asserted = _asserted(line)
        if pat.search(asserted) and CLOSURE.search(asserted):
            out.append(line.strip())
    return out


def section11_open_units() -> list[str]:
    """Ids en negrita del trailer 'unidades realmente abiertas' de §11.

    Solo el bloque final de §11: las lineas de progreso de CU-1/CU-2 estan
    repletas de `CU-x.y` y no son la lista de abiertas.
    """
    text = ROADMAP.read_text(encoding="utf-8")
    m = re.search(
        r"unidades realmente abiertas son.*?(?=\n\n|\n<!--|\Z)", text, re.DOTALL
    )
    if not m:
        return []
    return re.findall(r"\*\*([A-Z]\d+(?:\.\d)?)\*\*", m.group(0))


def closed_units_in_journal() -> set[str]:
    """Unidades que cualquier linea del Journal declara cerradas.

    Recorre por linea: un cierre es un hecho de la linea completa, no del
    identificador suelto. Buscar el id en un Journal entero perderia el
    contexto que distingue "H3 arranco" de "H3 cerro".
    """
    found: set[str] = set()
    for line in JOURNAL.read_text(encoding="utf-8").splitlines():
        line = _asserted(line)
        if not CLOSURE.search(line):
            continue
        for m in re.finditer(r"\b([A-Z]\d+(?:\.\d)?)\b", line):
            found.add(m.group(1))
    return found


# ---------- assertions (each is one mutation seen to fail) ----------

def test_the_journal_actually_declares_closures() -> None:
    """Control anti-vacuidad: sin cierres en el Journal, la regla mira sin ver.

    MEDIDO: el Journal registra cierres desde el programa PRF. Si esta falla,
    o el Journal cambio de formato, o la regla siguiente ha muerto en
    silencio — y una regla muerta es peor que no tenerla, porque parece
    que vigila.
    """
    found = closed_units_in_journal()
    assert found, (
        "FAIL: no se encontro ninguna unidad cerrada en "
        f"{JOURNAL.name}. La regla de §11 no tendria contra que aplicarse "
        "y pasaria sin comprobar nada."
    )


def test_section11_does_not_list_a_unit_the_journal_closed() -> None:
    """M1: el trailer de abiertas contra el Journal.

    Es la regla que impide reconstruir trabajo ya cerrado: una unidad
    listada como abierta con su cierre escrito abajo manda a rehacerla.
    """
    stale = []
    for unit in sorted(set(section11_open_units())):
        lines = _journal_closure_lines(unit)
        if lines:
            # Dos cierres independientes si los hay: el que cierra el ciclo
            # concreto y el que cierra el bloque entero. Citar solo el
            # primero deja al lector dudando de si fue casualidad.
            cited = "\n      ".join(l[:110] for l in lines[:2])
            stale.append(f"{unit} — Journal:\n      {cited}")
    assert not stale, (
        "FAIL: unidades que ROADMAP §11 lista como abiertas y el Journal ya "
        "cerro:\n  - " + "\n  - ".join(stale) + "\n"
        "Quitalas del trailer. Reconstruir trabajo cerrado es el fallo caro "
        "que este contrato existe para evitar."
    )


def test_a_quoted_closure_is_not_a_closure() -> None:
    """Una cita entre comillas inversas no cierra nada; el texto llano si.

    Los recibos del Journal describen mutaciones con el texto que inyectaron
    (`H24 closed`). El Journal es append-only, asi que la regla no puede
    pedirle al recibo que calle: tiene que distinguir citar de afirmar.
    """
    global JOURNAL
    real = JOURNAL
    try:
        with tempfile.TemporaryDirectory() as tmp:
            fake = Path(tmp) / "JOURNAL.md"
            fake.write_text(
                "Mutacion: `H24 closed` anadido al JOURNAL +2/0/+4.\n",
                encoding="utf-8",
            )
            JOURNAL = fake
            assert _journal_closure_lines("H24") == []
            assert "H24" not in closed_units_in_journal()
            fake.write_text(
                "Mutacion: `H24 closed` citada.\nH24 closed en este ciclo.\n",
                encoding="utf-8",
            )
            assert _journal_closure_lines("H24") == ["H24 closed en este ciclo."]
            assert "H24" in closed_units_in_journal()
    finally:
        JOURNAL = real


def test_section11_names_at_least_one_open_unit() -> None:
    """M2: un trailer de abiertas vacio no es agenda, es un hueco.

    Si §11 deja de nombrar unidades, el siguiente agente no tiene por donde
    empezar — que es como se pierde una unidad, no como se encuentra.
    """
    units = sorted(set(section11_open_units()))
    assert units, (
        "FAIL: el trailer 'unidades realmente abiertas' de ROADMAP §11 no "
        "nombra ninguna unidad. O el formato cambio, o §11 quedo sin agenda."
    )


def test_section11_control_all_green() -> None:
    """Control: en estado nominal, las tres anteriores pasan."""
    test_the_journal_actually_declares_closures()
    test_section11_does_not_list_a_unit_the_journal_closed()
    test_section11_names_at_least_one_open_unit()


# --- Parte 4: libro mayor y declaracion de la mision ---

# ======================= PARTE A: libro mayor de bloques =====================

WORK_LOOP = REPO_ROOT / "odd" / "tasks" / "2026-Q4-work-loop.md"

# Especificaciones detailizadas y archivadas. Se leen, no se reescriben.
PROTECTED = {"b5-safe-refactoring-authorized-changes.md"}

CLOSED_MARKER = "CLOSED"


def goal_docs() -> list[Path]:
    return sorted(
        p
        for p in (REPO_ROOT / "odd" / "tasks").glob("b[1-6]-*.md")
        if p.name not in PROTECTED
    )


def released_blocks() -> dict[str, str]:
    """`{bloque: tag}` segun ROADMAP §10 — la unica autoridad.

    Solo filas con tag de release publicado. Una fila que aun no tiene tag
    es trabajo en curso, que es exactamente lo que §10 no declara cerrado.
    """
    text = ROADMAP.read_text(encoding="utf-8")
    out: dict[str, str] = {}
    for block, tag in re.findall(
        r"\|\s*\*\*B([1-6])\*\*\s*\|\s*\**(v0\.\d+\.\d+)\**\s*\|", text
    ):
        out[f"B{block}"] = tag
    return out


def _declared_state(text: str) -> str | None:
    """La primera palabra tras `**Estado**:` en la cabecera."""
    m = re.search(r"\*\*Estado\*\*\s*:\s*([A-Za-z_]+)", text)
    return m.group(1).upper() if m else None


def block_of(doc: Path) -> str | None:
    """`b5-...` -> `B5`, por el nombre del fichero."""
    m = re.match(r"b([1-6])-", doc.name)
    return f"B{m.group(1)}" if m else None


def goal_doc_states() -> dict[str, tuple[str | None, Path]]:
    out: dict[str, tuple[str | None, Path]] = {}
    for doc in goal_docs():
        block = block_of(doc)
        if block:
            out[block] = (_declared_state(doc.read_text(encoding="utf-8")), doc)
    return out


def work_loop_states() -> dict[str, str]:
    """El estado por bloque en la tabla del work-loop."""
    text = WORK_LOOP.read_text(encoding="utf-8")
    out: dict[str, str] = {}
    for block, state in re.findall(
        r"\|\s*B([1-6])\s*\|\s*[^|]*\|\s*([A-Za-z_]+)", text
    ):
        out[f"B{block}"] = state.upper()
    return out


# ---------- assertions (each is one mutation seen to fail) ----------

def test_every_released_block_has_a_goal_doc() -> None:
    """Control: §10 y los goal docs tienen que poder referirse el uno al otro.

    Sin esto, un goal doc renombrado haria que las reglas siguientesFinder
    vacio un bloque sin comprobar nada — un contrato que pasa de no mirar.
    """
    docs = {block for block, _ in goal_doc_states().items()}
    missing = sorted(set(released_blocks()) - docs)
    assert not missing, (
        f"FAIL: ROADMAP §10 publica {missing} pero no hay goal doc "
        f"correspondiente en odd/tasks/. Las reglas siguientes no podrian "
        f"comprobar nada sobre esos bloques."
    )


def test_a_released_block_does_not_declare_itself_pending() -> None:
    """M1: cabecera del goal doc contra §10.

    Es la regla que impide reconstruir trabajo ya publicado: una cabecera
    `PENDING` sobre un tag vendido manda a rehacer el bloque entero.
    """
    states = goal_doc_states()
    wrong = []
    for block, tag in sorted(released_blocks().items()):
        declared = states.get(block, (None, None))[0]
        if declared != CLOSED_MARKER:
            wrong.append(
                f"{block} (tag {tag}) declara {declared!r} en "
                f"{states.get(block, (None, '?'))[1]}"
            )
    assert not wrong, (
        "FAIL: bloques publicados en ROADMAP §10 que su goal doc no declara "
        "CLOSED:\n  - " + "\n  - ".join(wrong) + "\n"
        "O el goal doc va atras, o §10 va erroneo. Reconcilia antes de "
        "trabajar sobre el bloque."
    )


def test_work_loop_table_agrees_with_the_roadmap() -> None:
    """M2: la tabla de cadencia contra §10.

    Es la segunda fuente de verdad, y la que un agente lee primero para
    saber por donde empezar. Si dice `BLOCKED (B1 prerequisite)` cuatro
    dias despues del cierre de B1, manda a parar donde hay trabajo.
    """
    table = work_loop_states()
    wrong = []
    for block, tag in sorted(released_blocks().items()):
        state = table.get(block)
        if state != CLOSED_MARKER:
            wrong.append(f"{block} (tag {tag}) figura como {state!r}")
    assert not wrong, (
        "FAIL: la tabla de `odd/tasks/2026-Q4-work-loop.md` contradice a "
        "ROADMAP §10 en:\n  - " + "\n  - ".join(wrong) + "\n"
        "La tabla manda a detenerse en bloques que ya estan cerrados."
    )


def test_control_ledger_all_green() -> None:
    """Control: en estado nominal, las tres anteriores pasan.

    Si esto falla con las fuentes ya reconciliadas, el bug es del contrato.
    """
    test_every_released_block_has_a_goal_doc()
    test_a_released_block_does_not_declare_itself_pending()
    test_work_loop_table_agrees_with_the_roadmap()


# ======================= PARTE B: declaracion de la mision ===================

DECLARATION = REPO_ROOT / "goal-compass.json"

# Los campos que el cargador de candidatos lee. Copiado de
# `adapters/mission.js::parseCandidate`. Un campo fuera de esta lista se
# pierde en silencio, que es justo lo que hay que cazar.
CAMPOS_QUE_EL_CARGADOR_LEE = {
    "candidateId",
    "objective",
    "whyNow",
    "evidenceRefs",
    "doneWhen",
    "scope",
    "risk",
    "priority",
    "prerequisites",
    "source",
    "tokenBudget",
}

# Campos que el cargador no lee pero que se permiten: son de lectura humana y
# no rompen nada. Lo que no se permite es un campo que *parezca* uno de los
# buenos, porque ahi es donde el autor cree que ha dejado la evidencia.
CAMPOS_TOLERADOS = {"title", "unit"}

# Campos que se parecen a uno bueno pero no lo son. Sus nombres alternativos
# son el fallo medido: el valor parece correcto y la ruta existe.
PARECIDOS_PERIGOSOS = {
    "evidence": "evidenceRefs",
    "evidenceRef": "evidenceRefs",
    "evidences": "evidenceRefs",
    "done": "doneWhen",
    "criteria": "doneWhen",
    "criterion": "doneWhen",
    "id": "candidateId",
}


def _read_declaration() -> str:
    if not DECLARATION.is_file():
        return ""
    return DECLARATION.read_text(encoding="utf-8")


def _split_reference(ref: str) -> tuple[str, str | None]:
    """Separa la referencia en ruta y ancla."""
    index = ref.find("#")
    if index == -1:
        return ref.strip(), None
    return ref[:index].strip(), ref[index + 1:].strip()


def _anchor_exists(content: str, anchor: str) -> bool:
    """El ancla tiene que estar en el fichero: id explicito o titulo de seccion."""
    if anchor == "":
        return True
    escaped = re.escape(anchor)
    if re.search(rf'(?:id|name)\s*=\s*["\']{escaped}["\']', content):
        return True
    if re.match(r"^[\d.]+$", anchor):
        if re.search(rf"^#{{1,6}}\s+{escaped}(?:\s|$)", content, flags=re.MULTILINE):
            return True
    return False


def test_declaration_exists_and_parses() -> None:
    """Sin esto, ninguna otra regla mira nada."""
    assert DECLARATION.is_file(), (
        f"FAIL: no existe {DECLARATION}. La mision se declara ahi."
    )
    raw = _read_declaration()
    try:
        data = json.loads(raw)
    except json.JSONDecodeError as err:
        raise AssertionError(f"FAIL: {DECLARATION.name} no es JSON valido: {err}")
    assert isinstance(data, dict), f"FAIL: {DECLARATION.name} no es un objeto"
    assert isinstance(data.get("backlog"), list), (
        f"FAIL: {DECLARATION.name} no tiene una lista 'backlog'."
    )
    assert data["backlog"], (
        "FAIL: el backlog esta vacio. Una mision sin trabajo declarado no encadena nada."
    )


def test_every_candidate_uses_the_fields_the_loader_reads() -> None:
    """Un campo que el cargador no lee se pierde en silencio.

    El caso peligroso es un campo con nombre parecido al bueno: su valor
    parece correcto y su ruta existe, asi que nadie se entera de que el
    cargador no lo mira. Es el fallo medido el 2026-10-21.
    """
    data = json.loads(_read_declaration())
    problems: list[str] = []
    for index, candidate in enumerate(data["backlog"]):
        if not isinstance(candidate, dict):
            problems.append(f"backlog[{index}] no es un objeto")
            continue
        candidate_id = candidate.get("candidateId", f"backlog[{index}]")
        for field in candidate:
            if field in CAMPOS_QUE_EL_CARGADOR_LEE or field in CAMPOS_TOLERADOS:
                continue
            parecido = PARECIDOS_PERIGOSOS.get(field)
            if parecido:
                problems.append(
                    f"{candidate_id}: el campo '{field}' no lo lee el cargador; "
                    f"el que si se lee es '{parecido}'"
                )
            else:
                problems.append(
                    f"{candidate_id}: el campo '{field}' no lo lee el cargador"
                )
    assert not problems, "FAIL: campos que el cargador no lee:\n  " + "\n  ".join(problems)


def test_every_candidate_carries_evidence_and_a_criterion() -> None:
    """Las dos condiciones que el cargador exige para que un candidato sea trabajo.

    Sin evidencia no se puede comprobar nada; sin criterio de cierre observable
    no se puede saber si esta hecho. Las dos se descartan en silencio.
    """
    data = json.loads(_read_declaration())
    problems: list[str] = []
    for candidate in data["backlog"]:
        candidate_id = candidate.get("candidateId", "(sin identificador)")
        evidence = candidate.get("evidenceRefs")
        if not isinstance(evidence, list) or not evidence:
            problems.append(f"{candidate_id}: sin 'evidenceRefs' utilizable")
        done_when = candidate.get("doneWhen")
        if not isinstance(done_when, list) or not done_when:
            problems.append(f"{candidate_id}: sin criterio de cierre observable")
    assert not problems, (
        "FAIL: candidatos que el cargador descartaria:\n  " + "\n  ".join(problems)
    )


def test_the_queue_is_not_written_backwards() -> None:
    """El selector ordena de mayor a menor prioridad.

    Una cola escrita al reves hace que la cadena empiece por el final: el
    primer candidato del fichero tiene que llevar el numero mas alto.
    """
    data = json.loads(_read_declaration())
    priorities = [c.get("priority") for c in data["backlog"]]
    assert all(isinstance(p, int) for p in priorities), (
        "FAIL: hay candidatos sin prioridad numerica."
    )
    assert priorities == sorted(priorities, reverse=True), (
        f"FAIL: la cola esta escrita al reves. El selector ordena de mayor a "
        f"menor, asi que el primer candidato ({priorities[0]}) tiene que llevar "
        f"el numero mas alto, no el mas bajo. Orden actual: {priorities}"
    )


def test_every_reference_points_at_a_real_file() -> None:
    """Una referencia que no apunta a nada no es evidencia.

    La ruta es relativa al proyecto donde vive la declaracion, y el ancla, si
    la hay, tiene que estar en el fichero.
    """
    data = json.loads(_read_declaration())
    problems: list[str] = []
    for candidate in data["backlog"]:
        candidate_id = candidate.get("candidateId", "(sin identificador)")
        for ref in candidate.get("evidenceRefs", []):
            path, anchor = _split_reference(str(ref))
            if not path:
                problems.append(f"{candidate_id}: la referencia '{ref}' esta vacia")
                continue
            resolved = (REPO_ROOT / path).resolve()
            if not resolved.exists():
                problems.append(
                    f"{candidate_id}: la referencia '{ref}' no apunta a ningun fichero"
                )
                continue
            if anchor and resolved.is_file():
                content = resolved.read_text(encoding="utf-8", errors="replace")
                if not _anchor_exists(content, anchor):
                    problems.append(
                        f"{candidate_id}: la referencia '{ref}' no encuentra el ancla "
                        f"'{anchor}' en {path}"
                    )
    assert not problems, (
        "FAIL: referencias que no apuntan a nada:\n  " + "\n  ".join(problems)
    )


def test_control_mission_all_green() -> None:
    """En estado nominal, las reglas anteriores pasan.

    Si esto falla sobre una declaracion sana, hay un bug en el contrato, no en
    la declaracion.
    """
    test_declaration_exists_and_parses()
    test_every_candidate_uses_the_fields_the_loader_reads()
    test_every_candidate_carries_evidence_and_a_criterion()
    test_the_queue_is_not_written_backwards()
    test_every_reference_points_at_a_real_file()


def main() -> int:
    """Ejecuta las comprobaciones y devuelve 1 si alguna falla.

    El contrato de los contratos exige esta forma: un fichero que acumula
    sus hallazgos en una lista tiene que convertir esa lista en un código
    de salida distinto de cero, o ejecutarlo a mano informaría de un
    éxito que no ocurrió.
    """
    failures: list[str] = []
    for name, function in sorted(globals().items()):
        if not name.startswith("test_") or not callable(function):
            continue
        try:
            function()
            print(f"PASS: {name}")
        except AssertionError as error:
            failures.append(f"FAIL: {name}: {error}")
            print(f"FAIL: {name}: {error}", file=sys.stderr)
        except Exception as error:  # noqa: BLE001 — el contrato informa, no aborta
            failures.append(f"ERROR: {name}: {error}")
            print(f"ERROR: {name}: {error}", file=sys.stderr)

    if failures:
        print(f"\n{len(failures)} contract(s) failed", file=sys.stderr)
        return 1
    units = sorted(set(section11_open_units()))
    print(f"ROADMAP section 11: OK ({len(units)} abiertas: {', '.join(units)})")
    print("\nall contracts passed")
    return 0



if __name__ == "__main__":
    raise SystemExit(main())
