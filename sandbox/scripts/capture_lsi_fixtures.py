#!/usr/bin/env python3
"""capture_lsi_fixtures.py — LSI M0 golden-fixture capture harness (E36 WU-1).

Captures graph / symbol / impact outputs for the pinned multi-language fixture
set (sandbox/fixtures/{python-hello,rust-hello,multi-lang-types}) through the
project CLI (`cognicode`) and the MCP surface (`mcp-client`), canonicalizes
them deterministically, and pins them as golden fixtures under
sandbox/fixtures/lsi-baseline/goldens/.

Modes
-----
  (default | --check)  Capture + canonicalize + compare against committed
                       goldens. WRITES NOTHING. Non-zero exit on any diff or
                       missing golden ("Regeneration is byte-identical").
  --accept             Explicit re-baseline: write canonical outputs into the
                       goldens directory (+ coverage.json). The ONLY mode that
                       writes. Silent overwrite is forbidden.
  --self-test          In-process threat-matrix checks (RED/GREEN evidence for
                       task 1.2/1.3/1.5): check-without-goldens fails cleanly,
                       no silent overwrite, --accept re-baselines, fixed argv
                       / no shell, double-run byte identity.

Determinism contract (specs/lsi-m0-baseline/spec.md):
  - stable element ordering   -> mermaid statement blocks, numbered "impacted"
                                 lists, "name at loc" lists and JSON payload
                                 arrays are sorted; JSON keys are sorted.
  - no timestamps             -> tracing log lines are dropped, durations are
                                 scrubbed to <duration>.
  - no environment paths      -> absolute project root / home dir scrubbed.

Subprocess safety (threat matrix, design.md): fixed argv built from constants,
relative in-repo fixture roots only, shell=False, no user-supplied paths.

Exit codes: 0 green, 1 contract failure (diff / missing goldens / self-test),
2 environment error (missing binary, inventory or fixture).
"""

from __future__ import annotations

import argparse
import inspect
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

SCHEMA_VERSION = 1

LSI_BASELINE_DIR = "sandbox/fixtures/lsi-baseline"
GOLDENS_SUBDIR = "goldens"
INVENTORY_FILE = "inventory.json"
COVERAGE_FILE = "coverage.json"

CLI_TIMEOUT_S = 120
MCP_TIMEOUT_S = 180

# ── Pinned fixture set ────────────────────────────────────────────────────────
# Multi-language micro-repos under sandbox/fixtures/ (in-repo constant roots).
# `impact_symbol` drives the graph impact / hierarchy / trace captures.

FIXTURES: list[dict[str, Any]] = [
    {
        "name": "python-hello",
        "path": "sandbox/fixtures/python-hello",
        "impact_symbol": "hello",
        "trace_pair": ["test_greet", "greet"],
        "outline_files": ["hello.py"],
    },
    {
        "name": "rust-hello",
        "path": "sandbox/fixtures/rust-hello",
        "impact_symbol": "greet",
        "trace_pair": ["test_greet", "greet"],
        "outline_files": ["src/lib.rs"],
    },
    {
        "name": "multi-lang-types",
        "path": "sandbox/fixtures/multi-lang-types",
        "impact_symbol": "find_by_id",
        "trace_pair": ["save", "query"],
        "outline_files": [
            "python/types.py",
            "go/types.go",
            "typescript/interfaces.ts",
            "src/lib.rs",
        ],
    },
]


def _slug(rel_path: str) -> str:
    return rel_path.replace("/", "_")


# ── Capture spec: fixed argv per surface ─────────────────────────────────────
# Each capture: key, kind (cli|mcp), inventory ids covered, argv builder.
# argv is ALWAYS a list[str] built from constants + relative fixture paths.

SURFACES: list[dict[str, Any]] = []


# ── Known-unstable surfaces (excluded from goldens, reported explicitly) ─────
# E36 WU-1 finding: the multi-lang-types fixture declares the same symbol names
# in several languages (Email, User, UserId, Page exist in go/types.go,
# python/types.py and src/lib.rs). Directory-level graph surfaces collapse
# same-named symbols first-file-wins, and file walk order is not sorted, so the
# captured NODE SET (not just its order) varies run-to-run — an engine
# nondeterminism, not a canonicalization gap. These fixture instances are
# excluded from goldens and surfaced in every report until the engine walk is
# order-stable. The inventory ids stay covered via python-hello / rust-hello.

KNOWN_UNSTABLE_SURFACES: list[dict[str, str]] = [
    {
        "fixture": "multi-lang-types",
        "surface": "cli_graph_mermaid",
        "reason": "duplicate symbol names across languages collapse first-file-wins; node set varies per run",
    },
    {
        "fixture": "multi-lang-types",
        "surface": "cli_graph_entry_points",
        "reason": "duplicate symbol names across languages; listed entry-point set varies per run",
    },
    {
        "fixture": "multi-lang-types",
        "surface": "cli_graph_leaf_functions",
        "reason": "duplicate symbol names across languages; listed leaf-function set varies per run",
    },
]


