#!/usr/bin/env python3
"""Nothing in a PipelineK stage body is inert.

The first end-to-end run of `merge-gate.pipeline.kts` on 2026-10-02 built three
release binaries successfully and then failed its own verification stage:

    FAIL: target/release/cognicode was not built

Nothing was wrong with the build. `~/.cargo/config.toml` on this machine sets
`build.target-dir = /var/home/rubentxu/cargo-targets`, because the checkout is
one of several sharing a machine. The stage that built the binaries asked cargo
where to put them; the stage that verified them had the directory hardcoded. A
verifier that hardcodes where the artifact lands is not verifying the artifact —
it is checking a directory nothing was ever going to write. On a machine with no
`build.target-dir` it would have passed while checking nothing at all, which is
the direction that matters: the bug was invisible exactly where it was harmless.

The same run surfaced the sibling defect, found by reading rather than by
failing. Inside a Kotlin raw string, `$cd` interpolates the Kotlin value, and
`${'$'}cd` emits a literal `$cd` for the shell. `cd` is a shell builtin, not a
variable, so the shell expanded it to nothing and this line:

    ${'$'}cd || exit 1

reached bash as

     || exit 1

— a guard that guards nothing, on a stage that never changed directory. It read
exactly like the guard used by the other 59 stages in the file, and it existed in
seven of them. Those seven passed only because the pipeline happened to be
launched from the repository root. Measured with `pipelinek` 0.46.0 rather than
assumed; the probe is `/tmp`-shaped and cheap to repeat.

Both are the shape the repository already has a name for: a written guarantee no
mechanism applies, the N+66 ghost-filter defect. The difference is that this
contract can see it. Both live in one file because they were found in the same
run and share a root — a stage body containing text that looks like it
constrains something and does not.

What is pinned:

- No pipeline may name a cargo artifact directory directly. Artifact paths are
  resolved through `scripts/ci/target-dir.sh`, which asks cargo rather than
  re-implementing the layering of `CARGO_TARGET_DIR` over repository config over
  user config.
- No pipeline may write `${'$'}cd` expecting a directory change.
- The resolver must exist and be executable, because stages run it as a command
  rather than through `bash` and a missing bit fails at run time, not at
  validation time.

Comments are exempt from the first two. The fix for each defect is a comment
naming it, and a contract that failed on those comments would be failing on the
record of why the defect existed.

Run:
    python3 scripts/ci/test_pipeline_stage_bodies.py
"""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

# The renderer's ORDER is the proof in `test_candidate_packaging.py`, and getting
# it wrong makes a contract pass on a broken pipeline. Two contracts that
# execute stage bodies have to agree on that order, so they share one renderer
# instead of each re-deriving it. `run_contract_tests.py` puts a contract's own
# directory on sys.path precisely so a suite can import a sibling.
import test_candidate_packaging as packaging

REPO_ROOT = Path(__file__).resolve().parents[2]

# The six lanes. Discovered from the directory rather than listed, so a pipeline
# added tomorrow is covered tomorrow. A hand-maintained list is the same ghost
# filter this file exists to prevent.
PIPELINE_GLOB = "*.pipeline.kts"

RESOLVER_REL = "scripts/ci/target-dir.sh"

# A direct path into a cargo output directory. `target/release/…` is where cargo
# writes only when nothing overrides it; `$TARGET_DIR/release/…` is the resolved
# form and is deliberately not matched, because the variable name is what
# separates "asked cargo" from "assumed".
HARDCODED_ARTIFACT = re.compile(
    r"(?<![\w/\-])target/(?:release|debug|[a-z0-9_]+-unknown-[a-z0-9\-]+)/(?:release/)?[\w.\-]+"
)

# The Kotlin escape for a shell variable, applied to a shell builtin.
INERT_CD = re.compile(r"\$\{'\$'\}cd\b")

