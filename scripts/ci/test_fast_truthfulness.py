#!/usr/bin/env python3
"""Que `test-fast.sh` no pueda devolver verde sin haber ejecutado nada.

MEDIDO 2026-10-14 (CD-1). Los tests de este repo para los wrappers
(`test_fast_full_wrappers.py`) comprueban **texto**: que el script mencione
`select-suites.sh`, que declare `pipefail`, que exista el fichero. Ese metodo
no puede detectar el defecto que importa, porque el defecto no es una linea que
falte sino codigo que devuelve el valor equivocado. Y aqui no es hipotetico:

    $ bash scripts/test-fast.sh --paths crates/cognicode-core/src/lib.rs
    selector -> strategy=changed suites=["ladybug","core"]
    scripts/test-fast.sh: linea 151: roadmap_rc: variable sin asignar
    (0.2 s, ningun test ejecutado)

El `sed` de la linea 97 se queda con las comillas del JSON:

    SUITES = ["ladybug","core"]       # tal cual, con comillas

y el bucle parte esa cadena por espacios, de modo que cada token es
`["ladybug","core"]`. El `case` de `run_suite` compara contra `ladybug`, contra
`core` y nunca encuentra ninguno. **Hoy `test-fast.sh` no puede ejecutar ni una
sola suite.** Todo lo que el wrapper ha "verificado" en las ultimas iteraciones
era el ratchet del roadmap.

Lo que estos tests refutan, uno por uno:

  (1) suite desconocida        -> `return 0` la convierte en PASS.
  (2) selector falla (rc != 0) -> el rc se descarta con `2>/dev/null` y la salida
                                  vacia degrada al fallback en silencio.
  (3) suites con comillas       -> cero suites ejecutadas, exit 0.
  (4) `suites: []`              -> ni fallback ni tests, exit 0.
  (5) `roadmap_rc` sin asignar  -> `set -u` aborta y el rc deja de atribuible.
  (6) JSON multilinea           -> LATENTE: el `sed` no cruza saltos de linea. El
                                  selector actual emite una linea, asi que hoy no
                                  dispara, pero reformatear la salida lo vuelve
                                  silencioso. Se fija para que no pueda.

Ninguno de los seis admite un `PASS`. Este fichero **no** los arregla: los
detecta. El arreglo es de CD-1 y va en `scripts/test-fast.sh`.

METODO. Estos tests no compilan el workspace: cada caso parte una copia del
repo con un `cargo` de mentira en el PATH. El defecto esta en el parseo y en el
`return 0`, no en la duracion de `cargo test`, y un test de 8 minutos que
compila seis veces no seria un test que se pudiera correr en cada iteracion.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

# Test runner de mentira. En `fake-cargo/`, junto al ejecutable, va un
# marcador `record` que este cargo acumula: prueba de que el wrapper ejecuto de
# verdad y de que ejecuta lo que dice la seleccion.
FAKE_CARGO = """#!/usr/bin/env bash
echo "fake-cargo $*" >> "$(dirname "$0")/record"
exit 0
"""

# Ratchet de roadmap de mentira. NO se puede simplemente borrarlo: el defecto
# de `roadmap_rc` solo aparece cuando el fichero EXISTE y el ratchet PASA, que
# es el caso normal. Si se borra, la rama `else` asigna `roadmap_rc=0` y el bug
# desaparece del harness. Este stub reproduce exactamente el camino real.
FAKE_RATCHET = 'import sys\nprint("ROADMAP version ratchet: OK (harness)")\nsys.exit(0)\n'

_TEMPLATE: Path | None = None


def _template() -> Path:
    """Arbol base del harness, copiado de los ficheros versionados una vez.

    Copiar el arbol entero (3.7 GB con artefactos sin versionar) siete veces
    costo 8 min 31 s de un test que deberia durar segundos. Se copian solo los
    ficheros que git tiene versionados, con su contenido del working tree: el
    wrapper bajo prueba puede tener cambios sin commitear y el test debe verlos.
    """
    global _TEMPLATE
    if _TEMPLATE is None:
        tmp = Path(tempfile.mkdtemp(prefix="cc-fast-template-"))
        repo = tmp / "repo"
        repo.mkdir()
        files = subprocess.run(
            ["git", "ls-files", "-z"],
            cwd=REPO_ROOT,
            capture_output=True,
            check=True,
        ).stdout.split(b"\0")
        for raw in files:
            if not raw:
                continue
            rel = raw.decode()
            src = REPO_ROOT / rel
            if not src.is_file():
                continue
            dst = repo / rel
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, dst)
        # Los ficheros que el wrapper necesita pero que estan generados.
        for rel in ("Cargo.lock", "rust-toolchain.toml"):
            src = REPO_ROOT / rel
            if src.is_file() and not (repo / rel).exists():
                shutil.copy2(src, repo / rel)
        _TEMPLATE = repo
    return _TEMPLATE


class Harness:
    """Ejecuta `test-fast.sh` de verdad contra un selector y un cargo falsos.

    Se parte una copia en un directorio temporal en vez de parchear el
    original: el wrapper resuelve `REPO_ROOT` con `git rev-parse`, asi que hace
    falta un arbol git real, y mutar el checkout del desarrollador durante un test
    seria una forma segura de perder trabajo no guardado.
    """

    def __init__(self, selection: str, selector_rc: int = 0) -> None:
        self.tmp = Path(tempfile.mkdtemp(prefix="cc-fast-harness-"))
        self.repo = self.tmp / "repo"
        shutil.copytree(_template(), self.repo, symlinks=True)
        # `git rev-parse --show-toplevel` es la primera linea del wrapper y sin
        # esto aborta con rc=2 antes de llegar a nada que hay que probar.
        for cmd in (
            ["git", "init", "-q"],
            ["git", "config", "user.email", "harness@example.invalid"],
            ["git", "config", "user.name", "harness"],
            ["git", "add", "-A"],
            ["git", "commit", "-q", "-m", "harness baseline"],
        ):
            subprocess.run(cmd, cwd=self.repo, check=True, capture_output=True)

        # Ratchet que pasa (ver FAKE_RATCHET).
        (self.repo / "scripts" / "ci" / "test_roadmap_version_ratchet.py").write_text(
            FAKE_RATCHET, encoding="utf-8"
        )

        # Selector de mentira: emite exactamente el JSON que se le pasa.
        # Con saltos de linea reales se usa un heredoc: `json.dumps` de una
        # cadena con `\n` produce los dos caracteresbackslash y n, no un
        # salto de linea, y el test multilinea no probaria nada.
        self.selector = self.repo / "scripts" / "ci" / "fake-selector.sh"
        if "\n" in selection:
            body = (
                "#!/usr/bin/env bash\ncat <<'JSON'\n"
                f"{selection}\nJSON\n"
                f"exit {selector_rc}\n"
            )
        else:
            body = (
                "#!/usr/bin/env bash\n"
                "printf '%s\\n' " + json.dumps(selection) + "\n"
                f"exit {selector_rc}\n"
            )
        self.selector.write_text(body, encoding="utf-8")
        os.chmod(self.selector, 0o755)

        fast = self.repo / "scripts" / "test-fast.sh"
        text = fast.read_text(encoding="utf-8")
        assert "SELECTOR=" in text, "no se encontro la asignacion de SELECTOR"
        text = text.replace(
            'SELECTOR="${REPO_ROOT}/scripts/ci/select-suites.sh"',
            f'SELECTOR="{self.selector}"',
            1,
        )
        fast.write_text(text, encoding="utf-8")

        # Cargo de mentira, delante del real en el PATH.
        bindir = self.tmp / "fake-cargo"
        bindir.mkdir()
        (bindir / "cargo").write_text(FAKE_CARGO, encoding="utf-8")
        os.chmod(bindir / "cargo", 0o755)
        self.bindir = bindir
        self.record = bindir / "record"

    def run(self, *args: str) -> tuple[int, str, str]:
        env = dict(os.environ)
        env["PATH"] = f"{self.bindir}{os.pathsep}{env.get('PATH', '')}"
        env["TMPDIR"] = str(self.tmp)
        proc = subprocess.run(
            ["bash", str(self.repo / "scripts" / "test-fast.sh"), *args],
            cwd=self.repo,
            capture_output=True,
            text=True,
            timeout=120,
            env=env,
        )
        return proc.returncode, proc.stdout, proc.stderr

    def invocations(self) -> list[str]:
        if not self.record.is_file():
            return []
        return [
            line for line in self.record.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]

    def cleanup(self) -> None:
        shutil.rmtree(self.tmp, ignore_errors=True)


def _sel(strategy: str, suites: list[str], reason: str = "harness") -> str:
    return json.dumps(
        {"strategy": strategy, "reason": reason, "suites": suites},
        separators=(",", ":"),
    )


def _changed(suites: list[str]) -> str:
    return _sel("changed", suites)


# ---------- (3) las comillas no pueden vaciar la ejecucion ----------


def test_suite_names_survive_json_quotes() -> None:
    """`suites:["core"]` debe ejecutar la suite core, no cero suites.

    MEDIDO: el `sed` devuelve `["core"]` con comillas; el token nunca casa con
    el `case` de `run_suite`; el bucle no ejecuta nada y el script sale 0.
    Este es el defecto activo, no un latente.
    """
    h = Harness(_changed(["core"]))
    try:
        rc, out, err = h.run("--no-baseline")
        assert "=== suite: core ===" in out, (
            f"FAIL: la suite llego pero no se ejecuto (token con comillas).\n"
            f"stdout:\n{out}\nstderr:\n{err}"
        )
        assert any("-p cognicode-core" in i for i in h.invocations()), (
            f"FAIL: no se invoco cargo para cognicode-core.\n"
            f"invocaciones: {h.invocations()}"
        )
    finally:
        h.cleanup()


def test_every_selected_suite_runs() -> None:
    """Todas las suites nombradas se ejecutan, en el orden nombrado.

    MEDIDO: con `suites:["ladybug","core"]` el token unico `["ladybug","core"]`
    no casa con nada. Se exige una invocacion por suite, para que un arreglo
    que solo arregle el primer elemento no pase.
    """
    h = Harness(_changed(["ladybug", "core"]))
    try:
        rc, out, err = h.run("--no-baseline")
        for suite in ("ladybug", "core"):
            assert f"=== suite: {suite} ===" in out, (
                f"FAIL: la suite {suite} no llego a ejecutarse.\n"
                f"stdout:\n{out}\nstderr:\n{err}"
            )
        invocations = h.invocations()
        assert any("-p cognicode-ladybug" in i for i in invocations), (
            f"FAIL: ladybug no se invoco. invocaciones: {invations}"
        )
        assert any("-p cognicode-core" in i for i in invocations), (
            f"FAIL: core no se invoco. invocaciones: {invocations}"
        )
    finally:
        h.cleanup()


# ---------- (1) suite desconocida no es un PASS ----------


def test_unknown_suite_is_not_a_pass() -> None:
    """Una suite que el wrapper no sabe correr debe fallar, no pasar.

    MEDIDO: `*) echo "WARN: suite desconocida" >&2; return 0` convierte el typo
    en verde. Un PASS sin pruebas detras es el falso positivo que CD-1 prohibe.

    El rc != 0 NO basta como prueba. Con `roadmap_rc` sin asignar, el script
    aborta por `set -u` en cualquier caso y este test pasaria sin corregir
    nada. Se exige ademas que el fallo sea atribuible a la suite desconocida y no
    a un fallo de shell.
    """
    h = Harness(_changed(["totally_bogus_suite"]))
    try:
        rc, out, err = h.run("--no-baseline")
        combined = out + err
        assert "variable sin asignar" not in combined and "unbound variable" not in combined, (
            f"FAIL: el script aborta por `roadmap_rc` y el rc no es atribuible "
            f"a la suite desconocida.\nstdout:\n{out}\nstderr:\n{err}"
        )
        assert rc != 0, (
            f"FAIL: una suite desconocida produjo exit 0 con cero tests "
            f"ejecutados.\nstdout:\n{out}\nstderr:\n{err}"
        )
        assert "totally_bogus_suite" in combined, (
            f"FAIL: el fallo debe nombrar la suite desconocida.\n"
            f"stdout:\n{out}\nstderr:\n{err}"
        )
    finally:
        h.cleanup()


# ---------- (2) un selector roto no degrada al fallback ----------


def test_selector_failure_is_not_silently_downgraded() -> None:
    """Si el selector falla, el wrapper no puede caer al fallback en verde.

    MEDIDO: linea 95 `SELECTION="$(bash "$SELECTOR" --paths "$PATHS" 2>/dev/null)"`
    tira el stderr y nunca mira el rc. Selector roto -> salida vacia ->
    `STRATEGY` vacio -> fallback, que puede pasar y ocultar que la seleccion
    nunca ocurrio.
    """
    h = Harness(_sel("changed", ["core"]), selector_rc=3)
    try:
        rc, out, err = h.run("--no-baseline")
        combined = out + err
        assert "variable sin asignar" not in combined and "unbound variable" not in combined, (
            f"FAIL: el script aborta por `roadmap_rc` y el rc no es atribuible "
            f"al fallo del selector.\nstdout:\n{out}\nstderr:\n{err}"
        )
        # La degradacion silenciosa al fallback es precisamente el defecto:
        # aunque el fallback pasara, el wrapper debe negar que la seleccion
        # ocurrio. No basta con "salio distinto de 0".
        assert "fallback: corriendo subset conservador" not in combined, (
            f"FAIL: un selector roto degrado al subset conservador en lugar de "
            f"fallar. La seleccion nunca ocurrio y el wrapper no lo dice.\n"
            f"stdout:\n{out}\nstderr:\n{err}"
        )
        assert rc != 0, (
            f"FAIL: el selector fallo (rc=3) y el wrapper salio 0 degradado al "
            f"fallback.\nstdout:\n{out}\nstderr:\n{err}"
        )
    finally:
        h.cleanup()


# ---------- (4) suites vacias no son un PASS ----------


def test_empty_suites_array_is_not_a_pass() -> None:
    """`suites: []` con strategy `changed` no puede correr nada y salir verde.

    MEDIDO: el `sed` capturaba `[]` como texto no vacio, `[ -z "$SUITES" ]` no
    saltava, y el bucle no iteraba. Ni fallback ni tests: el peor caso.

    El contrato aqui NO es "fallar". AGENTS.md, regla 2 de la tabla fast/full:
    "Si una suite cae en `fallback` [...] `fast` corre el subset conservador y
    avisa — no falla." Una seleccion vacia es una suite que no se pudo resolver,
    no un fallo del gate. Lo que el contrato prohibe es el resultado silencioso:
    asi que se exige que corran tests reales y que el aviso aparezca. Un arreglo
    que hiciera `exit 1` aqui seria correcto en cuanto al rc y falso en cuanto a
    la cobertura, porque no ejecutaria nada.
    """
    h = Harness(_changed([]))
    try:
        rc, out, err = h.run("--no-baseline")
        combined = out + err
        assert "variable sin asignar" not in combined and "unbound variable" not in combined, (
            f"FAIL: el script aborta por `roadmap_rc` y el rc no es atribuible a "
            f"la seleccion vacia.\nstdout:\n{out}\nstderr:\n{err}"
        )
        assert "fallback" in out, (
            f"FAIL: una seleccion vacia debe avisar de que cae al subset "
            f"conservador, no degradarse en silencio.\n"
            f"stdout:\n{out}\nstderr:\n{err}"
        )
        invocations = h.invocations()
        assert any("--workspace" in i for i in invocations), (
            f"FAIL: el subset conservador no se ejecuto: el wrapper salio sin "
            f"correr un solo test.\ninvocaciones: {invocations}"
        )
        assert rc == 0, (
            f"FAIL: el subset conservador y el ratchet pasan, asi que rc debe "
            f"ser 0. Un rc distinto indicaria que el wrapper no ejecuto lo que "
            f"prometio.\nstdout:\n{out}\nstderr:\n{err}"
        )
    finally:
        h.cleanup()


# ---------- (5) roadmap_rc sin inicializar ----------


def test_changed_branch_has_no_unbound_variable() -> None:
    """La rama `changed` no debe abortar por `roadmap_rc` sin asignar.

    MEDIDO: `overall_rc=$roadmap_rc` corre antes de que la rama lo asigne; con
    `set -u` el script aborta. El rc resultante mezcla un fallo de shell con el
    veredicto de tests, asi que un evaluador no puede atribuirlos.
    """
    h = Harness(_changed(["core"]))
    try:
        rc, out, err = h.run("--no-baseline")
        combined = out + err
        assert "variable sin asignar" not in combined and "unbound variable" not in combined, (
            f"FAIL: la rama changed aborta por una variable no inicializada.\n"
            f"stdout:\n{out}\nstderr:\n{err}"
        )
        assert "=== suite: core ===" in out, (
            f"FAIL: tras quitar el abort, la suite debe ejecutarse.\n"
            f"stdout:\n{out}"
        )
    finally:
        h.cleanup()


# ---------- (6) JSON multilinea: latente, se fija ahora ----------


def test_multiline_selector_json_is_parsed() -> None:
    """El parseo no puede depender de que el JSON quepa en una linea.

    MEDIDO: los dos `sed` son codigons `.*` que no cruzan saltos de linea. El
    selector actual emite una linea, asi que hoy no dispara; se fija para que
    reformatear la salida no lo vuelva silencioso.
    """
    h = Harness(
        '{\n'
        '  "strategy": "changed",\n'
        '  "suites": ["core"]\n'
        '}'
    )
    try:
        rc, out, err = h.run("--no-baseline")
        assert "=== suite: core ===" in out, (
            f"FAIL: JSON multilinea no se parseo.\nstdout:\n{out}\nstderr:\n{err}"
        )
    finally:
        h.cleanup()


def main() -> int:
    tests = (
        test_suite_names_survive_json_quotes,
        test_every_selected_suite_runs,
        test_unknown_suite_is_not_a_pass,
        test_selector_failure_is_not_silently_downgraded,
        test_empty_suites_array_is_not_a_pass,
        test_changed_branch_has_no_unbound_variable,
        test_multiline_selector_json_is_parsed,
    )
    failures: list[str] = []
    for fn in tests:
        try:
            fn()
            print(f"  ok   {fn.__name__}")
        except AssertionError as e:
            failures.append(f"{fn.__name__}: {e}")
            print(f"  FAIL {fn.__name__}")
        except Exception as e:
            failures.append(f"{fn.__name__}: {type(e).__name__}: {e}")
            print(f"  ERR  {fn.__name__}: {type(e).__name__}: {e}")

    if failures:
        print("test-fast truthfulness contract: FAIL")
        for f in failures:
            print(f"  - {f}")
        return 1

    print(f"test-fast truthfulness contract: OK ({len(tests)} checks)")
    return 0


if __name__ == "__main__":
    sys.exit(main())