def _is_known_unstable(fixture: str, surface_key_suffix: str) -> bool:
    return any(
        u["fixture"] == fixture and u["surface"] == surface_key_suffix
        for u in KNOWN_UNSTABLE_SURFACES
    )


def _build_surfaces() -> None:
    for fx in FIXTURES:
        rel, sym = fx["path"], fx["impact_symbol"]
        cli_items = [
            {
                "key": f"{fx['name']}/cli_graph_full",
                "kind": "cli",
                "covers": ["cli.graph.full"],
                "argv": ["graph", "full", rel],
            },
            {
                "key": f"{fx['name']}/cli_graph_mermaid",
                "kind": "cli",
                "covers": ["cli.graph.mermaid"],
                "argv": ["graph", "mermaid", rel],
                "canonical": "mermaid",
            },
            {
                "key": f"{fx['name']}/cli_graph_impact",
                "kind": "cli",
                "covers": ["cli.graph.impact"],
                "argv": ["graph", "impact", sym, rel],
                "canonical": "numbered",
            },
            {
                "key": f"{fx['name']}/cli_graph_hierarchy",
                "kind": "cli",
                "covers": ["cli.graph.hierarchy"],
                "argv": ["graph", "hierarchy", sym, "--depth", "3", "--direction", "out", rel],
            },
            {
                "key": f"{fx['name']}/cli_graph_trace_path",
                "kind": "cli",
                "covers": ["cli.graph.trace-path"],
                "argv": ["graph", "trace-path", fx["trace_pair"][0], fx["trace_pair"][1], rel],
            },
            {
                "key": f"{fx['name']}/cli_graph_entry_points",
                "kind": "cli",
                "covers": ["cli.graph.entry-points"],
                "argv": ["graph", "entry-points", rel],
                "canonical": "at_location",
            },
            {
                "key": f"{fx['name']}/cli_graph_leaf_functions",
                "kind": "cli",
                "covers": ["cli.graph.leaf-functions"],
                "argv": ["graph", "leaf-functions", rel],
                "canonical": "at_location",
            },
            {
                "key": f"{fx['name']}/cli_graph_on_demand",
                "kind": "cli",
                "covers": ["cli.graph.on-demand"],
                "argv": ["graph", "on-demand", sym, "--depth", "3", "--direction", "both", rel],
            },
        ]
        # Known-unstable instances are excluded from the capture spec (see
        # KNOWN_UNSTABLE_SURFACES); the inventory ids stay covered via the
        # other fixtures.
        cli_items = [
            item
            for item in cli_items
            if not _is_known_unstable(fx["name"], item["key"].split("/", 1)[1])
        ]
        SURFACES.extend(cli_items)
        for outline_rel in fx["outline_files"]:
            SURFACES.append(
                {
                    "key": f"{fx['name']}/cli_index_outline__{_slug(outline_rel)}",
                    "kind": "cli",
                    "covers": ["cli.index.outline"],
                    "argv": ["index", "outline", f"{rel}/{outline_rel}"],
                    "display_args": ["index", "outline", f"<fixture>/{outline_rel}"],
                }
            )
        per_file_rel = fx["outline_files"][0]
        SURFACES.append(
            {
                "key": f"{fx['name']}/cli_graph_per_file__{_slug(per_file_rel)}",
                "kind": "cli",
                "covers": ["cli.graph.per-file"],
                "argv": ["graph", "per-file", f"{rel}/{per_file_rel}"],
            }
        )
        SURFACES.extend(
            [
                {
                    "key": f"{fx['name']}/mcp_build_graph",
                    "kind": "mcp",
                    "covers": ["mcp.build_graph"],
                    "mcp_tool": "build_graph",
                    "mcp_args": {"directory": "."},
                },
                {
                    "key": f"{fx['name']}/mcp_analyze_impact",
                    "kind": "mcp",
                    "covers": ["mcp.analyze_impact"],
                    "mcp_tool": "analyze_impact",
                    "mcp_args": {"symbol_name": sym},
                },
                {
                    "key": f"{fx['name']}/mcp_get_file_symbols",
                    "kind": "mcp",
                    "covers": ["mcp.get_file_symbols"],
                    "mcp_tool": "get_file_symbols",
                    "mcp_args": {"file_path": fx["outline_files"][0]},
                },
                {
                    "key": f"{fx['name']}/mcp_query_symbol_index",
                    "kind": "mcp",
                    "covers": ["mcp.query_symbol_index"],
                    "mcp_tool": "query_symbol_index",
                    "mcp_args": {"symbol_name": sym},
                },
            ]
        )


_build_surfaces()


# ── Canonicalization ─────────────────────────────────────────────────────────

