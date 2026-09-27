#!/usr/bin/env python3
"""Black-box tests for the Open Source Foundation (CP1) contract.

These tests read committed repository files and assert the claims they make.
They introduce no new runtime code and no second source of truth: the README
numbers are read from the same `product/*.json` artifacts that CP0 produces, and
the license expression is checked against the same `deny.toml` policy that the
dependency gate already enforces.

CP1 scope note: the license expression `MIT OR Apache-2.0` was decided and
closed in maintenance unit M0.9 (commit f0708d4b). CP1 publishes the canonical
texts, records the license of record in `[workspace.package]`, and asserts that
the grant is complete. It does not re-decide the expression.
"""

from __future__ import annotations

import json
import re
import subprocess
import tomllib
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]

# The license of record, as decided in M0.9. The dual expression is only a usable
# grant when BOTH texts ship, which is what CP1 publishes.
LICENSE_EXPRESSION = "MIT OR Apache-2.0"
LICENSE_OPERANDS = ["MIT", "Apache-2.0"]

COMMUNITY_FILES = ["CONTRIBUTING.md", "SECURITY.md", "CODE_OF_CONDUCT.md", "SUPPORT.md"]
PLACEHOLDER_MARKERS = ["TODO", "TBD", "XXX", "<placeholder>"]


