#!/usr/bin/env python3
"""Contract: the Explorer decommission receipt only states what the tree shows."""

import json
import re
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DOC = ROOT / "docs" / "distribution" / "EXPLORER_HANDOFF.md"
EXTERNAL = "https://github.com/Rubentxu/cognicode-explorer"


def claims_removed(text: str) -> bool:
    """True when the text says the in-tree crate is already gone."""
    for m in re.finditer(r"[^.\n]*\b(has been|was|is) (removed|decommissioned|deleted)\b", text, re.I):
        if not re.search(r"\b(not|nothing|never|no)\b", m.group(0), re.I):
            return True
    return False


class ExplorerHandoff(unittest.TestCase):
    def test_crates_are_not_workspace_members(self):
        root = tomllib.loads((ROOT / "Cargo.toml").read_text())
        members = root["workspace"]["members"]
        self.assertTrue(members, "non-vacuity: the workspace has members")
        for name in ("crates/cognicode-explorer", "crates/cognicode-runtime"):
            self.assertNotIn(name, members)

    def test_classification_matches_sdk_surfaces(self):
        surfaces = json.loads((ROOT / "docs/distribution/SDK_SURFACES.json").read_text())
        crates = surfaces["crates"]
        self.assertTrue([n for n, c in crates.items() if c["class"] == "library"], "non-vacuity")
        for name in ("cognicode-explorer", "cognicode-runtime"):
            self.assertNotIn(name, crates)

    def test_receipt_names_the_external_boundary(self):
        text = DOC.read_text()
        self.assertIn(EXTERNAL, text)
        self.assertIn("`v0.102.1`", text)
        self.assertIn("`3f8e10f2`", text)
        self.assertTrue(claims_removed("The crate has been removed from the workspace."))

    def test_the_orphaned_crate_directories_are_gone(self):
        # The decommission is complete only when no tracked copy of the
        # extracted crates is left on disk: a directory outside the
        # workspace is still a second source of truth that drifts.
        for name in ("cognicode-explorer", "cognicode-runtime"):
            self.assertFalse(
                (ROOT / "crates" / name).exists(),
                f"crates/{name} still exists on disk",
            )
        text = DOC.read_text()
        self.assertTrue(
            claims_removed("The directory copy has been removed from the tree."),
            "the receipt must state the removal",
        )
        self.assertIn("The directory copy has been removed from the tree", text)

    def test_no_in_tree_surface_still_builds_the_explorer(self):
        # Dockerfile, entrypoint and justfile used to build/launch
        # explorer-api from crates/cognicode-runtime. The container and
        # dev-up story for the Explorer lives in the external repository.
        for path in (ROOT / "Dockerfile", ROOT / "docker-entrypoint.sh", ROOT / "justfile"):
            text = path.read_text()
            self.assertNotIn(
                "explorer-api",
                text,
                f"{path.name} still names explorer-api",
            )
            self.assertNotIn(
                "cognicode-runtime",
                text,
                f"{path.name} still names cognicode-runtime",
            )

    def test_removal_detector_discriminates(self):
        self.assertTrue(claims_removed("The in-tree copy has been removed."))
        self.assertFalse(claims_removed("Nothing in this repository has been removed."))

    def test_install_guide_agrees_that_explorer_binaries_are_unpublished(self):
        install = (ROOT / "docs/distribution/INSTALL.md").read_text()
        self.assertIn("`explorer-api` and `explorer-mcp` are not published", install)
        self.assertIn("explorer-api", DOC.read_text())


def test_explorer_handoff_unittest_cases_pass() -> None:
    """`scripts/run_contract_tests.py` reads module-level `test_*` functions only,
    so the `unittest` classes above would run zero cases there and look green."""
    import io

    suite = unittest.TestSuite()
    for case in (ExplorerHandoff,):
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(case))
    result = unittest.TextTestRunner(stream=io.StringIO(), verbosity=0).run(suite)
    assert result.testsRun > 0, "the test_explorer_handoff suite ran no case"
    assert result.wasSuccessful(), "; ".join(
        f"{case.id()}: {trace.strip().splitlines()[-1]}"
        for case, trace in result.failures + result.errors
    )


if __name__ == "__main__":
    unittest.main()
