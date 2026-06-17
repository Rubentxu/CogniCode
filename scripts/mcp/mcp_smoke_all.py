#!/usr/bin/env python3
"""Smoke-test CogniCode MCP tools against a real built graph.

Classification:
- OK: returned non-error, non-placeholder output
- STUB: returned placeholder, empty-by-construction, unknown/resource_type, or note-only output
  (STUB from a stable tool is a failure when --fail-on-stable-stub is set)
- STUB_WARN: STUB from an experimental tool (allowed, printed as warning)
- ERROR: runtime/tool-level error
- MISSING: tool not found / listed without handler
- SKIP: intentionally skipped (destructive or needs generated contract)
- GATED_OK: tool has a dispatch arm but returns capability/configuration error at runtime
  (e.g. PostgresRepository not available for graph_diff/graph_timeline)

Stability classification from meta.cognicode.stability (tools/list):
- stable: STUB is a failure (unless --no-fail-on-stable-stub)
- experimental: STUB is allowed (STUB_WARN)
- gated: GATED error response is expected (GATED_OK)

Usage:
    python3 scripts/mcp/mcp_smoke_all.py                    # smoke run (no persist)
    python3 scripts/mcp/mcp_smoke_all.py --persist <path>  # smoke run + persist baseline
    python3 scripts/mcp/mcp_smoke_all.py --help             # show this help

Exit codes:
    0 = smoke passed (no MISSING, no stable STUB when --fail-on-stable-stub)
    1 = smoke failed (≥1 MISSING, transport error, or stable STUB with flag set)
"""

import argparse
import http.client
import json
import socket
import sys
import time
import uuid
from collections import Counter
from pathlib import Path

HOST, PORT = "127.0.0.1", 9847
ROOT = Path("/var/home/rubentxu/Proyectos/rust/CogniCode")
RMCP = str(ROOT / "crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs")

# Tools explicitly skipped in smoke (destructive or require special setup)
SKIP = {
    "write_file": "destructive file write",
    "edit_file": "destructive edit",
    "safe_refactor": "mutating/refactor action; needs dedicated sandbox",
    "validate_contract": "requires contract_id from generated persistent contract",
}

# Fallback GATED set for tools without stability annotation.
# These have dispatch arms but return capability/configuration errors at runtime
# because the required backend (e.g. PostgresRepository) is not available.
# Superseded by meta.cognicode.stability="gated" from tools/list when available.
GATED = {
    "graph_diff": "requires PostgresRepository",
    "graph_timeline": "requires PostgresRepository",
    "generate_contract": "requires rustc/AVC contract infrastructure",
}

# Stability classification thresholds
# stable + STUB → failure (unless --fail-on-stable-stub=false)
# experimental + STUB → warning (allowed)
# gated + GATED → GATED_OK (expected)
STABILITY_STABLE = "stable"
STABILITY_EXPERIMENTAL = "experimental"
STABILITY_GATED = "gated"


def parse_sse(raw):
    events, cur = [], ""
    for line in raw.splitlines():
        if line.startswith("data:"):
            cur += line[5:].strip()
        elif line == "" and cur:
            try:
                events.append(json.loads(cur))
            except json.JSONDecodeError:
                events.append({"raw": cur[:500]})
            cur = ""
    if cur:
        try:
            events.append(json.loads(cur))
        except json.JSONDecodeError:
            events.append({"raw": cur[:500]})
    return events


def mcp(method, params=None, sid=None, timeout=45):
    body = {"jsonrpc": "2.0", "id": str(uuid.uuid4())[:8], "method": method}
    if params is not None:
        body["params"] = params
    conn = http.client.HTTPConnection(HOST, PORT, timeout=timeout)
    headers = {
        "Content-Type": "application/json",
        "Accept": "application/json, text/event-stream",
    }
    if sid:
        headers["Mcp-Session-Id"] = sid
    try:
        conn.request("POST", "/mcp", json.dumps(body), headers)
        resp = conn.getresponse()
        raw = resp.read().decode("utf-8", errors="replace")
        return parse_sse(raw), resp.getheader("Mcp-Session-Id"), resp.status, raw[:500]
    except (socket.timeout, Exception) as e:
        return None, None, 0, str(e)
    finally:
        conn.close()


def call_tool(name, args, sid, timeout=45):
    events, _, status, raw = mcp("tools/call", {"name": name, "arguments": args}, sid, timeout)
    if events is None:
        return None, f"transport error/status={status}: {raw[:180]}"
    if not events:
        return None, f"no events/status={status}: {raw[:180]}"
    last = events[-1]
    if isinstance(last, dict) and last.get("error"):
        return None, f"jsonrpc error: {last['error'].get('message', last['error'])}"
    for c in last.get("result", {}).get("content", []):
        if c.get("type") == "text":
            text = c.get("text", "")
            try:
                return json.loads(text), None
            except Exception:
                return text, None
    return None, "no text content"


