#!/usr/bin/env python3
"""openspec_conformance.py — openspec requirement conformance harness.

Counts requirements across openspec/specs/*/spec.md (dual-level headings),
auto-detects legacy-obsolete specs (OBSOLETE banner), supports an optional
--evidence-map for manual verified evidence, and emits a conformance matrix
(YAML + Markdown) with per-requirement status and triage summary.

Status per requirement: verified | legacy_obsolete | no_evidence
pct_verified = verified/(total-legacy_obsolete) ; pct_triaged = (verified+legacy_obsolete)/total
(denominator per ADR-031 §4 renegotiation: legacy-obsolete requirements are
removed from the product and excluded from active verification)
Exit code 0 always (reporting tool) unless --validate-paths is set and
some evidence_path values do not resolve to any file (exit 1).
"""
import argparse
import json
import re
import sys
from pathlib import Path

REQ_RE = re.compile(r"^#{2,3}\s+Requirement(?:\s*:|\s)", re.MULTILINE)
OBSOLETE_RE = re.compile(r"\bOBSOLETE\b", re.IGNORECASE)
PHANTOM_NOTE = " (phantom dir — no spec.md or no Requirement headers)"


def load_evidence_map(path):
    """--evidence-map: YAML list of {spec: {status, evidence_path}} (or JSON)."""
    if not path:
        return {}
    p = Path(path)
    if not p.exists():
        return {}
    text = p.read_text()
    try:
        import yaml
        data = yaml.safe_load(text) or {}
    except Exception:
        try:
            data = json.loads(text)
        except Exception:
            return {}
    return data if isinstance(data, dict) else {}


def validate_paths(evidence_map):
    """--validate-paths: check each evidence_path resolves to ≥1 existing file.

    Specs carrying only ``evidence_note`` (no valid evidence_path) are
    reported as needing evidence — evidence_note is a placeholder, not proof.
    """
    missing = []
    for spec_name, ev in evidence_map.items():
        if not isinstance(ev, dict):
            continue
        if ev.get("status") != "verified":
            continue
        # evidence_note without evidence_path is a placeholder, not verified proof
        has_note = bool(ev.get("evidence_note"))
        path_str = ev.get("evidence_path", "")
        if not path_str:
            reason = f"empty evidence_path (evidence_note: {ev['evidence_note']!r})" if has_note else "empty evidence_path"
            missing.append((spec_name, reason))
            continue
        # Glob expansion: split on commas/spaces for multi-path strings
        tokens = re.split(r"[,\s]+", path_str.strip())
        globs_valid = any(
            len(list(Path(".").glob(tok))) > 0
            for tok in tokens if tok
        )
        if not globs_valid:
            missing.append((spec_name, f"glob resolves empty: {path_str!r}"))
    return missing


