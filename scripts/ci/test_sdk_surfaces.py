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
import re
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RELEASE_ORDER = [
    "cognicode-graph-algos",
    "cognicode-macros",
    "cognicode-core",
    "cognicode-graph-wasm",
]
DECLARATION = ROOT / "docs" / "distribution" / "SDK_SURFACES.json"
CLASSES = {"library", "application", "internal"}
METADATA_DEADLINE_SECONDS = 120
PACKAGE_LIST_DEADLINE_SECONDS = 120
TEST_PAYLOAD_PREFIXES = ("tests/", "testdata/")
LICENSE_FILES = ("LICENSE-MIT", "LICENSE-APACHE")
MAX_KEYWORDS = 5
# crates.io category slugs this workspace may use. A closed list because the
# registry rejects unknown slugs at publish time and offline we cannot ask it.
SEMVER_POLICY = ROOT / "docs" / "distribution" / "SEMVER_POLICY.md"
SEMVER_POLICY_URL = (
    "https://github.com/Rubentxu/CogniCode/blob/main/docs/distribution/SEMVER_POLICY.md"
)
SDK_RELEASE = ROOT / "docs" / "distribution" / "SDK_RELEASE.md"
RELEASE_SECTIONS = (
    "Dependency graph",
    "Supported feature combinations",
    "Toolchain",
    "Examples",
    "Release and rollback",
)
COMBINATION_STATUSES = {"checked", "not measured", "unsupported"}
POLICY_SECTIONS = (
    "Scope",
    "Breaking changes",
    "Features",
    "Toolchain",
    "Deprecation",
    "Release cadence",
)
KNOWN_CATEGORIES = {
    "algorithms",
    "data-structures",
    "development-tools",
    "development-tools::procedural-macro-helpers",
    "parser-implementations",
    "wasm",
}


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


