#!/usr/bin/env python3
"""Que los wrappers de tests fast/full cumplan su contrato minimo.

MEDIDO 2026-10-10. AGENTS.md § 'Estrategia de tests (fast / full)' declara
una disciplina contractual: durante evolutivos, `bash scripts/test-fast.sh`;
antes de release, `bash scripts/test-full.sh`. El selector CR-08
(`scripts/ci/select-suites.sh`) ya pineaba que suite correr; el wrapper
lo expone al flujo del desarrollador.

Tres verificaciones que el contrato no admite que se rompan:

  (1) los dos wrappers existen y son ejecutables;
  (2) `test-fast.sh` delega en `select-suites.sh` y en
      `known_failures.yaml` para distinguir regresiones reales;
  (3) `test-full.sh` corre fmt + clippy `-D warnings` + tests --workspace
      + tests --workspace --doc (la bateria completa que el gate exige).

Sin estas guardas, una refactorizacion inocente podria borrar el wrapper
o cambiar su cableado y nadie se enteraria hasta que el gate fallase en
silencio. Pine los caminos, no los contenidos.

MEDIDO 2026-10-15. M5 era un gate sin dientes: grepeaba los cuatro needles
(`cargo fmt`, `cargo clippy`, `cargo test`, `--doc`) contra el TEXTO CRUDO de
`test-full.sh`, y los cuatro aparecen en el comentario de cabecera (lineas 5-8),
antes de una sola linea de codigo. Borrar las cuatro invocaciones `run_step` de
las lineas 41-44 dejaba el test en verde: la bateria de release, que AGENTS.md
declara prerrequisito duro de merge a `main` y de cualquier bump SemVer, podia
no correr nada y seguir reportando PASS.

El arreglo es el mismo que ya usa `test_candidate_packaging.py`: un comentario no
se ejecuta, asi que no puede ser la confusion que se busca, y por eso las
comparaciones de contenido van contra el texto sin comentarios. `strip_comments`
se IMPORTA de ese contrato y no se reimplementa aqui: una segunda copia seria
una segunda fuente de verdad para la misma regla, que es exactamente lo que este
repositorio se prohibe hacer.
"""
from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

# Sibling contract, same directory. `run_contract_tests.py` puts a contract's
# own directory on sys.path, and running this file directly puts it there too,
# so the import resolves identically under both entry points — the same reason
# `run_contract_tests.load_module` documents. Imported, not copied: one rule.
from test_candidate_packaging import strip_comments

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
FAST = REPO_ROOT / "scripts" / "test-fast.sh"
FULL = REPO_ROOT / "scripts" / "test-full.sh"
SELECTOR = REPO_ROOT / "scripts" / "ci" / "select-suites.sh"
BASELINE = REPO_ROOT / "scripts" / "known_failures.yaml"
BASELINE_CHECK = REPO_ROOT / "scripts" / "check_known_failures.py"


def _read(path: Path) -> str:
    if not path.is_file():
        return ""
    return path.read_text(encoding="utf-8")


def _code(path: Path) -> str:
    """El wrapper SIN comentarios: lo que bash llega a ejecutar.

    Un comentario no se ejecuta. Comparar contra el texto crudo permite que un
    contrato se satisfied por la prosa que explica que deberia hacer, que es
    justo el modo de fallo que estos wrappers no admiten. Solo se descartan
    lineas completas de comentario; un `#` al final de una linea de codigo si se
    cuenta, porque ahi no se puede distinguir de una cadena — la misma
    limitacion que declara `strip_comments`.
    """
    return strip_comments(_read(path))


# ---------- assertions (each pins one mutation that the contract refuses) ----------


def test_fast_wrapper_exists_and_is_executable() -> None:
    """M1: scripts/test-fast.sh debe existir y ser ejecutable.

    Sin el binario, el desarrollador no puede invocar la disciplina fast.
    AGENTS.md § 'Estrategia de tests' lo referencia como comando canonico.
    """
    assert FAST.is_file(), f"FAIL: {FAST} no existe"
    assert os.access(FAST, os.X_OK), f"FAIL: {FAST} no es ejecutable"


def test_full_wrapper_exists_and_is_executable() -> None:
    """M2: scripts/test-full.sh debe existir y ser ejecutable.

    El gate de release exige bateria completa; el wrapper la expone.
    """
    assert FULL.is_file(), f"FAIL: {FULL} no existe"
    assert os.access(FULL, os.X_OK), f"FAIL: {FULL} no es ejecutable"


def test_fast_delegates_to_selector() -> None:
    """M3: test-fast.sh debe invocar select-suites.sh.

    El wrapper no recalcula la seleccion; consume la salida JSON del
    selector CR-08. Si lo cambia, se pierde la trazabilidad de la
    decision de que correr (cambio de paths -> cambio de suites).
    """
    text = _code(FAST)
    assert "select-suites.sh" in text, (
        f"FAIL: {FAST.name} no referencia scripts/ci/select-suites.sh. "
        f"El wrapper DEBE delegar en el selector CR-08; no recalcular."
    )


