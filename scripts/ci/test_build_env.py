#!/usr/bin/env python3
"""The contract of the build environment: `scripts/build-time.sh` and the
machine's cargo configuration.

The build environment of this repository is NOT in the repository: it is the
machine's `~/.cargo/config.toml`, because the profile cannot be declared per
project without invalidating the shared cache of `/var/home/rubentxu/cargo-targets/`
(AGENTS.md forbids a workspace `[profile.test]`), and because the linker and
the compilation cache are properties of the machine, not of this tree.

A promise that lives outside the repository needs something in the repository
that says it is still true. These checks are that something:

  * the instrument that measured the times still declares the four scenarios,
    and still measures them in an isolated target directory;
  * the repository still does NOT declare a workspace profile (the reason the
    declaration has to live on the machine);
  * the machine's configuration, when it is the shared one, still declares the
    build environment this repository published, and every tool it names is
    actually installed — a configuration that names a linker or a cache that is
    not there is a build that fails for everybody, and it fails far from here.

The machine's checks are skipped, with the reason printed, when the machine
has no shared cargo configuration: the contract then has nothing to say about
that machine, and saying nothing is not passing.
"""
from __future__ import annotations

import re
import shutil
import sys
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
INSTRUMENT = REPO / "scripts" / "build-time.sh"
GLOBAL_CONFIG = Path.home() / ".cargo" / "config.toml"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def _shared_config() -> dict | None:
    """The machine's shared cargo configuration, or None.

    "Shared" is what makes it this machine's configuration and not somebody
    else's: it is the one that points the target directory at the shared disk.
    """
    if not GLOBAL_CONFIG.is_file():
        return None
    try:
        with GLOBAL_CONFIG.open("rb") as handle:
            data = tomllib.load(handle)
    except (tomllib.TOMLDecodeError, OSError) as exc:
        failures.append(f"la configuracion global de cargo no se puede leer: {exc}")
        return None
    if not data.get("build", {}).get("target-dir"):
        return None
    return data


def test_the_instrument_declares_the_four_scenarios() -> None:
    check(INSTRUMENT.is_file(), f"el instrumento no existe: {INSTRUMENT}")
    if not INSTRUMENT.is_file():
        return
    source = INSTRUMENT.read_text(encoding="utf-8")
    for scenario in ("cold", "no-change", "touch-test", "touch-core"):
        check(
            scenario in source,
            f"el instrumento no declara el escenario '{scenario}': "
            "una medicion que pierde un escenario parece mejor de lo que es",
        )
    check(
        "MEASUREMENTS=(" in source,
        "el instrumento no declara MEASUREMENTS: los escenarios tienen que estar "
        "declarados en un solo sitio, no repartidos por el script",
    )


def test_the_instrument_measures_in_an_isolated_target_directory() -> None:
    check(INSTRUMENT.is_file(), f"el instrumento no existe: {INSTRUMENT}")
    if not INSTRUMENT.is_file():
        return
    source = INSTRUMENT.read_text(encoding="utf-8")
    check(
        'export CARGO_TARGET_DIR="${WORK}"' in source,
        "el instrumento no exporta CARGO_TARGET_DIR: una medicion que reutiliza "
        "los artefactos del build del desarrollador mide esa cache, no el toolchain",
    )
    check(
        'WORK="${TARGET_ROOT}/build-time-' in source,
        "el instrumento no crea su propio directorio de destino: "
        "el directorio de destino del workspace es compartido",
    )
    check(
        "rm -rf" in source and "cleanup" in source,
        "el instrumento no limpia su directorio de destino: "
        "una medicion que deja gigabytes en el disco compartido es una tarea disfrazada",
    )


