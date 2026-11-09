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


def application_crates():
    declared = json.loads(DECLARATION.read_text(encoding="utf-8"))["crates"]
    return sorted(
        name for name, info in declared.items()
        if info["class"] == "application" and name != OUT_OF_SCOPE
    )


APPLICATION_EDGE = re.compile(
    r"^- \[observed\] `(?P<app>[^`]+)` depends on `(?P<lib>[^`]+)`", re.M
)


def documented_application_edges(body):
    return {(m.group("app"), m.group("lib")) for m in APPLICATION_EDGE.finditer(body)}


def application_edges(applications, libraries):
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--offline", "--format-version", "1"],
        cwd=ROOT, capture_output=True, text=True, check=True,
        timeout=COMMAND_DEADLINE_SECONDS,
    ).stdout
    packages = {p["name"]: p for p in json.loads(out)["packages"]}
    return {
        (name, dep["name"])
        for name in applications
        for dep in packages[name]["dependencies"]
        if dep["kind"] is None and dep["name"] in libraries
    }


ROOT_MODULES = re.compile(
    r"^- \[observed\] `(?P<app>[^`]+)` declares the root modules (?P<mods>.+)$", re.M
)
NO_ROOT_LIBRARY = re.compile(r"^- \[observed\] `(?P<app>[^`]+)` has no `src/lib\.rs`", re.M)
PUB_MOD = re.compile(r"^pub mod (\w+);", re.M)


def documented_root_modules(body):
    return {
        m.group("app"): set(re.findall(r"`(\w+)`", m.group("mods")))
        for m in ROOT_MODULES.finditer(body)
    }


def documented_without_root_library(body):
    return {m.group("app") for m in NO_ROOT_LIBRARY.finditer(body)}


def source_root_modules(crate):
    lib = ROOT / "crates" / crate / "src" / "lib.rs"
    if not lib.is_file():
        return None
    return set(PUB_MOD.findall(lib.read_text(encoding="utf-8")))


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
            if title in library_crates() or title in ("Dependency graph", "Scope", "Applications"):
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

    def test_the_generator_prints_the_diagram_lines_the_document_holds(self):
        generator = ROOT / "scripts" / "ci" / "sdk_dependency_edges.py"
        self.assertTrue(generator.is_file(), "no sdk_dependency_edges.py generator")
        out = subprocess.run(
            ["python3", "-I", str(generator)], cwd=ROOT, capture_output=True,
            text=True, check=True, timeout=COMMAND_DEADLINE_SECONDS,
        ).stdout
        printed = out.splitlines()
        self.assertTrue(printed, "the generator printed no edge")
        block = re.search(r"```mermaid\n(.*?)```", read_document(), re.S)
        self.assertIsNotNone(block, "no mermaid block")
        drawn = [l for l in block.group(1).splitlines() if "-->" in l]
        self.assertEqual(printed, drawn)

    def test_the_regenerate_section_runs_the_generator(self):
        body = sections(read_document()).get("Regenerate", "")
        self.assertIn("python3 -I scripts/ci/sdk_dependency_edges.py", body)

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

    def test_application_dependencies_on_the_libraries_equal_cargo_metadata(self):
        applications, libraries = application_crates(), library_crates()
        self.assertTrue(applications, "no application crate declared: nothing to describe")
        body = sections(read_document()).get("Applications")
        self.assertIsNotNone(body, "no Applications section")
        expected = application_edges(applications, libraries)
        self.assertTrue(expected, "cargo metadata reported no application->library edge")
        self.assertEqual(documented_application_edges(body), expected)

    def test_application_root_modules_equal_the_crate_roots(self):
        applications = application_crates()
        body = sections(read_document()).get("Applications")
        self.assertIsNotNone(body, "no Applications section")
        modules = documented_root_modules(body)
        bare = documented_without_root_library(body)
        compared = 0
        for crate in applications:
            with self.subTest(crate=crate):
                actual = source_root_modules(crate)
                if actual is None:
                    self.assertIn(crate, bare, "no src/lib.rs and not documented as such")
                    self.assertNotIn(crate, modules)
                else:
                    self.assertEqual(modules.get(crate), actual)
                    self.assertNotIn(crate, bare)
                    compared += len(actual)
        self.assertGreater(compared, 0, "no root module compared: the check would be vacuous")

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

    def test_the_section_states_publication_and_names_the_consumer_check(self):
        """The SDK is on crates.io at the workspace version; the section says so and how to re-verify it."""
        body = self.body()
        self.assertIn("crates.io", body)
        self.assertIn(f"`{workspace_version()}`", body)
        self.assertIn("scripts/distribution/verify-sdk-consumer.sh --confirm-published", body)
        self.assertNotRegex(body, r"(?i)not (yet )?published")

    def test_the_section_stays_inside_the_sdk_scope(self):
        self.assertNotIn("Explorer handoff", self.body())