ANSI_RE = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
TRACE_LINE_RE = re.compile(
    r"^\d{4}-\d{2}-\d{2}T[\d:.]+(?:Z|[+-][\d:]+)?\s+(?:INFO|WARN|ERROR|DEBUG|TRACE)\b"
)
DURATION_IN_RE = re.compile(r"\bin \d+(?:\.\d+)?\s?(?:ns|us|µs|ms|s)\b")
DURATION_BARE_RE = re.compile(r"\b\d+(?:\.\d+)?\s?(?:ms|µs|us|ns)\b")
NUM_LIST_RE = re.compile(r"^(\s*)(\d+)\.\s+(.*)$")
AT_LOCATION_RE = re.compile(r"^\s+\S.*\bat \S+:\d+:\d+$")

# JSON payload keys whose array order is engine-dependent (HashMap iteration)
UNSTABLE_ARRAY_KEYS = {
    "edges",
    "symbols",
    "impacted_symbols",
    "impacted_files",
    "locations",
}


def _scrub_text(text: str, project_root: Path) -> str:
    """Strip ANSI, drop tracing lines, scrub durations + environment paths."""
    root_variants = {
        str(project_root),
        os.path.realpath(project_root),
        os.getcwd(),
    }
    home = str(Path.home())
    lines = []
    for raw in text.splitlines():
        line = ANSI_RE.sub("", raw)
        if TRACE_LINE_RE.match(line):
            continue
        for variant in root_variants:
            line = line.replace(variant, "<PROJECT_ROOT>")
        if home and home != "/":
            line = line.replace(home, "<HOME>")
        line = DURATION_IN_RE.sub("in <duration>", line)
        lines.append(line.rstrip())
    return "\n".join(lines).rstrip("\n") + "\n"


def _sort_numbered_lists(text: str) -> str:
    """Sort consecutive numbered-list runs ("  1. item") by item content."""
    lines = text.splitlines()
    out: list[str] = []
    run: list[tuple[str, str]] = []

    def flush() -> None:
        if not run:
            return
        items = sorted(item for _, item in run)
        indent = run[0][0]
        out.extend(f"{indent}{i + 1}. {item}" for i, item in enumerate(items))
        run.clear()

    for line in lines:
        m = NUM_LIST_RE.match(line)
        if m:
            run.append((m.group(1), m.group(3)))
        else:
            flush()
            out.append(line)
    flush()
    return "\n".join(out)


def _sort_at_location_lists(text: str) -> str:
    """Sort consecutive runs of indented "name at path:line:col" lines."""
    lines = text.splitlines()
    out: list[str] = []
    run: list[str] = []
    for line in lines:
        if AT_LOCATION_RE.match(line):
            run.append(line)
        else:
            if run:
                out.extend(sorted(run))
                run.clear()
            out.append(line)
    if run:
        out.extend(sorted(run))
    return "\n".join(out)


def _sort_mermaid_statements(text: str) -> str:
    """Sort the indented statement block of a Mermaid flowchart output."""
    lines = text.splitlines()
    start = None
    for i, line in enumerate(lines):
        if line.strip() == "flowchart TD":
            start = i + 1
            break
    if start is None:
        return text
    end = start
    while end < len(lines) and lines[end].startswith("    "):
        end += 1
    lines[start:end] = sorted(lines[start:end])
    return "\n".join(lines)


def _sort_unstable_arrays(node: Any) -> None:
    if isinstance(node, dict):
        for key, value in node.items():
            if key in UNSTABLE_ARRAY_KEYS and isinstance(value, list):
                value.sort(key=lambda el: json.dumps(el, sort_keys=True))
            else:
                _sort_unstable_arrays(value)
    elif isinstance(node, list):
        for el in node:
            _sort_unstable_arrays(el)


def _scrub_strings(node: Any, project_root: Path) -> None:
    if isinstance(node, dict):
        for key, value in node.items():
            if isinstance(value, str):
                node[key] = _scrub_text_value(value, project_root)
            else:
                _scrub_strings(value, project_root)
    elif isinstance(node, list):
        for el in node:
            _scrub_strings(el, project_root)


def _scrub_text_value(value: str, project_root: Path) -> str:
    root_variants = {str(project_root), os.path.realpath(project_root)}
    home = str(Path.home())
    for variant in root_variants:
        value = value.replace(variant, "<PROJECT_ROOT>")
    if home and home != "/":
        value = value.replace(home, "<HOME>")
    return DURATION_BARE_RE.sub("<duration>", value)


def canonicalize(text: str, style: str, project_root: Path) -> str:
    """Serialize a raw capture into the canonical deterministic form."""
    scrubbed = _scrub_text(text, project_root)
    if style == "mermaid":
        scrubbed = _sort_mermaid_statements(scrubbed)
    elif style == "numbered":
        scrubbed = _sort_numbered_lists(scrubbed)
    elif style == "at_location":
        scrubbed = _sort_at_location_lists(scrubbed)
    if not scrubbed.endswith("\n"):
        scrubbed += "\n"
    return scrubbed


