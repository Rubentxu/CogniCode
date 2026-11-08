#!/usr/bin/env python3
"""README contract: the English and Spanish entry points describe one product.

Three things drift silently between `README.md` and `README.es.md`, and each
one sends a reader to something that does not work:

* the heading outline (a section written in one language only),
* the version that install and `--version` examples tell the reader to expect,
* relative links that point at files that are not in the tree.

The version oracle is `[workspace.package] version` in the root `Cargo.toml`.
The `## Versioning` section is history on purpose (it names old tags), so it is
excluded from the pinned-version check. The contract is its own control: a
README with no headings, no pinned example or no link fails instead of passing
because there was nothing to compare.
"""

import re
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EN = ROOT / "README.md"
ES = ROOT / "README.es.md"
HISTORY_HEADING = "versioning"
FENCE = "```"


def workspace_version() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    section = text.split("[workspace.package]", 1)[1]
    match = re.search(r'^version\s*=\s*"([^"]+)"', section, re.MULTILINE)
    assert match, "no version in [workspace.package]"
    return match.group(1)


def outline(path: Path) -> list[int]:
    """Heading levels in order, ignoring `#` lines inside code fences."""
    levels, in_fence = [], False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith(FENCE):
            in_fence = not in_fence
        elif not in_fence:
            m = re.match(r"(#{2,3}) \S", line)
            if m:
                levels.append(len(m.group(1)))
    return levels


def without_history(path: Path) -> str:
    """README text with the `## Versioning` section removed."""
    kept, skipping, in_fence = [], False, False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith(FENCE):
            in_fence = not in_fence
        if not in_fence and re.match(r"## ", line):
            skipping = line[3:].strip().lower() == HISTORY_HEADING
        if not skipping:
            kept.append(line)
    return "\n".join(kept)


PINNED = re.compile(
    r"(?:COGNICODE_VERSION=v|@v?|expect: [a-z-]+ )(\d+\.\d+\.\d+)"
)


def pinned_versions(path: Path) -> list[str]:
    return PINNED.findall(without_history(path))


