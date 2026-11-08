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

import json
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


SHELL_INFO = {"bash", "sh", "shell", "console"}
URL = re.compile(r"https?://[^\s)>\]\"'`]+")
FLAG = re.compile(r"(?<![\w-])--?[A-Za-z][\w-]*")
NUMBER = re.compile(r"(?<![\w.])\d+(?:\.\d+)*(?![\w.])")
WORD = re.compile(r"[A-Za-z_][\w-]*")


def readme_sections(path: Path) -> list[tuple[str, str]]:
    """(heading, body) for the intro and every `##`/`###` section, fences respected."""
    out, title, body, in_fence = [], "(intro)", [], False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith(FENCE):
            in_fence = not in_fence
        m = None if in_fence else re.match(r"#{2,3} (\S.*)", line)
        if m:
            out.append((title, "\n".join(body)))
            title, body = m.group(1), []
        else:
            body.append(line)
    out.append((title, "\n".join(body)))
    return out


def tool_names() -> set[str]:
    catalog = json.loads((ROOT / "product" / "tool-catalog-runtime.json").read_text())
    return {t["name"] for t in catalog}


def block_signature(info: str, body: str, tools: set[str]) -> tuple:
    """What a fenced block says regardless of the language of its prose.

    Comments and translated placeholders (`path` / `ruta`) are not compared;
    the number of live lines, the leading command of each shell line, flags,
    numbers, URLs and published tool names are.
    """
    lines = []
    for raw in body.splitlines():
        line = re.sub(r"\s+#.*$", "", raw).strip()
        if line and not line.startswith("#"):
            lines.append(line)
    text = "\n".join(lines)
    lead = (
        tuple(l.split()[0] for l in lines) if info.strip() in SHELL_INFO else ()
    )
    return (
        info.strip(),
        len(lines),
        lead,
        tuple(sorted(FLAG.findall(text))),
        tuple(sorted(NUMBER.findall(text))),
        tuple(sorted(URL.findall(text))),
        tuple(sorted(w for w in WORD.findall(text) if w in tools)),
    )


def section_facts(text: str, tools: set[str]) -> tuple[list[tuple], list[str]]:
    blocks = [block_signature(i, b, tools) for i, b in FENCED.findall(text)]
    urls = sorted({u.rstrip(".,;:") for u in URL.findall(text)})
    return blocks, urls


def parity_differences(en: Path, es: Path, tools: set[str]) -> list[str]:
    en_s, es_s = readme_sections(en), readme_sections(es)
    if len(en_s) != len(es_s):
        return [f"section count differs: EN {len(en_s)}, ES {len(es_s)}"]
    found = []
    for (t_en, b_en), (t_es, b_es) in zip(en_s, es_s):
        blocks_en, urls_en = section_facts(b_en, tools)
        blocks_es, urls_es = section_facts(b_es, tools)
        if blocks_en != blocks_es:
            found.append(f"'{t_en}' / '{t_es}': fenced blocks differ")
        if urls_en != urls_es:
            found.append(f"'{t_en}' / '{t_es}': URLs differ")
    return found


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

    def test_fenced_blocks_and_urls_match_section_by_section(self):
        tools = tool_names()
        self.assertTrue(tools, "empty tool catalogue: tool names cannot be compared")
        blocks = urls = named = 0
        for _, body in readme_sections(EN):
            sections_blocks, sections_urls = section_facts(body, tools)
            blocks += len(sections_blocks)
            urls += len(sections_urls)
            named += sum(len(b[-1]) for b in sections_blocks)
        self.assertTrue(blocks and urls, "README has no fenced block or URL: nothing checked")
        self.assertTrue(named, "no block names a published tool: catalogue check is vacuous")
        self.assertEqual([], parity_differences(EN, ES, tools))

    def test_parity_check_discriminates(self):
        import tempfile

        tools = {"build_graph"}
        en = "## A\n```bash\ncargo build --release # comment\nrun --version\n```\nsee https://x.io/a\n"
        ok = "## B\n```bash\ncargo build --release # comentario\nrun --version\n```\nver https://x.io/a\n"
        short = "## B\n```bash\ncargo build --release\n```\nver https://x.io/a\n"
        flag = ok.replace("--release", "--debug")
        url = ok.replace("x.io/a", "x.io/b")
        tool = "## B\n```json\n{ \"tool\": \"build_graph\" }\n```\n"
        tool_en = tool.replace("build_graph", "trace_path")
        with tempfile.TemporaryDirectory() as d:
            def write(name, text):
                path = Path(d) / name
                path.write_text(text, encoding="utf-8")
                return path
            base = write("en.md", en)
            self.assertEqual([], parity_differences(base, write("ok.md", ok), tools))
            for name, text in (("short", short), ("flag", flag), ("url", url)):
                self.assertTrue(
                    parity_differences(base, write(name + ".md", text), tools), name
                )
            self.assertTrue(
                parity_differences(
                    write("t1.md", tool), write("t2.md", tool_en), {"build_graph", "trace_path"}
                )
            )


if __name__ == "__main__":
    unittest.main()