# Output directories a stage owns and therefore has to establish before writing.
# The second element names the writer that has to do it, and is only here so the
# pair reads as the claim it is: this directory, written by that stage.
OWNED_OUTPUT_DIRS = (("release", "cognicode-release generate"),)


def pipelines() -> list[Path]:
    return sorted(p for p in REPO_ROOT.glob(PIPELINE_GLOB) if p.is_file())


def code_lines(source: str) -> list[tuple[int, str]]:
    """Lines that are code rather than commentary, with their 1-based numbers.

    Two comment syntaxes live in these files. `//` is Kotlin's, and a raw string
    body can also carry a shell `#` comment — several stages explain themselves
    inside the script they run. A contract that failed on those would be failing
    on the very comments that record why the defect existed, and a linter with a
    known false positive is worse than no linter: it teaches its reader to
    ignore it. Both are skipped only when they start the line.

    A trailing `//` or `#` after code is not skipped, because the code part is
    what runs. This is the conservative direction: it can miss a path hidden
    behind a trailing comment, which would be a false negative, and cannot
    invent one, which would be a false positive.
    """
    out: list[tuple[int, str]] = []
    for number, line in enumerate(source.splitlines(), start=1):
        stripped = line.lstrip()
        if stripped.startswith("//") or stripped.startswith("#"):
            continue
        out.append((number, line))
    return out


def test_every_pipeline_compiles() -> None:
    """El pipeline tiene que COMPILAR, no solo tener stages con forma.

    MEDIDO 2026-10-04, lane v0.101.7. Dos parrafos de comentario de
    `release-candidate.pipeline.kts` explicaban como escapar un signo de
    dollar escribiendo el signo de dollar, y al compilar Kotlin leyo esos dos
    signos como plantillas: `Unresolved reference 'key'` y `Unresolved
    reference 'host'`. La lane murio en el minuto dos, antes de la primera
    stage, con 212 contratos en verde.

    Y ese es el punto: los contratos de este fichero EJECUTAN el cuerpo de una
    stage, pero no compilan el script que la contiene. Renderizar un cuerpo y
    ejecutarlo dice que el shell es correcto; no dice que el fichero que lo
    contiene sea un programa de Kotlin. Un error de compilacion es
    invisible a toda esta suite, y por eso la asercion vive aqui y no en otro
    sitio: este es el fichero que ya vigila las stages.

    `pipelinek validate` compila y comprueba el grafo de stages sin ejecutar
    ninguna. Si no esta en el PATH, el contrato se degrada a "no se puede
    responder" en vez de fingir que ha comprobado algo: la misma regla que ya
    aplica el contrato de tags visibles.
    """
    if shutil.which("pipelinek") is None:
        print("SKIP - pipelinek no esta en el PATH")
        return
    failures: list[str] = []
    for pipeline in sorted(REPO_ROOT.glob("*.pipeline.kts")):
        done = subprocess.run(
            ["pipelinek", "validate", str(pipeline)],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            timeout=600,
        )
        # MEDIDO 2026-10-04: el veredicto va por stderr, no por stdout. Buscarlo
        # en el stream equivocado hace que este contrato reporte cinco pipelines
        # rotos que compilan bien, que es peor que no tener el contrato.
        combined = done.stdout + done.stderr
        if "VALIDATION SUCCESSFUL" not in combined:
            # Los diagnostics vienen como JSON en una sola linea enorme, asi
            # que se cortan al mensaje y la posicion: el detalle entero esta
            # en la salida del comando.
            errors = re.findall(
                r'"severity":"ERROR","message":"([^"]+)","line":(\d+)', combined
            )
            detail = "; ".join(f"{msg} (linea {line})" for msg, line in errors[:5])
            failures.append(
                f"{pipeline.name}: {detail or 'validacion fallida sin diagnostics'}"
            )
    assert not failures, "estos pipelines no compilan:\n  " + "\n  ".join(failures)


