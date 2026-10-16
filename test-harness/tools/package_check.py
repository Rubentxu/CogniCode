#!/usr/bin/env python3
"""Validate the archive overlay + inventory without executing app tests.

Called from a real repository root after unzip; --overlay-only checks package
internals without requiring the original CogniCode checkout.
"""
from __future__ import annotations
import argparse
import csv
import pathlib
import sys
from collections import Counter

INITIATIVE = 'docs/roadmap/initiatives/VDH-2026-10/'
CHANGE = 'openspec/changes/2026-10-15-value-delivery-functional-core/'

REQUIRED = [
    INITIATIVE + x for x in ('START-HERE.md', 'ROADMAP-ADDENDUM.md',
        'ACCEPTANCE-MATRIX.md', 'TEST-RETIREMENT.md', 'RELEASE-PROTOCOL.md',
        'AGENT-HANDOFF.md', 'GLOSSARY.md')
] + [
    'docs/adr/ADR-VDH-00'+str(i)+'-'+n+'.md' for i,n in enumerate((
       'truthful-analysis-outcomes', 'functional-core-minimal-ports',
       'real-explorer-acceptance', 'ephemeral-acceptance-retirement'), 1)
] + [CHANGE + x for x in ('proposal.md', 'design.md', 'tasks.md')] + [
    CHANGE + f'specs/{x}/spec.md' for x in
      ('truthful-analysis', 'functional-boundaries', 'real-user-vertical',
       'ephemeral-acceptance', 'release-consumer')
] + ['test-harness/inventory/test_inventory.csv', 'test-harness/tools/ephemeral_guard.py',
     'test-harness/examples/analysis_outcome.rs']

EXISTING = ['Cargo.toml', 'AGENTS.md', 'docs/roadmap/ROADMAP.md',
            'docs/roadmap/production-ready/EXECUTION-PLAN.md',
            'docs/adr/ADR-012-ui-visible-capability-contract.md',
            'docs/prf/specs/SPEC-DISTRIBUTION.md',
            'release.pipeline.kts', 'crates/cognicode-explorer/src/moldql/compile.rs']


def inspect(root: pathlib.Path, overlay_only: bool) -> tuple[list[str], dict]:
    errors = []
    for path in REQUIRED + ([] if overlay_only else EXISTING):
        if not (root / path).is_file():
            errors.append(f'missing: {path}')
    inventory = root / 'test-harness/inventory/test_inventory.csv'
    counts = Counter()
    if inventory.is_file():
        with inventory.open(newline='', encoding='utf-8') as f:
            rows = list(csv.DictReader(f))
        counts.update(row['group'] for row in rows)
        expected = {'A':47, 'B':41, 'C':4, 'D':14, 'E':19}
        if dict(counts) != expected:
            errors.append(f'inventory count drift: {dict(counts)} != {expected}')
        for row in rows:
            source = row['path']
            if source.startswith('/') or '..' in pathlib.PurePosixPath(source).parts:
                errors.append(f'unsafe source path in inventory: {source}')
            if not row['source_url'].endswith(source):
                errors.append(f'source URL / path mismatch: {source}')
            if not overlay_only and not (root / source).is_file():
                errors.append(f'source absent at HEAD (reconcile, do NOT invent replacement): {source}')
    return errors, dict(counts)


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=pathlib.Path, default=pathlib.Path('.'))
    p.add_argument('--overlay-only', action='store_true')
    a = p.parse_args()
    errors, counts = inspect(a.root.resolve(), a.overlay_only)
    for x in errors:
        print('FAIL: '+x)
    print('inventory groups:',counts)
    print('PASS: package valid' if not errors else f'FAIL: {len(errors)} issues')
    return 0 if not errors else 1

if __name__ == '__main__':
    raise SystemExit(main())
