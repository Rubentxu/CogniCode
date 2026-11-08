#!/usr/bin/env python3
"""SDK architecture document contract.

`docs/distribution/SDK_ARCHITECTURE.md` describes the four library crates that
`SDK_SURFACES.json` classifies as `library`. This contract keeps that document
honest in three ways:

* every statement carries a provenance label (`observed`, `inferred` or
  `curated`), so a reader can tell what a tool measured from what a person
  concluded;
* what the document calls observed is compared with what the installed
  `cognicode inspect` reports now, in both directions;
* the dependency diagram is compared with the edges `cargo metadata` reports,
  and the commands that regenerate the evidence are the ones the document
  documents.

The crate list is read from `SDK_SURFACES.json`, not repeated here. The checks
that decide are also exercised on synthetic text so a vacuous pass is visible.
"""

import json
import re
import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DECLARATION = ROOT / "docs" / "distribution" / "SDK_SURFACES.json"
DOCUMENT = ROOT / "docs" / "distribution" / "SDK_ARCHITECTURE.md"
LABELS = ("observed", "inferred", "curated")
COMMAND_DEADLINE_SECONDS = 180
OUT_OF_SCOPE = "cognicode-explorer"

CLAIM = re.compile(r"^- \[(?P<label>[a-z]+)\] (?P<text>.+)$")
FUNCTION_CLAIM = re.compile(r"`(?P<name>[A-Za-z_][A-Za-z0-9_]*)` \(function\)")


def library_crates():
    declared = json.loads(DECLARATION.read_text(encoding="utf-8"))["crates"]
    return sorted(name for name, info in declared.items() if info["class"] == "library")


def read_document():
    return DOCUMENT.read_text(encoding="utf-8")


def sections(text):
    """Map a `## ` heading to its body."""
    found = {}
    current = None
    for line in text.splitlines():
        if line.startswith("## "):
            current = line[3:].strip()
            found[current] = []
        elif current is not None:
            found[current].append(line)
    return {title: "\n".join(body) for title, body in found.items()}


def unlabelled_statements(body):
    """Bullet lines of a section that do not start with a known label."""
    bad = []
    for line in body.splitlines():
        if not line.startswith("- "):
            continue
        match = CLAIM.match(line)
        if match is None or match.group("label") not in LABELS:
            bad.append(line)
    return bad


def labels_in(body):
    return {m.group("label") for m in map(CLAIM.match, body.splitlines()) if m}


def documented_functions(body):
    names = set()
    for line in body.splitlines():
        match = CLAIM.match(line)
        if match and match.group("label") == "observed":
            names.update(f.group("name") for f in FUNCTION_CLAIM.finditer(match.group("text")))
    return names


def mermaid_edges(text):
    block = re.search(r"```mermaid\n(.*?)```", text, re.S)
    if block is None:
        return None
    return {
        (m.group(1), m.group(2))
        for m in re.finditer(r"(\w+)\s*-->\s*(\w+)", block.group(1))
    }


def metadata_edges(libraries):
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--offline", "--format-version", "1"],
        cwd=ROOT, capture_output=True, text=True, check=True,
        timeout=COMMAND_DEADLINE_SECONDS,
    ).stdout
    packages = {p["name"]: p for p in json.loads(out)["packages"]}
    edges = set()
    for name in libraries:
        for dep in packages[name]["dependencies"]:
            if dep["kind"] is None and dep["name"] in libraries:
                edges.add((name, dep["name"]))
    return edges


def inspected_functions(crate):
    out = subprocess.run(
        ["cognicode", "inspect", "--file", f"crates/{crate}/src/lib.rs"],
        cwd=ROOT, capture_output=True, text=True, check=True,
        timeout=COMMAND_DEADLINE_SECONDS,
    ).stdout
    return {m.group(1) for m in re.finditer(r"^\s+\S+:\d+ (\S+) \(function\)$", out, re.M)}


class SdkArchitectureDocument(unittest.TestCase):
    def test_the_document_exists_and_the_library_list_is_not_empty(self):
        self.assertTrue(DOCUMENT.is_file(), f"{DOCUMENT} is missing")
        self.assertTrue(library_crates(), "no library crate declared: nothing to describe")

    def test_scope_names_the_exclusion_of_the_explorer(self):
        body = sections(read_document()).get("Scope", "")
        self.assertIn(OUT_OF_SCOPE, body)
        self.assertIn("TASK-177", body)

    def test_every_library_crate_has_a_section_with_an_observed_statement(self):
        found = sections(read_document())
        for crate in library_crates():
            with self.subTest(crate=crate):
                self.assertIn(crate, found, f"no section for {crate}")
                self.assertIn("observed", labels_in(found[crate]))

    def test_every_statement_carries_a_provenance_label(self):
        found = sections(read_document())
        checked = 0
        for title, body in found.items():
            if title in library_crates() or title in ("Dependency graph", "Scope"):
                checked += len([ln for ln in body.splitlines() if ln.startswith("- ")])
                with self.subTest(section=title):
                    self.assertEqual(unlabelled_statements(body), [])
        self.assertGreater(checked, 0, "no statements found: the check would pass on nothing")

    def test_the_diagram_has_the_edges_cargo_metadata_reports(self):
        libraries = library_crates()
        drawn = mermaid_edges(read_document())
        self.assertIsNotNone(drawn, "no mermaid block")
        expected = metadata_edges(libraries)
        self.assertTrue(expected, "cargo metadata reported no edge between the libraries")
        short = lambda n: n.replace("-", "_")  # noqa: E731 (mermaid ids)
        self.assertEqual(drawn, {(short(a), short(b)) for a, b in expected})

    def test_the_diagram_is_labelled_curated(self):
        body = sections(read_document()).get("Dependency graph", "")
        self.assertIn("curated", labels_in(body))

    def test_regeneration_commands_cover_every_library_and_name_real_files(self):
        body = sections(read_document()).get("Regenerate", "")
        self.assertTrue(body, "no Regenerate section")
        for crate in library_crates():
            with self.subTest(crate=crate):
                command = f"cognicode inspect --file crates/{crate}/src/lib.rs"
                self.assertIn(command, body)
                self.assertTrue((ROOT / "crates" / crate / "src" / "lib.rs").is_file())
        self.assertIn("cargo metadata --no-deps --offline", body)

    def test_observed_functions_equal_what_cognicode_reports_now(self):
        if shutil.which("cognicode") is None:
            self.skipTest("cognicode is not on PATH: observed statements NOT verified")
        found = sections(read_document())
        compared = 0
        for crate in library_crates():
            with self.subTest(crate=crate):
                documented = documented_functions(found[crate])
                actual = inspected_functions(crate)
                self.assertEqual(documented, actual)
                compared += len(actual)
        self.assertGreater(compared, 0, "no function compared: the check would be vacuous")