def test_fast_uses_known_failures_baseline() -> None:
    """M4: test-fast.sh debe aplicar known_failures.yaml sobre core/lib.

    La red minima que evita que una regresion nueva se pierda en el
    ruido conocido. Si la quita, el feedback de fast pierde precision.
    """
    text = _code(FAST)
    assert "check_known_failures.py" in text or "known_failures" in text, (
        f"FAIL: {FAST.name} no aplica scripts/check_known_failures.py. "
        f"El baseline check es la red minima contra regresiones nuevas."
    )


def test_full_runs_the_release_battery() -> None:
    """M5: test-full.sh debe correr fmt + clippy + tests + doc-tests.

    El gate de release exige la bateria completa. Si el wrapper omite
    alguno, una falla silente pasara hasta el merge.

    MEDIDO 2026-10-15: esto grepeaba el TEXTO CRUDO, asi que los cuatro
    needles se satisfacian con el comentario de cabecera y las cuatro
    invocaciones `run_step` reales podian desaparecer sin que nadie se
    enterara. Ver el docstring del modulo.
    """
    text = _code(FULL)
    missing = []
    for needle in ("cargo fmt", "cargo clippy", "cargo test", "--doc"):
        if needle not in text:
            missing.append(needle)
    assert not missing, (
        f"FAIL: {FULL.name} no corre {missing}. "
        f"El gate de release exige fmt + clippy + test + doc-test."
    )


def test_full_invokes_known_failures_baseline() -> None:
    """M6: test-full.sh debe invocar check_known_failures al final.

    La bateria completa + baseline = exit 0 si no hay regresiones nuevas.
    Si omite el baseline, regresiones nuevas pasan en verde.
    """
    text = _code(FULL)
    assert "check_known_failures.py" in text or "known_failures" in text, (
        f"FAIL: {FULL.name} no invoca scripts/check_known_failures.py. "
        f"El baseline es la red contra regresiones nuevas."
    )


def test_both_wrappers_have_set_euo_pipefail() -> None:
    """M7: ambos wrappers deben usar `set -uo pipefail` (o equivalente).

    Sin errexit + nounset + pipefail, los wrappers silencian errores
    parciales (cargo falla con un test, el siguiente pasa y el rc final
    es 0). Disciplina minima de shell.
    """
    for path in (FAST, FULL):
        text = _code(path)
        assert "pipefail" in text, (
            f"FAIL: {path.name} no declara 'set -.*pipefail'. "
            f"Sin pipefail los rc parciales se silencian."
        )


def test_selector_dependency_exists() -> None:
    """M8: el selector CR-08 (dependencia de fast) debe existir.

    El wrapper lo invoca via bash $SELECTOR; si el selector desaparece,
    fast se rompe en runtime. Pine la dependencia.
    """
    assert SELECTOR.is_file(), (
        f"FAIL: {SELECTOR} no existe pero {FAST.name} lo invoca. "
        f"Discrepancia entre wrapper y dependencia declarada."
    )


# ---------- las ramas de run_suite(): las que no podian fallar ----------

MEDIDO_2026_10_15 = """
MEDIDO 2026-10-15. `run_suite()` tenia dos ramas que no podian fallar:

  - `*)` hacia `echo WARN` + `return 0`. Una suite que el selector emite y
    el runner no conoce se saltaba EN VERDE. Un WARN no cambia el rc, asi
    que lo unico quehacian era escribir en stderr, y stderr no es veredicto.
  - `merge)` corria `cargo test --workspace --lib --quiet` y devolvia 0. La
    cabecera del selector define `merge` como "merge-gate completo (e2e +
    pineados)", que son las stages del lane, no una suite de cargo. Correr
    `--lib` en su nombre reportaba exito sin haber ejecutado ni un test
    contractual de integracion.

Ninguna de las dos estaba cubierta: este fichero solo comprobaba que la
CADENA "select-suites.sh" apareciera en `test-fast.sh`. Las tres guardas de
abajo se ejecutan de verdad —extraen la funcion y la llaman en bash— porque
una rama de shell no se demuestra leyendola como texto.
"""


def run_suite_case(suite: str) -> subprocess.CompletedProcess[str]:
    """Ejecuta `run_suite <suite>` de verdad, en bash, y devuelve su rc.

    Se extrae el cuerpo de la funcion tal cual y se ejecuta en un subshell
    aislado. Se ejecuta en vez de inspeccionarse porque las ramas que hay que
    demostrar son las de EJECUCION: lo que se comprueba es el codigo de
    salida, que es lo que fast convierte en veredicto, y no una cadena que
    este contrato podria satisfacerse con un comentario.
    """
    text = _read(FAST)
    start = text.find("run_suite() {")
    assert start != -1, f"FAIL: {FAST.name} ya no define run_suite()"
    end = text.find("\n}\n", start)
    assert end != -1, f"FAIL: no se pudo delimitar run_suite() en {FAST.name}"
    body = text[start : end + 3]

    with tempfile.TemporaryDirectory() as directory:
        script = Path(directory) / "run_suite_only.sh"
        script.write_text(
            "#!/usr/bin/env bash\nset -uo pipefail\n" + body + f'\nrun_suite "$1"\n',
            encoding="utf-8",
        )
        return subprocess.run(
            ["bash", str(script), suite],
            capture_output=True,
            text=True,
            check=False,
        )