def canonicalize_mcp_payload(stdout_text: str, project_root: Path) -> str:
    """Canonicalize a JSON-RPC tools/call response from mcp-client.

    Parses the outer envelope, scrubs environment paths and durations inside
    the inner tool payload, sorts engine-ordered arrays, and re-serializes
    with sorted keys.
    """
    outer = json.loads(stdout_text)
    _scrub_strings(outer, project_root)
    content = outer.get("content") if isinstance(outer, dict) else None
    if isinstance(content, list) and content:
        inner_text = content[0].get("text") if isinstance(content[0], dict) else None
        if isinstance(inner_text, str):
            try:
                inner = json.loads(inner_text)
                _scrub_strings(inner, project_root)
                _sort_unstable_arrays(inner)
                content[0]["text"] = json.dumps(inner, sort_keys=True)
            except json.JSONDecodeError:
                pass  # non-JSON tool payload: keep the scrubbed text as-is
    return json.dumps(outer, indent=2, sort_keys=True) + "\n"


# ── Tool resolution ──────────────────────────────────────────────────────────

def cargo_target_bin_dir(project_root: Path) -> Path | None:
    """Resolve cargo's target dir (honours CARGO_TARGET_DIR / config)."""
    env_dir = os.environ.get("CARGO_TARGET_DIR")
    if env_dir:
        return Path(env_dir)
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"],
            cwd=str(project_root),
            capture_output=True,
            text=True,
            timeout=60,
            shell=False,
        )
        if result.returncode == 0:
            return Path(json.loads(result.stdout)["target_directory"])
    except (OSError, subprocess.SubprocessError, json.JSONDecodeError, KeyError):
        pass
    return None


def resolve_tool_bin(
    project_root: Path, name: str, env_var: str, override: str | None
) -> Path | None:
    candidates: list[Path] = []
    if override:
        candidates.append(Path(override))
    env_value = os.environ.get(env_var)
    if env_value:
        candidates.append(Path(env_value))
    target_dir = cargo_target_bin_dir(project_root)
    if target_dir:
        candidates.append(target_dir / "debug" / name)
    candidates.append(project_root / "target" / "debug" / name)
    for cand in candidates:
        if cand.is_file() and os.access(cand, os.X_OK):
            return cand
    return None


# ── Capture engine ───────────────────────────────────────────────────────────

def spawn_fixed_argv(
    argv: list[str], cwd: Path, timeout_s: int, shell: bool = False
) -> subprocess.CompletedProcess:
    """Run a fixed-argv command. shell=False is a hard threat-matrix rule."""
    return subprocess.run(
        argv,
        cwd=str(cwd),
        capture_output=True,
        text=True,
        timeout=timeout_s,
        shell=shell,
    )


def capture_surface(
    surface: dict[str, Any],
    project_root: Path,
    cli_bin: Path,
    mcp_client_bin: Path,
) -> str:
    """Return the canonical bytes for one capture surface."""
    if surface["kind"] == "cli":
        argv = [str(cli_bin), *surface["argv"]]
        proc = spawn_fixed_argv(argv, project_root, CLI_TIMEOUT_S)
    else:
        params = json.dumps(
            {"name": surface["mcp_tool"], "arguments": surface["mcp_args"]},
            sort_keys=True,
        )
        argv = [
            str(mcp_client_bin),
            "--workspace",
            fixture_root(_fixture_name_of(surface)),
            "--method",
            "tools/call",
            "--params",
            params,
        ]
        proc = spawn_fixed_argv(argv, project_root, MCP_TIMEOUT_S)
    if proc.returncode != 0:
        raise RuntimeError(
            f"capture failed for {surface['key']} (exit {proc.returncode}): "
            f"{proc.stderr.strip()[:400]}"
        )
    if surface["kind"] == "mcp":
        return canonicalize_mcp_payload(proc.stdout, project_root)
    return canonicalize(proc.stdout, surface.get("canonical", "plain"), project_root)


def _fixture_name_of(surface: dict[str, Any]) -> str:
    return surface["key"].split("/")[0]


def fixture_root(fixture_name: str) -> str:
    for fx in FIXTURES:
        if fx["name"] == fixture_name:
            return fx["path"]
    raise KeyError(fixture_name)


def capture_all(
    project_root: Path, cli_bin: Path, mcp_client_bin: Path
) -> dict[str, str]:
    outputs: dict[str, str] = {}
    for surface in SURFACES:
        outputs[surface["key"]] = capture_surface(
            surface, project_root, cli_bin, mcp_client_bin
        )
    return outputs


# ── Coverage ─────────────────────────────────────────────────────────────────

def load_inventory(fixtures_dir: Path) -> dict[str, Any]:
    inventory_path = fixtures_dir / INVENTORY_FILE
    if not inventory_path.exists():
        raise FileNotFoundError(
            f"consumer inventory not found: {inventory_path} "
            "(author it first — the harness never generates it)"
        )
    with open(inventory_path) as f:
        return json.load(f)


