#!/usr/bin/env python3
"""The candidate lane must produce the staging tree the flatten step consumes.

`scripts/ci/stage-platform-payloads.sh` reads exactly one shape:

    staging/payloads-<platform>/dist/<component>-<ver>-<plat>.tar.gz
    staging/payloads-<platform>/crates/<component>-<triple>.cdx.json
    staging/<bundle-id>-<version>.tar.gz          (pre-staged skill bundles)

and rejects anything else at the staging root. The GitHub workflow produced that
shape with `upload-artifact` / `download-artifact`: the `path:` list named the
files, the action created the `payloads-<platform>/` directories, and
`download-artifact` put them back under `staging/`.

PipelineK has no transfer, so the directories have to be created by the lane
that builds them. `release-candidate.pipeline.kts` did not: it packaged into
`dist/` at the repository root and never staged a skill bundle, so `payloads`
would have failed with "no payloads-* lane directories found" and `generate`
would have had an empty tree. `pipelinek validate` cannot see that — it
compiles the script and checks the stage graph, and both stages are perfectly
well formed.

This contract exists because the failure it guards is otherwise silent. The
flatten script's own tests pass, `check-release-matrix.sh` passes, and every
lane validates; the candidate is simply wrong in a way nothing observes until a
release is attempted. It is a wiring property — the lane and the script have to
agree about a directory name — so it is checked as wiring, against the lane that
does the work.

The first version of this file looked only for the string `staging/payloads-`
anywhere in the lane, and that was a false negative rather than a false pass: a
mutation that put packaging back into a bare `dist/` still left the string
behind, in the stage that *reads* the archives back for its standalone smoke
test. The check passed against a lane that does not package into the staging
layout — the exact defect it exists to catch. So the two rules below are one
property stated in the negative: the lane must build the staging layout, and it
must not write component tarballs anywhere else. Naming only the presence of a
string is not enough when the same string legitimately appears in a reader.

Run:
    python3 scripts/ci/test_release_candidate_layout.py
"""

from __future__ import annotations

import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
LANE = "release-candidate.pipeline.kts"
FLATTEN = "scripts/ci/stage-platform-payloads.sh"

# The two properties `stage-platform-payloads.sh` has to hold over a staging
# tree, spelled the way the script itself spells them. Kept here rather than
# imported so that a rename inside the script is a visible test failure instead
# of a silent import.
TIER1_TRIPLES = ("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu")
SHORT = {
    "x86_64-unknown-linux-gnu": "linux-x86-64",
    "aarch64-unknown-linux-gnu": "linux-aarch64",
}
COMPONENTS = ("cogh", "cognicode", "cognicode-mcp")

# Component tarballs written straight into a bare `dist/`. That is where the
# lane packaged before the layout was fixed, and the string `staging/payloads-`
# survives elsewhere in the file, so the absence of this is the half of the
# property that actually detects the regression.
BARE_DIST_WRITE = re.compile(r'tar\s+-czf\s+"?dist/')

# The staging layout, in either of the two places the lane may name it.
STAGING_LANE = "staging/payloads-"

# What the lane must arrange, and the consequence of not arranging it.
REQUIREMENTS = [
    (
        STAGING_LANE,
        "The lane does not build a `staging/payloads-<platform>/` directory.\n"
        "  `stage-platform-payloads.sh` reads exactly that layout and rejects a\n"
        "  staging root with anything else in it, so the candidate would be empty\n"
        "  and `generate` would have nothing to generate from. Under Actions the\n"
        "  directories were created by upload/download-artifact; PipelineK has no\n"
        "  transfer, so the lane that builds the payloads has to build the\n"
        "  directories.",
    ),
    (
        "skills --published",
        "The lane never stages the portable skill bundles.\n"
        "  `stage-platform-payloads.sh` expects `<bundle>-<version>.tar.gz` at the\n"
        "  staging root alongside the lane directories, and the release contract's\n"
        "  `skills --published` is what says which bundles those are. A list of\n"
        "  bundle ids written into the pipeline would be a second place to forget.",
    ),
]

failures: list[str] = []


def lane_text() -> str:
    path = REPO_ROOT / LANE
    if not path.is_file():
        raise SystemExit(
            f"FAIL: {LANE} is missing. It builds the candidate, so there is no "
            f"artifact whose layout could be checked."
        )
    return path.read_text(encoding="utf-8")


def test_the_candidate_lane_produces_the_layout_the_flatten_script_reads() -> None:
    """Both halves the flatten step needs, checked against the lane that builds."""
    text = lane_text()
    for needle, consequence in REQUIREMENTS:
        if needle not in text:
            failures.append(f"{LANE}: {consequence}")