def test_an_unknown_suite_is_a_hard_error_not_a_warning() -> None:
    """M10: una suite desconocida debe salir con rc != 0.

    Una suite desconocida es una divergencia entre el selector y el wrapper,
    no una novedad de la que avisar: o el wrapper no sabe correrla, o el
    selector no deberia emitirla. Devolver 0 hacia que `fast` reportara
    verde sin haber corrido esa suite.
    """
    done = run_suite_case("definitely_not_a_suite")
    assert done.returncode != 0, (
        f"FAIL: run_suite() devolvio rc=0 para una suite desconocida. "
        f"`fast` la saltaria en verde. stderr:\n{done.stderr}"
    )
    assert "ERROR" in done.stderr, (
        f"FAIL: el fallo de suite desconocida debe ir a stderr marcado como "
        f"ERROR, no como aviso. stderr:\n{done.stderr}"
    )


def test_the_merge_suite_is_not_silently_substituted() -> None:
    """M11: `merge` no se puede correr desde fast, y hay que decirlo.

    El comando correcto para `merge` es `bash scripts/test-full.sh`. La rama
    no puede correr otra cosa y devolver 0: eso es reportar verde tras
    `cargo test --workspace --lib`, que no ejecuta ningun test contractual de
    integracion (b5_*, b6_*, prf_*). Fallar cerrado con el comando correcto a
    la vista mantiene `fast` rapido —que es su contrato— sin que pueda
    fingir ser el gate, que es lo que no podia ser.
    """
    done = run_suite_case("merge")
    assert done.returncode != 0, (
        f"FAIL: run_suite() devolvio rc=0 para la suite `merge`. `fast` "
        f"reportaria verde sin correr el merge-gate completo."
    )
    assert "test-full.sh" in done.stderr, (
        f"FAIL: al rechazar `merge`, hay que nombrar el comando correcto "
        f"(`bash scripts/test-full.sh`). Un refusal sin salida no es accionable. "
        f"stderr:\n{done.stderr}"
    )


DOCUMENTED_SUITE = re.compile(r'^\s*#\s*-\s*"([a-z_]+)"\s*:', re.M)
CASE_LABEL = re.compile(r"^\s{4}([a-z_]+)\)", re.M)


def documented_suites() -> set[str]:
    """Suites que la cabecera de `select-suites.sh` promete emitir.

    Se leen del TEXTO CRUDO, comentarios incluidos: en este fichero la
    documentacion ES un comentario, asi que `strip_comments` aqui seria
   =self-destruccion. La excepcion es explicita y no se extiende: para los
    wrappers, un comentario no se ejecuta.
    """
    return set(DOCUMENTED_SUITE.findall(_read(SELECTOR)))


def handled_suites() -> set[str]:
    """Suites que `run_suite()` sabe ejecutar de verdad."""
    text = _read(FAST)
    start = text.find("run_suite() {")
    assert start != -1, f"FAIL: {FAST.name} ya no define run_suite()"
    end = text.find("\n}\n", start)
    assert end != -1, f"FAIL: no se pudo delimitar run_suite() en {FAST.name}"
    return set(CASE_LABEL.findall(text[start : end + 3]))


def test_the_selector_doc_and_the_runner_agree() -> None:
    """M12: lo que el selector documenta es lo que `fast` sabe correr.

    MEDIDO 2026-10-15: la cabecera documentaba `core_int`, que ningun path
    emitia, que no estaba en FULL_BATTERY y que `run_suite()` no tenia. Una
    suite que no puede ocurrir no protege nada y sugiere que el mapeo esta
    mas completo de lo que esta — que es exactamente la forma en que un
    selector se vuelve imposible de leer con confianza.

    Esto no obliga a que las suites documentadas se ejecuten: obliga a que
    el conjunto documentado y el conjunto manejado sean el mismo, de modo que
    anadir una suite exige, en el mismo commit, el caso que la corre.
    """
    documented = documented_suites()
    handled = handled_suites()
    assert documented, (
        f"FAIL: no se pudo leer ninguna suite de la cabecera de {SELECTOR.name}. "
        f"El regex del contrato y el formato de la cabecera han divergido, que "
        f"es un fallo silencioso del propio contrato."
    )
    orphans = sorted(documented - handled)
    assert not orphans, (
        f"FAIL: {SELECTOR.name} documenta las suites {orphans} pero "
        f"{FAST.name} no tiene caso para ellas en run_suite(). "
        f"Documentada y no ejecutable, o ejecutable y no documentada."
    )
    undocumented = sorted(handled - documented)
    assert not undocumented, (
        f"FAIL: run_suite() maneja {undocumented} y la cabecera de "
        f"{SELECTOR.name} no las documenta."
    )


def test_baseline_dependencies_exist() -> None:
    """M9: known_failures.yaml y check_known_failures.py deben existir.

    El baseline es parte de la red minima; si los ficheros desaparecen,
    el check se rompe en runtime.
    """
    assert BASELINE.is_file(), f"FAIL: {BASELINE} no existe"
    assert BASELINE_CHECK.is_file(), f"FAIL: {BASELINE_CHECK} no existe"