def compute_coverage(
    inventory: dict[str, Any], captured_keys: set[str]
) -> dict[str, Any]:
    """Per-surface coverage report against the consumer inventory."""
    weights = inventory.get("criticality_weights", {"high": 3, "medium": 2, "low": 1})
    consumers = inventory.get("consumers", [])
    direct_ids: set[str] = set()
    for consumer in consumers:
        if consumer.get("capture") == "direct":
            direct_ids.add(consumer["id"])
    mapped_ids: set[str] = set()
    for surface in SURFACES:
        if surface["key"] in captured_keys:
            mapped_ids.update(surface["covers"])
    unbacked = sorted(mapped_ids - direct_ids)
    if unbacked:
        raise RuntimeError(
            "capture spec covers inventory ids not marked capture=direct in "
            f"{INVENTORY_FILE} (inventory drift — fail closed): {unbacked}"
        )
    rows = []
    for consumer in consumers:
        cid = consumer["id"]
        fixture_paths = sorted(
            {
                fixture_root(surface["key"].split("/")[0])
                for surface in SURFACES
                if surface["key"] in captured_keys and cid in surface["covers"]
            }
        )
        covered = cid in mapped_ids
        rows.append(
            {
                "id": cid,
                "criticality": consumer.get("criticality", "low"),
                "weight": weights.get(consumer.get("criticality", "low"), 1),
                "covered": covered,
                "fixture_paths": fixture_paths,
                "note": None if covered else consumer.get("capture_note"),
            }
        )
    total_weight = sum(r["weight"] for r in rows)
    covered_weight = sum(r["weight"] for r in rows if r["covered"])
    return {
        "schema_version": SCHEMA_VERSION,
        "total_surfaces": len(rows),
        "covered_surfaces": sum(1 for r in rows if r["covered"]),
        "total_weight": total_weight,
        "covered_weight": covered_weight,
        "weighted_coverage_pct": round(100.0 * covered_weight / total_weight, 1)
        if total_weight
        else 0.0,
        "surfaces": rows,
    }


def render_coverage(coverage: dict[str, Any]) -> str:
    lines = [
        "Coverage of inventory surfaces:",
        (
            f"  covered {coverage['covered_surfaces']}/{coverage['total_surfaces']} surfaces "
            f"(weighted {coverage['covered_weight']}/{coverage['total_weight']} = "
            f"{coverage['weighted_coverage_pct']}%)"
        ),
    ]
    for row in coverage["surfaces"]:
        if row["covered"]:
            lines.append(
                f"  [x] {row['id']} ({row['criticality']}) <- {', '.join(row['fixture_paths'])}"
            )
    for row in coverage["surfaces"]:
        if not row["covered"]:
            note = f" — {row['note']}" if row.get("note") else ""
            lines.append(f"  [ ] {row['id']} ({row['criticality']}){note}")
    return "\n".join(lines)


# ── Modes ────────────────────────────────────────────────────────────────────

class EnvironmentError_(RuntimeError):
    """Exit-code 2 environment failure (missing binary / inventory / fixture)."""


def _golden_rel_path(key: str) -> Path:
    return Path(GOLDENS_SUBDIR) / f"{key}.golden"


def _diff_stat(committed: str, captured: str) -> str:
    import difflib

    diff = list(
        difflib.unified_diff(
            committed.splitlines(), captured.splitlines(), lineterm="", n=0
        )
    )
    added = sum(1 for l in diff if l.startswith("+") and not l.startswith("+++"))
    removed = sum(1 for l in diff if l.startswith("-") and not l.startswith("---"))
    return f"+{added}/-{removed} lines"


def _ensure_outputs(
    project_root: Path,
    cli_bin: Path,
    mcp_client_bin: Path,
    outputs: dict[str, str] | None,
) -> dict[str, str]:
    return outputs if outputs is not None else capture_all(project_root, cli_bin, mcp_client_bin)


def _render_known_unstable() -> str:
    if not KNOWN_UNSTABLE_SURFACES:
        return ""
    lines = ["  known-unstable surfaces (excluded from goldens):"]
    for u in KNOWN_UNSTABLE_SURFACES:
        lines.append(f"    SKIP {u['fixture']}/{u['surface']} — {u['reason']}")
    return "\n".join(lines)


