#!/usr/bin/env python3
"""Check SHA256 of every VDH file against VDH-MANIFEST.json. No mutation."""
from pathlib import Path
import argparse
import hashlib
import json
import sys

def verify(root: Path):
    mf=root/'VDH-MANIFEST.json'
    try:data=json.loads(mf.read_text())
    except (OSError,ValueError) as e:return [f'bad manifest: {e}']
    problems=[]
    for entry in data['files']:
        path=Path(entry['path'])
        if path.is_absolute() or '..' in path.parts:
            problems.append(f'unsafe path: {path}')
            continue
        loc=root/path
        if not loc.is_file():problems.append(f'missing: {path}');continue
        raw=loc.read_bytes()
        if hashlib.sha256(raw).hexdigest()!=entry['sha256']:
            problems.append(f'content changed: {path}')
    return problems

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root',type=Path,default=Path('.'))
    a=parser.parse_args()
    errors=verify(a.root.resolve())
    for e in errors:print('FAIL:',e,file=sys.stderr)
    print('PASS: VDH overlay manifest' if not errors else f'FAIL: {len(errors)}')
    raise SystemExit(1 if errors else 0)
