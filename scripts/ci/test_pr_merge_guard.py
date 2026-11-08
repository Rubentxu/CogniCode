"""Contract for scripts/git/gh-pr-merge-checked.sh.

The wrapper validates the squash title and body with git-control before
`gh pr merge`, so a message GitHub would create server-side (where the local
commit-msg hook never runs) is judged first. The plumbing is tested with stub
binaries; the policy itself is tested against the real git-control when it is
installed, because the stub cannot prove the wrapper reuses the real list.
"""

import json
import os
import shutil
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WRAPPER = ROOT / "scripts" / "git" / "gh-pr-merge-checked.sh"

DENY_BODY = {
    "decision": "deny",
    "admitted": False,
    "diagnostics": [
        {
            "id": "GCTL-MSG-002",
            "summary": "agent name in message",
            "evidence": [{"kind": "snippet", "label": "match", "text": "Zorblax\n"}],
            "location": {"kind": "message_line", "line": 3},
        }
    ],
}
DENY_TITLE = {
    "decision": "deny",
    "admitted": False,
    "diagnostics": [
        {
            "id": "GCTL-MSG-002",
            "summary": "agent name in message",
            "evidence": [{"kind": "snippet", "label": "match", "text": "Zorblax\n"}],
            "location": {"kind": "message_line", "line": 1},
        }
    ],
}
ALLOW = {"decision": "allow", "admitted": True, "diagnostics": []}

STUB_GIT_CONTROL = """#!/usr/bin/env bash
# args: check --file F --json
file=""
while [[ $# -gt 0 ]]; do
  if [[ "$1" == "--file" ]]; then file="$2"; shift; fi
  shift
done
cp "$file" "$STUB_DIR/message.txt"
cat "$STUB_DIR/verdict.json"
if grep -q '"deny"' "$STUB_DIR/verdict.json"; then exit 1; fi
exit 0
"""

STUB_GH = """#!/usr/bin/env bash
if [[ "$1 $2" == "pr view" ]]; then
  case "$*" in
    *title*) cat "$STUB_DIR/pr_title.txt" ;;
    *) cat "$STUB_DIR/pr_commits.txt" ;;
  esac
  exit 0
fi
printf '%s\\n' "$*" >> "$STUB_DIR/gh_calls.txt"
exit 0
"""


def _executable(path, text):
    path.write_text(text, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IEXEC)


