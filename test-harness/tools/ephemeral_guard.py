#!/usr/bin/env python3
"""Fail closed when cycle-only acceptance tests could be merged.

Read-only, no deletion. Does not mutate Git or any user files.
Tests with conventional Rust Cargo placement use __vdh_cycle_<id>_*.rs.
Other cycle artifacts live in test-harness/.cycles/<cycle-id>/.
"""
from __future__ import annotations
import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

MARK = '__vdh_cycle_'
SEARCH_DIRS = ('crates', 'apps', 'tests', 'sandbox', 'test-harness')
SKIP_DIRS = {'.git', 'target', 'node_modules', '.venv', 'dist', 'build', '.cache', 'playwright-report'}


def pending_files(root: Path) -> list[str]:
    """Pure-ish collection from filesystem; returns only relative paths, never edits."""
    found: set[str] = set()
    for rel in SEARCH_DIRS:
        base = root / rel
        if not base.exists():
            continue
        for directory, dirs, files in os.walk(base, topdown=True, followlinks=False):
            dirs[:] = [d for d in dirs if d not in SKIP_DIRS and not (Path(directory) / d).is_symlink()]
            dpath = Path(directory)
            # Workspace-local ephemeral directory: any content is a closure blocker.
            cycle_dir = (root / 'test-harness' / '.cycles')
            if dpath == cycle_dir or cycle_dir in dpath.parents:
                for item in files:
                    found.add(str((dpath / item).relative_to(root)))
                for item in dirs:
                    found.add(str((dpath / item).relative_to(root)) + '/')
                dirs.clear()
                continue
            for fn in files:
                if MARK in fn:
                    found.add(str((dpath / fn).relative_to(root)))
    # Catch tracked test names not currently present in working tree.
    try:
        result = subprocess.run(['git', '-C', str(root), 'ls-files', '-z'],
                                capture_output=True, check=False)
        if result.returncode == 0:
            for raw in result.stdout.split(b'\0'):
                if not raw:
                    continue
                f = os.fsdecode(raw)
                if MARK in f or f.startswith('test-harness/.cycles/'):
                    found.add(f)
    except FileNotFoundError:
        pass
    return sorted(found)


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=Path, default=Path('.'))
    p.add_argument('--phase', choices=('develop', 'merge'), default='merge')
    p.add_argument('--json', action='store_true', help='machine-readable output')
    args = p.parse_args()
    root = args.root.resolve()
    if not (root / 'test-harness').is_dir():
        print(f'ERROR: not a VDH workspace: {root}', file=sys.stderr)
        return 2
    paths = pending_files(root)
    status = 'FAIL' if args.phase == 'merge' and paths else 'PASS'
    report = {'phase': args.phase, 'status': status, 'ephemeral_remaining': len(paths),
              'paths': paths, 'rule': 'no cycle-only acceptance artifacts on merge'}
    if args.json:
        print(json.dumps(report, sort_keys=True, ensure_ascii=False))
    else:
        print(f'{status} [VDH ephemeral guard] {len(paths)} cycle-only paths remaining')
        for f in paths:
            print(f'  {f}')
    return 1 if status == 'FAIL' else 0

if __name__ == '__main__':
    raise SystemExit(main())