class InternalVersionsHaveOneOwner(unittest.TestCase):
    """A crate manifest must not pin a sibling's version: release-sdk.sh bumps
    only the root Cargo.toml, so a fixed `version = "x.y.z"` on an internal
    path dependency breaks the next minor bump (cargo update cannot resolve)."""

    INTERNAL = ("cognicode-core", "cognicode-graph-algos", "cognicode-macros")

    @staticmethod
    def pinned_internal_versions(text, names):
        found = []
        for line in text.splitlines():
            m = re.match(r"\s*([A-Za-z0-9_-]+)\s*=\s*\{(.*)\}", line)
            if m and m.group(1) in names and re.search(r'\bversion\s*=\s*"', m.group(2)):
                found.append(line.strip())
        return found

    def test_no_crate_manifest_pins_an_internal_version(self):
        offenders = []
        for manifest in sorted((ROOT / "crates").glob("*/Cargo.toml")):
            text = manifest.read_text(encoding="utf-8")
            for line in self.pinned_internal_versions(text, self.INTERNAL):
                offenders.append(f"{manifest.relative_to(ROOT)}: {line}")
        self.assertEqual(offenders, [], "use `workspace = true` for internal crates")

    def test_the_check_detects_a_pinned_version(self):
        bad = 'cognicode-graph-algos = { path = "../x", version = "0.1.0" }'
        good = 'cognicode-graph-algos = { workspace = true, features = [] }'
        self.assertEqual(len(self.pinned_internal_versions(bad, self.INTERNAL)), 1)
        self.assertEqual(self.pinned_internal_versions(good, self.INTERNAL), [])


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


def ledger_claim_counts(body):
    """Claims per crate, read only from observed bullets shaped "`crate`: ... N entity claims"."""
    counts = {}
    for line in body.splitlines():
        match = CLAIM.match(line)
        if match and match.group("label") == "observed":
            found = re.match(r"`(?P<crate>[\w-]+)`: .*?(?P<n>\d+) entity claims", match.group("text"))
            if found:
                counts[found.group("crate")] = int(found.group("n"))
    return counts


def duplicated_declarations(body):
    """Names the observed bullets say are declared twice."""
    names = set()
    for line in body.splitlines():
        match = CLAIM.match(line)
        if match and match.group("label") == "observed":
            names.update(re.findall(r"`(\w+)` is declared twice", match.group("text")))
    return names