def run_filter_verdict(filter_value: str, selected: int) -> subprocess.CompletedProcess[str]:
    """Ejecuta `filter_verdict()` de verdad con un FILTER y un SELECTED dados.

    Misma razon que `run_suite_case`: las ramas que hay que demostrar son de
    EJECUCION —el rc es lo que `fast` convierte en veredicto— y no una cadena
    que este contrato podria satisfacerse con un comentario. Se ejecuta sin
    cargo para que el contrato siga siendo barato.
    """
    text = _read(FAST)
    start = text.find("filter_verdict() {")
    assert start != -1, f"FAIL: {FAST.name} ya no define filter_verdict()"
    end = text.find("\n}\n", start)
    assert end != -1, f"FAIL: no se pudo delimitar filter_verdict() en {FAST.name}"
    body = text[start : end + 3]

    with tempfile.TemporaryDirectory() as directory:
        script = Path(directory) / "verdict_only.sh"
        script.write_text(
            "#!/usr/bin/env bash\nset -uo pipefail\n"
            + body
            + f'\nFILTER="{filter_value}"\nSELECTED={selected}\nfilter_verdict\n',
            encoding="utf-8",
        )
        return subprocess.run(
            ["bash", str(script)], capture_output=True, text=True, check=False
        )


def test_a_filter_that_matches_nothing_is_not_a_green_light() -> None:
    """M13: `--filter` acota, y un filtro mal escrito NO sale verde.

    MEDIDO 2026-10-19: `libtest` sale con rc=0 cuando el filtro no casa con
    ninguna prueba. Sin un guard, `--filter h42_typo` y `--filter h42_`
    devolvian el mismo rc: uno habia ejecutado pruebas y el otro ninguna, y
    esa diferencia —que es todo el riesgo de un filtro escrito a mano— era
    invisible. El filtro existe para acotar el trabajo, no para fabricar
    verde.

    El caso D (suites, no fallback) y el caso B (fallback, que sale antes de
    llegar al final del script) ejecutan la MISMA funcion; por eso se
    comprueba la funcion y no un bloque al final.
    """
    miss = run_filter_verdict("h42_typo_inexistente", 0)
    assert miss.returncode != 0, (
        "FAIL: un filtro que no selecciona ninguna prueba devolvio rc=0. "
        f"Eso es un verde sin haber ejecutado un solo test. stderr:\n{miss.stderr}"
    )
    assert "h42_typo_inexistente" in miss.stderr, (
        "FAIL: el rechazo tiene que NOMBRAR el filtro que fallo; un refusal "
        f"anonimo no es accionable. stderr:\n{miss.stderr}"
    )

    hit = run_filter_verdict("h42_filtered", 1)
    assert hit.returncode == 0, (
        f"FAIL: un filtro que SI selecciono pruebas devolvio rc!=0. "
        f"El guard no puede confundir 'acotado' con 'fallido'. stderr:\n{hit.stderr}"
    )


def test_a_filtered_run_announces_that_it_is_not_the_gate() -> None:
    """M14: un run acotado se declara, para que nadie lo lea como veredicto.

    `--dry-run` sale antes de cualquier cargo, asi que esto cuesta un
    arranque de bash y demuestra lo que importa: la linea aparece cuando se
    pasa `--filter` y el baseline se apaga con ella. Un baseline comparado
    contra un subconjunto de fallos no es una medicion, es ruido con
    apariencia de control.
    """
    with tempfile.TemporaryDirectory() as directory:
        script = Path(directory) / "dry.sh"
        # `--dry-run` imprime el plan y sale; `--paths` ahi vacio para que el
        # selector no dependa de que haya cambios en el arbol de quien corre.
        script.write_text(
            f'bash "{FAST}" --filter h42_ --dry-run --paths crates/cognicode-core/src/lib.rs\n',
            encoding="utf-8",
        )
        announced = subprocess.run(
            ["bash", str(script)], capture_output=True, text=True, check=False
        )
        script.write_text(
            f'bash "{FAST}" --dry-run --paths crates/cognicode-core/src/lib.rs\n',
            encoding="utf-8",
        )
        plain = subprocess.run(
            ["bash", str(script)], capture_output=True, text=True, check=False
        )

    assert announced.returncode == 0, (
        f"FAIL: `test-fast.sh --filter ... --dry-run` devolvio "
        f"rc={announced.returncode}. stderr:\n{announced.stderr}"
    )
    assert "NO es evidencia de gate" in announced.stdout, (
        "FAIL: un run filtrado tiene que decir en voz alta que no es el gate. "
        f"Solo asi un verde acotado se lee distinto de un verde de release. "
        f"stdout:\n{announced.stdout}"
    )
    assert "NO es evidencia de gate" not in plain.stdout, (
        "FAIL: el aviso aparece SIN filtro. O es ruido de mas, o el aviso "
        "no esta cableado al flag y sale siempre."
    )