def classify(name, result, err, stability=None, fail_on_stable_stub=True):
    """Classify a tool response.

    Args:
        name: tool name
        result: deserialized response or str
        err: error string or None
        stability: meta.cognicode.stability annotation from tools/list, or None
        fail_on_stable_stub: if True, STUB from a stable tool is a failure; if False, warn only

    Returns:
        (status, note)
    """
    if err:
        msg = err.lower()
        if "tool not found" in msg or "not found" in msg and "jsonrpc" in msg:
            return "MISSING", err
        return "ERROR", err

    text = json.dumps(result, ensure_ascii=False) if not isinstance(result, str) else result
    low = text.lower()

    # Internal/application errors — check if this is a known gated tool
    if low.startswith("internal:") or low.startswith("invalid input:") or low.startswith("application error:"):
        # Annotation-based gated classification takes priority over fallback GATED set
        if stability == STABILITY_GATED or name in GATED:
            reason = GATED.get(name, "gated capability unavailable")
            return "GATED_OK", f"{reason}: {text[:120]}"
        return "ERROR", text[:220]

    if "tool not found" in low:
        return "MISSING", text[:220]

    if isinstance(result, dict) and result.get("error"):
        errtxt = str(result.get("error"))
        if "tool not found" in errtxt.lower():
            return "MISSING", errtxt
        return "ERROR", errtxt[:220]

    # Known placeholder/stub patterns
    stub_patterns = [
        "placeholder", "not implemented", "baseline comparison requires",
        "resource_type\": \"unknown", "resource_type': 'unknown",
        "results\": []", "\"results\": []", "hot_paths\": []",
        "dependencies\": []", "dependents\": []",
        "run build_graph first for full insights",
    ]
    if any(p in low for p in stub_patterns):
        # Annotation-based stability overrides pattern-based classification
        if stability == STABILITY_EXPERIMENTAL:
            return "STUB_WARN", f"[experimental] {text[:220]}"
        if stability == STABILITY_STABLE:
            if fail_on_stable_stub:
                return "STUB", f"[stable] {text[:220]}"
            else:
                return "STUB_WARN", f"[stable] {text[:220]}"
        return "STUB", text[:220]

    if name == "smart_search" and isinstance(result, dict) and result.get("total") == 0:
        if stability == STABILITY_EXPERIMENTAL:
            return "STUB_WARN", "[experimental] empty results for known query"
        if stability == STABILITY_STABLE and fail_on_stable_stub:
            return "STUB", "[stable] empty results for known query"
        return "STUB_WARN", "empty results for known query"

    if name == "compare_graph":
        if stability == STABILITY_EXPERIMENTAL:
            return "STUB_WARN", f"[experimental] {text[:220]}"
        if stability == STABILITY_STABLE:
            if fail_on_stable_stub:
                return "STUB", f"[stable] {text[:220]}"
            else:
                return "STUB_WARN", f"[stable] {text[:220]}"
        return "STUB", text[:220]

    if name == "iac_query" and isinstance(result, dict) and result.get("resource_type") == "unknown":
        if stability == STABILITY_EXPERIMENTAL:
            return "STUB_WARN", f"[experimental] {text[:220]}"
        if stability == STABILITY_STABLE:
            if fail_on_stable_stub:
                return "STUB", f"[stable] {text[:220]}"
            else:
                return "STUB_WARN", f"[stable] {text[:220]}"
        return "STUB", text[:220]

    # Annotation-based gated classification for non-error responses
    if stability == STABILITY_GATED or name in GATED:
        return "GATED_OK", f"{GATED.get(name, 'gated')}: {text[:120]}"

    return "OK", summary(result)


def summary(result):
    if isinstance(result, dict):
        keys = list(result.keys())[:8]
        bits = []
        for k in ("symbol_count", "symbols_found", "total", "count", "edge_count", "health_score", "risk_level", "question_count"):
            if k in result:
                bits.append(f"{k}={result[k]}")
        return f"keys={keys}" + ("; " + ", ".join(bits) if bits else "")
    if isinstance(result, list):
        return f"list[{len(result)}]"
    return str(result)[:160]