def test_the_target_dir_resolver_is_runnable() -> None:
    """Stages run the resolver as a command, so it has to be one.

    `pipelinek validate` compiles the script and checks the stage graph. It does
    not execute a stage, so a resolver that is missing or lost its executable
    bit produces a pipeline that validates cleanly and fails on the first stage
    that needs a built binary — which is the same silent-until-late shape as the
    hardcoded path this file exists to catch.
    """
    resolver = REPO_ROOT / RESOLVER_REL
    assert resolver.is_file(), (
        f"{RESOLVER_REL} is missing. Every pipeline resolves cargo's target "
        f"directory through it, so without it each artifact path in this "
        f"repository is a guess again."
    )
    assert resolver.stat().st_mode & 0o111, (
        f"{RESOLVER_REL} is not executable. Stages invoke it directly rather "
        f"than through `bash`, so a missing bit fails at run time and not at "
        f"validation time."
    )


def test_no_stage_contains_an_inert_cd_guard() -> None:
    """`${'$'}cd` is not a directory change; it is a shell variable that is unset.

    Written that way it expands to nothing and the `|| exit 1` that follows
    guards a command that was never issued. The stage then runs in whatever
    directory `pipelinek` was launched from, which is a fact about the
    invocation and not about the pipeline.
    """
    offenders: list[str] = []
    for path in pipelines():
        for number, line in code_lines(path.read_text(encoding="utf-8")):
            if INERT_CD.search(line):
                offenders.append(
                    f"  {path.name}:{number}: `${{'$'}}cd` expands to a shell "
                    f"variable named `cd`, which does not exist, so the line runs "
                    f"as `|| exit 1` and the stage never changes directory. Write "
                    f"`$cd` so Kotlin interpolates the real path, or drop the guard."
                )
    assert not offenders, (
        f"{len(offenders)} inert cd guard(s) in pipeline stage bodies:\n"
        + "\n".join(offenders)
    )


def test_no_pipeline_hardcodes_a_cargo_artifact_path() -> None:
    """`target/release/` is an assumption about the machine, not a path.

    `CARGO_TARGET_DIR` and any `.cargo/config.toml` move it, and the release
    tool, the three release binaries and the cross-compiled per-target
    directories all move together. A stage that hardcodes it is asserting that
    the build wrote somewhere it was never told to write.
    """
    offenders: list[str] = []
    for path in pipelines():
        for number, line in code_lines(path.read_text(encoding="utf-8")):
            match = HARDCODED_ARTIFACT.search(line)
            if match:
                offenders.append(
                    f"  {path.name}:{number}: names cargo's output directory "
                    f"directly (`{match.group(0)}`). Cargo writes there only when "
                    f"nothing overrides it; `CARGO_TARGET_DIR` and any "
                    f"`.cargo/config.toml` move it, and this repository is built "
                    f"on a machine that sets one. Resolve it with "
                    f"`{RESOLVER_REL}` instead."
                )
    assert not offenders, (
        f"{len(offenders)} hardcoded cargo artifact path(s) in pipeline stage "
        f"bodies:\n" + "\n".join(offenders)
    )