def run_noop_verdict(reason: str) -> subprocess.CompletedProcess[str]:
    """Ejecuta `noop_verdict()` de verdad con un REASON dado.

    Se ejecuta la FUNCION, no el script entero, por la razon que el propio
    script documenta: `--paths` se mezcla con el diff del arbol de quien
    corre, asi que lanzar el wrapper entero mediria el estado del arbol en
    lugar del codigo. Igual que `run_filter_verdict` y `run_suite_case`.
    """
    text = _read(FAST)
    start = text.find("noop_verdict() {")
    assert start != -1, f"FAIL: {FAST.name} ya no define noop_verdict()"
    end = text.find("\n}\n", start)
    assert end != -1, f"FAIL: no se pudo delimitar noop_verdict() en {FAST.name}"
    body = text[start : end + 3]

    with tempfile.TemporaryDirectory() as directory:
        script = Path(directory) / "noop_only.sh"
        script.write_text(
            "#!/usr/bin/env bash\nset -uo pipefail\n"
            + body
            + f'\nREASON="{reason}"\nnoop_verdict\n',
            encoding="utf-8",
        )
        return subprocess.run(
            ["bash", str(script)], capture_output=True, text=True, check=False
        )


def test_a_noop_names_the_lane_that_owns_the_surface() -> None:
    """M15: un noop dice QUE lane cubre la superficie, no "solo metadata".

    MEDIDO 2026-10-19: un cambio solo en `sandbox/**` produce
    `strategy=noop` y `fast` contestaba "cambios no impactan suites (solo
    metadata)". Eso es FALSO — un manifiesto de escenario es un artefacto
    ejecutable con su propio lane (`certification.pipeline.kts`, etapa
    `sandbox` -> `sandbox-ci-smoke`), no documentacion. Y la regla 11 del
    selector justificaba el noop con "vive en ci.yml local", fichero que no
    existe (tampoco `.github/workflows/`).

    Un noop que no nombra a quien cubre la superficie se lee como "aqui no
    hay nada que probar", y de ahi sale el falso verde.

    Este contrato NO obliga a que `fast` corra el sandbox —no puede, exige
    podman rootless—. Obliga a que el noop diga en voz alta que la cubre
    otro lane, y cual es.
    """
    sandbox = run_noop_verdict(
        "no_cargo_suite(sandbox/manifests/e31/demo.json); "
        "sandbox vive en certification.pipeline.kts"
    )
    assert sandbox.returncode == 0, (
        f"FAIL: `noop_verdict` devolvio rc={sandbox.returncode}. Un noop que "
        "detiene el wrapper tiene que salir limpio; si no, `fast` no podria "
        f"terminar. stderr:\n{sandbox.stderr}"
    )
    assert "solo metadata" not in sandbox.stdout, (
        "FAIL: el noop sigue afirmando que el cambio es metadata. Un manifiesto "
        "de escenario es ejecutable: se prueba en certification.pipeline.kts. "
        f"stdout:\n{sandbox.stdout}"
    )
    assert "certification.pipeline.kts" in sandbox.stdout, (
        "FAIL: el noop no nombra el lane que cubre la superficie. Sin el nombre, "
        "'noop' se lee como 'no hay nada que probar aqui', que es el falso verde. "
        f"stdout:\n{sandbox.stdout}"
    )

    # Un noop que NO sea de sandbox no debe inventar un lane que no toca.
    # Sin este caso, el arreglo pasa por imprimir el nombre del lane siempre,
    # que es ruido que hace creer al Sandbox de algo que no dijo.
    docs = run_noop_verdict("doc_or_metadata_changed(docs/roadmap/ROADMAP.md)")
    assert docs.returncode == 0, (
        f"FAIL: `noop_verdict` devolvio rc={docs.returncode} en un noop sin "
        f"lane. stderr:\n{docs.stderr}"
    )
    assert "certification.pipeline.kts" not in docs.stdout, (
        "FAIL: el noop nombra el lane del sandbox para un cambio que no es del "
        "sandbox. Un aviso que aparece donde no toca entrena al lector a "
        f"ignorarlo. stdout:\n{docs.stdout}"
    )


# ---------- el target que se toca es el target que se corre ----------

MEDIDO_2026_10_22 = """
MEDIDO 2026-10-22. `run_suite()` mapeaba las cinco suites a
`cargo test -p <crate> --lib`. Tocar `crates/<crate>/tests/<fichero>.rs`
seleccionaba la suite correcta y no ejecutaba el fichero tocado: el lazo de
feedback daba verde sin haber corrido el test que se acababa de escribir.

El selector ya lo decia en su cabecera y lo dejaba como decision pendiente:
"cerrar esa brecha es una decision propia -correr 65 binarios de test
convierte `fast` en minutos-". La via que no convierte `fast` en minutos es
correr el target que el path toca, que es el unico que puede hablar del
cambio.
"""


