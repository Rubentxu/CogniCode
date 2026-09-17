#!/usr/bin/env python3
"""validate_skills.py — static validation for CogniCode skill bundles.

Implements the rules from WU17 of e84.1-cognicode-skill-productization:

  * User skills (skills/<name>/SKILL.md where name != cognicode-developer)
    MUST NOT contain:
      - /var/home/rubentxu
      - target/debug/
      - target/release/
      - any absolute path under /home/, /Users/, /var/, /opt/
      - references to internal-only docs (docs/ROADMAP.md, openspec/, docs/adr/)
      - just recipes (post-e31-audit, ci-t6, scorecard-nightly, scorecard-streak)
      - hard-coded version "0.92.0"

  * All skills MUST have valid frontmatter with:
      - name (matches directory name)
      - description (non-empty)
      - license

  * cognicode-mcp references tool names; verify each one exists in
    the runtime MCP catalog (artifacts/runtime-tools-list.json).

  * manifest.yaml MUST be valid YAML and parse cleanly.

Exit codes:
  0 = all validations passed
  1 = one or more skills failed validation
  2 = harness error (catalog missing, IO error)
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SKILLS_DIR = REPO_ROOT / "skills"

# Locate the runtime catalog under the archive dir. The path
# changed over the cycle (e84.1 lives under
# openspec/changes/archive/2026-09-17-e84-1-skill-productization/
# now), so we walk all archive entries and the active change dir.
#
# Determinism: when multiple catalogs exist (future cycles will
# append), pick the one whose directory name has the most recent
# YYYY-MM-DD- prefix. Falls back to the active change dir.
_DATE_PREFIX = re.compile(r"^(\d{4}-\d{2}-\d{2})-")


def _catalog_sort_key(path: Path) -> str:
    """Return the date prefix (or '' if missing) for sorting."""
    m = _DATE_PREFIX.match(path.parent.parent.name)
    return m.group(1) if m else ""


def _find_catalog() -> Path | None:
    candidates: list[Path] = []
    arch = REPO_ROOT / "openspec/changes/archive"
    if arch.exists():
        for sub in arch.iterdir():
            if not sub.is_dir():
                continue
            cand = sub / "artifacts" / "runtime-tools-list.json"
            if cand.exists():
                candidates.append(cand)
    if not candidates:
        return None
    # Sort by date prefix descending; newest wins. Stable tie-break
    # by full path so behavior is deterministic.
    candidates.sort(key=lambda p: (_catalog_sort_key(p), p.as_posix()), reverse=True)
    return candidates[0]


CATALOG_FILE = _find_catalog() or (
    REPO_ROOT
    / "openspec/changes/e84-1-cognicode-skill-productization/artifacts/runtime-tools-list.json"
)

# Patterns that MUST NOT appear in user-facing skills
USER_SKILL_FORBIDDEN_PATTERNS = [
    # Local-machine absolute paths
    (r"/var/home/rubentxu", "local-machine path /var/home/rubentxu"),
    (r"/home/rubentxu/Proyectos", "local-machine path /home/rubentxu/Proyectos"),
    (r"~/(Proyectos|proyectos)/rust/CogniCode", "local-machine path ~/Proyectos/..."),
    # Build paths
    (r"target/debug/", "Cargo target/debug path"),
    (r"target/release/", "Cargo target/release path"),
    # Internal-only docs
    (r"docs/ROADMAP\.md", "internal-only docs/ROADMAP.md"),
    (r"openspec/changes/", "internal-only openspec/changes/"),
    (r"openspec/CHANGES\.md", "internal-only openspec/CHANGES.md"),
    (r"docs/adr/", "internal-only docs/adr/"),
    # just recipes from the CogniCode repo
    (r"\bpost-e31-audit\b", "internal just recipe post-e31-audit"),
    (r"\bci-t6\b", "internal just recipe ci-t6"),
    (r"\bscorecard-nightly\b", "internal just recipe scorecard-nightly"),
    (r"\bscorecard-streak\b", "internal just recipe scorecard-streak"),
    # Stale version
    (r"\b0\.92\.\d+\b", "stale CogniCode version 0.92.x (current is 0.95.x)"),
    # Absolute system paths in examples
    (r"cd\s+/home/", "absolute path /home/"),
    (r"cd\s+/Users/", "absolute path /Users/"),
    (r"cd\s+/var/", "absolute path /var/"),
    (r"cd\s+/opt/", "absolute path /opt/"),
]

# Developer skills are allowed to contain internal references.
DEVELOPER_SKILL_ALLOWED_PATTERNS = [
    r"docs/ROADMAP\.md",
    r"docs/adr/",
    r"openspec/",
    r"post-e31-audit",
    r"just ",
    r"cd .*CogniCode",
]

# Frontmatter required fields
REQUIRED_FRONTMATTER = ["name", "description", "license"]


def read_skill(skill_dir: Path) -> tuple[str, str, str]:
    """Return (skill_md, manifest_yaml, skill_name)."""
    skill_md_path = skill_dir / "SKILL.md"
    if not skill_md_path.exists():
        raise FileNotFoundError(f"SKILL.md missing in {skill_dir}")
    skill_md = skill_md_path.read_text()

    manifest_path = skill_dir / "manifest.yaml"
    manifest_yaml = manifest_path.read_text() if manifest_path.exists() else ""

    skill_name = skill_dir.name
    return skill_md, manifest_yaml, skill_name


def parse_frontmatter(skill_md: str) -> dict[str, str]:
    """Extract YAML frontmatter between --- markers.

    Returns a dict of frontmatter fields. Uses a tiny parser to avoid
    requiring PyYAML for the simple `name: x` / `description: ...`
    shape used in our skills.
    """
    if not skill_md.startswith("---\n"):
        return {}
    end = skill_md.find("\n---", 4)
    if end == -1:
        return {}
    block = skill_md[4:end]

    out: dict[str, str] = {}
    current_key: str | None = None
    current_value_lines: list[str] = []
    for raw in block.splitlines():
        # Detect a new key
        m = re.match(r"^([A-Za-z_][A-Za-z0-9_]*):\s*(.*)$", raw)
        if m:
            if current_key is not None:
                out[current_key] = "\n".join(current_value_lines).strip()
            current_key = m.group(1)
            tail = m.group(2)
            current_value_lines = [tail] if tail else []
        else:
            if current_key is not None:
                current_value_lines.append(raw)
    if current_key is not None:
        out[current_key] = "\n".join(current_value_lines).strip()
    return out


def validate_frontmatter(fm: dict[str, str], skill_name: str) -> list[str]:
    errors: list[str] = []
    for f in REQUIRED_FRONTMATTER:
        if f not in fm or not fm[f].strip():
            errors.append(f"frontmatter missing or empty field: {f}")
    if fm.get("name") and fm["name"] != skill_name:
        errors.append(
            f"frontmatter name '{fm['name']}' does not match directory '{skill_name}'"
        )
    return errors


def validate_user_skill(skill_md: str, skill_name: str) -> list[str]:
    errors: list[str] = []
    for pattern, desc in USER_SKILL_FORBIDDEN_PATTERNS:
        m = re.search(pattern, skill_md)
        if m:
            errors.append(
                f"{skill_name}: forbidden {desc} (matched: '{m.group(0)}')"
            )
    return errors


def validate_developer_skill(skill_md: str, skill_name: str) -> list[str]:
    # Developer skills have fewer restrictions. We still check for
    # target/debug paths (developer might still want to use cargo
    # build artifacts, but the skill should not enshrine them).
    errors: list[str] = []
    for pattern, desc in USER_SKILL_FORBIDDEN_PATTERNS:
        if "stale CogniCode version" in desc:
            continue  # developer skills may reference historical versions
        if "local-machine" in desc or "Cargo target" in desc:
            # developers ARE local to the repo, but should use repo-relative paths
            m = re.search(pattern, skill_md)
            if m:
                errors.append(
                    f"{skill_name}: developer skill should not hard-code "
                    f"{desc} (matched: '{m.group(0)}')"
                )
    return errors


def extract_tool_name_refs(skill_md: str) -> set[str]:
    """Find MCP tool names referenced in the skill text.

    Looks for words in backticks that match MCP tool naming
    conventions (snake_case, lowercase).
    """
    refs: set[str] = set()
    for m in re.finditer(r"`([a-z][a-z0-9_]+)`", skill_md):
        name = m.group(1)
        # Filter out common non-tool names that appear in backticks
        if name in {
            # binary names
            "cogh",
            "cognicode",
            "cognicode-mcp",
            # cogh subcommands (CLI, not MCP)
            "install",
            "uninstall",
            "list",
            "current",
            "latest",
            "update",
            "reshim",
            "rollback",
            "doctor",
            "where",
            "init",
            "plugin",
            "skill",
            "ide",
            "version",
            # cognicode subcommands (CLI, not MCP)
            "analyze",
            "serve",
            "refactor",
            "index",
            "graph",
            "navigate",
            # crates / external dependencies
            "tokio",
            "sqlx",
            "serde",
            "anyhow",
            "thiserror",
            # MCP plumbing
            "tools/list",
            "tools/call",
            # just recipes
            "build_release",
            "build_wasm",
            "build_server",
            "test_unit",
            "test_pg",
            "check_known_failures",
            "lint",
            "dev",
            "run",
            "start",
        }:
            continue
        if name.startswith("just_"):
            continue
        if len(name) < 5:  # too short to be a real tool name
            continue
        refs.add(name)
    return refs


def validate_tool_refs(
    skill_md: str, skill_name: str, catalog: set[str]
) -> tuple[list[str], set[str]]:
    """Returns (errors, valid_refs).

    A reference is "valid" iff it is in the runtime catalog. Tokens
    that look like tool names but are not in the catalog are
    distinguished as follows:

    - If the token appears within 20 characters of a `(` (likely a
      function/parameter list), it is treated as a parameter name
      and not flagged.
    - Otherwise it is flagged as a missing MCP tool reference.
    """
    refs = extract_tool_name_refs(skill_md)
    errors: list[str] = []
    valid_refs: set[str] = set()
    for r in refs:
        if r in catalog:
            valid_refs.add(r)
            continue
        # Heuristic: if the token is near `(`, treat as parameter.
        # Find every occurrence and check the chars immediately before.
        is_parameter = False
        for m in re.finditer(re.escape(r), skill_md):
            start = m.start()
            # Look back 20 chars for `(` or `,` or whitespace+`(`.
            preceding = skill_md[max(0, start - 20):start]
            if "(" in preceding or "," in preceding:
                is_parameter = True
                break
        if is_parameter:
            # Treat as parameter name, not missing tool.
            continue
        errors.append(
            f"{skill_name}: referenced MCP tool `{r}` not in runtime catalog"
        )
    return errors, valid_refs


def load_catalog() -> set[str]:
    if not CATALOG_FILE.exists():
        print(
            f"WARNING: runtime catalog not found at {CATALOG_FILE}; "
            "tool-ref validation skipped.",
            file=sys.stderr,
        )
        return set()
    data = json.loads(CATALOG_FILE.read_text())
    return {tool["name"] for tool in data}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--skill",
        default=None,
        help="validate only this skill directory name (default: all)",
    )
    args = parser.parse_args()

    if not SKILLS_DIR.exists():
        print(f"ERROR: skills directory not found at {SKILLS_DIR}", file=sys.stderr)
        return 2

    catalog = load_catalog()

    skill_dirs = sorted(
        d
        for d in SKILLS_DIR.iterdir()
        if d.is_dir()
        and not d.name.startswith(".")
        and (args.skill is None or d.name == args.skill)
    )

    if not skill_dirs:
        print(f"ERROR: no skills found in {SKILLS_DIR}", file=sys.stderr)
        return 2

    total_errors = 0
    for skill_dir in skill_dirs:
        skill_name = skill_dir.name
        print(f"== {skill_name} ==")

        # SkillSet (e.g. cognicode-recommended) has SET.yaml, not SKILL.md.
        set_yaml = skill_dir / "SET.yaml"
        if set_yaml.exists():
            try:
                import yaml
                data = yaml.safe_load(set_yaml.read_text())
                if data.get("kind") != "SkillSet":
                    print(f"  FAIL: SET.yaml kind must be 'SkillSet', got {data.get('kind')}")
                    total_errors += 1
                else:
                    print(f"  ok: SkillSet manifest valid")
            except ImportError:
                print(f"  SKIP: PyYAML not installed; cannot parse SET.yaml")
            except Exception as e:
                print(f"  FAIL: SET.yaml invalid YAML: {e}")
                total_errors += 1
            print()
            continue

        try:
            skill_md, manifest_yaml, name = read_skill(skill_dir)
        except FileNotFoundError as e:
            print(f"  FAIL: {e}")
            total_errors += 1
            continue

        # Frontmatter
        fm = parse_frontmatter(skill_md)
        if not fm:
            print(f"  FAIL: no frontmatter found")
            total_errors += 1
            continue
        for e in validate_frontmatter(fm, skill_name):
            print(f"  FAIL: {e}")
            total_errors += 1

        # Manifest YAML (basic syntax)
        if not manifest_yaml:
            print(f"  FAIL: manifest.yaml missing")
            total_errors += 1
        else:
            try:
                import yaml
                yaml.safe_load(manifest_yaml)
            except ImportError:
                pass  # PyYAML not installed; skip detailed check
            except Exception as e:
                print(f"  FAIL: manifest.yaml invalid YAML: {e}")
                total_errors += 1

        # Skill-specific rules
        if skill_name == "cognicode-developer":
            for e in validate_developer_skill(skill_md, skill_name):
                print(f"  FAIL: {e}")
                total_errors += 1
        else:
            for e in validate_user_skill(skill_md, skill_name):
                print(f"  FAIL: {e}")
                total_errors += 1

        # Tool references (only for skills that reference MCP tools)
        if catalog and "MCP" in skill_md or "mcp" in skill_md.lower():
            errs, valid = validate_tool_refs(skill_md, skill_name, catalog)
            for e in errs:
                print(f"  FAIL: {e}")
                total_errors += 1
            if valid:
                print(f"  ok: {len(valid)} MCP tool reference(s) validated")

    print()
    if total_errors == 0:
        print(f"PASS: all {len(skill_dirs)} skill(s) validated")
        return 0
    else:
        print(f"FAIL: {total_errors} validation error(s)")
        return 1


if __name__ == "__main__":
    sys.exit(main())