def run_toolchain_gate(
    *,
    target: str,
    host: str = "x86_64-unknown-linux-gnu",
    installed: tuple[str, ...] | None = None,
    linker_env: str | None = None,
    cargo_config: str | None = None,
) -> subprocess.CompletedProcess[str]:
    """Runs the real `toolchain-for-$target` body against stubbed rustup/rustc.

    The stubs are the point: whether a linker exists is a property of the
    machine, and a contract that reads the maintainer's machine states the
    maintainer's configuration rather than the stage's decision. Everything the
    body can see is therefore constructed here — which targets are installed,
    which host it believes it is on, whether a linker is configured and whether
    that linker resolves.
    """
    text = (REPO_ROOT / "release-candidate.pipeline.kts").read_text(encoding="utf-8")

    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        repo = root / "repo"
        bindir = root / "bin"
        bindir.mkdir(parents=True)
        repo.mkdir()
        (repo / ".cargo").mkdir()

        # The stage calls the native-toolchain guard by relative path, after
        # `cd $repoRoot`, and that guard has its own file and its own stubs:
        # `test_cross_toolchain_guard.py`. This harness tests the LINKER guard,
        # and it must not depend on whether this machine can compile native code
        # for a foreign triple — that is the property the other file owns.
        #
        # So the guard is present and inert here. Absent, the stage fails with
        # "No such file or directory" and every positive test in this file
        # reports a linker problem that is not a linker problem. This is not a
        # fudge: a stage that stops calling the guard is caught by
        # `test_the_candidate_stage_actually_calls_the_guard`, and the guard's
        # own behaviour is caught by its own tests.
        (repo / "scripts/ci").mkdir(parents=True)
        native = repo / "scripts/ci/check-cross-toolchain.sh"
        native.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
        native.chmod(0o755)

        body = packaging.render(
            packaging.extract_stage_body(text, "toolchain-for-$target"),
            version="0.0.0",
            target=target,
            platform="unused",
            repo_root=str(repo),
        )

        for name, script in (
            (
                "rustup",
                "#!/usr/bin/env bash\n"
                'if [ "${2:-}" = "list" ] && [ "${3:-}" = "--installed" ]; then\n'
                f"  printf '%s\\n' {' '.join(installed if installed is not None else (target,))}\n"
                "fi\n",
            ),
            ("rustc", '#!/usr/bin/env bash\necho "host: %s"\n' % host),
        ):
            tool = bindir / name
            tool.write_text(script, encoding="utf-8")
            tool.chmod(0o755)

        if cargo_config is not None:
            (repo / ".cargo/config.toml").write_text(cargo_config, encoding="utf-8")

        # A real, executable linker. `command -v` accepts an executable path, so
        # this is enough to be "configured and resolving" without needing a
        # compiler on this machine.
        linker = root / "some-linker"
        linker.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
        linker.chmod(0o755)

        script = root / "stage.sh"
        script.write_text(f"#!/usr/bin/env bash\n{body}", encoding="utf-8")
        script.chmod(0o755)

        env = {
            "PATH": f"{bindir}:/usr/bin:/bin",
            # A CARGO_HOME of its own, so the second config the body reads is
            # this machine's file and not the one running the suite.
            "CARGO_HOME": str(root / "cargo-home"),
            "HOME": str(root / "home"),
        }
        (root / "cargo-home").mkdir()
        (root / "home").mkdir()
        if linker_env is not None:
            key = linker_env.format(
                triple=target.replace("-", "_"),
                upper=target.upper().replace("-", "_"),
                path=str(linker),
            )
            name, _, value = key.partition("=")
            env[name] = value

        return subprocess.run(
            [str(script)], capture_output=True, text=True, timeout=60, env=env
        )


def test_a_cross_target_with_no_linker_is_refused_before_the_build() -> None:
    """The stage names the requirement and has to check it.

    MEDIDO 2026-10-03, lane v0.101.4: `rustup target list --installed` contained
    `aarch64-unknown-linux-gnu`, so this stage reported success, and the build
    failed two stages later with exit 101 and a parser error about the target
    triple. The diagnostic named the compiler; the missing thing was the
    sentence this stage prints when the target is ABSENT — "a cross target also
    needs a linker for it". A guarantee that is printed on one branch and
    checked on another is the shape of defect this file exists for.
    """
    done = run_toolchain_gate(
        target="aarch64-unknown-linux-gnu", installed=("aarch64-unknown-linux-gnu",)
    )
    assert done.returncode != 0, (
        "un target cruzado instalado y sin linker configurado pasó la stage.\n"
        f"stdout:\n{done.stdout}"
    )
    assert "no linker is configured" in done.stdout, (
        "el rechazo no dice qué falta, así que no se puede distinguir de un "
        f"target no instalado.\nstdout:\n{done.stdout}"
    )