def fast_run_in_a_lab(tmp: Path, paths: str) -> tuple[subprocess.CompletedProcess[str], str]:
    """Ejecuta el wrapper de verdad en un arbol git de laboratorio.

    El wrapper necesita un arbol git —usa `git rev-parse --show-toplevel`— y
    el selector a su lado. El laboratorio es un repositorio con lo minimo,
    de modo que lo que se mide es lo que el wrapper hace y no el estado del
    arbol de quien lo corre. Un `cargo` falso anota los argumentos que
    recibe: es la unica forma de saber que binario se ejecuto de verdad, en
    vez de leer un texto que podria mentir.
    """
    lab = tmp / "lab"
    (lab / "scripts" / "ci").mkdir(parents=True)
    shutil.copy2(FAST, lab / "scripts" / "test-fast.sh")
    shutil.copy2(SELECTOR, lab / "scripts" / "ci" / "select-suites.sh")
    # Los ficheros que se tocan existen en el laboratorio: un target se
    # pide por un fichero que esta en disco, y el laboratorio tiene que
    # decir la verdad sobre eso.
    for touched in paths.split():
        target = lab / touched
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("// laboratorio\n", encoding="utf-8")
    subprocess.run(["git", "init", "-q"], cwd=lab, check=True)
    subprocess.run(["git", "add", "-A"], cwd=lab, check=True)
    git_env = {
        **os.environ,
        "GIT_AUTHOR_NAME": "contract",
        "GIT_AUTHOR_EMAIL": "contract@localhost",
        "GIT_COMMITTER_NAME": "contract",
        "GIT_COMMITTER_EMAIL": "contract@localhost",
        # El laboratorio no tiene remoto, y el envoltorio de git de esta
        # maquina no firma commits en un repositorio sin clasificar. Aqui
        # el commit es una escena, no un hecho que nadie vaya a firmar.
        "RANDOM_GIT_COMMITTER_DISABLED": "1",
    }
    subprocess.run(["git", "commit", "-q", "-m", "lab"], cwd=lab, check=True, env=git_env)

    fake_bin = tmp / "bin"
    fake_bin.mkdir()
    calls = tmp / "calls.txt"
    cargo = fake_bin / "cargo"
    cargo.write_text(
        '#!/usr/bin/env bash\nprintf "%s\\n" "$*" >> "$CARGO_CALLS"\nexit 0\n',
        encoding="utf-8",
    )
    cargo.chmod(0o755)

    env = {
        **os.environ,
        "PATH": f"{fake_bin}{os.pathsep}{os.environ['PATH']}",
        "CARGO_CALLS": str(calls),
    }
    done = subprocess.run(
        ["bash", "scripts/test-fast.sh", "--paths", paths, "--no-baseline"],
        cwd=lab,
        capture_output=True,
        text=True,
        check=False,
        env=env,
    )
    recorded = calls.read_text(encoding="utf-8") if calls.exists() else ""
    return done, recorded


def test_a_touched_integration_test_is_the_target_that_runs() -> None:
    """M13: el target de integracion tocado es el target que se ejecuta.

    MEDIDO 2026-10-22. `run_suite()` corria `cargo test -p <crate> --lib` para
    las cinco suites, asi que tocar `crates/<crate>/tests/<fichero>.rs`
    seleccionaba la suite y no ejecutaba el fichero tocado: el lazo de
    feedback daba verde sin haber corrido el test recien escrito, y el falso
    verde es peor que un rojo porque se lee como una cobertura.

    Correr los 99 binarios de integracion de `core` convertia `fast` en
    minutos. Correr el target que el path toca no: es el unico que puede
    hablar del cambio, y es el que el developer acaba de escribir.

    Este contrato no lee el codigo: ejecuta el wrapper en un arbol de
    laboratorio, con un `cargo` que anota lo que recibe, y mira la lista.
    """
    with tempfile.TemporaryDirectory() as directory:
        done, recorded = fast_run_in_a_lab(
            Path(directory), "crates/cognicode-core/tests/prf_demo.rs"
        )

    assert done.returncode == 0, (
        f"FAIL: el wrapper salio con rc={done.returncode} sobre un arbol de "
        f"laboratorio. stderr:\n{done.stderr}"
    )
    assert "--test prf_demo" in recorded, (
        "FAIL: el wrapper no ejecuto el target de integracion que se toco. "
        f"Tiene que ejecutar el binario del fichero tocado. Lo que ejecuto:\n{recorded}"
    )
    assert "--lib" in recorded, (
        "FAIL: el wrapper dejo de ejecutar la biblioteca del crate. El target "
        f"tocado se anade; la biblioteca no se sustituye. Lo que ejecuto:\n{recorded}"
    )


def test_a_shared_module_under_tests_is_not_a_target() -> None:
    """M14: un modulo compartido bajo `tests/` no es un target.

    `tests/common/mod.rs` no produce un binario de test: cargo crea un target
    por cada fichero `.rs` que esta directamente bajo `tests/`. Pedirle un
    binario llamado `common` hace que cargo falle, y ese fallo no habla del
    cambio que el developer estaba haciendo: ensucia el lazo de feedback con
    un error de nombres.
    """
    with tempfile.TemporaryDirectory() as directory:
        done, recorded = fast_run_in_a_lab(
            Path(directory), "crates/cognicode-core/tests/common/mod.rs"
        )

    assert done.returncode == 0, (
        f"FAIL: el wrapper salio con rc={done.returncode} sobre un arbol de "
        f"laboratorio. stderr:\n{done.stderr}"
    )
    assert "--test common" not in recorded, (
        "FAIL: el wrapper pidio un binario para un modulo compartido. Un "
        f"fichero dentro de un subdirectorio de `tests/` no es un target. "
        f"Lo que ejecuto:\n{recorded}"
    )


