#!/usr/bin/env python3
"""Measure the responsibilities inside WorkspaceSession and their radii.

The audit (CGH-010) found one file of ~4.5k lines holding the whole
application façade. Size alone is not a defect; unmeasured extraction
is. This script turns "should we split it?" into three measured
questions:

  1. RESPONSIBILITY — every public method belongs to exactly one
     declared group. A method that is not in any group fails the
     measurement: the façade grew and nobody said what it is for.
  2. IMPACT RADIUS — which crates outside this file call each method,
     and how many places. An extraction that moves a method with a
     wide radius moves the radius too; the number bounds the review.
  3. CHARACTERIZATION — is the method exercised by a test it cannot
     fake (a call site inside a `#[cfg(test)]` region or under any
     crate's `tests/`). A group is READY TO EXTRACT only when every
     public method in it is exercised AND its private helpers are
     closed: no helper shared with a method outside the group.

The verdict of the extraction rule lives here, not in prose: no
characterization, no extraction. The report this script writes
(`docs/debts/SEAM-WORKSPACE-SESSION.md`) is the measurement of the day;
re-run `--write` before trusting it, and `--check` in a lane that must
not drift.

Usage:
  python3 scripts/measure_workspace_session.py            # report to stdout
  python3 scripts/measure_workspace_session.py --write    # rewrite the doc
  python3 scripts/measure_workspace_session.py --check    # fail on drift
Exit codes: 0 ok, 1 drift or unassigned methods, 2 the tool could not measure.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
TARGET = REPO_ROOT / "crates/cognicode-core/src/application/workspace_session.rs"
DOC = REPO_ROOT / "docs/debts/SEAM-WORKSPACE-SESSION.md"

# The declared groups. Each one names a capability the façade offers;
# the method lists are what the measurement checks itself against.
GROUPS: dict[str, list[str]] = {
    "lifecycle": ["with_capabilities", "workspace_root"],
    "graph store": ["held_graph", "require_graph", "ensure_graph_built",
                    "set_graph_store", "load_from_store", "save_to_store"],
    "indexing": ["incremental_reindex", "build_graph",
                 "build_lightweight_index", "subscribe_graph_events"],
    "files": ["read_file", "write_file", "edit_file", "list_files",
              "resolve_path"],
    "search": ["search_content", "ensure_semantic_search", "semantic_search",
               "semantic_search_with_kinds", "map_kind_strings",
               "map_kind_string"],
    "symbol queries": ["get_file_symbols", "get_outline", "get_complexity",
                       "get_symbol_code", "get_all_symbols",
                       "query_symbol_index", "ranked_symbols_ai"],
    "graph queries": ["get_call_hierarchy", "analyze_impact", "trace_path",
                      "get_entry_points", "get_leaf_functions",
                      "export_mermaid", "build_subgraph", "get_graph_stats",
                      "get_hot_paths"],
    "architecture": ["check_architecture"],
    "refactoring": ["refactor_basis", "rename_symbol", "inline_symbol",
                    "move_symbol", "extract_function", "validate_syntax"],
    "lsp surface": ["document_symbols", "go_to_definition", "hover",
                    "find_references"],
    "agent insight": ["get_project_diagnostics", "smart_overview",
                      "auto_diagnose", "suggest_onboarding_plan",
                      "ask_about_code"],
}

FN_RE = re.compile(
    r"^    (?P<vis>pub(?:\([^)]*\))?\s+)?(?P<async>async\s+)?fn\s+(?P<name>\w+)"
)
CFG_TEST_RE = re.compile(r"#\[cfg\(test\)\]")
SELF_CALL_RE = re.compile(r"self\.(\w+)\(")
CALLSITE_RE_TMPL = ".{name}("


@dataclass
class Method:
    name: str
    start: int
    end: int
    public: bool
    body_calls: set[str] = field(default_factory=set)

    @property
    def loc(self) -> int:
        return self.end - self.start + 1


def find_impl_blocks(lines: list[str]) -> list[tuple[int, int]]:
    """Spans of `impl WorkspaceSession {` blocks as (first, last) line
    indices, 0-based inclusive, found by brace counting."""
    spans = []
    for i, line in enumerate(lines):
        if re.match(r"^(pub\(crate\)\s+)?impl\s+(.*\s)?WorkspaceSession\b.*\{\s*$", line):
            depth = 0
            for j in range(i, len(lines)):
                depth += lines[j].count("{") - lines[j].count("}")
                if depth == 0:
                    spans.append((i, j))
                    break
    return spans


def test_spans(lines: list[str]) -> list[tuple[int, int]]:
    """Regions gated by `#[cfg(test)]` (the attribute gates the item
    that follows it, `mod` or not)."""
    spans = []
    i = 0
    while i < len(lines):
        if CFG_TEST_RE.match(lines[i].strip()) or lines[i].strip() == "#[cfg(test)]":
            k = i + 1
            while k < len(lines) and not lines[k].strip():
                k += 1
            if k < len(lines):
                depth = 0
                seen_open = False
                for j in range(k, len(lines)):
                    depth += lines[j].count("{") - lines[j].count("}")
                    seen_open = seen_open or lines[j].count("{") > 0
                    if seen_open and depth <= 0:
                        spans.append((i, j))
                        i = j
                        break
        i += 1
    return spans


def parse_methods(lines: list[str]) -> tuple[list[Method], list[tuple[int, int]]]:
    methods: list[Method] = []
    inline_tests = test_spans(lines)

    def in_inline_test(start: int) -> bool:
        return any(a <= start <= b for a, b in inline_tests)

    for lo, hi in find_impl_blocks(lines):
        i = lo + 1
        while i < hi:
            m = FN_RE.match(lines[i])
            if m and not in_inline_test(i):
                depth = 0
                seen_open = False
                end = i
                for j in range(i, hi + 1):
                    depth += lines[j].count("{") - lines[j].count("}")
                    seen_open = seen_open or lines[j].count("{") > 0
                    if seen_open and depth == 0:
                        end = j
                        break
                body = "\n".join(lines[i:end + 1])
                methods.append(
                    Method(
                        name=m.group("name"),
                        start=i + 1,
                        end=end + 1,
                        public=bool(m.group("vis")),
                        body_calls={c for c in SELF_CALL_RE.findall(body)
                                    if c != m.group("name")},
                    )
                )
                i = end + 1
                continue
            i += 1
    return methods, inline_tests


def rs_files(rel: bool) -> list[Path]:
    # rel=True: production sources; rel=False: every crate's tests dir.
    if rel:
        return sorted(REPO_ROOT.glob("crates/*/src/**/*.rs"))
    return sorted(REPO_ROOT.glob("crates/*/tests/**/*.rs"))


def read_all(paths: list[Path]) -> list[tuple[Path, str]]:
    """One filesystem pass: the radii of ~50 methods are then counted
    against texts in memory, not against the disk."""
    return [(p, p.read_text(encoding="utf8", errors="replace")) for p in paths]


def measure_callsites(name: str, prod: list[tuple[Path, str]]) -> dict[str, int]:
    """crate -> number of call sites, over production sources outside
    the measured file itself."""
    needle = CALLSITE_RE_TMPL.format(name=name)
    hits: dict[str, int] = {}
    for path, text in prod:
        if path == TARGET:
            continue
        n = text.count(needle)
        if n:
            crate = path.relative_to(REPO_ROOT).parts[1]
            hits[crate] = hits.get(crate, 0) + n
    return hits


def measure_characterization(
    name: str, inline_tests: list[tuple[int, int]], tests_texts: list[tuple[Path, str]],
    target_lines: list[str],
) -> int:
    """Call sites inside `#[cfg(test)]` regions of the file plus any
    occurrence in a crate's `tests/` tree."""
    needle = CALLSITE_RE_TMPL.format(name=name)
    count = 0
    for a, b in inline_tests:
        count += sum(line.count(needle) for line in target_lines[a:b + 1])
    for _, text in tests_texts:
        count += text.count(needle)
    return count