ARGS = {
    "build_graph": {},
    "get_file_symbols": {"file_path": RMCP, "compressed": False},
    "get_call_hierarchy": {"symbol_name": "build_graph", "direction": "outgoing", "depth": 1, "compressed": False},
    "analyze_impact": {"symbol_name": "build_graph", "compressed": False},
    "find_usages": {"symbol_name": "build_graph", "include_declaration": True},
    "get_complexity": {"file_path": RMCP, "function_name": "call_tool_handler"},
    "get_entry_points": {"compressed": False},
    "get_leaf_functions": {"compressed": False},
    "trace_path": {"source": "build_graph", "target": "save_graph", "max_depth": 5},
    "export_mermaid": {"root_symbol": "build_graph", "max_depth": 2, "include_external": False, "format": "code", "theme": "default"},
    "get_hot_paths": {"limit": 5, "min_fan_in": 2},
    "query_symbol_index": {"directory": str(ROOT), "symbol_name": "build_graph"},
    "build_call_subgraph": {"directory": str(ROOT), "symbol_name": "build_graph", "direction": "both", "depth": 2},
    "get_per_file_graph": {"file_path": RMCP},
    "get_symbol_code": {"file": RMCP, "line": 966, "col": 10},
    "go_to_definition": {"file_path": RMCP, "line": 966, "column": 9},
    "hover": {"file_path": RMCP, "line": 966, "column": 9},
    "find_references": {"file_path": RMCP, "line": 966, "column": 9, "include_declaration": True},
    "read_file": {"path": RMCP, "mode": "outline"},
    "search_content": {"pattern": "handle_build_graph", "path": str(ROOT), "file_glob": "*.rs", "regex": False, "case_insensitive": False, "context_lines": 2, "max_results": 5},
    "list_files": {"path": str(ROOT), "glob": "**/*.rs", "limit": 5, "offset": 0, "max_depth": 3, "recursive": True},
    "retrieve_and_verify": {"query": "handle_build_graph", "language": "rust", "max_results": 3, "verify": False},
    "nl_to_symbol": {"query": "build graph handler", "limit": 5},
    "ask_about_code": {"question": "What calls handle_build_graph?", "limit": 3},
    "find_pattern_by_intent": {"intent": "find code that builds the graph", "list_patterns": False},
    "detect_god_functions": {"min_lines": 50, "min_complexity": 10, "min_fan_in": 2},
    "detect_long_parameter_lists": {"max_params": 5},
    "generate_contract": {"file_path": RMCP, "function_name": "call_tool_handler"},
    "graph_pagerank": {"alpha": 0.85, "max_iterations": 20},
    "graph_all_paths": {"from_symbol": "build_graph", "to_symbol": "save_graph", "max_hops": 3},
    "graph_condensed": {},
    "graph_god_nodes": {"percentile": 0.95},
    "graph_reduced": {},
    "graph_feedback_arcs": {},
    "graph_communities": {"max_iterations": 20},
    "graph_community_detail": {"community_id": 0, "max_iterations": 20},
    "graph_surprising_connections": {"top_n": 5, "max_iterations": 20},
    "graph_search_idf": {"query": "build", "max_results": 5},
    "graph_insights": {},
    "graph_suggest_questions": {},
    "graph_query": {"question": "what connects build_graph to save_graph", "max_depth": 3, "budget": 10},
    "graph_explain": {"symbol": "build_graph", "depth": 2},
    "detect_drift": {"file_path": RMCP, "function_name": "call_tool_handler", "threshold": 0.5},
    "get_type_references": {"symbol": "CallGraph"},
    "get_imports": {"file_path": RMCP},
    "get_implementors": {"trait_name": "GraphStore"},
    "get_members": {"class_name": "HandlerContext"},
    "graph_query_filtered": {"question": "build_graph save_graph", "filters": {}, "limit": 5},
    "export_callflow": {"format": "mermaid", "max_sections": 5},
    "solid_audit": {},
    "graph_diff": {"baseline_date": "2026-06-16", "current": True},
    "graph_timeline": {"days": 7},
    "smart_search": {"query": "build graph", "algorithm": "fuzzy", "limit": 5},
    "graph_analyze": {"mode": "scc"},
    "project_overview": {"detail": "quick"},
    "compare_graph": {"mode": "diff"},
    "codebase_map": {"format": "compact"},
    "project_insights": {},
    "review_pr": {"files": ["crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs"]},
    "iac_query": {"resource_id": "aws_instance.web", "depth": 2},
    # Ghost dispatch tools not listed in tools/list
    "get_all_symbols": {"limit": 5, "offset": 0},
    "find_dead_code": {"limit": 5},
    "get_module_dependencies": {"limit": 5},
}


def list_tools(sid):
    """Call tools/list and return (tools, stability_map)."""
    all_tools, cursor = [], None
    for _ in range(20):
        params = {"cursor": cursor} if cursor else {}
        events, _, _, _ = mcp("tools/list", params, sid, 30)
        for e in events or []:
            if isinstance(e, dict) and "result" in e:
                r = e["result"]
                all_tools.extend(r.get("tools", []))
                cursor = r.get("nextCursor")
                break
        if not cursor:
            break

    # Build {tool_name: stability} map from meta.cognicode.stability
    stability_map = {}
    for tool in all_tools:
        name = tool.get("name")
        meta = tool.get("meta", {})
        # meta may be a dict or a list; normalize
        if isinstance(meta, dict):
            stability = meta.get("cognicode", {}).get("stability")
        elif isinstance(meta, list):
            # some tools return meta as a list of annotations
            stability = None
            for m in meta:
                if isinstance(m, dict) and m.get("cognicode", {}).get("stability"):
                    stability = m["cognicode"]["stability"]
                    break
        else:
            stability = None
        if name and stability:
            stability_map[name] = stability

    return all_tools, stability_map


