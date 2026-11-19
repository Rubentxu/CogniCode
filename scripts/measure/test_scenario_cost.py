import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("scenario_cost.py")
REPO_ROOT = SCRIPT.resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("scenario_cost", SCRIPT)
assert SPEC and SPEC.loader
scenario_cost = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = scenario_cost
SPEC.loader.exec_module(scenario_cost)


def git(repo: Path, *args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=repo, check=True, text=True, capture_output=True
    ).stdout.strip()


class FixtureRepoContracts(unittest.TestCase):
    def test_language_scoped_fixture_keeps_rust_and_excludes_other_languages(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tmp_path = Path(temporary)
            corpus = tmp_path / "corpus"
            rust = corpus / "base" / "rust"
            other = corpus / "base" / "go"
            rust.mkdir(parents=True)
            other.mkdir(parents=True)
            (rust / "Cargo.toml").write_text("[package]\\nname='fixture'\\nversion='0.1.0'\\nedition='2021'\\n")
            (rust / "src").mkdir()
            (rust / "src" / "lib.rs").write_text("pub fn sample() {}\\n")
            (other / "unrelated.go").write_text("package unrelated\\n")
            dest = tmp_path / "fixture"

            scenario_cost.fixture_repo(
                corpus, dest, None, language="rust", identity_source=REPO_ROOT
            )

            self.assertTrue((dest / "rust" / "Cargo.toml").is_file())
            self.assertTrue((dest / "rust" / "src" / "lib.rs").is_file())
            self.assertFalse((dest / "go").exists())

    def test_fixture_repo_has_admitted_origin_and_conventional_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            tmp_path = Path(temporary)
            corpus = tmp_path / "corpus"
            (corpus / "base").mkdir(parents=True)
            (corpus / "base" / "source.txt").write_text("base\\n")
            dest = tmp_path / "fixture"

            scenario_cost.fixture_repo(corpus, dest, None, identity_source=REPO_ROOT)

            author_name, author_email = scenario_cost.git_identity(REPO_ROOT)
            self.assertEqual(git(dest, "remote", "get-url", "origin"), str(tmp_path / "fixture-origin.git"))
            self.assertEqual(git(dest, "config", "user.name"), author_name)
            self.assertEqual(git(dest, "config", "user.email"), author_email)
            self.assertEqual(
                git(dest, "show", "-s", "--format=%an <%ae>%n%cn <%ce>%n%s", "HEAD"),
                f"{author_name} <{author_email}>\n{author_name} <{author_email}>\n"
                "chore: establish benchmark base",
            )


if __name__ == "__main__":
    unittest.main()