def cargo_test_targets(package: str) -> tuple[set[str], list[str]]:
    """Los targets de test que cargo declara, y los paths que los producen.

    `cargo metadata` es la autoridad: es el mismo manifiesto que cargo usa
    para saber que un target existe. No compila nada, y no reimplementa la
    resolucion de nombres de cargo, que es justamente lo que se quiere
    comprobar aqui.
    """
    proc = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, (
        f"FAIL: `cargo metadata` no respondio (rc={proc.returncode}). "
        f"stderr:\n{proc.stderr}"
    )
    metadata = json.loads(proc.stdout)
    names: set[str] = set()
    paths: list[str] = []
    for pkg in metadata["packages"]:
        if pkg["name"] != package:
            continue
        for target in pkg["targets"]:
            if "test" not in target["kind"]:
                continue
            source = target["src_path"]
            if "/tests/" not in source:
                continue
            names.add(target["name"])
            paths.append(str(Path(source).relative_to(REPO_ROOT)))
    return names, paths


def test_the_derived_target_names_are_the_ones_cargo_declares() -> None:
    """M15: el nombre del target se deriva como cargo lo nombra.

    Cargo crea un target de test por cada fichero `.rs` directamente bajo
    `tests/`, y el nombre del target es el del fichero. Si un crate declarase
    un `[[test]]` con otro nombre, derivar el nombre del fichero pediria a
    cargo un binario que no existe, y el lazo de feedback se romperia con un
    error que no habla del cambio.

    Este contrato contrasta la derivacion entera, no un caso: todos los
    targets de un crate, contra todos los que cargo declara.
    """
    package = "cognicode-core"
    expected, paths = cargo_test_targets(package)
    assert expected, (
        f"FAIL: cargo no declara ningun target de test en {package}. El "
        f"contrato no puede compararse con nada, y un contrato que no "
        f"compara nada esta verde por construccion."
    )

    text = _read(FAST)
    start = text.find("touched_targets() {")
    assert start != -1, f"FAIL: {FAST.name} ya no define touched_targets()"
    end = text.find("\n}\n", start)
    assert end != -1, f"FAIL: no se pudo delimitar touched_targets() en {FAST.name}"
    body = text[start : end + 3]

    with tempfile.TemporaryDirectory() as directory:
        script = Path(directory) / "targets_only.sh"
        script.write_text(
            "#!/usr/bin/env bash\nset -uo pipefail\n" + body + '\nPATHS="$*"\ntouched_targets '
            + package
            + "\n",
            encoding="utf-8",
        )
        derived = set(
            subprocess.run(
                ["bash", str(script), *paths],
                capture_output=True,
                text=True,
                check=False,
            ).stdout.split()
        )

    missing = sorted(expected - derived)
    extra = sorted(derived - expected)
    assert not missing and not extra, (
        f"FAIL: la derivacion de targets no coincide con los que cargo "
        f"declara en {package}. Faltan {missing[:5]}, sobran {extra[:5]}. "
        f"Un target que cargo no declara no existe, y uno que se deja de "
        f"derivar es un test que el lazo de feedback deja de correr."
    )


def test_a_deleted_test_is_not_asked_for_as_a_binary() -> None:
    """M16: un test borrado no se pide como binario.

    Un fichero borrado sigue en el diff: `git diff --name-only` lo nombra
    aunque ya no este en disco. Pedirle a cargo el binario de ese nombre
    hace que falle con "no test target named ...", un error que no habla
    del cambio que el developer esta haciendo: parece un fallo del repo
    y es una regla de nombres.
    """
    with tempfile.TemporaryDirectory() as directory:
        lab = Path(directory) / "lab"
        (lab / "scripts" / "ci").mkdir(parents=True)
        shutil.copy2(FAST, lab / "scripts" / "test-fast.sh")
        shutil.copy2(SELECTOR, lab / "scripts" / "ci" / "select-suites.sh")
        subprocess.run(["git", "init", "-q"], cwd=lab, check=True)
        git_env = {
            **os.environ,
            "GIT_AUTHOR_NAME": "contract",
            "GIT_AUTHOR_EMAIL": "contract@localhost",
            "GIT_COMMITTER_NAME": "contract",
            "GIT_COMMITTER_EMAIL": "contract@localhost",
            "RANDOM_GIT_COMMITTER_DISABLED": "1",
        }
        subprocess.run(["git", "add", "-A"], cwd=lab, check=True)
        subprocess.run(
            ["git", "commit", "-q", "-m", "lab"], cwd=lab, check=True, env=git_env
        )

        fake_bin = Path(directory) / "bin"
        fake_bin.mkdir()
        calls = Path(directory) / "calls.txt"
        cargo = fake_bin / "cargo"
        cargo.write_text(
            '#!/usr/bin/env bash\nprintf "%s\\n" "$*" >> "$CARGO_CALLS"\nexit 0\n',
            encoding="utf-8",
        )
        cargo.chmod(0o755)
        env = {
            **os.environ,
            "PATH": f"{fake_bin}{os.pathsep}{os.environ['PATH']}",
            "CARGO_CALLS": str(calls),
        }
        done = subprocess.run(
            [
                "bash",
                "scripts/test-fast.sh",
                "--paths",
                "crates/cognicode-core/tests/prf_borrado.rs",
                "--no-baseline",
            ],
            cwd=lab,
            capture_output=True,
            text=True,
            check=False,
            env=env,
        )
        recorded = calls.read_text(encoding="utf-8") if calls.exists() else ""

    assert done.returncode == 0, (
        f"FAIL: el wrapper salio con rc={done.returncode} sobre un arbol de "
        f"laboratorio. stderr:\n{done.stderr}"
    )
    assert "prf_borrado" not in recorded, (
        "FAIL: el wrapper pidio un binario para un fichero que no esta en "
        f"disco. Lo que ejecuto:\n{recorded}"
    )