def main():
    global HOST, PORT

    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--persist", metavar="PATH", help="Write baseline JSON to this path after smoke run")
    parser.add_argument("--server", default=f"{HOST}:{PORT}", help=f"Server address (default: {HOST}:{PORT})")
    parser.add_argument(
        "--fail-on-stable-stub",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Fail smoke if a stable tool returns STUB (default: true; use --no-fail-on-stable-stub to disable)",
    )
    args = parser.parse_args()

    if ":" in args.server:
        HOST, port_str = args.server.rsplit(":", 1)
        PORT = int(port_str)

    events, sid, _, raw = mcp("initialize", {
        "protocolVersion": "2025-03-26",
        "capabilities": {},
        "clientInfo": {"name": "mcp-smoke", "version": "1"},
    }, timeout=15)
    if not sid:
        print(f"initialize failed: {raw}", file=sys.stderr)
        return 1
    mcp("notifications/initialized", None, sid, 5)

    tools, stability_map = list_tools(sid)
    listed_names = [t.get("name") for t in tools]
    counts = Counter(listed_names)
    unique = sorted(set(listed_names))
    duplicates = {k: v for k, v in counts.items() if v > 1}

    # Count tools by stability annotation
    stability_counts = Counter(stability_map.values())
    unannotated = [n for n in unique if n not in stability_map and n not in SKIP]

    # Build graph once as precondition
    t0 = time.time()
    build_res, build_err = call_tool("build_graph", {}, sid, 120)
    build_stability = stability_map.get("build_graph")
    build_status, build_note = classify("build_graph", build_res, build_err, build_stability, args.fail_on_stable_stub)

    rows = []
    targets = unique + ["get_all_symbols", "find_dead_code", "get_module_dependencies"]
    for name in targets:
        if name == "build_graph":
            continue
        if name in SKIP:
            rows.append({"name": name, "status": "SKIP", "ms": 0, "note": SKIP[name], "stability": None})
            continue
        tool_args = ARGS.get(name, {})
        t = time.time()
        result, err = call_tool(name, tool_args, sid, 60)
        ms = int((time.time() - t) * 1000)
        stability = stability_map.get(name)
        status, note = classify(name, result, err, stability, args.fail_on_stable_stub)
        rows.append({"name": name, "status": status, "ms": ms, "note": note, "stability": stability})

    out = {
        "listed_total": len(listed_names),
        "unique_listed": len(unique),
        "duplicates": duplicates,
        "stability_counts": dict(stability_counts),
        "unannotated_tools": unannotated,
        "build_graph": {"status": build_status, "ms": int((time.time()-t0)*1000), "note": build_note, "stability": build_stability},
        "rows": rows,
        "gated": list(GATED.keys()),
    }

    if args.persist:
        Path(args.persist).write_text(json.dumps(out, indent=2, ensure_ascii=False))

    print(f"listed_total={out['listed_total']} unique_listed={out['unique_listed']} duplicates={duplicates}")
    print(f"stability_counts={out['stability_counts']} unannotated={len(unannotated)}")
    print(f"build_graph={build_status} {build_note}")
    c = Counter(r["status"] for r in rows)
    print("summary", dict(c))

    # Fail conditions
    missing = [r for r in rows if r["status"] == "MISSING"]
    if missing:
        print("\n## MISSING (smoke FAILED)")
        for r in missing:
            print(f"- {r['name']}: {r['note']}")
        return 1

    # Fail on stable STUB when flag is set
    stable_stubs = [r for r in rows if r["status"] == "STUB" and r["stability"] == STABILITY_STABLE]
    if stable_stubs and args.fail_on_stable_stub:
        print("\n## STABLE STUB (smoke FAILED)")
        for r in stable_stubs:
            print(f"- {r['name']}: {str(r['note'])[:180]}")
        return 1

    for status in ["ERROR", "STUB", "STUB_WARN", "GATED_OK", "SKIP", "OK"]:
        status_rows = [r for r in rows if r["status"] == status]
        if status_rows:
            print(f"\n## {status}")
            for r in status_rows:
                stability_tag = f"[{r['stability']}]" if r.get("stability") else ""
                print(f"- {r['name']} ({r['ms']}ms){stability_tag}: {str(r['note'])[:180]}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