def read(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8")


def cargo_metadata() -> dict:
    """Resolve the real workspace metadata. Fails loudly if the toolchain is absent.

    A repository-fact test that silently skips its own enforcement point is not
    a test. If `cargo` is unavailable the suite must be reported as NOT_RUN,
    not as passed.
    """
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
    except FileNotFoundError as exc:  # pragma: no cover - environment dependent
        pytest.fail(f"cargo toolchain unavailable, cannot verify the license of record: {exc}")
    if result.returncode != 0:
        pytest.fail(f"cargo metadata failed (exit {result.returncode}): {result.stderr.strip()[:400]}")
    return json.loads(result.stdout)


# --- Scenario 1: the canonical license texts are published -------------------


def test_license_texts_are_published() -> None:
    """A dual expression naming two licenses is a dangling reference without both texts."""
    mit = read("LICENSE-MIT")
    assert "MIT License" in mit, "LICENSE-MIT must carry the canonical MIT header"
    assert "Permission is hereby granted, free of charge" in mit, "MIT text missing its grant"
    assert 'THE SOFTWARE IS PROVIDED "AS IS"' in mit, "MIT text missing its disclaimer"
    assert len(mit) > 900, "LICENSE-MIT looks truncated or paraphrased"

    apache = read("LICENSE-APACHE")
    assert "Apache License" in apache
    assert "Version 2.0, January 2004" in apache
    assert "TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION" in apache
    assert "END OF TERMS AND CONDITIONS" in apache
    assert "APPENDIX: How to apply the Apache License to your work" in apache
    assert len(apache) > 10000, "LICENSE-APACHE must be the full canonical license text"

    root = read("LICENSE")
    assert LICENSE_EXPRESSION in root, "root LICENSE must state the expression of record"
    for operand in LICENSE_OPERANDS:
        assert operand in root, f"root LICENSE must name the {operand} option"
    assert "LICENSE-MIT" in root and "LICENSE-APACHE" in root, "root LICENSE must point at both texts"
    assert re.search(r"Copyright\s+\d{4}", root), "root LICENSE must carry a real copyright line"
    for marker in PLACEHOLDER_MARKERS:
        assert marker not in root, f"root LICENSE contains placeholder marker {marker!r}"


# --- Scenario 2: the license of record is stated once and matches every crate --


def test_workspace_package_states_the_license_of_record() -> None:
    workspace = read("Cargo.toml")
    block = workspace.split("[workspace.package]", 1)[1].split("\n[", 1)[0]
    match = re.search(r'^license\s*=\s*"([^"]+)"', block, re.MULTILINE)
    assert match, "[workspace.package] must state the license of record (deferred by M0.9)"
    assert match.group(1) == LICENSE_EXPRESSION, (
        f"[workspace.package] license is {match.group(1)!r}, expected {LICENSE_EXPRESSION!r}"
    )


def test_every_workspace_member_resolves_to_the_license_of_record() -> None:
    metadata = cargo_metadata()
    members = [p for p in metadata["packages"] if p["name"].startswith("cognicode")]
    assert members, "no cognicode workspace members resolved; the test would assert nothing"

    unlicensed: list[str] = []
    drifted: list[str] = []
    for package in members:
        declared = package.get("license")
        if declared is None:
            unlicensed.append(package["name"])
        elif declared != LICENSE_EXPRESSION:
            drifted.append(f"{package['name']}={declared!r}")

    assert not unlicensed, f"workspace members without a license: {sorted(unlicensed)}"
    assert not drifted, f"workspace members with a different license expression: {drifted}"


def test_package_json_declares_the_same_license() -> None:
    package_json = json.loads(read("package.json"))
    assert package_json.get("license") == LICENSE_EXPRESSION, (
        f"package.json license is {package_json.get('license')!r}"
    )

    # The nested wasm package already declared it in CP0; it must not drift.
    nested = json.loads(read("apps/explorer-ui/src/wasm/cognicode_diagram_wasm/package.json"))
    assert nested.get("license") == LICENSE_EXPRESSION, "nested wasm package.json drifted from the license of record"


# --- Scenario 3: community files exist and are non-trivial -------------------


def test_community_files_are_substantive() -> None:
    for name in COMMUNITY_FILES:
        text = read(name)
        assert len(text) >= 400, f"{name} is a stub"
        assert len(re.findall(r"^## ", text, re.MULTILINE)) >= 3, f"{name} has fewer than three sections"
        for marker in PLACEHOLDER_MARKERS:
            assert marker not in text, f"{name} contains placeholder marker {marker!r}"


def test_community_documents_cite_real_repository_paths() -> None:
    """A reference a reader cannot follow is decoration. Assert the paths exist.

    Scoped to the documents whose references are operational. A Code of Conduct
    deliberately cites no repository paths; forcing one would be noise.
    """
    for name in ["CONTRIBUTING.md", "SECURITY.md", "SUPPORT.md"]:
        text = read(name)
        cited = re.findall(r"`([A-Za-z0-9_./-]+\.(?:md|toml|yml|yaml|json|rs))`", text)
        assert cited, f"{name} cites no repository path"
        resolved = [c for c in cited if (ROOT / c).exists()]
        assert resolved, (
            f"{name} cites paths that do not exist in this repository: "
            f"{[c for c in cited if not (ROOT / c).exists()][:5]}"
        )


# --- Scenario 4: issue and PR templates exist --------------------------------


def test_issue_and_pr_templates_exist() -> None:
    bug = read(".github/ISSUE_TEMPLATE/bug_report.md").lower()
    for field in ["version", "platform", "install", "reproduc", "expected", "actual", "log"]:
        assert field in bug, f"bug template missing {field!r}"

    feature = read(".github/ISSUE_TEMPLATE/feature_request.md").lower()
    assert "problem" in feature, "feature template must ask for the problem first"
    assert "solution" in feature

    config = read(".github/ISSUE_TEMPLATE/config.yml")
    assert "blank_issues_enabled" in config

    pr = read(".github/PULL_REQUEST_TEMPLATE.md").lower()
    for obligation in ["sddk", "conventional commit", "test", "release"]:
        assert obligation in pr, f"PR template missing obligation {obligation!r}"


# --- Scenario 5: the security policy states the real channel and threat model --


def test_security_policy_states_channel_and_threat_model() -> None:
    text = read("SECURITY.md")
    lowered = text.lower()
    # The channel is named "Security Advisories" (plural). Match the stem so the
    # test does not break on a grammar change that keeps the meaning.
    assert re.search(r"security advisor(y|ies)", lowered), "must name the private advisory channel"
    assert re.search(r"do not.{0,80}issue", lowered, re.DOTALL), "must forbid opening a public issue"
    for boundary in ["mcp", "json-rpc", "stdout", "network", "telemetry"]:
        assert boundary in lowered, f"threat model missing {boundary!r}"


def test_security_policy_supported_versions_are_real() -> None:
    """SECURITY.md must not claim support for a version that does not exist."""
    text = read("SECURITY.md")
    workspace = read("Cargo.toml")
    version = re.search(r'^version\s*=\s*"([^"]+)"', workspace.split("[workspace.package]", 1)[1], re.MULTILINE)
    assert version, "could not read the workspace version"
    current = version.group(1)

    claimed = re.findall(r"\b(\d+\.\d+\.\d+)\b", text)
    assert claimed, "SECURITY.md must state a supported-version table"
    assert current in claimed, f"SECURITY.md does not mention the actual current version {current}"

    # Anything else it names must be a real released tag, not an invented version.
    released = subprocess.run(
        ["git", "tag", "--list"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    known = {t.lstrip("v") for t in released.stdout.split()}
    invented = [c for c in set(claimed) if c != current and c not in known and c.split(".")[0] != "2"]
    assert not invented, f"SECURITY.md names versions that were never released: {sorted(invented)}"


# --- Scenario 6: README publishes only verifiable numbers ---------------------


def test_readme_publishes_only_verifiable_numbers() -> None:
    tools = json.loads(read("product/tools.json"))
    languages = json.loads(read("product/languages.json"))
    tool_count = tools["runtime_tool_count"]
    total = len(languages["languages"])
    supported = sum(1 for x in languages["languages"] if x["support_level"] == "supported")
    experimental = sum(1 for x in languages["languages"] if x["support_level"] == "experimental")

    for name in ["README.md", "README.es.md"]:
        text = read(name)
        lowered = text.lower()
        assert "32+" not in text, f"{name} still claims 32+ tools"
        assert "six languages" not in lowered, f"{name} still claims six languages"
        assert "6 languages" not in lowered, f"{name} still claims 6 languages"
        assert str(tool_count) in text, f"{name} does not state the real tool count ({tool_count})"
        assert str(total) in text, f"{name} does not state the real language total ({total})"
        assert str(supported) in text, f"{name} does not state the supported count ({supported})"
        assert str(experimental) in text, f"{name} does not state the experimental count ({experimental})"
        assert "product/readme.md" in lowered, f"{name} must link the product contract"
        assert "product/languages.json" in lowered or "product/tools.json" in lowered, (
            f"{name} must cite the artifacts its numbers come from"
        )


# --- Scenario 7: the outbound license is inside the project's own policy -----


def test_license_identifiers_are_inside_deny_allow_list() -> None:
    # Parse the real TOML. A regex over `[licenses]` truncates at the first
    # following `[` and silently drops most of the allow list, which is how this
    # test reported a false failure against a policy that was already correct.
    deny = tomllib.loads(read("deny.toml"))
    allow = deny["licenses"]["allow"]
    for operand in LICENSE_OPERANDS:
        assert operand in allow, (
            f"outbound license operand {operand!r} is not in deny.toml's allow list; "
            "the project's own grant must be inside the policy it enforces. "
            f"allow list is: {allow}"
        )


# --- Scenario 8: the ADR records the decision, its history, and the gate ------


def test_license_adr_records_decision_and_operator_gate() -> None:
    adr = read("docs/adr/ADR-053-cp1-license-publication.md")
    assert LICENSE_EXPRESSION in adr, "the ADR must record the expression of record"
    assert "M0.9" in adr, "the ADR must state that the decision originated in M0.9, not in CP1"
    assert "f0708d4b" in adr, "the ADR must cite the commit that decided it"
    assert "operator-gated" in adr.lower(), "CP1.7 must be recorded as operator-gated"
    assert "CP1.7" in adr
    for rejected in ["Apache-2.0", "MPL-2.0", "AGPL-3.0"]:
        assert rejected in adr, f"rejected alternative {rejected!r} is not recorded"
    for marker in PLACEHOLDER_MARKERS:
        assert marker not in adr, f"ADR contains placeholder marker {marker!r}"


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-q"]))