def head_sha() -> str:
    return subprocess.run(
        ["git", "rev-parse", "--short", "HEAD"],
        cwd=REPO_ROOT, capture_output=True, text=True, check=True,
    ).stdout.strip()


def build_report() -> tuple[str, int]:
    """Returns (markdown, problems). problems>0 means the measurement
    cannot be trusted as an extraction basis."""
    if not TARGET.is_file():
        print(f"ERROR: no existe {TARGET}", file=sys.stderr)
        sys.exit(2)
    lines = TARGET.read_text(encoding="utf8", errors="replace").splitlines()
    methods, inline_tests = parse_methods(lines)
    by_name = {m.name: m for m in methods}
    pub_names = {m.name for m in methods if m.public}

    problems = 0
    assigned: set[str] = set()
    for group, names in GROUPS.items():
        for n in names:
            assigned.add(n)
            if n not in by_name:
                print(f"AVISO: '{n}' del grupo '{group}' ya no es un metodo "
                      f"de WorkspaceSession: el grupo quedo obsoleto",
                      file=sys.stderr)
                problems += 1

    unassigned = sorted(pub_names - assigned)
    if unassigned:
        problems += len(unassigned)

    prod = read_all(rs_files(rel=True))
    tests_texts = read_all(rs_files(rel=False))
    n_inline_tests = len(inline_tests)

    rows = []
    for group, names in GROUPS.items():
        present = [by_name[n] for n in names if n in by_name]
        loc = sum(m.loc for m in present)
        crates: set[str] = set()
        sites = 0
        untested = []
        for m in present:
            hits = measure_callsites(m.name, prod)
            sites += sum(hits.values())
            crates |= set(hits)
            if measure_characterization(m.name, inline_tests, tests_texts,
                                        lines) == 0:
                untested.append(m.name)
        helpers = set().union(*(m.body_calls for m in present)) if present else set()
        outside = sorted(
            h for h in helpers
            if h in by_name and by_name[h].name not in names
        )
        ready = bool(present) and not untested and not outside
        rows.append(
            {
                "group": group, "methods": len(present), "loc": loc,
                "crates": len(crates), "sites": sites,
                "untested": untested, "shared": outside, "ready": ready,
            }
        )

    total_loc = len(lines)
    md = [
        f"# SEAM — WorkspaceSession (medicion, no opinion)",
        "",
        f"- Herramienta: `python3 scripts/measure_workspace_session.py --write`",
        f"- HEAD medido: `{head_sha()}` — fichero: `{TARGET.relative_to(REPO_ROOT)}` ({total_loc} lineas)",
        f"- Regions `#[cfg(test)]` inline: {n_inline_tests}; ficheros `tests/` leidos: {len(tests_texts)}",
        "",
        "La regla de extraccion (CGH-010 AC#2): un grupo solo se extrae",
        "cuando este instrumento lo marca READY: cada metodo publico tiene",
        "al menos un punto de caracterizacion y sus ayudantes privados no",
        "estan compartidos con metodos de fuera. Lo demas es opinion.",
        "",
        "| grupo | metodos | LOC | crates consumidores | puntos de llamada | sin caracterizar | ayudantes compartidos | veredicto |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for r in rows:
        md.append(
            f"| {r['group']} | {r['methods']} | {r['loc']} | {r['crates']} "
            f"| {r['sites']} | {', '.join(r['untested']) or '—'} "
            f"| {', '.join(r['shared']) or '—'} "
            f"| {'READY' if r['ready'] else 'NO EXTRAER AUN'} |"
        )
    if unassigned:
        md += [
            "",
            "## Metodos publicos sin grupo",
            "",
            "Estos metodos existen en la fachada y no pertenecen a ningun",
            "grupo declarado: la medicion los considera deudas del mapa y",
            "falla (--check sale rojo) hasta que se les declare uno.",
            "",
        ] + [f"- `{n}`" for n in unassigned]

    ready_groups = [r["group"] for r in rows if r["ready"]]
    md += [
        "",
        "## Lectura",
        "",
        f"- Grupos READY para extraer hoy: {', '.join(ready_groups) or 'ninguno'}.",
        f"- Resto: sin caracterizacion suficiente o con ayudantes compartidos",
        "  que hay que mover primero; extraerlos ahora romperiamos la",
        "  fachada publica con un cambio que ningun test notaria.",
        "",
    ]
    return "\n".join(md), problems


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--write", action="store_true", help=f"reescribe {DOC}")
    ap.add_argument("--check", action="store_true", help="falla si el doc no coincide")
    args = ap.parse_args()

    md, problems = build_report()
    md = md.rstrip() + "\n"

    if args.check:
        if not DOC.is_file():
            print(f"FAIL: falta {DOC}", file=sys.stderr)
            return 1
        current = DOC.read_text(encoding="utf8")
        # The HEAD line changes with every commit; compare everything else.
        strip = lambda t: [l for l in t.rstrip().splitlines()
                           if not l.startswith("- HEAD medido")]
        if strip(current) != strip(md):
            print(f"FAIL: {DOC} no coincide con la medicion actual", file=sys.stderr)
            return 1
        if problems:
            print(f"FAIL: la medicion tiene {problems} problema(s)", file=sys.stderr)
            return 1
        print(f"OK: {DOC} coincide con la medicion ({head_sha()})")
        return 0

    if args.write:
        DOC.write_text(md + "\n", encoding="utf8")
        print(f"escrito: {DOC.relative_to(REPO_ROOT)}", file=sys.stderr)
    else:
        print(md)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
