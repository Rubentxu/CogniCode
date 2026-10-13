#!/usr/bin/env python3
"""Call every advertised tool in a fresh process each, to find which one kills
the server. One process per tool: the point is to attribute the death, so a
death must not contaminate the next measurement.
"""

import json
import subprocess
import sys

BIN = "/var/home/rubentxu/cargo-targets/release/cognicode-mcp"
WS = "/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode"

INIT = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "sweep", "version": "0"},
    },
}


def discover() -> list[str]:
    proc = subprocess.Popen(
        [BIN], cwd=WS, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL, text=True, bufsize=1,
    )
    names: list[str] = []
    n = 1
    cursor = None
    proc.stdin.write(json.dumps(INIT) + "\n")
    proc.stdin.flush()
    proc.stdout.readline()
    while True:
        n += 1
        params = {} if cursor is None else {"cursor": cursor}
        proc.stdin.write(
            json.dumps({"jsonrpc": "2.0", "id": n, "method": "tools/list", "params": params}) + "\n"
        )
        proc.stdin.flush()
        res = json.loads(proc.stdout.readline())["result"]
        names.extend(t["name"] for t in res.get("tools", []))
        cursor = res.get("nextCursor")
        if not cursor:
            break
    proc.stdin.close()
    proc.wait(timeout=30)
    return names


def probe(name: str) -> tuple[str, str]:
    """Hold the request open until answered, then shut down cleanly.

    `communicate(payload)` closes stdin the instant the payload is written. The
    server sees EOF mid-tool and exits, so a long tool looks like a silent death
    (exit 0). A real client keeps the session open, so this reads the response
    first and only then closes stdin.
    """
    proc = subprocess.Popen(
        [BIN], cwd=WS, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL, text=True, bufsize=1,
    )
    try:
        proc.stdin.write(json.dumps(INIT) + "\n")
        proc.stdin.flush()
        proc.stdout.readline()  # initialize result

        call = {
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {"name": name, "arguments": {}},
        }
        proc.stdin.write(json.dumps(call) + "\n")
        proc.stdin.flush()
    except BrokenPipeError:
        rc = proc.poll()
        return "DIED", f"server died before accepting the call, poll={rc}"

    line = proc.stdout.readline()
    proc.stdin.close()
    try:
        proc.wait(timeout=30)
    except subprocess.TimeoutExpired:
        proc.kill()

    if not line:
        rc = proc.returncode
        sig = f"signal {-rc}" if rc is not None and rc < 0 else f"exit {rc}"
        return "DIED", f"no JSON-RPC response; process {sig}"

    msg = json.loads(line)
    if "error" in msg:
        return "REJECTED", json.dumps(msg["error"])[:140]
    res = msg.get("result", {})
    if res.get("isError"):
        return "ISERROR", str(res.get("content"))[:120]
    return "OK", ""


def main() -> int:
    names = discover()
    print(f"discovered {len(names)} tools\n")
    tally: dict[str, list[str]] = {}
    for name in names:
        status, detail = probe(name)
        tally.setdefault(status, []).append(name)
        mark = "   " if status == "OK" else "!! "
        print(f"{mark}{name:<34} {status}{('  ' + detail) if detail else ''}", flush=True)

    print("\n===== summary =====")
    for status, group in sorted(tally.items()):
        print(f"{status}: {len(group)}")
        if status != "OK":
            for n in group:
                print(f"    {n}")
    return 1 if "DIED" in tally else 0


if __name__ == "__main__":
    sys.exit(main())