def check_goldens(
    project_root: Path,
    fixtures_dir: Path,
    cli_bin: Path,
    mcp_client_bin: Path,
    outputs: dict[str, str] | None = None,
) -> tuple[int, str]:
    """Verify mode: capture + compare against committed goldens. Writes nothing.

    `outputs` allows injecting a pre-captured canonical set (self-test reuse).
    Returns (exit_code, report).
    """
    outputs = _ensure_outputs(project_root, cli_bin, mcp_client_bin, outputs)
    goldens_dir = fixtures_dir / GOLDENS_SUBDIR
    lines = [
        "LSI fixture capture — check (verify-only; nothing written)",
        f"  cli binary: {cli_bin}",
        f"  mcp client: {mcp_client_bin}",
        f"  surfaces:   {len(outputs)}",
    ]

    missing = [key for key in sorted(outputs) if not (goldens_dir / f"{key}.golden").exists()]
    if missing:
        lines.append(
            f"  RESULT: FAIL — {len(missing)} of {len(outputs)} goldens missing "
            "(no committed baseline at this location)"
        )
        for key in missing[:10]:
            lines.append(f"    MISSING {key}")
        if len(missing) > 10:
            lines.append(f"    ... and {len(missing) - 10} more")
        lines.append(
            "  Capture an explicit baseline with: "
            "python3 sandbox/scripts/capture_lsi_fixtures.py --accept"
        )
        return 1, "\n".join(lines)

    diffs: list[str] = []
    for key in sorted(outputs):
        committed = (goldens_dir / f"{key}.golden").read_text()
        if committed != outputs[key]:
            diffs.append((key, _diff_stat(committed, outputs[key])))
    if diffs:
        lines.append(f"  RESULT: FAIL — {len(diffs)} of {len(outputs)} goldens differ")
        for key, stat in diffs:
            lines.append(f"    DIFF {key} ({stat})")
        lines.append(
            "  Intended behavior change requires explicit re-baseline: "
            "python3 sandbox/scripts/capture_lsi_fixtures.py --accept"
        )
        return 1, "\n".join(lines)

    coverage = compute_coverage(load_inventory(fixtures_dir), set(outputs))
    lines.append(f"  RESULT: PASS — all {len(outputs)} goldens byte-identical")
    unstable_block = _render_known_unstable()
    if unstable_block:
        lines.append(unstable_block)
    lines.append(render_coverage(coverage))
    return 0, "\n".join(lines)


def accept_goldens(
    project_root: Path,
    fixtures_dir: Path,
    cli_bin: Path,
    mcp_client_bin: Path,
    outputs: dict[str, str] | None = None,
) -> tuple[int, str]:
    """Re-baseline mode: the ONLY path that writes goldens + coverage.json.

    `outputs` allows injecting a pre-captured canonical set (self-test reuse).
    Returns (exit_code, report).
    """
    outputs = _ensure_outputs(project_root, cli_bin, mcp_client_bin, outputs)
    goldens_dir = fixtures_dir / GOLDENS_SUBDIR
    coverage = compute_coverage(load_inventory(fixtures_dir), set(outputs))
    for key, content in sorted(outputs.items()):
        golden_path = fixtures_dir / _golden_rel_path(key)
        golden_path.parent.mkdir(parents=True, exist_ok=True)
        golden_path.write_text(content)
    coverage_path = fixtures_dir / COVERAGE_FILE
    with open(coverage_path, "w") as f:
        json.dump(coverage, f, indent=2, sort_keys=True)
        f.write("\n")
    lines = [
        "LSI fixture capture — accept (explicit re-baseline)",
        f"  wrote {len(outputs)} goldens under {goldens_dir}",
        (
            f"  wrote {coverage_path} "
            f"(covered {coverage['covered_surfaces']}/{coverage['total_surfaces']} surfaces, "
            f"weighted {coverage['weighted_coverage_pct']}%)"
        ),
        "  RESULT: OK",
    ]
    unstable_block = _render_known_unstable()
    if unstable_block:
        lines.append(unstable_block)
    return 0, "\n".join(lines)


# ── Self-test (threat-matrix mapped cases) ───────────────────────────────────

def _self_test_setup(project_root: Path) -> tuple[Path, Path, Path]:
    cli_bin = resolve_tool_bin(project_root, "cognicode", "COGNICODE_BIN", None)
    mcp_bin = resolve_tool_bin(project_root, "mcp-client", "COGNICODE_MCP_CLIENT_BIN", None)
    if cli_bin is None or mcp_bin is None:
        raise EnvironmentError_(
            "self-test requires built binaries: cognicode and mcp-client "
            "(cargo build -p cognicode-cli --bin cognicode && cargo build -p cognicode-mcp --bin mcp-client)"
        )
    fixtures_dir = project_root / LSI_BASELINE_DIR
    return fixtures_dir, cli_bin, mcp_bin


