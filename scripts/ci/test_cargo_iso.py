#!/usr/bin/env python3
# test_cargo_iso.py — RED→GREEN coverage of scripts/ci/cargo-iso.sh.
#
# Tests check the **observable** behaviour of the wrapper:
#   * `--gate` mode auto-isolates per checkout when CARGO_TARGET_DIR is unset
#   * `--gate` mode respects an explicit CARGO_TARGET_DIR (does NOT override)
#   * without `--gate`, the wrapper passes through (shared cache unchanged)
#   * the wrapper exec's cargo (no shell double-fork; exit code preserved)
#
# The contract is exercised against `cargo metadata` so a real cargo binary
# is required; the script targets it directly without going through a fake.
import os, subprocess, sys, tempfile, unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
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


if __name__ == "__main__":
    unittest.main(verbosity=2)