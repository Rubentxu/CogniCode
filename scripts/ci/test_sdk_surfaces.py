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
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DECLARATION = ROOT / "docs" / "distribution" / "SDK_SURFACES.json"
CLASSES = {"library", "application", "internal"}
METADATA_DEADLINE_SECONDS = 120
PACKAGE_LIST_DEADLINE_SECONDS = 120
TEST_PAYLOAD_PREFIXES = ("tests/", "testdata/")
LICENSE_FILES = ("LICENSE-MIT", "LICENSE-APACHE")
MAX_KEYWORDS = 5
# crates.io category slugs this workspace may use. A closed list because the
# registry rejects unknown slugs at publish time and offline we cannot ask it.
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


if __name__ == "__main__":
    unittest.main()
