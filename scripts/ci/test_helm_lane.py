#!/usr/bin/env python3
"""Contract: the faithful-render lane pins helm and fails when the engine is absent.

The Helm tests that drive the real `helm template` engine skip loudly on a
machine without helm, and a skip is a green run. `scripts/ci/helm-lane.sh` is
the lane that cannot skip: it resolves the helm pinned in `.tool-versions`,
checks the binary, sets COGNICODE_REQUIRE_HELM=1 and runs the Helm tests.

The behaviour is exercised against a copy of the script with stub `asdf`,
`helm` and `cargo` executables, so the contract needs no network, no helm and
no compilation.

Run: python3 scripts/run_contract_tests.py scripts/ci/test_helm_lane.py
"""

from __future__ import annotations

import hashlib
import os
import re
import shutil
import stat
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LANE = ROOT / "scripts" / "ci" / "helm-lane.sh"
PIN_RE = re.compile(r'^HELM_LINUX_AMD64_SHA256="([0-9a-f]{64})"$', re.MULTILINE)
REAL_TESTS = (
    "helm_model::the_real_engine_renders_the_chart_when_it_is_installed",
    "helm_pack::the_real_engine_feeds_the_pack_when_it_is_installed",
    "helm_model::the_real_engine_agrees_with_the_double",
    "mode_equivalence::helm_chart_rendered_by_the_real_engine_reaches_pass",
)
PASSING_RUN = "".join(f"test {name} ... ok\n" for name in REAL_TESTS)


