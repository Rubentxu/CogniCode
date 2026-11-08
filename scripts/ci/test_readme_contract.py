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


if __name__ == "__main__":
    unittest.main()
