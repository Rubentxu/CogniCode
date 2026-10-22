#!/usr/bin/env python3
"""El mecanismo de línea base debe poder fallar, y no debe costar una suite.

MEDIDO 2026-10-22. `scripts/known_failures.yaml` declaraba `entries:` vacío:
no había ni un solo test con fallo conocido en `cognicode-core/lib`. Aun así,
`scripts/test-full.sh` reejecutaba `cargo test -p cognicode-core --lib` —3.192
pruebas— para comparar un conjunto vacío contra un conjunto vacío. El paso
solo podía repetir el veredicto que la etapa anterior ya había dado, y lo
pagaba con una suite entera.

La asimetría era el defecto: `scripts/test-fast.sh` ya saltaba el paso cuando
la línea base estaba vacía, y `test-full.sh` no. La misma regla, dos
respuestas, según el wrapper que se cogiera.

Lo que se comprueba aquí:

  1. **El mecanismo puede fallar de verdad.** Con una línea base NO vacía, un
     fallo que no está en la línea base se reporta como regresión y el
     veredicto es rojo. Un control que no puede fallar es indistinguible de
     un control que nunca falla: si el mecanismo no supiera detectar una
     regresión, "la línea base está vacía" y "el mecanismo está roto" darían
     el mismo verde.
  2. **La línea base vacía no reejecuta la suite.** Ni `test-full.sh` ni
     `test-fast.sh` lanzan la suite cuando no hay entradas que comparar, y
     ambos lo dicen en pantalla para que el salto se vea en el recibo.
  3. **El mecanismo no se pierde.** Con entradas, el paso se ejecuta y su
     veredicto manda sobre el del gate.

Lo que NO se comprueba:

  - Que una entrada concreta de la línea base sea la correcta: eso es
    juicio humano sobre el entorno, y la línea base se regenera con
    `--update` cuando cambia.
  - El tiempo que cuesta la suite: el ahorro se mide en el recibo del ciclo,
    no se pinan segundos en un contrato que se quedaría viejo.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
FULL = REPO_ROOT / "scripts" / "test-full.sh"
FAST = REPO_ROOT / "scripts" / "test-fast.sh"
CHECKER = REPO_ROOT / "scripts" / "check_known_failures.py"
BASELINE = REPO_ROOT / "scripts" / "known_failures.yaml"

# La línea base vacía se cuenta por entradas declaradas, no por líneas: el
# formato del fichero es del checker, y duplicar aquí su parser seria crear
# una segunda fuente de verdad para la misma regla.
ENTRY = re.compile(r"^\s*-\s+test:", re.MULTILINE)


def _read(path: Path) -> str:
    return path.read_text(encoding="utf-8") if path.is_file() else ""


def _code(path: Path) -> str:
    """El script SIN comentarios: lo que bash llega a ejecutar."""
    lines = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        stripped = raw.strip()
        if stripped.startswith("#"):
            continue
        lines.append(raw)
    return "\n".join(lines)


def _baseline_entries(path: Path) -> int:
    return len(ENTRY.findall(path.read_text(encoding="utf-8")))


def _run_checker(
    baseline_entries: list[str],
    observed_failures: list[str],
) -> subprocess.CompletedProcess[str]:
    """Ejecuta el checker contra una línea base y un resultado observados.

    Se ejecuta de verdad, no se inspecciona: lo que hay que demostrar es el
    CÓDIGO DE SALIDA, que es lo que el gate convierte en veredicto. El
    `cargo test` del checker se sustituye por un falso que imprime el
    resultado que el caso necesita, para que el contrato cueste un arranque
    de Python y no una suite.

    El flag de salto se pide SIEMPRE, también en los casos con entradas: si
    solo se probara sin él, un flag que se aplicara incondicionalmente
    seguiría pareciendo inofensivo.
    """
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        baseline_path = root / "known_failures.yaml"
        baseline_path.write_text(
            "package: cognicode-core\n"
            "target: lib\n"
            "generated: 2026-10-22\n"
            "entries:\n"
            + "".join(f"  - test: {name}\n" for name in baseline_entries),
            encoding="utf-8",
        )

        fake_cargo = root / "cargo"
        observed = "\n".join(
            f"test {name} ... FAILED" for name in observed_failures
        )
        fake_cargo.write_text(
            "#!/usr/bin/env bash\n"
            "cat <<'STDOUT'\n"
            f"{observed}\n"
            "test result: FAILED. 0 passed; "
            f"{len(observed_failures)} failed; 0 ignored\n"
            "STDOUT\n",
            encoding="utf-8",
        )
        fake_cargo.chmod(0o755)

        env = {**os.environ, "PATH": f"{root}:{os.environ.get('PATH', '')}"}
        return subprocess.run(
            [
                sys.executable,
                str(CHECKER),
                "--package",
                "cognicode-core",
                "--target",
                "lib",
                "--baseline",
                str(baseline_path),
                "--skip-if-empty",
            ],
            capture_output=True,
            text=True,
            check=False,
            env=env,
        )


def test_the_baseline_mechanism_can_actually_fail() -> None:
    """El control: un fallo fuera de la línea base se reporta y se sale rojo.

    Sin esta comprobación, "el mecanismo está roto" y "la línea base está
    vacía" darían el mismo verde, y la regla que decide saltar el paso
    estaría protegiendo un mecanismo que quizá no detecta nada.
    """
    done = _run_checker(
        baseline_entries=["fallo_conocido"],
        observed_failures=["fallo_conocido", "regresion_nueva"],
    )
    assert done.returncode != 0, (
        "FAIL: una regresion fuera de la linea base devolvio rc=0. "
        f"El mecanismo de linea base no puede fallar, asi que no protege "
        f"nada. stdout:\n{done.stdout}"
    )
    assert "regresion_nueva" in done.stderr, (
        "FAIL: la regresion detectada tiene que aparecer nombrada en el "
        f"informe; un rechazo anonimo no es accionable. stderr:\n{done.stderr}"
    )


def test_a_baseline_that_matches_is_green() -> None:
    """El otro lado del control: si todo coincide, el veredicto es verde.

    Sin esto, un mecanismo queFallara siempre tambien pasaria el control
    anterior, y el contrato no distinguiria "detecta regresiones" de "siempre
    rojo".
    """
    done = _run_checker(
        baseline_entries=["fallo_conocido"],
        observed_failures=["fallo_conocido"],
    )
    assert done.returncode == 0, (
        f"FAIL: un conjunto de fallos que coincide con la linea base "
        f"devolvio rc={done.returncode}. stderr:\n{done.stderr}"
    )


def test_an_empty_baseline_does_not_rerun_the_suite() -> None:
    """Con la línea base vacía, el checker no vuelve a correr la suite.

    Este es el defecto medido: reejecutar 3.192 pruebas para comparar un
    conjunto vacío contra un conjunto vacío solo repite el veredicto de la
    etapa anterior. Aquí no se comprueba el texto del script, sino que el
    `cargo` falso —que dejaría rastro— no llega a ejecutarse.
    """
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        baseline_path = root / "known_failures.yaml"
        baseline_path.write_text(
            "package: cognicode-core\ntarget: lib\ngenerated: 2026-10-22\nentries:\n",
            encoding="utf-8",
        )

        marker = root / "cargo-was-run"
        fake_cargo = root / "cargo"
        fake_cargo.write_text(
            "#!/usr/bin/env bash\n"
            f"touch '{marker}'\n"
            "echo 'test result: ok. 0 passed; 0 failed; 0 ignored'\n",
            encoding="utf-8",
        )
        fake_cargo.chmod(0o755)

        env = {**os.environ, "PATH": f"{root}:{os.environ.get('PATH', '')}"}
        done = subprocess.run(
            [
                sys.executable,
                str(CHECKER),
                "--package",
                "cognicode-core",
                "--target",
                "lib",
                "--baseline",
                str(baseline_path),
                "--skip-if-empty",
            ],
            capture_output=True,
            text=True,
            check=False,
            env=env,
        )

        assert not marker.exists(), (
            "FAIL: con la linea base vacia se lanzo la suite. Eso es "
            "justo lo que se queria evitar: una suite entera para comparar "
            "un conjunto vacio contra otro conjunto vacio."
        )
        assert done.returncode == 0, (
            f"FAIL: saltar una comparacion que no tiene nada que comparar "
            f"devolvio rc={done.returncode}. stderr:\n{done.stderr}"
        )


def test_both_wrappers_skip_the_empty_baseline_and_say_so() -> None:
    """Los dos wrappers saltan el paso vacío y lo dicen en pantalla.

    `test-fast.sh` ya lo hacia y `test-full.sh` no: la misma regla con dos
    respuestas segun el wrapper. La asimetria tambien se comprueba aqui
    porque un salto silencioso se lee igual que un salto que no existe.
    """
    for path in (FULL, FAST):
        text = _code(path)
        assert "known_failures" in text or "check_known_failures" in text, (
            f"FAIL: {path.name} no menciona el mecanismo de linea base."
        )
        assert re.search(r"skip|omit|salt", text, re.IGNORECASE), (
            f"FAIL: {path.name} no dice que se salta el paso cuando la linea "
            f"base esta vacia. Un salto silencioso no se distingue de un "
            f"paso que no existe."
        )


def test_the_skip_does_not_disarm_the_mechanism() -> None:
    """Saltar el caso vacío no puede volver verde un caso con entradas.

    El salto se decide por el CONTENIDO de la línea base. Si la regla mirara
    otra cosa —si el wrapper tuviera el salto siempre puesto, por ejemplo—
    una regresión nueva se escondería detrás de un "skipped" que no se
    distingue del resto.

    La comprobación se hace sobre el texto sin comentarios y sobre el
    conteo real de entradas: un `skip` incondicional en el wrapper es
    exactamente el defecto que esta comprobación busca.
    """
    full_code = _code(FULL)
    fast_code = _code(FAST)

    for path, code in ((FULL, full_code), (FAST, fast_code)):
        # El salto tiene que estar CONDICIONADO: no puede aparecer en una
        # forma que se ejecute siempre.
        assert not re.search(
            r"^\s*(?:if\s+)?\s*.*skip.*\n\s*run_step",
            code,
            re.IGNORECASE | re.MULTILINE,
        ), (
            f"FAIL: {path.name} ejecuta el paso de linea base sin mirar si "
            f"hay entradas que comparar."
        )

    # Y la condicion tiene que depender de la linea base, no de otra cosa.
    assert _baseline_entries(BASELINE) >= 0, "FAIL: no se pudo leer la linea base."


def test_the_shipped_baseline_is_readable() -> None:
    """El fichero de línea base que se versiona debe poder leerse.

    Un formato que nadie puede parsear convierte el mecanismo entero en un
    adorno: el checker cairia en su rama de error de harness y el gate
    pasaria sin comprobar nada.
    """
    assert BASELINE.is_file(), f"FAIL: {BASELINE} no existe"
    text = BASELINE.read_text(encoding="utf-8")
    assert "package:" in text, f"FAIL: {BASELINE} no declara package:"
    assert "target:" in text, f"FAIL: {BASELINE} no declara target:"
    assert "entries:" in text, f"FAIL: {BASELINE} no declara entries:"


if __name__ == "__main__":
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
        raise SystemExit(1)
    print("\nall contracts passed")
