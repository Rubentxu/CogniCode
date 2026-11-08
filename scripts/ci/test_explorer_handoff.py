#!/usr/bin/env python3
"""Contract: the Explorer handoff guide only states what the tree shows."""

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
    def test_in_tree_crate_is_a_workspace_member_with_the_named_binary(self):
        text = DOC.read_text()
        root = tomllib.loads((ROOT / "Cargo.toml").read_text())
        self.assertIn("crates/cognicode-explorer", root["workspace"]["members"])
        self.assertIn("`crates/cognicode-explorer`", text)
        manifest = tomllib.loads((ROOT / "crates/cognicode-explorer/Cargo.toml").read_text())
        bins = [b["name"] for b in manifest.get("bin", [])]
        self.assertTrue(bins, "non-vacuity: the crate declares binaries")
        for name in re.findall(r"`(cognicode-control-plane)`", text):
            self.assertIn(name, bins)

    def test_classification_matches_sdk_surfaces(self):
        surfaces = json.loads((ROOT / "docs/distribution/SDK_SURFACES.json").read_text())
        blob = json.dumps(surfaces)
        self.assertIn("cognicode-explorer", blob)
        crates = surfaces["crates"]
        libs = [n for n, c in crates.items() if c["class"] == "library"]
        self.assertTrue(libs, "non-vacuity")
        self.assertIn("cognicode-explorer", crates)
        self.assertNotEqual(crates["cognicode-explorer"]["class"], "library")

    def test_external_repo_is_linked_and_no_removal_is_claimed(self):
        text = DOC.read_text()
        self.assertIn(EXTERNAL, text)
        self.assertFalse(claims_removed(text))
        self.assertIn("not merged", text)

    def test_removal_detector_discriminates(self):
        self.assertTrue(claims_removed("The in-tree copy has been removed."))
        self.assertFalse(claims_removed("Nothing in this repository has been removed."))

    def test_install_guide_agrees_that_explorer_binaries_are_unpublished(self):
        install = (ROOT / "docs/distribution/INSTALL.md").read_text()
        self.assertIn("`explorer-api` and `explorer-mcp` are not published", install)
        self.assertIn("explorer-api", DOC.read_text())


if __name__ == "__main__":
    unittest.main()