def scan_specs(specs_dir, evidence_map):
    specs_dir = Path(specs_dir)
    rows = []
    summary = {"total": 0, "verified": 0, "legacy_obsolete": 0, "no_evidence": 0,
               "specs": 0, "phantom_dirs": 0}
    if not specs_dir.exists():
        return rows, summary
    for spec_dir in sorted(specs_dir.iterdir()):
        if not spec_dir.is_dir():
            continue
        spec_file = spec_dir / "spec.md"
        if not spec_file.exists():
            summary["phantom_dirs"] += 1
            continue
        text = spec_file.read_text(errors="replace")
        matches = list(REQ_RE.finditer(text))
        if not matches:
            summary["phantom_dirs"] += 1
            continue
        # OBSOLETE banner detection is header-scoped (first 8 lines): the real
        # banner lives in the spec title or status block, never in requirement
        # prose. Body mentions (e.g. specs that DOCUMENT the banner) must not
        # mark the spec obsolete.
        header = "\n".join(text.splitlines()[:8])
        is_obsolete = bool(OBSOLETE_RE.search(header))
        spec_name = spec_dir.name
        ev = evidence_map.get(spec_name, {})
        ev_status = ev.get("status") if isinstance(ev, dict) else None
        ev_path = ev.get("evidence_path") if isinstance(ev, dict) else None
        ev_note = ev.get("evidence_note") if isinstance(ev, dict) else None
        summary["specs"] += 1
        for i, m in enumerate(matches, 1):
            rid = f"{spec_name}/{i}"
            # evidence_note without evidence_path is not verified proof
            if ev_status == "legacy_obsolete":
                status = "legacy_obsolete"
            elif ev_status == "verified":
                # verified requires actual evidence_path; evidence_note alone is insufficient
                if ev_note and not ev_path:
                    status = "no_evidence"
                else:
                    status = "verified"
            elif is_obsolete:
                status = "legacy_obsolete"
            else:
                status = "no_evidence"
            rows.append({"id": rid, "spec": spec_name, "index": i,
                         "status": status,
                         "evidence": ev_path if status == "verified" else None,
                         "evidence_note": ev_note if (status == "no_evidence" and ev_note) else None})
            summary[status] += 1
            summary["total"] += 1
    active_total = summary["total"] - summary["legacy_obsolete"]
    summary["pct_verified"] = round(summary["verified"] / active_total * 100, 1) if active_total else 0.0
    summary["pct_triaged"] = round((summary["verified"] + summary["legacy_obsolete"]) / summary["total"] * 100, 1) if summary["total"] else 0.0
    return rows, summary


def emit_yaml(rows, summary, out):
    import yaml
    doc = {"summary": summary, "requirements": rows}
    with open(out, "w") as f:
        yaml.dump(doc, f, default_flow_style=False, sort_keys=False)


def emit_md(rows, summary, out):
    lines = [f"# Openspec Conformance Matrix", "",
             f"Total: **{summary['total']}** requirements across **{summary['specs']}** specs "
             f"({summary['phantom_dirs']} phantom dirs skipped).",
             f"- verified: **{summary['verified']}** ({summary['pct_verified']}%)",
             f"- legacy_obsolete: **{summary['legacy_obsolete']}**",
             f"- no_evidence: **{summary['no_evidence']}**",
             f"- **pct_triaged: {summary['pct_triaged']}%**",
             "",
             "| Requirement | Spec | Status | Evidence |",
             "|-------------|------|--------|----------|"]
    for r in rows:
        lines.append(f"| {r['id']} | {r['spec']} | {r['status']} | {r['evidence'] or '—'} |")
    with open(out, "w") as f:
        f.write("\n".join(lines) + "\n")


def main():
    ap = argparse.ArgumentParser(description="Openspec conformance harness")
    ap.add_argument("--specs-dir", default="openspec/specs", help="Directory with spec dirs")
    ap.add_argument("--evidence-map", default=None, help="YAML/JSON map spec->{status,evidence_path}")
    ap.add_argument("--output-prefix", default="sandbox/reports/conformance_matrix",
                    help="Output prefix for .yaml and .md")
    ap.add_argument("--validate-paths", action="store_true",
                    help="Validate that evidence_path files exist before scanning")
    args = ap.parse_args()
    ev = load_evidence_map(args.evidence_map)
    if args.validate_paths:
        missing = validate_paths(ev)
        if missing:
            for spec_name, reason in missing:
                print(f"VALIDATION ERROR: {spec_name}: {reason}", file=sys.stderr)
            sys.exit(1)
    rows, summary = scan_specs(args.specs_dir, ev)
    out_y = f"{args.output_prefix}.yaml"
    out_m = f"{args.output_prefix}.md"
    Path(out_y).parent.mkdir(parents=True, exist_ok=True)
    emit_yaml(rows, summary, out_y)
    emit_md(rows, summary, out_m)
    print(f"total={summary['total']} specs={summary['specs']} phantom={summary['phantom_dirs']} "
          f"verified={summary['verified']} legacy={summary['legacy_obsolete']} "
          f"no_evidence={summary['no_evidence']} pct_verified={summary['pct_verified']}% "
          f"pct_triaged={summary['pct_triaged']}%")
    print(f"YAML: {out_y}\nMD: {out_m}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
