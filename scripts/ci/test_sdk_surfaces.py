#!/usr/bin/env python3
"""SDK surface contract: every workspace crate declares what it is.

The declaration lives in `docs/distribution/SDK_SURFACES.json` and is the only
place that says which crates are a supported public library, which are
applications distributed as binaries, and which are internal. The oracle for
"which crates exist" is `cargo metadata`, so a crate added to the workspace
without a classification fails here instead of silently becoming public (or
silently staying private).

The contract is also its own control: an empty workspace listing fails rather
than passing because there was nothing to compare.
"""

import json
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DECLARATION = ROOT / "docs" / "distribution" / "SDK_SURFACES.json"
CLASSES = {"library", "application", "internal"}
METADATA_DEADLINE_SECONDS = 120


def workspace_packages():
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=METADATA_DEADLINE_SECONDS,
        check=True,
    )
    return {p["name"]: p for p in json.loads(out.stdout)["packages"]}


def declared_crates():
    return json.loads(DECLARATION.read_text(encoding="utf-8"))["crates"]


class SdkSurfaceContract(unittest.TestCase):
    def test_workspace_listing_is_not_empty(self):
        self.assertTrue(workspace_packages(), "cargo metadata returned no packages")

    def test_declaration_exists(self):
        self.assertTrue(DECLARATION.is_file(), f"missing {DECLARATION.relative_to(ROOT)}")

    def test_every_workspace_crate_is_classified_and_none_is_stale(self):
        actual = set(workspace_packages())
        declared = set(declared_crates())
        self.assertEqual(actual - declared, set(), "crates without a classification")
        self.assertEqual(declared - actual, set(), "classified crates not in the workspace")

    def test_each_entry_has_a_known_class_and_a_reason(self):
        for name, entry in declared_crates().items():
            with self.subTest(crate=name):
                self.assertIn(entry.get("class"), CLASSES)
                self.assertTrue(entry.get("summary", "").strip(), "summary is required")

    def test_libraries_name_their_consumers(self):
        for name, entry in declared_crates().items():
            if entry.get("class") != "library":
                continue
            with self.subTest(crate=name):
                self.assertTrue(entry.get("consumers"), "a library must name its consumers")

    def test_a_library_is_not_marked_unpublishable(self):
        packages = workspace_packages()
        for name, entry in declared_crates().items():
            if entry.get("class") != "library" or name not in packages:
                continue
            with self.subTest(crate=name):
                self.assertNotEqual(
                    packages[name]["publish"], [], "library crate has `publish = false`"
                )

    def test_internal_crates_do_not_claim_consumers(self):
        for name, entry in declared_crates().items():
            if entry.get("class") == "internal":
                with self.subTest(crate=name):
                    self.assertFalse(entry.get("consumers"), "internal crates have no external consumers")


if __name__ == "__main__":
    unittest.main()