def local_links(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8")
    targets = re.findall(r"\]\(([^)\s]+)\)", text)
    return [
        t.split("#", 1)[0]
        for t in targets
        if not re.match(r"[a-z]+:|#|mailto:", t) and t.split("#", 1)[0]
    ]


def install_docs() -> list[Path]:
    """Install and contribution guides a reader follows with a pinned version."""
    docs = [ROOT / "INSTALL.md", ROOT / "CONTRIBUTING.md"]
    docs += sorted((ROOT / "docs" / "distribution").glob("*.md"))
    return docs


GUIDE_PINNED = re.compile(r"(?:--version |@)v?(\d+\.\d+\.\d+)")


def guide_versions(path: Path) -> list[str]:
    return PINNED.findall(path.read_text(encoding="utf-8")) + GUIDE_PINNED.findall(
        path.read_text(encoding="utf-8")
    )


ILLUSTRATIVE = "illustrative"
FENCED = re.compile(r"```([^\n]*)\n(.*?)```", re.S)
INLINE = re.compile(r"`([^`\n]+)`")
JUST_CALL = re.compile(r"\bjust ([A-Za-z_][\w-]*)")
RECIPE_DEF = re.compile(r"^@?([A-Za-z_][\w-]*)(?:[ \t][^:\n]*)?:(?!=)", re.M)


def tracked_files() -> set[str]:
    out = subprocess.check_output(["git", "ls-files"], cwd=ROOT, text=True)
    return {line for line in out.splitlines() if line}


def justfile_recipes() -> set[str]:
    return set(RECIPE_DEF.findall((ROOT / "justfile").read_text(encoding="utf-8")))


def code_spans(text: str) -> list[str]:
    """Fenced blocks and inline code; a fence tagged `illustrative` is skipped."""
    spans = [body for info, body in FENCED.findall(text) if ILLUSTRATIVE not in info]
    return spans + INLINE.findall(FENCED.sub("", text))


def dev_references(text: str, tops: set[str]) -> tuple[set[str], set[str]]:
    """(repo paths, `just` recipes) cited in code spans, placeholders excluded."""
    paths, recipes = set(), set()
    for span in code_spans(text):
        recipes.update(JUST_CALL.findall(span))
        for token in re.findall(r"[^\s'\"=]+/[^\s'\"]*|[^\s'\"]+\.(?:sh|py)\b", span):
            token = token.rstrip(".,;:)")
            if "<" in token or "*" in token or "$" in token:
                continue
            if token.split("/", 1)[0] in tops or token.startswith("scripts"):
                paths.add(token)
    return paths, recipes


def missing_references(
    text: str, files: set[str], recipes: set[str]
) -> tuple[list[str], list[str]]:
    tops = {f.split("/", 1)[0] for f in files}
    paths, cited = dev_references(text, tops)
    gone = sorted(
        p for p in paths
        if p not in files and not any(f.startswith(p.rstrip("/") + "/") for f in files)
    )
    return gone, sorted(cited - recipes)


class ReadmeContract(unittest.TestCase):
    def test_both_readmes_have_the_same_heading_outline(self):
        en, es = outline(EN), outline(ES)
        self.assertTrue(en and es, "a README without headings proves nothing")
        self.assertEqual(
            en, es, f"heading outline differs: EN has {len(en)}, ES has {len(es)}"
        )

    def test_pinned_versions_are_the_workspace_version(self):
        expected = workspace_version()
        for path in (EN, ES):
            found = pinned_versions(path)
            self.assertTrue(found, f"{path.name} pins no version: nothing checked")
            stale = sorted({v for v in found if v != expected})
            self.assertEqual([], stale, f"{path.name} pins stale versions")

    def test_relative_links_exist(self):
        for path in (EN, ES):
            links = local_links(path)
            self.assertTrue(links, f"{path.name} has no local link: nothing checked")
            missing = sorted({l for l in links if not (ROOT / l).exists()})
            self.assertEqual([], missing, f"{path.name} links to missing files")

    def test_install_guides_pin_the_workspace_version(self):
        expected = workspace_version()
        docs = install_docs()
        self.assertGreaterEqual(len(docs), 3, "install guides not found: nothing checked")
        all_found = []
        for path in docs:
            found = guide_versions(path)
            all_found += found
            stale = sorted({v for v in found if v != expected})
            self.assertEqual([], stale, f"{path.relative_to(ROOT)} pins stale versions")
        self.assertTrue(all_found, "no install guide pins a version: nothing checked")

    def test_install_guide_links_exist_relative_to_the_document(self):
        total = 0
        for path in install_docs():
            links = local_links(path)
            total += len(links)
            missing = sorted(
                {l for l in links if not (path.parent / l).exists()}
            )
            self.assertEqual(
                [], missing, f"{path.relative_to(ROOT)} links to missing files"
            )
        self.assertTrue(total, "no install guide has a local link: nothing checked")

    def test_developer_guides_cite_scripts_paths_and_recipes_that_exist(self):
        files, recipes = tracked_files(), justfile_recipes()
        self.assertTrue(recipes, "justfile has no recipe: nothing to compare against")
        tops = {f.split("/", 1)[0] for f in files}
        cited_paths = cited_recipes = 0
        for path in install_docs():
            text = path.read_text(encoding="utf-8")
            paths, cited = dev_references(text, tops)
            cited_paths += len(paths)
            cited_recipes += len(cited)
            gone, unknown = missing_references(text, files, recipes)
            self.assertEqual([], gone, f"{path.name} cites paths that do not exist")
            self.assertEqual([], unknown, f"{path.name} cites unknown just recipes")
        self.assertTrue(cited_paths, "no guide cites a repo path: nothing checked")
        self.assertTrue(cited_recipes, "no guide cites a just recipe: nothing checked")

    def test_guides_do_not_require_pytest(self):
        # The repo declares no pytest and its harness is pytest-free
        # (test_ci_contracts), so a guide that needs it fails on a clean checkout.
        for path in install_docs():
            spans = code_spans(path.read_text(encoding="utf-8"))
            offending = [s for s in spans if re.search(r"\bpytest\b", s)]
            self.assertEqual([], offending, f"{path.name} requires pytest")

    def test_the_reference_check_discriminates_and_honours_illustrative(self):
        files, recipes = {"scripts/real.sh", "docs/a.md"}, {"check"}
        bad = "Run `bash scripts/ghost.sh` then `just nope` and `just check`."
        self.assertEqual(
            (["scripts/ghost.sh"], ["nope"]), missing_references(bad, files, recipes)
        )
        skipped = "```bash illustrative\nbash scripts/ghost.sh\njust nope\n```"
        self.assertEqual(([], []), missing_references(skipped, files, recipes))
        placeholder = "`bash scripts/<name>.sh` and `docs/*.md`"
        self.assertEqual(([], []), missing_references(placeholder, files, recipes))


if __name__ == "__main__":
    unittest.main()
