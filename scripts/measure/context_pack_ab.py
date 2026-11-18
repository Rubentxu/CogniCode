#!/usr/bin/env python3
"""Context pack against the tools an agent uses today, on one real tree.

The question: to find the symbols that answer a request, what does `pack` cost
and what does it return, next to `smart_search` (ranked symbols) and
`codebase_map` (a map of the project)?

What is measured, per tool and per case, over several MCP sessions:

  relevance   `name_hit`: an expected symbol is among the first K returned.
              `file_precision`: the share of the first K returned that sit in
              a file the case expects. K is the same for every tool.
  latency     wall time of one `tools/call` on a warm session (the graph the
              server holds is already built), and the time of the first call
              of a session, reported apart.
  cost        UTF-8 bytes of the text an agent reads (`content[0].text`), of the
              structured answer, and for `pack` of the text its budget bounds.
              Bytes are an upper bound of tokens for the tokenizers in use; they
              are not tokens, and nothing here converts one into the other.
  calls       the calls made to get the answer, and the evidence fields the
              answer carries (a range, a signature, a digest of the file, the
              reason, the snapshot): what is absent needs further calls that
              are NOT executed or counted as made.

Disciplines the numbers obey:
  - One shared host: the report records the load average before and after and
    the cores, and says so. Percentiles are nearest-rank; with fewer than 20
    samples the p90 is close to the maximum, and the report says how many
    samples it has.
  - A call that fails is counted as a failure next to the times and is never
    folded into a number.
  - Nothing is estimated: a field the answer does not carry is `false`.
  - The server binary is whatever `--server` points at. A debug build is far
    slower than a release one; the comparison is valid inside one binary only.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import platform
import re
import subprocess
import sys
import time
from pathlib import Path

K = 10
DEFAULT_WORKSPACE = "crates/cognicode-core/src/application"

# Ground truth, written by hand from the declarations of the tree: what a
# person looking for this would expect to land on. `names` are declared symbol
# names, `files` the files that declare them. A phrase in words that never
# appears in an identifier is a "natural" case: it is where a name-weight ranker
# is expected to be weakest, and it is in the set on purpose.
CASES = [
    {"id": "single-flight", "style": "identifier", "query": "single flight",
     "names": ["SingleFlight"], "files": ["application/single_flight.rs"]},
    {"id": "file-facts-cache", "style": "identifier", "query": "file facts cache",
     "names": ["FileFactsCache"], "files": ["services/file_facts_cache.rs"]},
    {"id": "workspace-session", "style": "identifier", "query": "workspace session",
     "names": ["WorkspaceSession"], "files": ["application/workspace_session.rs"]},
    {"id": "hot-paths", "style": "identifier", "query": "hot paths",
     "names": ["hot_paths"], "files": ["services/graph_queries.rs"]},
    {"id": "usage-search", "style": "identifier", "query": "usage search",
     "names": ["UsageSearch"], "files": ["services/usage_search.rs"]},
    {"id": "context-compressor", "style": "identifier", "query": "context compressor",
     "names": ["ContextCompressorService"], "files": ["services/context_compressor.rs"]},
    {"id": "dedupe-work", "style": "natural", "query": "run identical concurrent work only once",
     "names": ["SingleFlight"], "files": ["application/single_flight.rs"]},
    {"id": "facts-per-file", "style": "natural", "query": "remember what was extracted from each source file",
     "names": ["FileFactsCache"], "files": ["services/file_facts_cache.rs"]},
    {"id": "most-called", "style": "natural", "query": "which symbols are called by the most others",
     "names": ["hot_paths"], "files": ["services/graph_queries.rs"]},
]

# `smart_search_ident` is `smart_search` asked the way an agent that knows the
# repo writes an identifier: the words of the query joined with underscores,
# which is also how `pack` hands the intent to the ranker.
TOOLS = ("smart_search", "smart_search_ident", "pack")

EVIDENCE_FIELDS = {
    "line": ("line", "start_line"),
    "range": ("range",),
    "signature": ("signature",),
    "file_digest": ("source_digest",),
    "reason": ("reason",),
    "unambiguous_id": ("id",),
}


# ---- pure core -------------------------------------------------------------


def percentile(samples: list[float], q: float) -> float | None:
    """Nearest-rank percentile; `None` when there is nothing to rank."""
    if not samples:
        return None
    ordered = sorted(samples)
    rank = max(1, math.ceil(q / 100 * len(ordered)))
    return ordered[min(rank, len(ordered)) - 1]


def spread(samples: list[float]) -> dict:
    """The median and how noisy it is: the p10-p90 band relative to the p50."""
    p50 = percentile(samples, 50)
    if p50 is None:
        return {"n": 0}
    p10, p90 = percentile(samples, 10), percentile(samples, 90)
    return {
        "n": len(samples),
        "p50": round(p50, 2),
        "p10": round(p10, 2),
        "p90": round(p90, 2),
        "min": round(min(samples), 2),
        "max": round(max(samples), 2),
        "noise": round((p90 - p10) / p50, 3) if p50 else None,
    }


def relevance(returned: list[dict], case: dict) -> dict:
    """How well the first K symbols returned match what the case expects."""
    top = returned[:K]
    names = {item.get("name") for item in top}
    in_file = [
        item for item in top if any(str(item.get("file", "")).endswith(f) for f in case["files"])
    ]
    return {
        "returned": len(returned),
        "name_hit": any(name in names for name in case["names"]),
        "file_precision": round(len(in_file) / len(top), 3) if top else 0.0,
    }


def evidence_of(item: dict | None) -> dict:
    """Which evidence fields one returned symbol carries."""
    item = item or {}
    return {name: any(key in item for key in keys) for name, keys in EVIDENCE_FIELDS.items()}


def map_names(text: str, case: dict) -> dict:
    """A map of the project names symbols in prose: the expected ones are
    found by containment, and there is no ranked list to take a precision of."""
    return {
        "returned": None,
        "name_hit": any(re.search(rf"\b{re.escape(n)}\b", text) for n in case["names"]),
        "file_precision": None,
    }


# ---- effects: one MCP session over stdio -----------------------------------


class Session:
    def __init__(self, server: str, workspace: str):
        self.proc = subprocess.Popen(
            [server, "--cwd", workspace, "--profile", "full"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            text=True, bufsize=1,
        )
        self.next_id = 1
        self._rpc("initialize", {
            "protocolVersion": "2025-03-26", "capabilities": {},
            "clientInfo": {"name": "context-pack-ab", "version": "1"},
        })
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def _send(self, message: dict) -> None:
        self.proc.stdin.write(json.dumps(message) + "\n")
        self.proc.stdin.flush()

    def _rpc(self, method: str, params: dict) -> dict:
        ident = self.next_id
        self.next_id += 1
        self._send({"jsonrpc": "2.0", "id": ident, "method": method, "params": params})
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError("the server closed the session")
            message = json.loads(line)
            if message.get("id") == ident:
                return message

    def call(self, tool: str, arguments: dict) -> dict:
        """One tool call: its wall time, the text and structure it answered."""
        started = time.perf_counter()
        reply = self._rpc("tools/call", {"name": tool, "arguments": arguments})
        elapsed_ms = (time.perf_counter() - started) * 1000
        result = reply.get("result") or {}
        failed = "error" in reply or result.get("isError") is True
        content = result.get("content") or [{}]
        text = content[0].get("text", "")
        structured = result.get("structuredContent")
        return {
            "ms": elapsed_ms,
            "failed": failed,
            "text": text,
            "text_bytes": len(text.encode()),
            "structured_bytes": len(json.dumps(structured).encode()) if structured else 0,
        }

    def close(self) -> None:
        self.proc.kill()
        self.proc.wait()


def identifier_of(query: str) -> str:
    """The words of a query as one snake_case identifier."""
    return "_".join(re.findall(r"[A-Za-z0-9]+", query.lower()))


def parsed(text: str) -> dict:
    try:
        value = json.loads(text)
    except ValueError:
        return {}
    return value if isinstance(value, dict) else {}


def measure_case(session: Session, case: dict, max_tokens: int) -> dict:
    """One round of one case over one session."""
    search = session.call("smart_search", {"query": case["query"], "limit": K})
    joined = session.call("smart_search", {"query": identifier_of(case["query"]), "limit": K})
    pack = session.call("pack", {"intent": case["query"], "max_tokens": max_tokens})
    search_items = parsed(search["text"]).get("results") or []
    joined_items = parsed(joined["text"]).get("results") or []
    pack_answer = parsed(pack["text"])
    pack_items = pack_answer.get("items") or []
    return {
        "smart_search": {
            **{k: search[k] for k in ("ms", "failed", "text_bytes", "structured_bytes")},
            **relevance(search_items, case),
            "evidence": evidence_of(search_items[0] if search_items else None),
        },
        "smart_search_ident": {
            **{k: joined[k] for k in ("ms", "failed", "text_bytes", "structured_bytes")},
            **relevance(joined_items, case),
            "evidence": evidence_of(joined_items[0] if joined_items else None),
        },
        "pack": {
            **{k: pack[k] for k in ("ms", "failed", "text_bytes", "structured_bytes")},
            **relevance(pack_items, case),
            "evidence": evidence_of(pack_items[0] if pack_items else None),
            "budget": pack_answer.get("budget"),
            "status": pack_answer.get("status"),
        },
    }


def measure_session(server: str, workspace: str, rounds: int, max_tokens: int) -> dict:
    session = Session(server, workspace)
    try:
        first = session.call("smart_search", {"query": "warm up", "limit": 1})
        first_pack = session.call("pack", {"intent": "warm up", "max_tokens": max_tokens})
        maps = {}
        for fmt in ("compact", "detailed"):
            maps[fmt] = session.call("codebase_map", {"format": fmt})
        rounds_data = []
        for _ in range(rounds):
            round_data = {case["id"]: measure_case(session, case, max_tokens) for case in CASES}
            for fmt in ("compact", "detailed"):
                reply = session.call("codebase_map", {"format": fmt})
                round_data[f"codebase_map:{fmt}"] = {
                    **{k: reply[k] for k in ("ms", "failed", "text_bytes", "structured_bytes")},
                    "hits": {c["id"]: map_names(reply["text"], c) for c in CASES},
                }
            rounds_data.append(round_data)
        return {
            "first_call_ms": {"smart_search": first["ms"], "pack": first_pack["ms"]},
            "rounds": rounds_data,
        }
    finally:
        session.close()


# ---- report ----------------------------------------------------------------


def summarise(sessions: list[dict]) -> dict:
    rounds = [r for s in sessions for r in s["rounds"]]
    report: dict = {"samples_per_case": len(rounds), "cases": {}, "codebase_map": {}}
    for case in CASES:
        entry = {"style": case["style"], "query": case["query"]}
        for tool in TOOLS:
            runs = [r[case["id"]][tool] for r in rounds]
            ok = [r for r in runs if not r["failed"]]
            entry[tool] = {
                "failed_calls": len(runs) - len(ok),
                "latency_ms": spread([r["ms"] for r in ok]),
                "text_bytes": spread([r["text_bytes"] for r in ok]),
                "structured_bytes": spread([r["structured_bytes"] for r in ok]),
                "returned": spread([r["returned"] for r in ok]),
                "name_hit_rate": round(sum(r["name_hit"] for r in ok) / len(ok), 3) if ok else None,
                "file_precision": spread([r["file_precision"] for r in ok]),
                "evidence": ok[0]["evidence"] if ok else None,
            }
        pack_runs = [r[case["id"]]["pack"] for r in rounds if not r[case["id"]]["pack"]["failed"]]
        if pack_runs:
            entry["pack"]["budget"] = pack_runs[0]["budget"]
            entry["pack"]["status"] = pack_runs[0]["status"]
        report["cases"][case["id"]] = entry
    for fmt in ("compact", "detailed"):
        runs = [r[f"codebase_map:{fmt}"] for r in rounds]
        ok = [r for r in runs if not r["failed"]]
        report["codebase_map"][fmt] = {
            "failed_calls": len(runs) - len(ok),
            "latency_ms": spread([r["ms"] for r in ok]),
            "text_bytes": spread([r["text_bytes"] for r in ok]),
            "structured_bytes": spread([r["structured_bytes"] for r in ok]),
            "name_hit_rate_by_case": {
                c["id"]: round(sum(r["hits"][c["id"]]["name_hit"] for r in ok) / len(ok), 3)
                for c in CASES
            } if ok else None,
        }
    report["first_call_ms"] = {
        tool: spread([s["first_call_ms"][tool] for s in sessions]) for tool in ("smart_search", "pack")
    }
    return report


def print_table(report: dict) -> None:
    print(f"samples per case: {report['samples_per_case']}")
    header = f"{'case':<20}{'tool':<20}{'p50 ms':>9}{'noise':>7}{'text B':>10}{'hit':>6}{'prec':>6}{'n':>4}"
    print(header)
    for case_id, entry in report["cases"].items():
        for tool in TOOLS:
            m = entry[tool]
            lat = m["latency_ms"]
            print(
                f"{case_id:<20}{tool:<20}{lat.get('p50', '-'):>9}{str(lat.get('noise', '-')):>7}"
                f"{m['text_bytes'].get('p50', '-'):>10}{str(m['name_hit_rate']):>6}"
                f"{m['file_precision'].get('p50', '-'):>6}{m['returned'].get('p50', '-'):>4}"
            )
    for fmt, m in report["codebase_map"].items():
        hits = m["name_hit_rate_by_case"] or {}
        print(
            f"codebase_map:{fmt}: p50 {m['latency_ms'].get('p50')} ms, text {m['text_bytes'].get('p50')} B, "
            f"structured {m['structured_bytes'].get('p50')} B, cases whose symbol it names: "
            f"{sum(1 for v in hits.values() if v > 0)}/{len(hits)}"
        )
    print("first call of a session (ms):", json.dumps(report["first_call_ms"]))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--server", required=True, help="path to the cognicode-mcp binary")
    parser.add_argument("--workspace", default=DEFAULT_WORKSPACE, help="the tree to measure over")
    parser.add_argument("--sessions", type=int, default=3)
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--max-tokens", type=int, default=2000)
    parser.add_argument("--out", help="write the full JSON report here")
    args = parser.parse_args()

    workspace = str(Path(args.workspace).resolve())
    host = {
        "cores": os.cpu_count(),
        "platform": platform.platform(),
        "loadavg_before": os.getloadavg(),
        "server": args.server,
        "workspace": workspace,
        "max_tokens": args.max_tokens,
        "k": K,
    }
    sessions = [measure_session(args.server, workspace, args.rounds, args.max_tokens)
                for _ in range(args.sessions)]
    host["loadavg_after"] = os.getloadavg()
    report = {"host": host, **summarise(sessions)}
    print_table(report)
    print("host:", json.dumps(host))
    if args.out:
        Path(args.out).write_text(json.dumps(report, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