def test_a_cross_target_with_a_resolving_linker_passes() -> None:
    """The positive half, so refusing everything is not a way to pass the above.

    This is also the shape the maintainer's machine actually has: the linker
    lives in the environment, not in the repository, and a lane that could not
    run there would be a gate nobody can pass.
    """
    done = run_toolchain_gate(
        target="aarch64-unknown-linux-gnu",
        installed=("aarch64-unknown-linux-gnu",),
        linker_env="CC_{triple}={path}",
    )
    assert done.returncode == 0, (
        "un target cruzado con un linker que resuelve fue rechazado.\n"
        f"stdout:\n{done.stdout}\nstderr:\n{done.stderr}"
    )


def test_a_linker_declared_in_cargo_config_is_enough() -> None:
    """The third way a linker gets configured, and the one a repository owns.

    `CC_<triple>` and `CARGO_TARGET_<TRIPLE>_LINKER` are both environment. A
    `[target.<triple>]` table is the only one of the three that can be
    committed, so it is the one that would make a cross build reproducible
    rather than merely configured.
    """
    done = run_toolchain_gate(
        target="aarch64-unknown-linux-gnu",
        installed=("aarch64-unknown-linux-gnu",),
        cargo_config=(
            "[target.aarch64-unknown-linux-gnu]\n"
            'linker = "aarch64-linux-gnu-gcc"\n'
            "\n"
            "[target.x86_64-unknown-linux-musl]\n"
            'linker = "clang"\n'
        ),
    )
    assert done.returncode == 0, (
        "un linker declarado en .cargo/config.toml fue rechazado.\n"
        f"stdout:\n{done.stdout}\nstderr:\n{done.stderr}"
    )


def test_a_linker_that_does_not_resolve_is_named() -> None:
    """Configured is not the same as usable, and the message says which."""
    done = run_toolchain_gate(
        target="aarch64-unknown-linux-gnu",
        installed=("aarch64-unknown-linux-gnu",),
        linker_env="CARGO_TARGET_{upper}_LINKER=/nonexistent/linker-for-this-test",
    )
    assert done.returncode != 0, (
        "un linker configurado que no existe pasó la stage; fallaría dentro del "
        f"build, donde el error culpa al compilador.\nstdout:\n{done.stdout}"
    )
    assert "does not resolve" in done.stdout, (
        f"el rechazo no nombra el linker.\nstdout:\n{done.stdout}"
    )


def test_a_native_target_needs_no_linker_configuration() -> None:
    """The gate must not fire on the leg that always worked.

    A cross-build check written without a host comparison rejects every build
    on a machine where the host triple happens to equal a target, and then the
    release lane stops working for a reason that has nothing to do with the
    release.
    """
    done = run_toolchain_gate(
        target="x86_64-unknown-linux-gnu",
        host="x86_64-unknown-linux-gnu",
        installed=("x86_64-unknown-linux-gnu",),
    )
    assert done.returncode == 0, (
        "un target nativo sin linker configurado fue rechazado; la comprobación "
        f"no distingue host de cruzado.\nstdout:\n{done.stdout}\nstderr:\n{done.stderr}"
    )


def test_a_missing_target_still_fails_the_way_it_did() -> None:
    """The pre-existing check keeps its behaviour and its message."""
    done = run_toolchain_gate(
        target="aarch64-unknown-linux-gnu", installed=("x86_64-unknown-linux-gnu",)
    )
    assert done.returncode != 0, "un target no instalado pasó la stage"
    assert "is not installed" in done.stdout, (
        f"el mensaje preexistente de target ausente cambió.\nstdout:\n{done.stdout}"
    )