class EvidenceLedgerSection(unittest.TestCase):
    """The cognicode-diagrams ledger is external; the document must stay honest about it."""

    def test_the_section_states_one_claim_per_documented_function_declaration(self):
        document = sections(read_document())
        body = document.get("Evidence ledger", "")
        self.assertTrue(body, "no Evidence ledger section")
        counts = ledger_claim_counts(body)
        for crate in ("cognicode-graph-wasm", "cognicode-macros"):
            with self.subTest(crate=crate):
                crate_body = document[crate]
                documented = documented_functions(crate_body)
                self.assertTrue(documented, "no documented functions to compare")
                twice = duplicated_declarations(crate_body) & documented
                self.assertEqual(counts.get(crate), len(documented) + len(twice))

    def test_the_section_labels_the_external_ledger_and_the_partial_coverage_as_curated(self):
        body = sections(read_document()).get("Evidence ledger", "")
        self.assertEqual(unlabelled_statements(body), [])
        self.assertIn("curated", labels_in(body))
        self.assertIn("not versioned", body)
        self.assertIn("partial", body)

    def test_the_regenerate_section_names_the_recording_commands(self):
        body = sections(read_document()).get("Regenerate", "")
        for command in ("scripts/record.py query", "scripts/record.py claim",
                        "scripts/evidence.py --repo <repo-root> --check", "scripts/atlas.py sync"):
            with self.subTest(command=command):
                self.assertIn(command, body)

    def test_the_claim_counts_are_read_only_from_observed_bullets_of_one_crate(self):
        self.assertEqual(ledger_claim_counts("- [observed] `a-b`: 5 entity claims"), {"a-b": 5})
        self.assertEqual(ledger_claim_counts("- [curated] `a`: 5 entity claims"), {})
        self.assertEqual(ledger_claim_counts("- [observed] `a`: no count here"), {})
        self.assertEqual(ledger_claim_counts("- [observed] `a`: 2 queries and 6 entity claims"), {"a": 6})

    def test_a_declared_twice_name_is_read_only_from_observed_bullets(self):
        self.assertEqual(duplicated_declarations("- [observed] `parse` is declared twice"), {"parse"})
        self.assertEqual(duplicated_declarations("- [curated] `parse` is declared twice"), set())


class ContractDiscriminates(unittest.TestCase):
    """The checks above must be able to fail."""

    def test_an_unlabelled_bullet_is_found(self):
        self.assertEqual(unlabelled_statements("- [observed] ok\n- plain\n- [guess] x"),
                         ["- plain", "- [guess] x"])

    def test_functions_are_read_only_from_observed_bullets(self):
        body = "- [observed] `a` (function)\n- [inferred] `b` (function)"
        self.assertEqual(documented_functions(body), {"a"})

    def test_application_edges_are_read_only_from_observed_bullets(self):
        body = ("- [observed] `a` depends on `b`\n- [curated] `c` depends on `d`\n"
                "- [observed] `e` mentions `f`")
        self.assertEqual(documented_application_edges(body), {("a", "b")})
        self.assertEqual(documented_application_edges(""), set())

    def test_root_module_claims_are_read_only_from_observed_bullets(self):
        body = ("- [observed] `a` declares the root modules `x`, `y`\n"
                "- [curated] `b` declares the root modules `z`\n"
                "- [observed] `c` has no `src/lib.rs`")
        self.assertEqual(documented_root_modules(body), {"a": {"x", "y"}})
        self.assertEqual(documented_without_root_library(body), {"c"})
        self.assertEqual(PUB_MOD.findall("pub mod a;\n//! pub mod b;\nmod c;\npub mod d;"), ["a", "d"])

    def test_a_missing_or_extra_edge_changes_the_edge_set(self):
        text = "```mermaid\ngraph LR\n  a_b --> c_d\n```\n"
        self.assertEqual(mermaid_edges(text), {("a_b", "c_d")})
        self.assertIsNone(mermaid_edges("no diagram"))


def test_sdk_architecture_unittest_cases_pass() -> None:
    """`scripts/run_contract_tests.py` reads module-level `test_*` functions only,
    so the `unittest` classes above would run zero cases there and look green."""
    import io

    suite = unittest.TestSuite()
    for case in (SdkArchitectureDocument, SdkConsumptionGuide, ConsumptionCheckDiscriminates, DeveloperGuideLinks, EvidenceLedgerSection, ContractDiscriminates,):
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(case))
    result = unittest.TextTestRunner(stream=io.StringIO(), verbosity=0).run(suite)
    assert result.testsRun > 0, "the test_sdk_architecture suite ran no case"
    assert result.wasSuccessful(), "; ".join(
        f"{case.id()}: {trace.strip().splitlines()[-1]}"
        for case, trace in result.failures + result.errors
    )


if __name__ == "__main__":
    unittest.main()
