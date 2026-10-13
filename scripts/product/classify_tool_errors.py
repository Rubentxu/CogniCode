#!/usr/bin/env python3
"""Classify every ISERROR from the dispatch sweep into honest buckets.

`sweep_tool_dispatch.py` reports raw `isError` text. That conflates four
different situations, and only two of them are defects:

  VALIDATION  the tool has an arm; empty args fail input validation. Correct.
  MISSING_ARM no arm; falls through to ToolNotFound. Defect: advertised but
              uncallable.
  CONTEXT     has an arm, but the run lacks a precondition (no graph, no index,
              feature disabled). Correct and informative.
  UNKNOWN     anything else, left visible rather than assumed benign.

Only MISSING_ARM justifies a code change; the rest are the honest answer for a
tool called with no arguments.
"""

import json
import re
import subprocess
import sys

BIN = "/var/home/rubentxu/cargo-targets/release/cognicode-mcp"
WS = "/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode"
INIT = {
    "jsonrpc": "2.0", "id": 1, "method": "initialize",
    "params": {"protocolVersion": "2024-11-05", "capabilities": {},
               "clientInfo": {"name": "classify", "version": "0"}},
}


def discover() -> list[str]:
    proc = subprocess.Popen([BIN], cwd=WS, stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                            text=True, bufsize=1)
    names, n, cursor = [], 1, None
    proc.stdin.write(json.dumps(INIT) + "\n"); proc.stdin.flush()
    proc.stdout.readline()
    while True:
        n += 1
        p = {} if cursor is None else {"cursor": cursor}
        proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": n,
                                     "method": "tools/list", "params": p}) + "\n")
        proc.stdin.flush()
        res = json.loads(proc.stdout.readline())["result"]
        names.extend(t["name"] for t in res.get("tools", []))
        cursor = res.get("nextCursor")
        if not cursor:
            break
    proc.stdin.close(); proc.wait(timeout=30)
    return names


def error_text(name: str) -> tuple[str, str]:
    """Return (status, detail) where status in OK/ISERROR/REJECTED/DIED."""
    proc = subprocess.Popen([BIN], cwd=WS, stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                            text=True, bufsize=1)
    try:
        proc.stdin.write(json.dumps(INIT) + "\n"); proc.stdin.flush()
        proc.stdout.readline()
        proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                                     "params": {"name": name, "arguments": {}}}) + "\n")
        proc.stdin.flush()
    except BrokenPipeError:
        return "DIED", "server died before accepting the call"
    line = proc.stdout.readline()
    proc.stdin.close()
    try:
        proc.wait(timeout=120)
    except subprocess.TimeoutExpired:
        proc.kill()
    if not line:
        rc = proc.returncode
        return "DIED", f"no response, {'signal ' + str(-rc) if rc and rc < 0 else 'exit ' + str(rc)}"
    msg = json.loads(line)
    if "error" in msg:
        return "REJECTED", json.dumps(msg["error"])[:200]
    res = msg.get("result", {})
    if not res.get("isError"):
        return "OK", ""
    parts = res.get("content", [])
    text = " ".join(p.get("text", "") for p in parts if isinstance(p, dict))
    return "ISERROR", text[:300]


def classify(status: str, detail: str) -> str:
    if status == "OK":
        return "OK"
    if status == "DIED":
        return "CRASH"
    if status == "REJECTED":
        return "REJECTED"
    if "tool not found" in detail:
        return "MISSING_ARM"
    if re.search(r"missing field `|invalid input", detail, re.I):
        return "VALIDATION"
    if re.search(r"No graph available|no index|not initialized|feature", detail, re.I):
        return "CONTEXT"
    return "UNKNOWN"


def main() -> int:
    names = discover()
    buckets: dict[str, list[str]] = {}
    print(f"{len(names)} advertised tools, each called with empty arguments\n")
    for name in names:
        status, detail = error_text(name)
        bucket = classify(status, detail)
        buckets.setdefault(bucket, []).append(f"{name}: {detail[:110]}")
        print(f"  {bucket:<12} {name}", flush=True)

    print("\n===== buckets =====")
    for b in ("OK", "VALIDATION", "CONTEXT", "MISSING_ARM", "CRASH", "REJECTED", "UNKNOWN"):
        if b in buckets:
            print(f"\n{b} ({len(buckets[b])}):")
            for item in buckets[b]:
                print(f"    {item}")

    print("\n===== verdict =====")
    defects = buckets.get("MISSING_ARM", []) + buckets.get("CRASH", []) + buckets.get("UNKNOWN", [])
    if defects:
        print(f"{len(defects)} defect(s): advertised-but-uncallable or crashing")
        return 1
    print("every advertised tool either dispatches or fails honestly")
    return 0


if __name__ == "__main__":
    sys.exit(main())