def self_test(project_root: Path) -> tuple[int, str]:
    """Run the threat-matrix mapped checks. Returns (exit_code, report)."""
    results: list[tuple[str, bool, str]] = []

    def record(name: str, ok: bool, detail: str = "") -> None:
        results.append((name, ok, detail))

    try:
        fixtures_dir, cli_bin, mcp_bin = _self_test_setup(project_root)
    except EnvironmentError_ as exc:
        return 2, f"SELF-TEST BLOCKED: {exc}"

    inventory = load_inventory(fixtures_dir)

    # Case D: subprocess safety — fixed argv lists, in-repo roots, no shell.
    cli_surfaces = [s for s in SURFACES if s["kind"] == "cli"]
    argv_ok = all(
        isinstance(s["argv"], list)
        and all(isinstance(part, str) for part in s["argv"])
        and not any(ch in part for s in cli_surfaces for part in s["argv"] for ch in ("&&", ";", "|", "`"))
        for s in cli_surfaces
    )
    mcp_ok = all(
        isinstance(s["mcp_tool"], str)
        and isinstance(s["mcp_args"], dict)
        for s in SURFACES
        if s["kind"] == "mcp"
    )
    roots_ok = all(fx["path"].startswith("sandbox/fixtures/") for fx in FIXTURES)
    sig = inspect.signature(spawn_fixed_argv)
    no_shell = sig.parameters["shell"].default is False
    record(
        "argv_fixed_no_shell_inrepo_roots",
        argv_ok and roots_ok and no_shell and mcp_ok,
        f"argv_ok={argv_ok} mcp_params_ok={mcp_ok} roots_ok={roots_ok} shell_default_false={no_shell}",
    )

    # Case E: canonicalizer unit checks.
    sample = (
        "\x1b[2m2026-09-12T08:26:10.665295Z\x1b[0m \x1b[32m INFO\x1b[0m Starting\n"
        "Full graph built in 123ms\n"
        f"kept {project_root}/x\n"
    )
    canon = canonicalize(sample, "plain", project_root)
    canon_ok = (
        "INFO" not in canon
        and "2026-09-12" not in canon
        and "in <duration>" in canon
        and "<PROJECT_ROOT>/x" in canon
        and canon.endswith("\n")
    )
    mermaid_sample = "flowchart TD\n    b[second]\n    a[first]\n"
    mermaid_ok = _sort_mermaid_statements(mermaid_sample).splitlines()[1] == "    a[first]"
    numbered_ok = (
        _sort_numbered_lists("x\n  2. beta\n  1. alpha\ny").splitlines()[1:] == ["  1. alpha", "  2. beta", "y"]
    )
    payload = json.dumps(
        {
            "content": [
                {
                    "type": "text",
                    "text": json.dumps(
                        {
                            "edges": [{"from": "b", "to": "a"}, {"from": "a", "to": "b"}],
                            "message": "built in 20ms",
                            "file": str(project_root) + "/f.py",
                        }
                    ),
                }
            ],
            "isError": False,
        }
    )
    canon_payload = json.loads(canonicalize_mcp_payload(payload, project_root))
    inner = json.loads(canon_payload["content"][0]["text"])
    payload_ok = (
        inner["edges"][0]["from"] == "a"
        and inner["message"] == "built in <duration>"
        and inner["file"] == "<PROJECT_ROOT>/f.py"
    )
    record(
        "canonicalizer_sorted_scrubbed",
        canon_ok and mermaid_ok and numbered_ok and payload_ok,
        f"scrub={canon_ok} mermaid={mermaid_ok} numbered={numbered_ok} payload={payload_ok}",
    )

    # Cases A/B/C need one real capture pass (reused).
    try:
        outputs = capture_all(project_root, cli_bin, mcp_bin)
    except (RuntimeError, subprocess.SubprocessError, json.JSONDecodeError) as exc:
        record("capture_all", False, str(exc)[:300])
        return _self_test_report(results)
    record("capture_all", True, f"{len(outputs)} surfaces captured")

    # Case H: known-unstable fixture instances are never captured/pinned.
    leaked = sorted(
        key for key in outputs
        if any(
            key == f"{u['fixture']}/{u['surface']}" for u in KNOWN_UNSTABLE_SURFACES
        )
    )
    record(
        "known_unstable_excluded_from_goldens",
        not leaked,
        f"leaked={leaked}" if leaked else f"{len(KNOWN_UNSTABLE_SURFACES)} registered, 0 captured",
    )

    with tempfile.TemporaryDirectory(prefix="lsi-capture-selftest-") as tmp:
        # Simulate a real fixtures dir layout: the consumer inventory is present.
        shutil.copyfile(fixtures_dir / INVENTORY_FILE, Path(tmp) / INVENTORY_FILE)

        # Case A: --check with NO goldens -> clean failure, zero writes.
        def safe(fn, *a, **kw) -> tuple[int, str]:
            try:
                return fn(*a, **kw)
            except Exception as exc:  # noqa: BLE001 — contract must fail cleanly
                return 1, f"ERROR: {type(exc).__name__}: {exc}"

        files_before = sorted(p.name for p in Path(tmp).rglob("*") if p.is_file())
        exit_a, report_a = safe(
            check_goldens, project_root, Path(tmp), cli_bin, mcp_bin, outputs
        )
        files_after = sorted(p.name for p in Path(tmp).rglob("*") if p.is_file())
        clean = "Traceback" not in report_a and "ERROR" not in report_a and report_a.strip() != ""
        record(
            "check_without_goldens_fails_clean_and_writes_nothing",
            exit_a == 1 and clean and files_after == files_before,
            f"exit={exit_a} clean_report={clean} files_added={[f for f in files_after if f not in files_before]}",
        )

        # Case B: tampered golden -> --check fails WITHOUT overwriting.
        first_key = sorted(outputs)[0]
        target = Path(tmp) / GOLDENS_SUBDIR / f"{first_key}.golden"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("TAMPERED\n")
        for key, content in outputs.items():
            if key == first_key:
                continue
            p = Path(tmp) / GOLDENS_SUBDIR / f"{key}.golden"
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(content)
        exit_b, report_b = safe(
            check_goldens, project_root, Path(tmp), cli_bin, mcp_bin, outputs
        )
        not_overwritten = target.read_text() == "TAMPERED\n"
        reported = first_key in report_b
        record(
            "check_reports_diff_without_overwrite",
            exit_b == 1 and not_overwritten and reported,
            f"exit={exit_b} tampered_intact={not_overwritten} diff_reported={reported}",
        )

        # Case C: --accept re-baselines the tampered golden.
        exit_c, report_c = safe(
            accept_goldens, project_root, Path(tmp), cli_bin, mcp_bin, outputs
        )
        rebaselined = target.read_text() == outputs[first_key]
        coverage_written = (Path(tmp) / COVERAGE_FILE).exists()
        record(
            "accept_rebaselines_explicitly",
            exit_c == 0 and rebaselined and coverage_written,
            f"exit={exit_c} rebaselined={rebaselined} coverage_json={coverage_written}",
        )

        # Case F: double-run byte identity — a SECOND capture pass compared
        # against the just-accepted goldens must be identical.
        exit_f, _ = safe(check_goldens, project_root, Path(tmp), cli_bin, mcp_bin)
        record(
            "regeneration_byte_identical",
            exit_f == 0,
            f"second_capture_check_exit={exit_f}",
        )

    # Case G: coverage report covers every direct inventory surface.
    coverage = compute_coverage(inventory, set(outputs))
    direct_ids = {
        c["id"] for c in inventory.get("consumers", []) if c.get("capture") == "direct"
    }
    covered_ids = {row["id"] for row in coverage["surfaces"] if row["covered"]}
    record(
        "coverage_reports_per_surface",
        direct_ids <= covered_ids and coverage["total_surfaces"] >= len(direct_ids),
        f"direct={len(direct_ids)} covered={len(covered_ids)} "
        f"weighted={coverage['weighted_coverage_pct']}%",
    )

    return _self_test_report(results)