def a_build_output_directory_starts_from_itself() -> None:
    """A directory a stage OWNS does not survive from the previous run.

    MEDIDO 2026-10-04. `staging/` se limpia —`skill-bundles` borra los ficheros
    sueltos de la raiz antes de escribir, y la stage `package` crea su lane
    desde cero— y `release/`, que es la salida de `generate`, no se limpiaba.
    La lane v0.101.9, que corre en el worktree donde la v0.101.8 habia dejado
    su candidato, llego hasta `verify` y murio ahi:

        Error: artifact `cogh-0.101.8-x86_64-unknown-linux-gnu.tar.gz`
        declares version `0.101.8` but the release version is `0.101.9`

    El gate hizo bien su trabajo, y aun asi la lane no deberia depender de que
    un gate posterior lo detecte: `generate` es la duena de `release/`, y una
    duena que escribe sobre lo que encontro no es duena de nada.

    Y el fallo tiene que ser FATAL, no un aviso. QW-04 ("la limpieza no puede ser
    el veredicto") no aplica aqui, y la diferencia es el punto: esa regla es
    sobre lo que pasa DESPUES de decidir; esto es antes de decidir. Si no se
    puede establecer el estado de entrada, no hay estado conocido donde
    generar, y seguir seria generar sobre lo que la lane anterior dejo por
    casualidad.
    """
    offenders: list[str] = []
    for path in pipelines():
        text = path.read_text(encoding="utf-8")
        body = code_lines(text)
        for index, (number, line) in enumerate(body):
            for directory, _writer in OWNED_OUTPUT_DIRS:
                # Anchored on `--out <dir>`, not on the invocation: the command
                # is a shell continuation, so `generate` and its `--out` are on
                # different lines and a test that needs both on one line never
                # fires.
                if f"--out {directory}" not in line:
                    continue
                # The window is over INDICES into the code lines, not over line
                # numbers: `code_lines` drops commentary, so a slice taken with a
                # line number would walk a different distance than it looks and
                # silently look at the wrong place.
                window = "\n".join(entry for _, entry in body[max(0, index - 40) : index])
                if f"rm -rf -- {directory}" not in window:
                    offenders.append(
                        f"  {path.name}:{number}: se escribe en "
                        f"`{directory}/` sin limpiarlo antes. Un `mkdir -p` "
                        f"sobre un directorio que ya existe es el mecanismo "
                        f"por el que un directorio de build hereda estado de "
                        f"una corrida anterior, y el worktree que construye "
                        f"el candidato es de larga vida."
                    )
                elif "exit 1" not in window:
                    offenders.append(
                        f"  {path.name}:{number}: `{directory}/` se limpia pero "
                        f"sin fallo fatal si no se puede. Un directorio de "
                        f"salida que no se pudo establecer no es un aviso, es "
                        f"un estado que no se conoce."
                    )
    assert not offenders, (
        f"{len(offenders)} directorio(s) de salida sin estado propio:\n"
        + "\n".join(offenders)
    )


def main() -> int:
    failures: list[str] = []
    tests = [
        test_every_pipeline_compiles,
        test_the_target_dir_resolver_is_runnable,
        test_no_stage_contains_an_inert_cd_guard,
        test_no_pipeline_hardcodes_a_cargo_artifact_path,
        test_a_cross_target_with_no_linker_is_refused_before_the_build,
        test_a_cross_target_with_a_resolving_linker_passes,
        test_a_linker_declared_in_cargo_config_is_enough,
        test_a_linker_that_does_not_resolve_is_named,
        test_a_native_target_needs_no_linker_configuration,
        test_a_missing_target_still_fails_the_way_it_did,
        a_build_output_directory_starts_from_itself,
    ]
    for func in tests:
        try:
            func()
        except AssertionError as exc:
            failures.append(f"{func.__name__}: {exc}")

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(failure)
        return 1

    print(
        f"PASS — {len(pipelines())} pipelines: no inert stage text, no hardcoded "
        f"cargo artifact paths, resolver present and executable."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
