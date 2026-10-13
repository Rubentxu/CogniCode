#!/usr/bin/env python3
"""Drive the real release cognicode-mcp binary the way an MCP client does.

Speaks raw JSON-RPC over stdio to the actual built binary. No imports from the
project, no shared types, no fixture: this is the same path an agent takes when
it launches the server.
"""

from __future__ import annotations

import json
import select
import subprocess
import sys
import time

BIN = "/var/home/rubentxu/cargo-targets/release/cognicode-mcp"
WS = "/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode"


def _read_with_deadline(proc: subprocess.Popen, deadline: float) -> str | None:
    """Read one line, or return None once `deadline` passes.

    `readline()` on a pipe has no timeout of its own. Selecting on the raw fd
    lets the deadline be honoured without a reader thread, so a slow tool
    surfaces as a timeout rather than a hang.
    """
    assert proc.stdout is not None
    fd = proc.stdout.fileno()
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return None
        ready, _, _ = select.select([fd], [], [], min(remaining, 1.0))
        if not ready:
            if proc.poll() is not None:
                return ""  # process exited; treat as end of stream
            continue
        line = proc.stdout.readline()
        return line or ""


class Client:
    def __init__(self, cwd: str) -> None:
        self.proc = subprocess.Popen(
            [BIN],
            cwd=cwd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self.n = 0
        self._send(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "acceptance-probe", "version": "0"},
            },
        )

    def _send(self, method: str, params: dict, timeout: int = 60) -> dict:
        self.n += 1
        req = {
            "jsonrpc": "2.0",
            "id": self.n,
            "method": method,
            "params": params,
        }
        assert self.proc.stdin is not None
        self.proc.stdin.write(json.dumps(req) + "\n")
        self.proc.stdin.flush()

        # Read with a deadline. A blanket readline() blocks forever if a tool
        # takes minutes -- `build_graph` over this repo takes ~37s -- which
        # looks exactly like a crashed server. The timeout distinguishes the two.
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            line = _read_with_deadline(self.proc, deadline)
            if line is None:
                break
            line = line.strip()
            if not line:
                continue
            msg = json.loads(line)
            if msg.get("id") == self.n:
                return msg
        raise TimeoutError(
            f"no response to {method} within {timeout}s (pid {self.proc.pid} "
            f"alive={self.proc.poll() is None})"
        )

    def list_all_pages(self) -> list[dict]:
        """Follow nextCursor to completion, as a real client must."""
        tools: list[dict] = []
        cursor = None
        pages = 0
        seen_cursors: set[str] = set()
        while True:
            pages += 1
            params = {} if cursor is None else {"cursor": cursor}
            result = self._send("tools/list", params)["result"]
            tools.extend(result.get("tools", []))
            cursor = result.get("nextCursor")
            if not cursor:
                break
            assert cursor not in seen_cursors, f"cursor loop detected: {cursor}"
            seen_cursors.add(cursor)
            assert pages < 50, "pagination did not terminate"
        return tools

    def call(self, name: str, args: dict, timeout: int = 60) -> dict:
        return self._send("tools/call", {"name": name, "arguments": args}, timeout=timeout)

    def close(self) -> None:
        try:
            if self.proc.stdin:
                self.proc.stdin.close()
            self.proc.wait(timeout=10)
        except Exception:
            self.proc.kill()


def main() -> int:
    failures: list[str] = []

    def check(cond: bool, label: str, detail: str = "") -> None:
        if cond:
            print(f"  PASS  {label}")
        else:
            print(f"  FAIL  {label}  {detail}")
            failures.append(label)

    c = Client(WS)
    tools = c.list_all_pages()
    names = [t["name"] for t in tools]
    print(f"paginated tools/list yielded {len(tools)} tools across all pages\n")

    print("-- advertised surface --")
    check("cogh" not in names, "phantom cogh tool is not advertised")
    for gated in ("list_evidence", "search_evidence", "interproc_summary"):
        check(gated not in names, f"feature-gated {gated} is correctly absent")

    catalog = json.loads(open(f"{WS}/product/tools.json").read())
    catalog_names = {t["name"] for t in catalog["tools"]}
    check(
        set(names) == catalog_names,
        "live tools/list matches product/tools.json exactly",
        f"live_only={sorted(set(names) - catalog_names)} catalog_only={sorted(catalog_names - set(names))}",
    )
    check(len(names) == len(set(names)), "no duplicate tool names advertised")

    print("\n-- wire metadata actually reaches the client --")
    by_name = {t["name"]: t for t in tools}
    meta = lambda n: by_name[n].get("_meta", {}).get("cognicode", {})  # noqa: E731
    check(
        meta("get_file_symbols").get("precision_floor") == "lsp",
        "precision_floor=lsp reaches the wire for get_file_symbols",
        f"got {meta('get_file_symbols').get('precision_floor')!r}",
    )
    check(
        meta("query_symbol_index").get("intent_tags") == ["Locate"],
        "intent_tags=[Locate] reaches the wire for query_symbol_index",
        f"got {meta('query_symbol_index').get('intent_tags')!r}",
    )
    check(
        meta("safe_refactor_apply").get("mutates_workspace") is True,
        "safe_refactor_apply is flagged mutates_workspace",
    )

    print("\n-- every advertised tool actually dispatches --")
    undispatchable = []
    for name in names:
        try:
            resp = c.call(name, {}, timeout=180)
        except TimeoutError as exc:
            undispatchable.append((name, f"TIMEOUT: {exc}"))
            continue
        if "error" in resp:
            undispatchable.append((name, str(resp["error"])[:90]))
    check(
        not undispatchable,
        f"all {len(names)} advertised tools accept a tools/call",
        f"rejected={undispatchable[:5]}",
    )

    print("\n-- error paths a real client can hit --")
    resp = c.call("no_such_tool_at_all", {})
    check("error" in resp, "unknown tool returns a JSON-RPC error", str(resp)[:120])
    resp = c.call("read_file", {})
    is_err = resp.get("result", {}).get("isError")
    check(bool(is_err), "missing required arg returns isError, not success", str(resp)[:160])

    print("\n-- write authority on the real wire (AAT-B5-03) --")
    for tool in ("safe_refactor_preview", "safe_refactor"):
        if tool not in by_name:
            print(f"  SKIP  {tool} not advertised in this build")
            continue
        resp = c.call(
            tool,
            {"action": "rename", "target": "foo", "authorization_token": "tk-evil"},
        )
        is_err = resp.get("result", {}).get("isError")
        check(
            bool(is_err),
            f"{tool} rejects a smuggled authorization_token",
            f"isError={is_err} resp={str(resp)[:200]}",
        )
        resp = c.call(tool, {"action": "rename", "target": "foo"})
        check(
            not resp.get("result", {}).get("isError"),
            f"{tool} still accepts the legitimate input",
            str(resp)[:160],
        )

    c.close()

    print()
    if failures:
        print(f"{len(failures)} FAILURE(S): {failures}")
        return 1
    print("all live MCP acceptance checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
