#!/usr/bin/env python3
"""Los `rerun-if-changed` de los build scripts apuntan a rutas que existen.

## Por qué esto es un contrato, y no una convención

Cargo no distingue «esta entrada declarada no existe» de «esta entrada cambió»:
si un `build.rs` declara `cargo:rerun-if-changed=<ruta>` y la ruta no está, la
unidad se considera **sucia en cada invocación** y se recompila para siempre.

MEDIDO 2026-10-16. `crates/cognicode-cli/build.rs` declaraba
`rerun-if-changed=../../.git/HEAD` y `../../.git/refs/heads/`, que son correctos
en un checkout normal y **no existen en un worktree de git**, donde `.git` es un
fichero. En el worktree del operador, `cargo doc --workspace --no-deps` tardaba
163 s con la documentación ya construida y no la reutilizaba nunca
(`Dirty cognicode-cli: the file crates/cognicode-cli/../../.git/HEAD is
missing`), y todo crate que depende de `cognicode-cli` se recompilaba con él en
cada etapa del gate. Con la ruta resuelta (`git rev-parse --absolute-git-dir`),
la segunda pasada tarda 1 s y `-v` no reporta ninguna unidad `Dirty`.

Un contrato que sólo mira el texto no habría visto esto, y por eso comprueba las
rutas contra el sistema de ficheros: la ruta declarada tiene que existir de
verdad, relativa al directorio del paquete (que es como cargo la interpreta).

## Qué se comprueba

- Sólo se miran los `build.rs` **versionados** (`git ls-files`): un crate
  aparcado bajo un directorio ignorado no se compila aquí y juzgarlo sería
  ruido, no una garantía.
- Cada literal `cargo:rerun-if-changed="<ruta>"` en una línea de **código**
  (los comentarios que explican el defecto no son declaraciones) resuelve a algo
  que existe, relativo al directorio del paquete, que es como cargo lo interpreta.
- La forma dinámica `cargo:rerun-if-changed={...}` sí se permite —es la que
  resuelve el gitdir de un worktree— pero exige una comprobación de existencia en
  el mismo fichero: sin ella, el script puede declarar una ruta ausente y volver
  al defecto que este contrato existe para evitar.

## Lo que este contrato no comprueba

- Que la ruta sea la *correcta* (que apunte al gitdir de verdad en vez de a un
  `.git` que casualmente existe). Eso lo demuestra la medición del ADR-TH-014.
- Que el build script haga lo que dice: sólo que sus entradas declaradas existen
  o que la forma dinámica está guardada por una comprobación.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# Literal: en Rust la directiva y la ruta viven en el MISMO literal de cadena —
# `println!("cargo:rerun-if-changed=../../.git/HEAD")` — así que exigir comillas
# justo después del `=` no encuentra nada y deja pasar el defecto original. La
# ruta es lo que hay hasta la comilla de cierre o hasta un `{` de formato.
# `(?!\{)` deja fuera la forma dinámica (`rerun-if-changed={}`), que no es un
# literal con una ruta vacía: la cubre el otro test.
RERUN = re.compile(r"rerun-if-changed\s*=\s*(?!\{)(?P<path>[^\"{}]*)")

# Dinámica: `rerun-if-changed={...}` (formato de `println!`). Permitida sólo con
# una comprobación de existencia en el mismo fichero.
DYNAMIC = re.compile(r"rerun-if-changed\s*=\s*\{")
EXISTS_CHECK = re.compile(r"\.exists\(\)")

_failures: list[str] = []


def build_scripts() -> list[Path]:
    """`build.rs` de los **miembros del workspace**: los que cargo compila.

    Un crate aparcado (`docs/parked-crates/...`) está versionado pero no lo
    construye ningún workspace, así que juzgarlo sería ruido: el defecto que este
    contrato persigue se paga al compilar, y ahí no se compila. La lista sale de
    `cargo metadata`, que es la autoridad sobre qué es un miembro.
    """
    import json
    import subprocess

    proc = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=str(REPO_ROOT),
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        raise AssertionError(
            "no se pudo resolver el workspace con `cargo metadata`: sin esa lista "
            "este contrato no sabe qué build scripts se compilan, y adivinarlo "
            "sería comprobar un conjunto que nadie compila"
        )
    metadata = json.loads(proc.stdout)
    scripts = []
    for package in metadata.get("packages", ()):
        manifest_dir = Path(package["manifest_path"]).parent
        candidate = manifest_dir / "build.rs"
        if candidate.is_file():
            scripts.append(candidate)
    return sorted(set(scripts))


def code_lines(script: Path) -> list[tuple[int, str]]:
    """Líneas numeradas que no son comentario.

    Un comentario que explica el defecto cita la directiva rota a propósito; si
    contara, el contrato pediría borrar la explicación del fallo.
    """
    out = []
    for number, line in enumerate(script.read_text(encoding="utf-8").splitlines(), start=1):
        stripped = line.lstrip()
        if stripped.startswith("//") or stripped.startswith("/*") or stripped.startswith("*"):
            continue
        out.append((number, line))
    return out


def declared_inputs(script: Path) -> list[tuple[int, str]]:
    found = []
    for number, line in code_lines(script):
        for match in RERUN.finditer(line):
            found.append((number, match.group("path")))
    return found


def test_no_build_script_declares_a_path_that_does_not_exist() -> None:
    """La ruta declarada tiene que existir, relativa al directorio del paquete."""
    checked = 0
    for script in build_scripts():
        package_dir = script.parent
        for number, declared in declared_inputs(script):
            if not declared:
                _failures.append(
                    f"{script.relative_to(REPO_ROOT)}:{number}: un `rerun-if-changed` vacío no "
                    f"declara nada y deja la unidad sucia para siempre"
                )
                continue
            checked += 1
            target = (package_dir / declared).resolve()
            if not target.exists() and not list(package_dir.glob(declared)):
                _failures.append(
                    f"{script.relative_to(REPO_ROOT)}:{number} declara "
                    f"`rerun-if-changed={declared}` y esa ruta no existe "
                    f"(resuelve a {target}). Cargo marca la unidad sucia en cada "
                    f"invocación: MEDIDO 2026-10-16, `../../.git/HEAD` en un worktree "
                    f"costaba 163 s por corrida de `cargo doc --workspace` con la "
                    f"documentación ya construida. Resuelve la ruta con una "
                    f"herramienta que funcione en checkout y en worktree, o declara "
                    f"sólo lo que exista."
                )
    dynamics = sum(
        1
        for script in build_scripts()
        for _ in DYNAMIC.finditer(script.read_text(encoding="utf-8"))
    )
    assert checked or dynamics or not build_scripts(), (
        "ningún build script del repositorio declara entradas de `rerun-if-changed`, "
        "ni literales ni dinámicas: este contrato estaría comprobando nada, y una "
        "lista vacía de sujetos no es una lista limpia"
    )
    assert not _failures, "\n".join(_failures)


def test_a_dynamic_path_is_guarded_by_an_existence_check() -> None:
    """La forma dinámica se permite, pero sólo con comprobación de existencia.

    Es la forma que resuelve el gitdir de un worktree; sin la guarda, el script
    puede declarar una ruta ausente y volver al defecto medido.
    """
    offenders = []
    for script in build_scripts():
        text = script.read_text(encoding="utf-8")
        if not DYNAMIC.search(text):
            continue
        if not EXISTS_CHECK.search(text):
            offenders.append(str(script.relative_to(REPO_ROOT)))
    assert not offenders, (
        f"estos build scripts declaran una ruta dinámica "
        f"(`rerun-if-changed={{...}}`) sin ninguna comprobación de existencia en el "
        f"fichero: {offenders}. Una ruta que puede no existir deja la unidad sucia en "
        f"cada invocación (MEDIDO 2026-10-16: 163 s por corrida de `cargo doc`). "
        f"Declara sólo lo que existe, como hace `git rev-parse --absolute-git-dir` "
        f"más `path.exists()`."
    )


def main() -> int:
    failures: list[str] = []
    for fn in (
        test_no_build_script_declares_a_path_that_does_not_exist,
        test_a_dynamic_path_is_guarded_by_an_existence_check,
    ):
        try:
            fn()
        except AssertionError as exc:
            failures.append(str(exc))
        except Exception as exc:  # pragma: no cover - se reporta, no se traga
            failures.append(f"{fn.__name__}: {type(exc).__name__}: {exc}")
    failures.extend(_failures)
    if failures:
        print("build inputs contract: FAIL")
        for failure in dict.fromkeys(failures):
            print(f"  - {failure}")
        return 1
    scripts = build_scripts()
    literals = sum(len(declared_inputs(script)) for script in scripts)
    dynamics = sum(
        len(DYNAMIC.findall(script.read_text(encoding="utf-8"))) for script in scripts
    )
    print(
        f"build inputs contract: OK ({len(scripts)} build.rs, {literals} entradas "
        f"literales existentes, {dynamics} dinámicas con guarda de existencia)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