def _self_test_report(results: list[tuple[str, bool, str]]) -> tuple[int, str]:
    lines = ["LSI capture self-test (threat-matrix mapped cases):"]
    for name, ok, detail in results:
        lines.append(f"  {'PASS' if ok else 'FAIL'}  {name}  {detail}")
    failed = sum(1 for _, ok, _ in results if not ok)
    lines.append(f"  {len(results) - failed}/{len(results)} passed")
    return (1 if failed else 0), "\n".join(lines)


# ── Main ─────────────────────────────────────────────────────────────────────

def main() -> int:
    parser = argparse.ArgumentParser(
        description="LSI M0 golden-fixture capture harness (E36 WU-1)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify against committed goldens; writes nothing (default mode)",
    )
    parser.add_argument(
        "--accept",
        action="store_true",
        help="explicit re-baseline: write goldens + coverage.json",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run in-process threat-matrix checks",
    )
    parser.add_argument(
        "--cognicode-bin",
        default=None,
        help="override path to the cognicode CLI binary",
    )
    parser.add_argument(
        "--mcp-client-bin",
        default=None,
        help="override path to the mcp-client binary",
    )
    parser.add_argument(
        "--project-root",
        default=None,
        help="project root override (self-test plumbing; defaults to repo root)",
    )
    args = parser.parse_args()

    project_root = (
        Path(args.project_root)
        if args.project_root
        else Path(__file__).resolve().parent.parent.parent
    )
    fixtures_dir = project_root / LSI_BASELINE_DIR

    if args.self_test:
        code, report = self_test(project_root)
        print(report)
        return code

    if args.accept and args.check:
        print("--accept and --check are mutually exclusive", file=sys.stderr)
        return 2

    try:
        cli_bin = resolve_tool_bin(project_root, "cognicode", "COGNICODE_BIN", args.cognicode_bin)
        mcp_bin = resolve_tool_bin(
            project_root, "mcp-client", "COGNICODE_MCP_CLIENT_BIN", args.mcp_client_bin
        )
        missing = [
            name
            for name, path in (("cognicode", cli_bin), ("mcp-client", mcp_bin))
            if path is None
        ]
        if missing:
            print(
                "ERROR: required binaries not found: "
                + ", ".join(missing)
                + " — build with `cargo build -p cognicode-cli --bin cognicode` "
                "and `cargo build -p cognicode-mcp --bin mcp-client`, or set "
                "COGNICODE_BIN / COGNICODE_MCP_CLIENT_BIN",
                file=sys.stderr,
            )
            return 2
        if not (fixtures_dir / INVENTORY_FILE).exists():
            print(
                f"ERROR: {fixtures_dir / INVENTORY_FILE} not found — author the "
                "consumer inventory first (the harness never generates it)",
                file=sys.stderr,
            )
            return 2
        for fx in FIXTURES:
            if not (project_root / fx["path"]).is_dir():
                print(f"ERROR: pinned fixture missing: {fx['path']}", file=sys.stderr)
                return 2

        if args.accept:
            code, report = accept_goldens(project_root, fixtures_dir, cli_bin, mcp_bin)
        else:
            code, report = check_goldens(project_root, fixtures_dir, cli_bin, mcp_bin)
        print(report)
        return code
    except EnvironmentError_ as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2
    except (RuntimeError, subprocess.SubprocessError, json.JSONDecodeError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