def test_the_repository_does_not_declare_a_workspace_profile() -> None:
    """The reason the build environment lives on the machine.

    A workspace `[profile.test]` invalidates the shared cache of the machine
    (AGENTS.md), so the profile cannot be declared here. If somebody declares
    it anyway, the two declarations drift and the published times stop meaning
    anything.
    """
    manifest = REPO / "Cargo.toml"
    if manifest.is_file():
        text = manifest.read_text(encoding="utf-8")
        check(
            "[profile." not in text,
            "el manifiesto del workspace declara un [profile.*]: "
            "el perfil tiene que vivir en la configuracion global de la maquina "
            "(AGENTS.md lo prohibe aqui porque invalida la cache compartida)",
        )
    repo_config = REPO / ".cargo" / "config.toml"
    if repo_config.is_file():
        text = repo_config.read_text(encoding="utf-8")
        check(
            "[profile." not in text,
            ".cargo/config.toml declara un [profile.*]: "
            "el perfil tiene que vivir en la configuracion global de la maquina",
        )
        check(
            "CARGO_BUILD_JOBS" not in text and "jobs" not in text,
            ".cargo/config.toml declara jobs: la cache compartida de la maquina "
            "es de todos los proyectos y none la puede reescribir",
        )


def test_the_machine_config_declares_the_published_environment() -> None:
    data = _shared_config()
    if data is None:
        print(
            "SKIP: esta maquina no tiene una configuracion global de cargo "
            "compartida; el contrato no dice nada sobre ella."
        )
        return
    for profile in ("dev", "test"):
        section = data.get("profile", {}).get(profile)
        check(
            section is not None,
            f"la configuracion global de cargo no declara [profile.{profile}]: "
            "el perfil que publico el recibo es el que se midio",
        )
        if section is None:
            continue
        check(
            section.get("debug") == "line-tables-only",
            f"[profile.{profile}].debug es {section.get('debug')!r} y no "
            "'line-tables-only': las cifras del recibo se midieron con esa "
            "informacion de depuracion, y con otra el improvement medido no es el mismo",
        )
    targets = data.get("target", {})
    host = targets.get("x86_64-unknown-linux-gnu", {})
    check(
        bool(host.get("linker")),
        "la configuracion global de cargo no declara el linker del host: "
        "el enlace que se midio fue con un linker declarado, no con el que "
        "salga por defecto",
    )
    check(
        bool(data.get("build", {}).get("rustc-wrapper")),
        "la configuracion global de cargo no declara cache de compilacion "
        "(build.rustc-wrapper): la tasa de aciertos que se publico es la de "
        "una cache activa",
    )


def test_every_tool_the_machine_config_names_is_installed() -> None:
    """A configuration that names a tool that is not there breaks every build.

    This is the failure that hurts the most and is the hardest to see: the
    configuration is fine, the tool was uninstalled or the PATH changed, and
    the next `cargo build` on any project of the machine fails with a message
    that names the linker and not the cause.
    """
    data = _shared_config()
    if data is None:
        print(
            "SKIP: esta maquina no tiene una configuracion global de cargo "
            "compartida; no hay herramientas que verificar."
        )
        return
    tools: list[str] = []
    wrapper = data.get("build", {}).get("rustc-wrapper")
    if wrapper:
        tools.append(wrapper)
    for target in data.get("target", {}).values():
        linker = target.get("linker")
        if linker:
            tools.append(linker)
        for flag in target.get("rustflags", []):
            found = re.search(r"-fuse-ld=([A-Za-z0-9_.+-]+)", flag)
            if found:
                # gcc's driver looks for "ld.<name>" ("ld.mold", "ld.lld"); the
                # historical name for the default is plain "ld".
                emulation = found.group(1)
                tools.append("ld" if emulation == "bfd" else f"ld.{emulation}")
    for tool in tools:
        check(
            shutil.which(tool) is not None,
            f"la configuracion global de cargo nombra '{tool}' y no esta en el PATH: "
            "todo build de la maquina fallaria, y el fallo no diria por que",
        )


def main() -> int:
    # `globals()`, not `dir()`: inside a function `dir()` returns the local
    # scope, which does not contain the module-level test functions, and the
    # suite would pass without having run a single check.
    for name in sorted(n for n in globals() if n.startswith("test_")):
        globals()[name]()
    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(f"  - {failure}")
        return 1
    print("PASS — el entorno de compilacion esta declarado y es coherente")
    return 0


if __name__ == "__main__":
    sys.exit(main())