def test_component_tarballs_are_not_written_into_a_bare_dist() -> None:
    """The negative half: the payloads land in the staging lane, nowhere else.

    Stated separately because the presence of `staging/payloads-` does not
    detect the regression it looks like it detects. A lane can name the staging
    layout in the stage that reads the archives back and still package into
    `dist/`, and the flatten step would find an empty candidate — which is the
    failure this file is here for.
    """
    text = lane_text()
    for number, line in enumerate(text.splitlines(), start=1):
        if BARE_DIST_WRITE.search(line):
            failures.append(
                f"{LANE}:{number}: writes component tarballs into a bare `dist/`: "
                f"{line.strip()}\n"
                f"  `stage-platform-payloads.sh` never looks there. It reads "
                f"`staging/payloads-<platform>/dist/`, and `generate` is given "
                f"`--staging staging`. Packaging into `dist/` leaves the "
                f"candidate empty, and every check downstream of it passes on "
                f"an empty set."
            )


def test_the_flatten_script_still_declares_that_layout() -> None:
    """The two sides are checked against each other, so both have to exist.

    If the script's expected shape changed, a lane matching the old one would be
    wrong in a way this contract would happily confirm. Asserting the script's
    own literals keeps the pair honest without parsing either into an AST.
    """
    path = REPO_ROOT / FLATTEN
    assert path.is_file(), f"{FLATTEN} is missing, so there is no layout to match"
    text = path.read_text(encoding="utf-8")
    for literal in ('payloads-', 'PLATFORM_SHORT_TO_TRIPLE', 'cdx.json'):
        assert literal in text, (
            f"{FLATTEN} no longer mentions `{literal}`. This contract checks the "
            f"lane against the script by name; if the script's layout was "
            f"renamed, update both together or the check is comparing two "
            f"things that no longer have a relationship."
        )


def build_staging(root: Path, version: str) -> Path:
    """A staging tree the flatten script should accept, built from the contract.

    Every Tier-1 platform is present with all three components and their SBOMs,
    because the script's closing sanity requires exactly that: a fixture with a
    single platform would exit non-zero for a reason that has nothing to do with
    the property under test, and a contract that cannot tell those two failures
    apart is not a contract.
    """
    staging = root / "staging"
    for triple in TIER1_TRIPLES:
        lane = staging / f"payloads-{SHORT[triple]}"
        (lane / "dist").mkdir(parents=True)
        (lane / "crates").mkdir(parents=True)
        for comp in COMPONENTS:
            (lane / "dist" / f"{comp}-{version}-{triple}.tar.gz").write_bytes(
                f"{comp} {version} {triple}".encode()
            )
            (lane / "crates" / f"{comp}-{triple}.cdx.json").write_text("{}")
    return staging


def run_flatten(staging: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", str(REPO_ROOT / FLATTEN), str(staging)],
        capture_output=True,
        text=True,
        check=False,
    )


def test_the_flatten_script_will_not_pick_between_two_candidate_payloads() -> None:
    """Two payloads for one component is a question, and the script must ask it.

    MEDIDO 2026-10-03, lane `v0.101.5`. El worktree que construye el candidato es
    de larga vida y `package-$target` crea su directorio de lane con `mkdir -p`
    sin limpiarlo, asi que la salida de la candidate anterior seguia ahi:

        staging/payloads-linux-x86-64/dist/
          cogh-0.101.4-...  cogh-0.101.5-...
          cognicode-0.101.4-...  cognicode-0.101.5-...
          cognicode-mcp-0.101.4-...  cognicode-mcp-0.101.5-...

    La seleccion de este script es `find ... -name "${comp}-[0-9]*-${platform}
    .tar.gz" -print -quit`: se queda con el primer archivo que encuentre, sin
    preguntar. Reproduciendo esas dos lineas sobre ese directorio, `cognicode`
    resolvio a `cognicode-0.101.4-x86_64-unknown-linux-gnu.tar.gz`, y el regex
    defensivo de la linea siguiente lo **aprobo**, porque ese regex tampoco
    menciona la version. `copy_unique` tampoco habria dicho nada: sus claves son
    los nombres de archivo, y `0.101.4` y `0.101.5` no colisionan.

    La unica razon por la que `v0.101.5` no se publico con el binario de
    `0.101.4` fue que otra stage habia comparado antes un numero de archivos. Eso
    es un accidente de orden, no una garantia: este script es la ultima cosa
    entre un staging tree y una release publicada, y no puede elegir un candidato
    sin decirlo.

    La version no se pasa como argumento a proposito. La autoridad de "que
    version es esta release" es del tag y de la lane, y la lane ya afirma que su
    `dist/` no contiene nada de otra version. Este script no necesita conocerla
    para poder rechazar la ambiguedad, y no conocerla le evita ser una segunda
    fuente de verdad.
    """
    with tempfile.TemporaryDirectory() as tmp:
        staging = build_staging(Path(tmp), "0.101.5")
        for comp in COMPONENTS:
            (staging / f"payloads-linux-x86-64/dist/{comp}-0.101.4-x86_64-unknown-linux-gnu.tar.gz").write_bytes(
                f"{comp} 0.101.4".encode()
            )

        result = run_flatten(staging)

        assert result.returncode != 0, (
            "el script aplanó un staging tree con dos payloads por componente y "
            "siguió adelante. Eso es elegir un release por orden de directorio, "
            "y el release elegido puede ser el de la versión anterior.\n"
            f"stdout:\n{result.stdout}"
        )
        assert "0.101.4" in result.stderr, (
            "el rechazo no nombra el payload competidor, así que no se puede "
            "distinguir este caso de cualquier otro fallo. stderr fue:\n"
            f"{result.stderr}"
        )
        assert not (staging / "cognicode-0.101.4-x86_64-unknown-linux-gnu.tar.gz").exists(), (
            "un payload de la versión antigua llegó a la raíz del staging antes "
            "de que el script fallara"
        )