def packaged_files(name):
    """Files `cargo package` would ship for `name`, as crate-relative paths."""
    out = subprocess.run(
        ["cargo", "package", "--list", "-p", name, "--offline", "--allow-dirty"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=PACKAGE_LIST_DEADLINE_SECONDS,
        check=True,
    )
    return [line.strip() for line in out.stdout.splitlines() if line.strip()]


def library_packages():
    packages = workspace_packages()
    return {
        name: packages[name]
        for name, entry in declared_crates().items()
        if entry.get("class") == "library" and name in packages
    }


def declared_msrv():
    """The toolchain floor the product publishes in manifest.toml (`rust>=X`)."""
    for line in (ROOT / "manifest.toml").read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line.startswith("compatibility") and "rust>=" in line:
            return line.split("rust>=", 1)[1].strip().strip('"')
    raise AssertionError("manifest.toml declares no `rust>=` compatibility")


def section(text, title):
    """Body of the first `## <title...>` section of a markdown document, else None."""
    match = re.search(
        rf"^#{{2,3}}\s+{re.escape(title)}\b.*?$(.*?)(?=^#{{1,3}}\s|\Z)",
        text,
        re.MULTILINE | re.DOTALL,
    )
    return match.group(1) if match else None


def library_graph():
    """Normal (non-dev, non-build) dependencies between the library crates."""
    libs = library_packages()
    return {
        name: sorted(
            d["name"] for d in pkg["dependencies"] if d["kind"] is None and d["name"] in libs
        )
        for name, pkg in libs.items()
    }


def release_text():
    return SDK_RELEASE.read_text(encoding="utf-8")


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

    def test_every_library_declares_its_registry_metadata(self):
        packages = workspace_packages()
        for name, entry in declared_crates().items():
            if entry.get("class") != "library" or name not in packages:
                continue
            for field in ("description", "license", "repository", "readme"):
                with self.subTest(crate=name, field=field):
                    self.assertTrue(
                        (packages[name].get(field) or "").strip(),
                        f"library crate does not declare `{field}`",
                    )

    def test_library_readme_files_exist_inside_the_crate(self):
        packages = workspace_packages()
        for name, entry in declared_crates().items():
            if entry.get("class") != "library" or name not in packages:
                continue
            readme = packages[name].get("readme")
            if not readme:
                continue
            crate_dir = Path(packages[name]["manifest_path"]).parent
            with self.subTest(crate=name):
                self.assertTrue((crate_dir / readme).is_file(), f"{readme} is not a file in the crate")

    def test_library_dependencies_are_not_path_only(self):
        packages = workspace_packages()
        for name, entry in declared_crates().items():
            if entry.get("class") != "library" or name not in packages:
                continue
            for dep in packages[name]["dependencies"]:
                if dep.get("kind") == "dev":
                    continue
                if dep.get("path") is None and dep.get("source") is not None:
                    continue
                with self.subTest(crate=name, dependency=dep["name"]):
                    self.assertNotEqual(
                        dep["req"], "*", "path dependency without a `version` cannot be packaged"
                    )

    def test_a_library_does_not_package_tests_or_fixtures_unless_declared(self):
        for name, entry in declared_crates().items():
            if entry.get("class") != "library":
                continue
            allowed = tuple(entry.get("packaged_test_paths", []))
            with self.subTest(crate=name):
                files = packaged_files(name)
                self.assertTrue(files, "cargo package --list returned nothing")
                self.assertTrue(
                    any(f.startswith("src/") for f in files), "package list has no src/"
                )
                leaked = [
                    f
                    for f in files
                    if f.startswith(TEST_PAYLOAD_PREFIXES) and not f.startswith(allowed)
                ]
                self.assertEqual(
                    leaked[:5], [], f"{len(leaked)} test/fixture files are packaged"
                )

    def test_declared_test_payload_exceptions_are_narrow_and_justified(self):
        for name, entry in declared_crates().items():
            allowed = entry.get("packaged_test_paths")
            if allowed is None:
                continue
            with self.subTest(crate=name):
                self.assertEqual(entry.get("class"), "library")
                self.assertTrue(allowed, "an empty exception list must be omitted")
                self.assertTrue(
                    all(p in TEST_PAYLOAD_PREFIXES or p.startswith(TEST_PAYLOAD_PREFIXES) for p in allowed),
                    "exceptions may only name tests/ or testdata/ paths",
                )
                self.assertTrue(
                    (entry.get("packaged_test_paths_reason") or "").strip(),
                    "an exception needs a reason",
                )

    def test_every_library_declares_discovery_metadata(self):
        for name, pkg in library_packages().items():
            for field in ("homepage", "documentation"):
                with self.subTest(crate=name, field=field):
                    self.assertTrue((pkg.get(field) or "").strip(), f"no `{field}`")
            with self.subTest(crate=name, field="keywords"):
                keywords = pkg.get("keywords") or []
                self.assertTrue(keywords, "no keywords")
                self.assertLessEqual(len(keywords), MAX_KEYWORDS)
                for kw in keywords:
                    self.assertRegex(kw, r"^[a-z0-9][a-z0-9_-]{0,19}$")
            with self.subTest(crate=name, field="categories"):
                categories = pkg.get("categories") or []
                self.assertTrue(categories, "no categories")
                self.assertLessEqual(len(categories), 5)
                self.assertEqual(set(categories) - KNOWN_CATEGORIES, set())

    def test_every_library_declares_the_published_toolchain_floor(self):
        floor = declared_msrv()
        for name, pkg in library_packages().items():
            with self.subTest(crate=name):
                declared = pkg.get("rust_version")
                self.assertTrue(declared, "no `rust-version`")
                self.assertEqual(
                    tuple(int(x) for x in declared.split(".")),
                    tuple(int(x) for x in floor.split(".")),
                    "rust-version must equal the floor published in manifest.toml",
                )

    def test_every_library_package_carries_readme_and_both_licenses(self):
        for name, pkg in library_packages().items():
            with self.subTest(crate=name):
                files = set(packaged_files(name))
                for needed in (pkg["readme"], *LICENSE_FILES):
                    self.assertIn(needed, files, f"{needed} is not in the package")

    def test_license_files_in_crates_are_the_root_license_text(self):
        for name, pkg in library_packages().items():
            crate_dir = Path(pkg["manifest_path"]).parent
            for lic in LICENSE_FILES:
                with self.subTest(crate=name, file=lic):
                    path = crate_dir / lic
                    self.assertTrue(path.is_file(), f"{lic} missing in the crate")
                    self.assertEqual(
                        path.read_bytes(), (ROOT / lic).read_bytes(), "license text differs from the root"
                    )

    def test_library_readmes_document_every_feature(self):
        for name, pkg in library_packages().items():
            crate_dir = Path(pkg["manifest_path"]).parent
            readme = (crate_dir / pkg["readme"]).read_text(encoding="utf-8")
            features = sorted(set(pkg["features"]) - {"default"})
            with self.subTest(crate=name):
                body = section(readme, "Features")
                self.assertIsNotNone(body, "README has no `## Features` section")
                if not features:
                    self.assertIn(
                        "no optional features",
                        body.lower(),
                        "a crate without features must say so",
                    )
                    continue
                for feature in features:
                    self.assertIn(f"`{feature}`", body, f"feature `{feature}` is undocumented")
                listed = set(re.findall(r"^\|\s*`([^`]+)`", body, re.MULTILINE))
                self.assertEqual(
                    listed - set(pkg["features"]), set(), "README lists features the crate does not declare"
                )

    def test_default_features_are_stated_in_the_readme(self):
        for name, pkg in library_packages().items():
            crate_dir = Path(pkg["manifest_path"]).parent
            readme = (crate_dir / pkg["readme"]).read_text(encoding="utf-8")
            body = section(readme, "Features") or ""
            default = pkg["features"].get("default", [])
            with self.subTest(crate=name):
                if default:
                    for feature in default:
                        self.assertIn(f"`{feature}`", body)
                    self.assertIn("default", body.lower())

    def test_one_semver_policy_exists_and_covers_every_library(self):
        self.assertTrue(SEMVER_POLICY.is_file(), f"missing {SEMVER_POLICY.relative_to(ROOT)}")
        stray = [
            str(p.relative_to(ROOT))
            for p in (ROOT / "docs").rglob("*")
            if p.is_file() and "semver" in p.name.lower() and p != SEMVER_POLICY
        ]
        self.assertEqual(stray, [], "a second semver policy document exists")
        text = SEMVER_POLICY.read_text(encoding="utf-8")
        for title in POLICY_SECTIONS:
            with self.subTest(section=title):
                body = section(text, title)
                self.assertIsNotNone(body, f"policy has no `## {title}` section")
                self.assertTrue(body.strip(), f"policy section `{title}` is empty")
        for name in library_packages():
            with self.subTest(crate=name):
                self.assertIn(f"`{name}`", text)

    def test_semver_policy_does_not_restate_the_toolchain_floor(self):
        text = SEMVER_POLICY.read_text(encoding="utf-8")
        self.assertNotIn(declared_msrv(), text, "the floor lives in manifest.toml only")

    def test_library_readmes_link_the_policy_without_copying_it(self):
        policy_lines = {
            line.strip()
            for line in SEMVER_POLICY.read_text(encoding="utf-8").splitlines()
            if len(line.strip()) > 40
        }
        for name, pkg in library_packages().items():
            crate_dir = Path(pkg["manifest_path"]).parent
            readme = (crate_dir / pkg["readme"]).read_text(encoding="utf-8")
            with self.subTest(crate=name):
                self.assertIn(SEMVER_POLICY_URL, readme, "README does not link the semver policy")
                copied = [l.strip() for l in readme.splitlines() if l.strip() in policy_lines]
                self.assertEqual(copied, [], "README copies policy text instead of linking it")

    def test_one_sdk_release_document_has_every_section(self):
        self.assertTrue(SDK_RELEASE.is_file(), f"missing {SDK_RELEASE.relative_to(ROOT)}")
        stray = [
            str(p.relative_to(ROOT))
            for p in (ROOT / "docs").rglob("*")
            if p.is_file() and "sdk_release" in p.name.lower() and p != SDK_RELEASE
        ]
        self.assertEqual(stray, [], "a second SDK release document exists")
        for title in RELEASE_SECTIONS:
            with self.subTest(section=title):
                body = section(release_text(), title)
                self.assertIsNotNone(body, f"no `## {title}` section")
                self.assertTrue(body.strip(), f"section `{title}` is empty")

    def test_documented_dependency_graph_equals_cargo_metadata(self):
        body = section(release_text(), "Dependency graph") or ""
        documented = {}
        for crate, deps in re.findall(r"^- `([^`]+)` ->(.*)$", body, re.MULTILINE):
            documented[crate] = sorted(re.findall(r"`([^`]+)`", deps))
        self.assertTrue(library_graph(), "metadata returned no library graph")
        self.assertEqual(documented, library_graph())

    def test_documented_publication_order_respects_the_graph(self):
        body = section(release_text(), "Release and rollback") or ""
        order = re.findall(r"^\d+\. `(cognicode[^`]*)`", body, re.MULTILINE)
        graph = library_graph()
        self.assertEqual(sorted(order), sorted(graph), "order must list every library once")
        for position, crate in enumerate(order):
            for dep in graph[crate]:
                with self.subTest(crate=crate, dependency=dep):
                    self.assertLess(order.index(dep), position, "a dependency is published after its dependent")

    def test_internal_dependency_versions_equal_the_workspace_version(self):
        root = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
        version = re.search(r'^version = "([^"]+)"', root, re.MULTILINE).group(1)
        pinned = re.findall(
            r'^(cognicode-[\w-]+) = \{ path = "[^"]+", version = "([^"]+)"', root, re.MULTILINE
        )
        self.assertTrue(pinned, "no internal dependency pins a version")
        for name, pin in pinned:
            with self.subTest(dependency=name):
                self.assertEqual(pin, version, "internal pin differs from [workspace.package] version")

    def test_release_section_states_the_rollback_mechanism(self):
        body = (section(release_text(), "Release and rollback") or "").lower()
        for word in ("cargo package", "yank", "patch release", "semver_policy.md"):
            with self.subTest(word=word):
                self.assertIn(word, body)

    def test_feature_combinations_are_declared_with_a_measured_status(self):
        body = section(release_text(), "Supported feature combinations") or ""
        rows = re.findall(r"^\|\s*`([^`]+)`\s*\|\s*`([^`]*)`\s*\|\s*([^|]+?)\s*\|", body, re.MULTILINE)
        self.assertTrue(rows, "no combination rows")
        by_crate = {}
        for crate, flags, status in rows:
            with self.subTest(crate=crate, flags=flags):
                self.assertIn(status, COMBINATION_STATUSES)
            by_crate.setdefault(crate, set()).add(flags)
        for name, pkg in library_packages().items():
            declared = by_crate.get(name, set())
            with self.subTest(crate=name):
                self.assertIn("(default)", declared, "default build has no row")
                for feature in sorted(set(pkg["features"]) - {"default"}):
                    self.assertIn(
                        f"--no-default-features --features {feature}",
                        declared,
                        f"feature `{feature}` has no row",
                    )
                for flags in declared:
                    named = re.findall(r"--features (\S+)", flags)
                    self.assertEqual(set(named) - set(pkg["features"]), set())
        self.assertEqual(set(by_crate) - set(library_packages()), set())

    def test_toolchain_section_points_at_the_owner_and_repeats_no_number(self):
        body = section(release_text(), "Toolchain") or ""
        self.assertIn("manifest.toml", body)
        self.assertIn("rust-version", body)
        self.assertNotIn(declared_msrv(), release_text(), "the floor lives in manifest.toml only")

    def test_examples_section_matches_the_examples_on_disk(self):
        body = section(release_text(), "Examples") or ""
        declared = {}
        for crate, what in re.findall(r"^- `([^`]+)`:\s*(.+)$", body, re.MULTILINE):
            declared[crate] = what.strip()
        self.assertEqual(set(declared), set(library_packages()), "every library needs an entry")
        for name, pkg in library_packages().items():
            crate_dir = Path(pkg["manifest_path"]).parent
            on_disk = sorted(p.name for p in (crate_dir / "examples").glob("*.rs"))
            with self.subTest(crate=name):
                if on_disk:
                    for example in on_disk:
                        self.assertIn(f"examples/{example}", declared[name], "undeclared example")
                else:
                    self.assertTrue(declared[name].lower().startswith("none"))
                for path in re.findall(r"examples/([\w-]+\.rs)", declared[name]):
                    self.assertIn(path, on_disk, "declared example does not exist")

    def test_internal_crates_are_marked_unpublishable(self):
        packages = workspace_packages()
        for name, entry in declared_crates().items():
            if entry.get("class") != "internal" or name not in packages:
                continue
            with self.subTest(crate=name):
                self.assertEqual(packages[name]["publish"], [], "internal crate lacks `publish = false`")

    def test_internal_crates_do_not_claim_consumers(self):
        for name, entry in declared_crates().items():
            if entry.get("class") == "internal":
                with self.subTest(crate=name):
                    self.assertFalse(entry.get("consumers"), "internal crates have no external consumers")

    def test_consumer_verification_script_is_read_only_and_covers_every_library(self):
        script = ROOT / "scripts" / "distribution" / "verify-sdk-consumer.sh"
        self.assertTrue(script.is_file(), "missing scripts/distribution/verify-sdk-consumer.sh")
        text = script.read_text(encoding="utf-8")
        for name in library_packages():
            with self.subTest(crate=name):
                self.assertIn(name, text)
        self.assertNotRegex(text, r"cargo\s+publish(?!\s+--dry-run)", "the script must never publish")
        self.assertNotIn("cargo login", text)
        self.assertIn("mktemp -d", text, "the consumer must be built in a fresh directory")
        self.assertIn("--version", text, "the consumer must pin the published version")
        self.assertNotRegex(text, r"path\s*=", "a path dependency would not prove the registry copy")
        mode = subprocess.run(["bash", "-n", str(script)], capture_output=True, text=True, timeout=30)
        self.assertEqual(mode.returncode, 0, mode.stderr)

    def test_consumer_script_refuses_without_explicit_confirmation(self):
        script = ROOT / "scripts" / "distribution" / "verify-sdk-consumer.sh"
        done = subprocess.run(["bash", str(script)], capture_output=True, text=True, timeout=30)
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("--confirm-published", done.stderr)

    def test_publish_rehearsal_is_one_dry_run_in_release_order(self):
        script = ROOT / "scripts" / "distribution" / "rehearse-sdk-publish.sh"
        self.assertTrue(script.is_file(), "missing scripts/distribution/rehearse-sdk-publish.sh")
        text = script.read_text(encoding="utf-8")
        self.assertRegex(text, r"cargo\s+publish\s+--dry-run")
        self.assertNotRegex(text, r"cargo\s+publish(?!\s+--dry-run)", "the rehearsal must never publish")
        self.assertNotIn("cargo login", text)
        self.assertIn("--locked", text)
        self.assertIn("CARGO_TARGET_DIR", text, "the rehearsal must not reuse the shared target dir")
        positions = [text.find(f"-p {name}") for name in RELEASE_ORDER]
        self.assertNotIn(-1, positions, "every library must be named with -p")
        self.assertEqual(positions, sorted(positions), "crates must follow the release order")
        self.assertEqual(sorted(RELEASE_ORDER), sorted(library_packages()))
        mode = subprocess.run(["bash", "-n", str(script)], capture_output=True, text=True, timeout=30)
        self.assertEqual(mode.returncode, 0, mode.stderr)

    def test_release_guide_names_the_rehearsal(self):
        guide = (ROOT / "docs" / "distribution" / "SDK_RELEASE.md").read_text(encoding="utf-8")
        self.assertIn("scripts/distribution/rehearse-sdk-publish.sh", guide)


def test_sdk_surfaces_unittest_cases_pass() -> None:
    """`scripts/run_contract_tests.py` reads module-level `test_*` functions only,
    so the `unittest` class above would run zero cases there and look green."""
    import io

    suite = unittest.defaultTestLoader.loadTestsFromTestCase(SdkSurfaceContract)
    result = unittest.TextTestRunner(stream=io.StringIO(), verbosity=0).run(suite)
    assert result.testsRun > 0, "the SdkSurfaceContract suite ran no case"
    assert result.wasSuccessful(), "; ".join(
        f"{case.id()}: {trace.strip().splitlines()[-1]}"
        for case, trace in result.failures + result.errors
    )


if __name__ == "__main__":
    unittest.main()
