#!/usr/bin/env python3
"""Structural verifier for VDH acceptance receipts; does NOT prove execution.

Usage: python3 test-harness/tools/acceptance_receipt.py receipt.json --root .
A PASS receipt also needs real CI logs and source/artifact hashes verifiable
outside this program. No files are deleted or modified.
"""
from __future__ import annotations
import argparse
import json
import re
import sys
from pathlib import Path

SHA = re.compile(r'^[a-fA-F0-9]{40}$')
DIGEST = re.compile(r'^[a-fA-F0-9]{64}$')
KINDS = ('UAT', 'E2E', 'AAT')


def verify(data: object, root: Path) -> list[str]:
    faults: list[str] = []
    if not isinstance(data, dict):
        return ['receipt is not a JSON object']
    block = data.get('block')
    if block not in ('B0','B1','B2','B3','B4'):
        faults.append('block must be B0..B4')
        return faults
    sha = data.get('source_sha','')
    if not isinstance(sha,str) or not SHA.fullmatch(sha):
        faults.append('source_sha must be full 40-hex commit SHA')
    cases = data.get('cases')
    if not isinstance(cases,list):
        faults.append('cases must be a list')
        return faults
    names = [x.get('id') for x in cases if isinstance(x,dict)]
    expected = {f'{block}-{kind}-01' for kind in KINDS}
    if len(cases)!=3 or set(names)!=expected or len(set(names))!=3:
        faults.append(f'exactly one of each is required: {sorted(expected)}')
    for item in cases:
        if not isinstance(item,dict):
            faults.append('case is not an object')
            continue
        cid = item.get('id','<unknown>')
        if item.get('status')!='PASS' or item.get('exit_code')!=0:
            faults.append(f'{cid}: PASS requires exit_code=0')
        cmd = item.get('command')
        if not isinstance(cmd,str) or not cmd.strip():
            faults.append(f'{cid}: missing execution command')
        dur = item.get('duration_ms')
        if not isinstance(dur,int) or dur<0:
            faults.append(f'{cid}: duration_ms must be nonnegative integer')
        for name in ('scenario_sha256','artifact_sha256'):
            value = item.get(name,'')
            if not isinstance(value,str) or not DIGEST.fullmatch(value):
                faults.append(f'{cid}: {name} must be SHA256 hex')
        contract=item.get('permanent_contract')
        if not isinstance(contract,str) or not contract or Path(contract).is_absolute() or '..' in Path(contract).parts:
            faults.append(f'{cid}: unsafe/missing permanent_contract path')
        elif not (root / contract).is_file():
            faults.append(f'{cid}: permanent contract does not exist: {contract}')
        if cid.endswith('-AAT-01') and not str(item.get('negative_proof','')).strip():
            faults.append(f'{cid}: negative_proof required; mutation must be observed')
        ephemeral=item.get('ephemeral_path','')
        if not isinstance(ephemeral,str) or not ephemeral:
            faults.append(f'{cid}: ephemeral_path missing')
        else:
            path=Path(ephemeral)
            if path.is_absolute() or '..' in path.parts:
                faults.append(f'{cid}: unsafe ephemeral_path')
            elif '__vdh_cycle_' not in path.name and not ephemeral.startswith('test-harness/.cycles/'):
                faults.append(f'{cid}: ephemeral_path must be a dedicated cycle-owned file')
            elif (root / path).exists():
                faults.append(f'{cid}: ephemeral still present: {ephemeral}')
    return faults


def main() -> int:
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('receipt',type=Path)
    p.add_argument('--root',type=Path,default=Path('.'))
    args=p.parse_args()
    try:
        data=json.loads(args.receipt.read_text())
    except (ValueError,OSError) as err:
        print(f'FAIL: cannot read receipt: {err}',file=sys.stderr)
        return 2
    failures=verify(data,args.root.resolve())
    for error in failures:
        print('FAIL: '+error)
    print('STRUCTURAL PASS (not proof of runtime execution)' if not failures
          else f'FAIL: {len(failures)} receipt defects')
    return 0 if not failures else 1

if __name__=='__main__':
    raise SystemExit(main())