class PrMergeGuardContract(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp, True)
        _executable(self.tmp / "git-control", STUB_GIT_CONTROL)
        _executable(self.tmp / "gh", STUB_GH)
        (self.tmp / "pr_title.txt").write_text("feat: derived title\n", encoding="utf-8")
        (self.tmp / "pr_commits.txt").write_text(
            "* feat: first\n\nbody of first\n\n* fix: second\n\n", encoding="utf-8"
        )

    def run_wrapper(self, args, verdict=ALLOW, git_control=None, gh=None):
        (self.tmp / "verdict.json").write_text(json.dumps(verdict), encoding="utf-8")
        env = dict(
            os.environ,
            STUB_DIR=str(self.tmp),
            GIT_CONTROL_BIN=git_control or str(self.tmp / "git-control"),
            GH_BIN=gh or str(self.tmp / "gh"),
        )
        return subprocess.run(
            ["bash", str(WRAPPER), *args],
            capture_output=True,
            text=True,
            env=env,
            timeout=60,
        )

    def gh_calls(self):
        log = self.tmp / "gh_calls.txt"
        return log.read_text(encoding="utf-8") if log.exists() else ""

    def test_wrapper_exists_and_parses(self):
        self.assertTrue(WRAPPER.is_file(), "missing scripts/git/gh-pr-merge-checked.sh")
        done = subprocess.run(["bash", "-n", str(WRAPPER)], capture_output=True, text=True, timeout=30)
        self.assertEqual(done.returncode, 0, done.stderr)

    def test_blocked_body_aborts_before_merging_and_names_term_and_rule(self):
        done = self.run_wrapper(["7", "--subject", "feat: ok", "--body", "written by Zorblax"], DENY_BODY)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("GCTL-MSG-002", done.stderr)
        self.assertIn("Zorblax", done.stderr)
        self.assertEqual(self.gh_calls(), "", "gh pr merge must not run when blocked")

    def test_blocked_title_aborts_before_merging(self):
        done = self.run_wrapper(["7", "--subject", "feat: Zorblax", "--body", "clean"], DENY_TITLE)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("GCTL-MSG-002", done.stderr)
        self.assertEqual(self.gh_calls(), "")

    def test_clean_message_merges_with_exactly_what_was_validated(self):
        done = self.run_wrapper(["7", "--subject", "feat: ok", "--body", "clean body"])
        self.assertEqual(done.returncode, 0, done.stderr)
        calls = self.gh_calls()
        self.assertIn("pr merge 7 --squash", calls)
        self.assertIn("--subject feat: ok", calls)
        self.assertIn("--body clean body", calls)

    def test_missing_body_is_derived_from_the_branch_commits_and_validated(self):
        done = self.run_wrapper(["7", "--subject", "feat: ok"])
        self.assertEqual(done.returncode, 0, done.stderr)
        validated = (self.tmp / "message.txt").read_text(encoding="utf-8")
        self.assertIn("body of first", validated)
        self.assertIn("body of first", self.gh_calls(), "the merged body must be the validated one")

    def test_missing_subject_is_derived_from_the_pull_request(self):
        done = self.run_wrapper(["7", "--body", "clean"])
        self.assertEqual(done.returncode, 0, done.stderr)
        validated = (self.tmp / "message.txt").read_text(encoding="utf-8")
        self.assertTrue(validated.startswith("feat: derived title (#7)"), validated)

    def test_the_squash_suffix_is_part_of_the_validated_header(self):
        self.run_wrapper(["123", "--subject", "feat: ok", "--body", "b"])
        header = (self.tmp / "message.txt").read_text(encoding="utf-8").splitlines()[0]
        self.assertEqual(header, "feat: ok (#123)")

    def test_a_missing_pull_request_number_is_refused(self):
        done = self.run_wrapper([])
        self.assertNotEqual(done.returncode, 0)
        self.assertEqual(self.gh_calls(), "")

    def test_a_missing_git_control_fails_closed(self):
        done = self.run_wrapper(["7", "--subject", "feat: ok", "--body", "b"], git_control="/nonexistent/git-control")
        self.assertNotEqual(done.returncode, 0)
        self.assertEqual(self.gh_calls(), "", "no validator means no merge")

    def test_the_term_list_is_not_duplicated_in_the_wrapper(self):
        text = WRAPPER.read_text(encoding="utf-8").lower()
        for term in ("anthropic", "openai", "codex", "copilot", "gemini"):
            with self.subTest(term=term):
                self.assertNotIn(term, text)

    @unittest.skipUnless(shutil.which("git-control"), "git-control is not installed on this machine")
    def test_real_git_control_blocks_a_blocked_term_and_counts_the_suffix(self):
        real = shutil.which("git-control")
        blocked = self.run_wrapper(
            ["7", "--subject", "feat: ok", "--body", "written with Claude"], git_control=real
        )
        self.assertNotEqual(blocked.returncode, 0, blocked.stderr)
        self.assertIn("GCTL-MSG-002", blocked.stderr)
        self.assertEqual(self.gh_calls(), "")

        clean = self.run_wrapper(["7", "--subject", "feat: ok", "--body", "plain body"], git_control=real)
        self.assertEqual(clean.returncode, 0, clean.stderr)

        # A header of 115 characters fits alone but not with " (#123)" (122 > 120).
        subject = "feat: " + "x" * 109
        self.assertEqual(len(subject), 115)
        self.tmp.joinpath("gh_calls.txt").unlink(missing_ok=True)
        long = self.run_wrapper(["123", "--subject", subject, "--body", "b"], git_control=real)
        self.assertNotEqual(long.returncode, 0, "the suffix must be counted against the header limit")
        self.assertEqual(self.gh_calls(), "")


def test_pr_merge_guard_unittest_cases_pass() -> None:
    """`scripts/run_contract_tests.py` reads module-level `test_*` functions only,
    so the `unittest` class above would run zero cases there and look green."""
    import io

    suite = unittest.defaultTestLoader.loadTestsFromTestCase(PrMergeGuardContract)
    result = unittest.TextTestRunner(stream=io.StringIO(), verbosity=0).run(suite)
    assert result.testsRun > 0, "the PrMergeGuardContract suite ran no case"
    assert result.wasSuccessful(), "; ".join(
        f"{case.id()}: {trace.strip().splitlines()[-1]}"
        for case, trace in result.failures + result.errors
    )


if __name__ == "__main__":
    unittest.main()
