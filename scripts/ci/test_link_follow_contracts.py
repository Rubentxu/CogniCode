#!/usr/bin/env python3
"""Contrato del contador de seguimiento de enlaces (`scripts/measure/link_follow.py`).

El sobre de cada respuesta ofrece `_links` (`rel`, `tool`, `required_args`). El
contador responde una sola pregunta: de los enlaces que se ofrecieron, cuales
siguio quien llamo despues. Es un modulo puro sobre una traza que aporta quien
corre un agente; no toca el core ni activa telemetria.

Ejecutar: python3 scripts/ci/test_link_follow_contracts.py
"""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE = ROOT / "scripts" / "measure" / "link_follow.py"
failures: list[str] = []


def load():
    spec = importlib.util.spec_from_file_location("link_follow", MODULE)
    assert spec is not None and spec.loader is not None, f"cannot load {MODULE}"
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def link(rel: str, tool: str, **args) -> dict:
    return {"rel": rel, "tool": tool, "required_args": args}


def call(tool: str, arguments: dict | None = None, links: list | None = None) -> dict:
    return {"tool": tool, "arguments": arguments or {}, "links": links or []}


def test_a_link_is_followed_by_a_later_call_with_its_tool_and_args():
    m = load()
    session = [
        call("find", links=[link("inspect-callers", "callers", symbol="a:f:1")]),
        call("callers", {"symbol": "a:f:1", "depth": 2}),
    ]
    counts = m.follow_counts([session])
    assert counts["inspect-callers"] == {"offered": 1, "followed": 1}, counts


def test_the_same_tool_with_other_args_is_not_following_the_link():
    m = load()
    session = [
        call("find", links=[link("inspect-callers", "callers", symbol="a:f:1")]),
        call("callers", {"symbol": "b:g:2"}),
    ]
    assert m.follow_counts([session])["inspect-callers"] == {"offered": 1, "followed": 0}


def test_a_call_before_the_offer_does_not_count_as_following():
    m = load()
    session = [
        call("callers", {"symbol": "a:f:1"}),
        call("find", links=[link("inspect-callers", "callers", symbol="a:f:1")]),
    ]
    assert m.follow_counts([session])["inspect-callers"] == {"offered": 1, "followed": 0}


def test_a_call_that_offers_itself_has_not_followed_its_own_offer():
    m = load()
    session = [call("find", {"q": "x"}, links=[link("retry", "find", q="x")])]
    assert m.follow_counts([session])["retry"] == {"offered": 1, "followed": 0}


def test_sessions_do_not_follow_each_others_links():
    m = load()
    offered = [call("find", links=[link("retry", "find", q="x")])]
    other = [call("find", {"q": "x"})]
    assert m.follow_counts([offered, other])["retry"] == {"offered": 1, "followed": 0}


def test_each_offer_is_counted_once_even_when_several_calls_match():
    m = load()
    session = [
        call("find", links=[link("continue", "page", cursor="c1")]),
        call("page", {"cursor": "c1"}),
        call("page", {"cursor": "c1"}),
    ]
    assert m.follow_counts([session])["continue"] == {"offered": 1, "followed": 1}


def test_a_rel_never_offered_is_no_data_not_zero():
    m = load()
    rows = {r["rel"]: r for r in m.report(m.follow_counts([[call("find")]]))}
    assert rows["reindex"]["offered"] == 0, rows["reindex"]
    assert rows["reindex"]["rate"] is None, "sin ofertas no hay tasa: seria un cero inventado"
    assert rows["reindex"]["verdict"] == "sin datos", rows["reindex"]


def test_zero_rate_is_a_removal_candidate_only_with_enough_offers():
    m = load()
    few = {"retry": {"offered": m.MIN_OFFERS - 1, "followed": 0}}
    enough = {"retry": {"offered": m.MIN_OFFERS, "followed": 0}}
    assert {r["rel"]: r for r in m.report(few)}["retry"]["verdict"] == "pocas ofertas"
    row = {r["rel"]: r for r in m.report(enough)}["retry"]
    assert row["rate"] == 0.0 and row["verdict"] == "candidato a retirar", row


def test_a_followed_rel_is_never_a_removal_candidate():
    m = load()
    rows = m.report({"continue": {"offered": m.MIN_OFFERS * 5, "followed": 1}})
    row = {r["rel"]: r for r in rows}["continue"]
    assert row["verdict"] == "seguido" and 0 < row["rate"] < 1, row


def test_an_unknown_rel_is_reported_not_dropped():
    m = load()
    session = [call("find", links=[link("brand-new", "t")])]
    rows = {r["rel"]: r for r in m.report(m.follow_counts([session]))}
    assert rows["brand-new"]["offered"] == 1, "un rel nuevo no se descarta en silencio"


def test_the_jsonl_trace_groups_calls_by_session():
    m = load()
    lines = [
        {"session": "s1", **call("find", links=[link("retry", "find", q="x")])},
        {"session": "s1", **call("find", {"q": "x"})},
        {"session": "s2", **call("find", {"q": "x"})},
    ]
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "trace.jsonl"
        path.write_text("\n".join(json.dumps(x) for x in lines) + "\n", encoding="utf-8")
        sessions = m.load_sessions(path)
    assert sorted(len(s) for s in sessions) == [1, 2], sessions
    assert m.follow_counts(sessions)["retry"] == {"offered": 1, "followed": 1}


def test_a_malformed_trace_line_is_an_error_not_a_silent_skip():
    m = load()
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "trace.jsonl"
        path.write_text('{"tool": "find"}\nnot json\n', encoding="utf-8")
        try:
            m.load_sessions(path)
        except ValueError as exc:
            assert "2" in str(exc), "el error debe decir la linea"
        else:
            raise AssertionError("una linea rota se ignoro en silencio")


def test_the_harness_client_returns_the_links_of_the_response():
    source = (ROOT / "scripts" / "measure" / "harness_ab.py").read_text(encoding="utf-8")
    assert '"links"' in source and '"_links"' in source, (
        "McpClient.call debe devolver los _links de structuredContent"
    )


def main() -> int:
    module = sys.modules[__name__]
    checks = sorted(
        (getattr(module, n) for n in dir(module) if n.startswith("test_") and callable(getattr(module, n))),
        key=lambda c: c.__code__.co_firstlineno,
    )
    assert checks, "ningun test_ encontrado"
    for check_ in checks:
        before = len(failures)
        try:
            check_()
        except AssertionError as exc:
            failures.append(f"{check_.__name__}: {exc}")
        except Exception as exc:  # noqa: BLE001
            failures.append(f"{check_.__name__}: {type(exc).__name__}: {exc}")
        print(f"  {'PASS' if len(failures) == before else 'FAIL'} {check_.__name__}")
    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for f in failures:
            print(f"  - {f}")
        return 1
    print(f"PASS — contador de seguimiento de enlaces ({len(checks)} comprobaciones)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