def write_executable(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


class Sandbox:
    """A throwaway repository holding a copy of the lane and the stubs."""

    def __init__(self, tmp: Path, *, tool_versions: str, helm_version: str, cargo_out: str, cargo_rc: int = 0):
        self.root = tmp / "repo"
        self.record = tmp / "cargo-record.txt"
        self.helm_dir = tmp / "asdf" / "installs" / "helm" / "9.9.9"
        stubs = tmp / "stubs"
        (self.root / "scripts" / "ci").mkdir(parents=True)
        (self.root / ".tool-versions").write_text(tool_versions, encoding="utf-8")

        helm = self.helm_dir / "bin" / "helm"
        write_executable(helm, f"#!/bin/sh\necho v{helm_version}\n")
        # The copy pins the checksum of the stub binary, so the checksum gate
        # is exercised with a hash that is real rather than skipped.
        digest = hashlib.sha256(helm.read_bytes()).hexdigest()
        source = LANE.read_text(encoding="utf-8")
        self.script = self.root / "scripts" / "ci" / "helm-lane.sh"
        self.script.write_text(PIN_RE.sub(f'HELM_LINUX_AMD64_SHA256="{digest}"', source), encoding="utf-8")

        write_executable(
            stubs / "asdf",
            "#!/bin/sh\n"
            'case "$1" in\n'
            "  plugin) echo helm ;;\n"
            f'  where) echo "{self.helm_dir}" ;;\n'
            "  install) exit 1 ;;\n"
            "esac\n",
        )
        write_executable(
            stubs / "cargo",
            "#!/bin/sh\n"
            f'echo "REQUIRE=$COGNICODE_REQUIRE_HELM FIRST_PATH=${{PATH%%:*}}" > "{self.record}"\n'
            f"printf '%s' '{cargo_out}'\n"
            f"exit {cargo_rc}\n",
        )
        self.stubs = stubs

    def run(self, *, with_asdf: bool = True) -> subprocess.CompletedProcess[str]:
        env = {"PATH": f"{self.stubs}:/usr/bin:/bin", "HOME": str(self.root)}
        if not with_asdf:
            (self.stubs / "asdf").unlink()
            env["PATH"] = "/usr/bin:/bin"
        return subprocess.run(
            ["bash", str(self.script)], env=env, capture_output=True, text=True, timeout=60
        )


def sandbox(**kwargs) -> tuple[tempfile.TemporaryDirectory, Sandbox]:
    tmp = tempfile.TemporaryDirectory()
    defaults = dict(tool_versions="helm 9.9.9\n", helm_version="9.9.9", cargo_out=PASSING_RUN)
    return tmp, Sandbox(Path(tmp.name), **{**defaults, **kwargs})


def lane_outcome(**kwargs) -> tuple[subprocess.CompletedProcess[str], Sandbox, tempfile.TemporaryDirectory]:
    tmp, box = sandbox(**kwargs)
    return box.run(), box, tmp


def test_the_lane_exports_the_requirement_and_puts_the_pinned_helm_first() -> None:
    result, box, tmp = lane_outcome()
    with tmp:
        assert result.returncode == 0, result.stderr + result.stdout
        record = box.record.read_text(encoding="utf-8")
        assert "REQUIRE=1" in record, record
        assert f"FIRST_PATH={box.helm_dir}/bin" in record, record


def test_the_lane_fails_when_tool_versions_does_not_pin_helm() -> None:
    result, _, tmp = lane_outcome(tool_versions="java temurin-24.0.2+12\n")
    with tmp:
        assert result.returncode != 0
        assert "no 'helm <version>' line" in result.stderr, result.stderr


def test_the_lane_fails_without_asdf() -> None:
    tmp, box = sandbox()
    with tmp:
        if shutil.which("asdf", path="/usr/bin:/bin"):
            return
        result = box.run(with_asdf=False)
        assert result.returncode != 0
        assert "asdf is not installed" in result.stderr, result.stderr


def test_the_lane_fails_when_the_installed_helm_is_not_the_pin() -> None:
    result, _, tmp = lane_outcome(helm_version="9.9.8")
    with tmp:
        assert result.returncode != 0
        assert "the pin is v9.9.9" in result.stderr, result.stderr


def test_the_lane_fails_when_the_binary_does_not_match_the_pinned_checksum() -> None:
    result, box, tmp = lane_outcome()
    with tmp:
        if os.uname().sysname != "Linux" or os.uname().machine != "x86_64":
            return
        tampered = box.script.read_text(encoding="utf-8")
        tampered = PIN_RE.sub('HELM_LINUX_AMD64_SHA256="' + "0" * 64 + '"', tampered)
        box.script.write_text(tampered, encoding="utf-8")
        result = box.run()
        assert result.returncode != 0
        assert "expected " + "0" * 64 in result.stderr, result.stderr


def test_the_lane_fails_when_a_real_engine_test_skipped() -> None:
    skipped = PASSING_RUN.replace("... ok", "... SKIPPED: `helm` is not installed\nok", 1)
    result, _, tmp = lane_outcome(cargo_out=skipped)
    with tmp:
        assert result.returncode != 0
        assert "skipped" in result.stderr, result.stderr


def test_the_lane_fails_when_a_real_engine_test_did_not_run() -> None:
    result, _, tmp = lane_outcome(cargo_out=PASSING_RUN.split("\n", 1)[1])
    with tmp:
        assert result.returncode != 0
        assert "did not run and pass" in result.stderr, result.stderr


def test_the_lane_fails_when_the_tests_fail() -> None:
    result, _, tmp = lane_outcome(cargo_rc=101)
    with tmp:
        assert result.returncode != 0


def test_the_checksum_pin_is_a_sha256_and_the_pin_is_a_helm_release() -> None:
    assert PIN_RE.search(LANE.read_text(encoding="utf-8")), "no pinned sha256 in the lane"
    pins = re.findall(r"^helm (\d+\.\d+\.\d+)$", (ROOT / ".tool-versions").read_text(encoding="utf-8"), re.M)
    assert len(pins) == 1, f"expected one exact helm release in .tool-versions, got {pins}"


def test_the_tests_the_lane_requires_exist_in_the_sources() -> None:
    tests = ROOT / "crates" / "cognicode-core" / "tests" / "it"
    for name in REAL_TESTS:
        module, function = name.split("::")
        assert f"fn {function}" in (tests / f"{module}.rs").read_text(encoding="utf-8"), name


def test_the_justfile_exposes_the_lane() -> None:
    justfile = (ROOT / "justfile").read_text(encoding="utf-8")
    assert re.search(r"^helm-lane:\n(?:.*\n)*?\s+bash scripts/ci/helm-lane\.sh", justfile, re.M)
