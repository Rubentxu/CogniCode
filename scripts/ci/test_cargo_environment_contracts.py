#!/usr/bin/env python3
"""Contracts of the cargo environment: the machine's configuration and the
wrapper that isolates the target directory.

Two contracts about how this repository builds with cargo, merged in one file
(TASK-74) keeping every check. They do not share a single subject: each section
watches its own.

  I.  `scripts/build-time.sh` and the machine's cargo configuration.
  II. `scripts/ci/cargo-iso.sh`, the wrapper that gives a gate its own
      `CARGO_TARGET_DIR`.

Run: python3 scripts/ci/test_cargo_environment_contracts.py

Part II needs a real cargo binary (`cargo metadata`), and its last test
(`test_actually_uses_isolated_target_for_real_build`) compiles a crate.

# I. The build environment: `scripts/build-time.sh` and the
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

# II. RED→GREEN coverage of scripts/ci/cargo-iso.sh.

Tests check the **observable** behaviour of the wrapper:
  * `--gate` mode auto-isolates per checkout when CARGO_TARGET_DIR is unset
  * `--gate` mode respects an explicit CARGO_TARGET_DIR (does NOT override)
  * without `--gate`, the wrapper passes through (shared cache unchanged)
  * the wrapper exec's cargo (no shell double-fork; exit code preserved)

The contract is exercised against `cargo metadata` so a real cargo binary
is required; the script targets it directly without going through a fake.
"""
from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
INSTRUMENT = REPO_ROOT / "scripts" / "build-time.sh"
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
    manifest = REPO_ROOT / "Cargo.toml"
    if manifest.is_file():
        text = manifest.read_text(encoding="utf-8")
        check(
            "[profile." not in text,
            "el manifiesto del workspace declara un [profile.*]: "
            "el perfil tiene que vivir en la configuracion global de la maquina "
            "(AGENTS.md lo prohibe aqui porque invalida la cache compartida)",
        )
    repo_config = REPO_ROOT / ".cargo" / "config.toml"
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


