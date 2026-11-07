#!/usr/bin/env python3
"""Contract: `openspec_conformance.py --validate-paths` finishes on a healthy
tree even when the specs directory is absent, and names the missing paths."""
from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[2] / "sandbox" / "scripts" / "openspec_conformance.py"


def run(cwd: Path, evidence: str) -> subprocess.CompletedProcess:
    (cwd / "map.json").write_text(evidence)
    return subprocess.run(
        [sys.executable, "-I", str(SCRIPT), "--validate-paths",
         "--evidence-map", "map.json", "--specs-dir", "none",
         "--output-prefix", "out/m"],
        cwd=cwd, capture_output=True, text=True, timeout=30,
    )


class ValidatePaths(unittest.TestCase):
    def test_healthy_tree_without_specs_dir_ends_with_rc_0(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root / "ev.txt").write_text("x")
            done = run(root, '{"a": {"status": "verified", "evidence_path": "ev.txt"}}')
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertNotIn("KeyError", done.stderr)

    def test_missing_path_is_named_and_fails(self):
        with tempfile.TemporaryDirectory() as d:
            done = run(Path(d), '{"a": {"status": "verified", "evidence_path": "gone.txt"}}')
            self.assertEqual(done.returncode, 1)
            self.assertIn("a", done.stderr)
            self.assertIn("gone.txt", done.stderr)


if __name__ == "__main__":
    unittest.main()
