#!/usr/bin/env python3
"""Que la limpieza del UAT de instalacion pueda limpiar.

MEDIDO 2026-10-03. `release-install-smoke.sh` certificaba entero y la stage
salia con 64:

    PASS: published-layout CLI + MCP + skills install/update/reshim/uninstall
    mavis-trash: refusing to trash protected path '.../tmp.XXXX'
    mavis-trash: '.../tmp.XXXX' is the parent of the home directory

Install, update, reshim, uninstall y los dos bundles de skills pasaron. Lo que
fallo fue el trap de salida.

Y no fue casualidad de un entorno: el script exporta `HOME="$TMP/home"` para que
la instalacion no toque el HOME de quien la corre. Cuando corre el trap, `$TMP`
contiene el HOME vigente, y el envoltorio de borrado se niega a llevar un arbol
que contiene el directorio protegido. Medido con una sonda de tres casos:

    HOME dentro, cwd dentro   -> el arbol sobrevive
    HOME dentro, cwd fuera    -> el arbol sobrevive
    HOME restaurado, cwd fuera -> el arbol se va

Cambiar de directorio no basta mientras HOME apunte dentro. Este contrato
ejecuta la limpieza REAL del script, no una reimplementacion, para que no haya
dos versiones de la que medir.
"""
from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SMOKE = REPO_ROOT / "scripts" / "ci" / "release-install-smoke.sh"


def extract_cleanup(text: str) -> str:
    """La definicion de `cleanup` tal cual, sin reescribirla."""
    start = text.index("cleanup() {")
    depth = 0
    i = start
    while i < len(text):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[start : i + 1]
        i += 1
    raise AssertionError("cleanup() no cierra")


def test_the_cleanup_survives_a_redirected_home() -> None:
    """El caso medido: la limpieza tiene que funcionar con HOME redirigido."""
    text = SMOKE.read_text(encoding="utf-8")
    cleanup = extract_cleanup(text)
    assert "ORIGINAL_HOME" in text, (
        "el script tiene que guardar el HOME original antes de redirigirlo: sin "
        "eso, la limpieza no tiene un HOME al que volver"
    )

    with tempfile.TemporaryDirectory() as raw:
        base = Path(raw) / "lane-tmp"
        home = base / "home"
        home.mkdir(parents=True)
        (home / "installed.yaml").write_text("installed\n", encoding="utf-8")

        script = Path(raw) / "cleanup.sh"
        script.write_text(
            "#!/usr/bin/env bash\n"
            'ORIGINAL_HOME="$HOME"\n'
            f'SERVER_PID=""\n'
            "export HOME=" + str(home) + "\n"
            f"cd {base}\n"
            f'TMP="{base}"\n'
            + cleanup
            + "\ncleanup\n",
            encoding="utf-8",
        )
        script.chmod(0o755)
        done = subprocess.run(
            ["bash", str(script)],
            capture_output=True,
            text=True,
            timeout=120,
            env={"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": str(Path.home())},
        )

    assert not base.exists(), (
        f"la limpieza dejo el arbol en pie, que es lo que hacia que un UAT que "
        f"pasaba saliera con 64.\nstdout: {done.stdout}\nstderr: {done.stderr}"
    )


def test_the_cleanup_restores_home_before_removing() -> None:
    """El orden importa: cambiar de directorio con HOME dentro no basta."""
    text = SMOKE.read_text(encoding="utf-8")
    cleanup = extract_cleanup(text)
    body = "\n".join(
        line for line in cleanup.splitlines() if not line.strip().startswith("#")
    )
    home_at = body.find("export HOME=")
    cd_at = body.find("cd /")
    rm_at = body.find("rm -rf")
    assert home_at != -1, "la limpieza tiene que devolver HOME"
    assert cd_at != -1, "la limpieza tiene que salir del arbol"
    assert rm_at != -1, "la limpieza tiene que borrar el arbol"
    assert home_at < rm_at, "HOME se devuelve antes de borrar, no despues"
    assert cd_at < rm_at, "se sale del arbol antes de borrar"


def test_the_script_still_asserts_the_real_install() -> None:
    """Un UAT que se limpia solo y no comprueba nada tambien sale 0."""
    text = SMOKE.read_text(encoding="utf-8")
    for needle, why in (
        ("skills/cognicode/SKILL.md", "el bundle de skills instalado"),
        ("skills/cognicode-mcp/SKILL.md", "el bundle de skills del MCP"),
        # El script invoca el binario por su ruta (`"$COGH_BIN" doctor`), no
        # por su nombre. Un needle escrito con el nombre no encuentra nada y
        # hace fallar el contrato por una razon que no tiene con el UAT.
        ('"$COGH_BIN" doctor', "la salud de la instalacion"),
        ("uninstall", "el ciclo completo"),
    ):
        assert needle in text, f"el UAT ya no comprueba {why}"


def main() -> int:
    module = sys.modules[__name__]
    tests = sorted(
        (
            name
            for name in dir(module)
            if name.startswith("test_") and callable(getattr(module, name))
        ),
        key=lambda name: getattr(module, name).__code__.co_firstlineno,
    )
    if not tests:
        print("FAIL: este modulo no expone ninguna funcion test_*")
        return 1
    failures = 0
    for name in tests:
        try:
            getattr(module, name)()
        except AssertionError as exc:
            failures += 1
            print(f"  FAIL {name}")
            for line in str(exc).splitlines():
                print(f"       {line}")
        else:
            print(f"  PASS {name}")
    print(f"  -> {len(tests) - failures} passed, {failures} failed")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