def test_the_fallback_also_runs_the_touched_integration_target() -> None:
    """M17: el camino de fallback tambien corre el target que se ha tocado.

    MEDIDO 2026-10-29, en el cierre de TASK-16. `test-fast.sh` tiene dos
    caminos: `changed`, que llama a `run_suite()` —y desde 2026-10-22 corre
    el target de integracion que los paths tocan— y `fallback`, que se salta
    `run_suite()` entero y corre `cargo test --workspace --lib`, que son solo
    pruebas unitarias.

    Se entra en `fallback` cuando algun path del diff no lo conoce el
    selector, y eso ocurre en cambios de verdad: `product/tools.json` esta
    versionado y ninguna regla de CR-08 lo clasifica. Con un test de
    integracion nuevo y ese path en el mismo diff, el lazo de feedback daba
    verde sin haber corrido el test recien escrito. Es el mismo falso verde
    que M13 cierra en el camino `changed`, un camino mas alla.

    El fallback no puede correr la bateria de integracion —eso es `full`, y
    correr los 99 binarios de `core` convertira `fast` en minutos—, pero si
    el target que los paths tocan, que es el unico que puede hablar del
    cambio y que el developer acaba de escribir.

    Este contrato no lee el codigo: ejecuta el wrapper de verdad en un arbol
    de laboratorio, con un `cargo` que anota lo que recibe, y mira la lista.
    """
    with tempfile.TemporaryDirectory() as directory:
        done, recorded = fast_run_in_a_lab(
            Path(directory),
            "crates/cognicode-core/tests/prf_demo.rs product/tools.json",
        )

    assert done.returncode == 0, (
        f"FAIL: el wrapper salio con rc={done.returncode} sobre un arbol de "
        f"laboratorio. stderr:\n{done.stderr}"
    )
    assert "--test prf_demo" in recorded, (
        "FAIL: el camino de fallback no ejecuto el target de integracion que "
        "se toco. El target tocado es el unico que puede hablar del cambio, "
        "y el fallback lo es tambien: no tiene excusa para no correrlo. "
        f"Lo que ejecuto:\n{recorded}"
    )
    assert "prf_demo" in done.stdout, (
        "FAIL: el wrapper no nombro el target que corrio. Un wrapper que "
        "calla sobre lo que ha ejecutado se lee como si lo hubiera hecho "
        f"todo. stdout:\n{done.stdout}"
    )
    assert "--workspace --lib" in recorded, (
        "FAIL: el fallback dejo de correr el subconjunto conservador. El "
        "target tocado se anade; el subconjunto no se sustituye. "
        f"Lo que ejecuto:\n{recorded}"
    )
    assert "--workspace --tests" not in recorded, (
        "FAIL: el fallback se ha convertido en la bateria de integracion. "
        "Eso es `full`, y `fast` no es `full`: correr los 99 binarios de "
        f"integracion de core lo convierte en minutos. Lo que ejecuto:\n{recorded}"
    )


def main() -> int:
    failures: list[str] = []
    for fn in (
        test_fast_wrapper_exists_and_is_executable,
        test_full_wrapper_exists_and_is_executable,
        test_fast_delegates_to_selector,
        test_fast_uses_known_failures_baseline,
        test_full_runs_the_release_battery,
        test_full_invokes_known_failures_baseline,
        test_both_wrappers_have_set_euo_pipefail,
        test_selector_dependency_exists,
        test_baseline_dependencies_exist,
        test_an_unknown_suite_is_a_hard_error_not_a_warning,
        test_the_merge_suite_is_not_silently_substituted,
        test_the_selector_doc_and_the_runner_agree,
        test_a_filter_that_matches_nothing_is_not_a_green_light,
        test_a_filtered_run_announces_that_it_is_not_the_gate,
        test_a_noop_names_the_lane_that_owns_the_surface,
        test_a_touched_integration_test_is_the_target_that_runs,
        test_a_shared_module_under_tests_is_not_a_target,
        test_the_derived_target_names_are_the_ones_cargo_declares,
        test_a_deleted_test_is_not_asked_for_as_a_binary,
        test_the_fallback_also_runs_the_touched_integration_target,
    ):
        try:
            fn()
        except AssertionError as e:
            failures.append(f"{fn.__name__}: {e}")
        except Exception as e:
            failures.append(f"{fn.__name__}: {type(e).__name__}: {e}")

    if failures:
        print("test-fast/full wrappers contract: FAIL")
        for f in failures:
            print(f"  - {f}")
        return 1

    print(
        f"test-fast/full wrappers contract: OK "
        f"(fast={FAST.stat().st_size}B, full={FULL.stat().st_size}B)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())