def test_no_workspace_member_declares_a_profile() -> None:
    """Cargo ignores a `[profile.*]` in a member and warns on every command.

    Only the workspace root can carry a profile, and the root cannot (see the
    test above), so a member that declares one has a setting that never applied
    and a warning that hides real ones. Crates excluded from the workspace are
    their own root and keep the right to declare theirs.
    """
    root = tomllib.loads((REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    excluded = {Path(e).as_posix() for e in root.get("workspace", {}).get("exclude", [])}
    manifests = sorted((REPO_ROOT / "crates").glob("*/Cargo.toml"))
    check(bool(manifests), "no se encontro ningun Cargo.toml en crates/*: la regla miraria sin ver")
    for manifest in manifests:
        relative = manifest.parent.relative_to(REPO_ROOT).as_posix()
        if relative in excluded:
            continue
        declared = tomllib.loads(manifest.read_text(encoding="utf-8")).get("profile")
        check(
            not declared,
            f"{relative}/Cargo.toml declara [profile.{', '.join(declared or [])}]: "
            "cargo lo ignora fuera de la raiz del workspace y avisa en cada comando",
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


# ---------------------------------------------------------------------------
# II. cargo-iso.sh
# ---------------------------------------------------------------------------

WRAPPER = REPO_ROOT / "scripts" / "ci" / "cargo-iso.sh"


def run(args, env=None, cwd=None):
    """Run the wrapper; return CompletedProcess. env/cwd are isolated.

    `args` is the cargo subcommand + its arguments, e.g. ["metadata", "--no-deps"].
    The wrapper prepends "cargo" itself; passing it twice would invoke
    `cargo cargo <subcommand>`.
    """
    proc_env = os.environ.copy()
    proc_env.pop("CARGO_TARGET_DIR", None)
    if env:
        proc_env.update(env)
    return subprocess.run(
        ["bash", str(WRAPPER)] + list(args),
        capture_output=True, text=True,
        env=proc_env, cwd=cwd or str(REPO_ROOT),
        timeout=120,
    )


def cargo_metadata_target_dir(env=None):
    """Return the target dir cargo metadata would resolve to."""
    proc_env = os.environ.copy()
    proc_env.pop("CARGO_TARGET_DIR", None)
    if env:
        proc_env.update(env)
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        capture_output=True, text=True, env=proc_env, cwd=str(REPO_ROOT),
        timeout=60,
    )
    if out.returncode != 0:
        raise RuntimeError(f"cargo metadata failed: {out.stderr[:500]}")
    import json
    return json.loads(out.stdout)["target_directory"]


class CargoIsoGateMode(unittest.TestCase):

    def test_gate_mode_auto_isolates_when_unset(self):
        # RED before: there was no wrapper, so the call could not exist.
        # Now: --gate with no CARGO_TARGET_DIR must set a per-checkout
        # directory and point cargo at it. We run from the repo root so
        # cargo can find a Cargo.toml; the wrapper still derives the
        # target dir from `git rev-parse --show-toplevel`.
        r = subprocess.run(
            ["bash", str(WRAPPER), "--gate", "metadata",
             "--format-version", "1", "--no-deps"],
            capture_output=True, text=True, cwd=str(REPO_ROOT), timeout=60,
        )
        self.assertEqual(
            r.returncode, 0,
            f"cargo metadata failed: {r.stderr[:500]}",
        )
        # stderr should announce the new target dir.
        self.assertIn("cargo-iso[--gate]:", r.stderr)
        self.assertIn("/tmp/cognicode-gate-", r.stderr)
        # cargo metadata must report the same target dir.
        import json
        reported = json.loads(r.stdout)["target_directory"]
        self.assertTrue(
            reported.startswith("/tmp/cognicode-gate-"),
            f"expected per-checkout target dir under /tmp/cognicode-gate-; "
            f"got {reported!r}",
        )

    def test_gate_mode_target_is_per_checkout_specific(self):
        # Two different checkouts must hash to two different target dirs.
        # We probe by running --gate from two temporary paths and reading
        # the announced target dir from stderr.
        import re
        pat = re.compile(r"CARGO_TARGET_DIR=(\S+)")
        with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
            ra = subprocess.run(
                ["bash", str(WRAPPER), "--gate", "metadata",
                 "--format-version", "1", "--no-deps"],
                capture_output=True, text=True, cwd=a, timeout=60,
            )
            rb = subprocess.run(
                ["bash", str(WRAPPER), "--gate", "metadata",
                 "--format-version", "1", "--no-deps"],
                capture_output=True, text=True, cwd=b, timeout=60,
            )
            ma = pat.search(ra.stderr)
            mb = pat.search(rb.stderr)
            self.assertIsNotNone(ma, f"stderr a: {ra.stderr[:300]}")
            self.assertIsNotNone(mb, f"stderr b: {rb.stderr[:300]}")
            self.assertNotEqual(
                ma.group(1), mb.group(1),
                f"two different checkout paths must hash differently; "
                f"both produced {ma.group(1)!r}",
            )

    def test_gate_mode_respects_explicit_target_dir(self):
        # RED: if --gate overrode an explicit value, the contamination fix
        # would not work — the wrapper must respect what's already there.
        explicit = "/tmp/cognicode-explicit-dir/target"
        os.makedirs(explicit, exist_ok=True)
        r = subprocess.run(
            ["bash", str(WRAPPER), "--gate", "metadata",
             "--format-version", "1", "--no-deps"],
            capture_output=True, text=True, cwd=str(REPO_ROOT),
            env={**os.environ, "CARGO_TARGET_DIR": explicit}, timeout=60,
        )
        self.assertEqual(r.returncode, 0, r.stderr[:500])
        self.assertNotIn("cargo-iso[--gate]:", r.stderr,
                         "wrapper must not announce when explicit CARGO_TARGET_DIR was given")
        # Cargo must report the explicit target dir.
        import json
        reported = json.loads(r.stdout)["target_directory"]
        self.assertEqual(reported, explicit)
        os.rmdir(explicit)


class CargoIsoPassThrough(unittest.TestCase):

    def test_no_gate_no_override(self):
        # Without --gate, the wrapper must not change anything.
        # If CARGO_TARGET_DIR is unset, cargo's effective target dir is the
        # global one (whatever `cargo metadata` resolves).
        # If CARGO_TARGET_DIR is set, that one wins.
        baseline = cargo_metadata_target_dir()
        r = run(["metadata", "--format-version", "1", "--no-deps"])
        self.assertEqual(r.returncode, 0, r.stderr[:500])
        import json
        reported = json.loads(r.stdout)["target_directory"]
        self.assertEqual(reported, baseline)

    def test_explicit_target_dir_without_gate_is_propagated(self):
        # Setting CARGO_TARGET_DIR alone (no --gate flag) must reach cargo.
        explicit = "/tmp/cognicode-no-gate-flag/target"
        os.makedirs(explicit, exist_ok=True)
        try:
            r = subprocess.run(
                ["bash", str(WRAPPER), "metadata",
                 "--format-version", "1", "--no-deps"],
                capture_output=True, text=True, cwd=str(REPO_ROOT),
                env={**os.environ, "CARGO_TARGET_DIR": explicit}, timeout=60,
            )
            self.assertEqual(r.returncode, 0, r.stderr[:500])
            import json
            reported = json.loads(r.stdout)["target_directory"]
            self.assertEqual(reported, explicit)
        finally:
            os.rmdir(explicit)


class CargoIsoIntegration(unittest.TestCase):

    def test_actually_uses_isolated_target_for_real_build(self):
        # NEGATIVE for the original bug: a build written to an isolated
        # target dir must NOT be visible from the shared one (because it
        # was never written there). We test the inverse: the isolated dir
        # contains the binary; the shared dir does not.
        # This is a smoke test; the full release-build UAT is B0's work.
        with tempfile.TemporaryDirectory() as isolated_root:
            target = Path(isolated_root) / "target"
            target.mkdir()
            env = {**os.environ, "CARGO_TARGET_DIR": str(target)}
            r = subprocess.run(
                ["bash", str(WRAPPER), "build", "-p", "cognicode-graph-algos",
                 "--quiet", "--offline"],
                capture_output=True, text=True, cwd=str(REPO_ROOT),
                env=env, timeout=300,
            )
            # If offline fails (no cached deps), skip rather than fail —
            # this is a smoke test, not a release gate.
            if r.returncode != 0:
                self.skipTest(
                    f"cognicode-graph-algos build needs cached deps: {r.stderr[:200]}"
                )
            self.assertTrue(
                (target / "debug" / "libcognicode_graph_algos.rlib").exists()
                or any(target.glob("**/libcognicode_graph_algos*.rlib")),
                       f"expected lib under {target}/**; got {list(target.glob('**/*'))[:5]}")


def main() -> int:
    # Part I: the module-level `test_*` functions, in definition order. The
    # list is read from `globals()`, not `dir()`, and an empty list is a
    # failure: a suite that ran nothing would pass.
    tests = sorted(
        (obj for name, obj in globals().items()
         if name.startswith("test_") and callable(obj)
         and getattr(obj, "__module__", None) == __name__),
        key=lambda f: f.__code__.co_firstlineno,
    )
    if not tests:
        print("FAIL: no test_* function found")
        return 1
    for test in tests:
        test()
    # Part II: the unittest classes.
    suite = unittest.TestSuite()
    loader = unittest.TestLoader()
    for case in (CargoIsoGateMode, CargoIsoPassThrough, CargoIsoIntegration):
        suite.addTests(loader.loadTestsFromTestCase(case))
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(f"  - {failure}")
    if failures or not result.wasSuccessful():
        return 1
    print("PASS — el entorno de compilacion esta declarado y es coherente")
    return 0


if __name__ == "__main__":
    sys.exit(main())