def consuming_section(text):
    """Body of the `## Consuming the SDK` section, or None."""
    return sections(text).get("Consuming the SDK")


def cargo_add_lines(body, version):
    """Crates named by `cargo add <crate>@=<version>` lines, in document order."""
    pattern = r"^cargo add (\S+)@=" + re.escape(version) + r"\s*$"
    return re.findall(pattern, body, re.M)


def release_order():
    """Publication order, read from the numbered list of SDK_RELEASE.md."""
    text = (ROOT / "docs" / "distribution" / "SDK_RELEASE.md").read_text(encoding="utf-8")
    start = text.index("Publication order")
    return re.findall(r"^\d+\. `([^`]+)`$", text[start:].split("Procedure", 1)[0], re.M)


def workspace_version():
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    return re.search(r"\[workspace\.package\][^\[]*?\nversion = \"([^\"]+)\"", text).group(1)


class SdkConsumptionGuide(unittest.TestCase):
    GUIDE = ROOT / "docs" / "distribution" / "INSTALL.md"

    def body(self):
        body = consuming_section(self.GUIDE.read_text(encoding="utf-8"))
        self.assertIsNotNone(body, "INSTALL.md has no 'Consuming the SDK' section")
        return body

    def test_every_library_crate_is_added_at_the_workspace_version_in_release_order(self):
        libraries = library_crates()
        order = release_order()
        self.assertTrue(libraries and order, "no crate or order read: the check would be vacuous")
        self.assertEqual(sorted(order), libraries, "SDK_RELEASE.md order and SDK_SURFACES.json disagree")
        self.assertEqual(cargo_add_lines(self.body(), workspace_version()), order)

    def test_registry_dependent_instructions_are_marked_as_a_limitation(self):
        body = self.body()
        self.assertIn("crates.io", body)
        self.assertIn("TASK-178", body)
        self.assertRegex(body, r"(?i)not (yet )?published")

    def test_the_section_stays_inside_the_sdk_scope(self):
        self.assertNotIn("Explorer handoff", self.body())


class ConsumptionCheckDiscriminates(unittest.TestCase):
    def test_a_missing_section_or_wrong_version_or_order_is_detected(self):
        self.assertIsNone(consuming_section("## Other\n- x"))
        doc = "## Consuming the SDK\ncargo add a@=1.0.0\ncargo add b@=1.0.0\n## Next\n"
        body = consuming_section(doc)
        self.assertEqual(cargo_add_lines(body, "1.0.0"), ["a", "b"])
        self.assertEqual(cargo_add_lines(body, "2.0.0"), [])
        self.assertNotEqual(cargo_add_lines(body, "1.0.0"), ["b", "a"])


class DeveloperGuideLinks(unittest.TestCase):
    """CONTRIBUTING.md must lead a developer to the SDK architecture sources."""

    GUIDE = ROOT / "CONTRIBUTING.md"
    TARGETS = ("docs/distribution/SDK_ARCHITECTURE.md", "docs/distribution/SDK_SURFACES.json")

    def test_contributing_links_to_the_architecture_document_and_the_surface_declaration(self):
        text = self.GUIDE.read_text(encoding="utf-8")
        self.assertTrue(text.strip(), "CONTRIBUTING.md is empty: the check would pass on nothing")
        links = {m.group(1) for m in re.finditer(r"\]\(([^)#\s]+)", text)}
        for target in self.TARGETS:
            with self.subTest(target=target):
                self.assertIn(target, links, f"CONTRIBUTING.md does not link to {target}")
                self.assertTrue((ROOT / target).is_file(), f"{target} does not exist")


class ContractDiscriminates(unittest.TestCase):
    """The checks above must be able to fail."""

    def test_an_unlabelled_bullet_is_found(self):
        self.assertEqual(unlabelled_statements("- [observed] ok\n- plain\n- [guess] x"),
                         ["- plain", "- [guess] x"])

    def test_functions_are_read_only_from_observed_bullets(self):
        body = "- [observed] `a` (function)\n- [inferred] `b` (function)"
        self.assertEqual(documented_functions(body), {"a"})

    def test_a_missing_or_extra_edge_changes_the_edge_set(self):
        text = "```mermaid\ngraph LR\n  a_b --> c_d\n```\n"
        self.assertEqual(mermaid_edges(text), {("a_b", "c_d")})
        self.assertIsNone(mermaid_edges("no diagram"))


if __name__ == "__main__":
    unittest.main()
