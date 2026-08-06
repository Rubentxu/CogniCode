#!/usr/bin/env bash
# Paginated probe of MCP tools/list — collects all tools via base64(offset) cursor.
# Uses stdio JSON-RPC (the MCP binary speaks MCP over stdin/stdout).
# Exit: 0 with canonical JSON on stdout; 2 on error.
set -euo pipefail

MCP_BIN="${COGNICODE_MCP_BINARY:-./target/release/cognicode-mcp}"

if [ ! -x "$MCP_BIN" ]; then
    echo "ERROR: MCP binary not found or not executable: $MCP_BIN" >&2
    exit 2
fi

python3 - "$MCP_BIN" <<'PYEOF'
import json, subprocess, sys, select, time, base64

mcp_bin = sys.argv[1]
p = subprocess.Popen([mcp_bin], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)

def send(obj):
    p.stdin.write(json.dumps(obj) + "\n")
    p.stdin.flush()

def read_until_id(target, timeout=10):
    msgs = []
    end = time.time() + timeout
    while time.time() < end:
        r, _, _ = select.select([p.stdout], [], [], 0.2)
        if r:
            line = p.stdout.readline()
            if not line:
                break
            try:
                d = json.loads(line)
                msgs.append(d)
                if d.get("id") == target:
                    return msgs
            except json.JSONDecodeError:
                pass
        else:
            if msgs:
                return msgs
    return msgs

send({"jsonrpc": "2.0", "id": 1, "method": "initialize",
      "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                 "clientInfo": {"name": "list_mcp_tools", "version": "1.0"}}})
read_until_id(1)
send({"jsonrpc": "2.0", "method": "notifications/initialized"})
time.sleep(0.2)

all_tools = []
offset = 0
pages = 0
while pages < 20:
    params = {"cursor": base64.b64encode(str(offset).encode()).decode()} if offset else {}
    send({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": params})
    result = None
    for d in read_until_id(2):
        if d.get("id") == 2 and "result" in d:
            result = d["result"]
    if result is None:
        print("ERROR: no tools/list response", file=sys.stderr)
        sys.exit(2)
    page_tools = result.get("tools", [])
    all_tools.extend({"name": t["name"], "description": t.get("description", "")} for t in page_tools)
    pages += 1
    nc = result.get("nextCursor")
    if not nc:
        break
    offset = int(base64.b64decode(nc))
p.terminate()

# Canonical output: sorted list of {name, description}
canonical = sorted(all_tools, key=lambda t: t["name"])
print(json.dumps({"tools": canonical, "total": len(canonical),
                  "pagination": {"page_size": 20, "pages_consumed": pages}}, indent=2))
PYEOF
