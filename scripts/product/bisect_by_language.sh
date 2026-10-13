#!/usr/bin/env bash
# Bisect the build_lightweight_index SIGABRT by language.
#
# The abort is glibc "corrupted size vs. prev_size" inside malloc on a tokio
# worker. Single-language dirs and per-directory runs both passed, so the trigger
# is a *combination*. This builds one directory per language holding only that
# language's files from the real repo, then calls the tool per directory.
#
# Usage: bisect_by_language.sh
set -uo pipefail

BIN=/var/home/rubentxu/cargo-targets/release/cognicode-mcp
WS=/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode
SCRATCH="${JCODE_SCRATCH_DIR:-/home/rubentxu/.jcode/scratch}/langbisect-$(date +%s)"
mkdir -p "$SCRATCH"
cd "$WS"

probe() {  # $1 = directory
    python3 - "$1" <<'PY'
import json, subprocess, sys, os
BIN = "/var/home/rubentxu/cargo-targets/release/cognicode-mcp"
WS = "/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode"
INIT = {"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                   "clientInfo": {"name": "bisect", "version": "0"}}}
p = subprocess.Popen([BIN], cwd=WS, stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                     text=True, bufsize=1)
try:
    p.stdin.write(json.dumps(INIT) + "\n"); p.stdin.flush(); p.stdout.readline()
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                              "params": {"name": "build_lightweight_index",
                                         "arguments": {"directory": sys.argv[1]}}}) + "\n")
    p.stdin.flush()
except BrokenPipeError:
    print("ABORT(preexec)"); sys.exit(1)
line = p.stdout.readline()
p.stdin.close()
try:
    rc = p.wait(timeout=900)
except subprocess.TimeoutExpired:
    p.kill(); rc = "timeout"
if line:
    print(f"ok files={len(os.listdir(sys.argv[1]))}")
else:
    sig = f"signal {-rc}" if isinstance(rc, int) and rc < 0 else f"exit {rc}"
    print(f"ABORT files={len(os.listdir(sys.argv[1]))} {sig}")
PY
}

# Every extension the indexer recognises, per the Language enum's file mapping.
EXTS=$(find . -type f -not -path './target/*' -not -path './.git/*' \
        -not -path './.jcode/*' -not -path './node_modules/*' \
        | sed -n 's/.*\.\([A-Za-z0-9_]*\)$/\1/p' | sort -u)

echo "=== per-extension isolation ==="
while read -r ext; do
    [ -z "$ext" ] && continue
    d="$SCRATCH/ext_$ext"
    mkdir -p "$d"
    # Cap per extension so a huge language cannot dominate runtime.
    find . -type f -name "*.$ext" -not -path './target/*' -not -path './.git/*' \
         -not -path './.jcode/*' -not -path './node_modules/*' -size -2M 2>/dev/null \
        | head -300 | while read -r f; do
        base=$(printf '%s' "$f" | tr '/' '_')
        cp "$f" "$d/$base" 2>/dev/null
    done
    n=$(ls "$d" | wc -l)
    [ "$n" -eq 0 ] && { rm -rf "$d"; continue; }
    printf '%-10s %4s  ' "$ext" "$n"
    probe "$d"
done <<< "$EXTS"

echo
echo "SCRATCH=$SCRATCH"