def test_the_flatten_script_still_flattens_a_clean_tree() -> None:
    """The positive half, through the same invocation as the negative one.

    Stated separately so that making the script refuse everything is not a way
    to pass the contract above. It has to accept a tree where each component has
    exactly one payload and put all of them at the staging root.
    """
    with tempfile.TemporaryDirectory() as tmp:
        staging = build_staging(Path(tmp), "0.101.5")

        result = run_flatten(staging)

        assert result.returncode == 0, (
            "un staging tree sin ambigüedad por componente fue rechazado.\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
        for triple in TIER1_TRIPLES:
            for comp in COMPONENTS:
                assert (staging / f"{comp}-0.101.5-{triple}.tar.gz").is_file(), (
                    f"falta el payload aplanado de {comp} para {triple}"
                )
                assert (staging / f"{comp}-{triple}.cdx.json").is_file(), (
                    f"falta el SBOM aplanado de {comp} para {triple}"
                )


def test_the_flatten_script_refuses_a_root_holding_two_versions() -> None:
    """The same ambiguity, one level up, where it is easiest to miss.

    The lane directories here are clean — one payload per component each — so
    nothing above this line objects. The contamination is at the staging root,
    which is where a previous *flatten* run leaves its output; the script's own
    header names that case. And the root cannot filter a stray payload out by
    shape: it accepts every `*-*.tar.gz` so that skill bundles
    (`{id}-{version}.tar.gz`) pass through, and a component payload
    (`{comp}-{version}-{token}.tar.gz`) matches the same glob. So a leftover
    payload sits beside the right one, and a check that asks "is there a
    `cognicode-*-<platform>.tar.gz` here?" is answered yes by both.

    This is why the check is a count and not a presence test, and it is a
    separate test because it is a different reachability: the lane-directory
    refusal above happens earlier in the run, so a fixture that triggers only
    this one has to keep the lanes clean.
    """
    with tempfile.TemporaryDirectory() as tmp:
        staging = build_staging(Path(tmp), "0.101.5")
        (staging / "cognicode-0.101.4-x86_64-unknown-linux-gnu.tar.gz").write_bytes(
            b"cognicode 0.101.4"
        )

        result = run_flatten(staging)

        assert result.returncode != 0, (
            "el script aplanó un staging root con dos versiones del mismo "
            f"componente y siguió adelante.\nstdout:\n{result.stdout}"
        )
        assert "0.101.4" in result.stderr, (
            "el rechazo no nombra el archivo competidor.\nstderr:\n"
            f"{result.stderr}"
        )


def main() -> int:
    tests = [
        test_the_candidate_lane_produces_the_layout_the_flatten_script_reads,
        test_component_tarballs_are_not_written_into_a_bare_dist,
        test_the_flatten_script_still_declares_that_layout,
        test_the_flatten_script_will_not_pick_between_two_candidate_payloads,
        test_the_flatten_script_refuses_a_root_holding_two_versions,
        test_the_flatten_script_still_flattens_a_clean_tree,
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
        "PASS — the candidate lane creates the staging layout the flatten "
        "script reads, and stages the skill bundles the contract declares."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
