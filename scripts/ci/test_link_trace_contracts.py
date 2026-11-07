#!/usr/bin/env python3
"""Contrato del productor de trazas de enlaces (`scripts/measure/link_trace.py`).

Abre UNA sesion `cognicode-mcp` real contra un proyecto fixture, encadena
llamadas y aplica una politica determinista de seguimiento a los `_links` que
el servidor ofrece. Mide el contrato de cable y la politica, no el
comportamiento de un agente. No toca el core ni activa telemetria.

Necesita el binario release (`cargo build --release --bin cognicode-mcp`): si
falta, el contrato FALLA con ese motivo, no se salta.

Ejecutar: python3 scripts/ci/test_link_trace_contracts.py
"""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MEASURE = ROOT / "scripts" / "measure"
failures: list[str] = []
_cache: dict[str, object] = {}


def load(name: str):
    path = MEASURE / f"{name}.py"
    assert path.exists(), f"falta {path.relative_to(ROOT)}"
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def produce(follow):
    """Una sesion real por politica; se reutiliza dentro del contrato."""
    key = repr(sorted(follow) if follow is not None else None)
    if key not in _cache:
        trace = load("link_trace")
        harness = load("harness_ab")
        binary = harness.default_binary()
        assert binary.exists(), (
            f"missing release binary cognicode-mcp: cargo build --release --bin cognicode-mcp ({binary})"
        )
        with tempfile.TemporaryDirectory() as tmp:
            project = Path(tmp) / "fixture"
            trace.build_fixture(project)
            _cache[key] = trace.run_session(binary, project, follow=follow, session="s1")
    return _cache[key]


def offers(calls: list[dict]) -> list[dict]:
    return [offer for call in calls for offer in call["links"]]


def test_the_server_offers_a_real_inspect_risk_link() -> None:
    calls = produce(None)
    rels = {offer["rel"] for offer in offers(calls)}
    assert "inspect-risk" in rels, f"el servidor no ofrecio inspect-risk; rels={sorted(rels)}"


def test_every_call_has_the_fields_the_counter_reads() -> None:
    for call in produce(None):
        assert {"tool", "arguments", "links", "session"} <= call.keys(), call
        assert call["session"] == "s1", call


def test_the_counter_accepts_the_trace() -> None:
    counter = load("link_follow")
    calls = produce(None)
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "trace.jsonl"
        path.write_text("".join(json.dumps(c) + "\n" for c in calls), encoding="utf-8")
        done = subprocess.run(
            [sys.executable, str(MEASURE / "link_follow.py"), str(path)],
            capture_output=True,
            text=True,
            timeout=60,
        )
        assert done.returncode == 0, done.stderr
        report = json.loads(done.stdout)
    sessions = counter.load_sessions(path) if path.exists() else None
    assert sessions is None or sessions, "traza vacia"
    assert report, "informe vacio"


def counts_for(calls: list[dict]) -> dict:
    counter = load("link_follow")
    return counter.follow_counts([calls])


def test_following_everything_follows_the_risk_offer() -> None:
    entry = counts_for(produce(None))["inspect-risk"]
    assert entry["offered"] >= 1 and entry["followed"] == entry["offered"], entry


def test_following_nothing_leaves_every_offer_unfollowed() -> None:
    calls = produce(frozenset())
    counts = counts_for(calls)
    assert counts, "sin ofertas: la politica 'no seguir' no tiene nada que no seguir"
    assert all(e["followed"] == 0 for e in counts.values()), counts


def test_a_follow_call_sends_exactly_the_required_args() -> None:
    calls = produce(None)
    offer = next(o for o in offers(calls) if o["rel"] == "inspect-risk")
    sent = [c for c in calls if c["tool"] == offer["tool"] and c["arguments"] == offer["required_args"]]
    assert sent, f"ninguna llamada envio {offer['required_args']} a {offer['tool']}"


def test_a_repeated_offer_is_followed_once() -> None:
    trace = load("link_trace")
    harness = load("harness_ab")
    binary = harness.default_binary()
    assert binary.exists(), f"missing release binary cognicode-mcp ({binary})"
    with tempfile.TemporaryDirectory() as tmp:
        project = Path(tmp) / "fixture"
        trace.build_fixture(project)
        impact = ("analyze_impact", {"symbol_name": "hub"})
        calls = trace.run_session(
            binary,
            project,
            follow=None,
            session="s2",
            script=[("build_graph", {"directory": str(project)}), impact, impact],
        )
    risk = [o for o in offers(calls) if o["rel"] == "inspect-risk"]
    assert len(risk) == 2, f"se esperaban dos ofertas iguales, hay {len(risk)}"
    followed = [c for c in calls if c["tool"] == risk[0]["tool"]]
    assert len(followed) == 1, f"la misma oferta se siguio {len(followed)} veces"


def test_the_server_writes_the_trace_the_counter_reads() -> None:
    """`COGNICODE_LINK_TRACE` hace que el SERVIDOR escriba la traza: lo que
    ofrecio y a que se llamo, con `rel`, que un transcript del cliente no
    conserva. Debe coincidir con lo que el cliente observo y ser contable."""
    import os

    trace = load("link_trace")
    harness = load("harness_ab")
    binary = harness.default_binary()
    assert binary.exists(), f"missing release binary cognicode-mcp ({binary})"
    with tempfile.TemporaryDirectory() as tmp:
        project = Path(tmp) / "fixture"
        trace.build_fixture(project)
        out = Path(tmp) / "server-trace.jsonl"
        os.environ["COGNICODE_LINK_TRACE"] = str(out)
        try:
            observed = trace.run_session(binary, project, follow=None, session="client")
        finally:
            del os.environ["COGNICODE_LINK_TRACE"]
        written = [json.loads(row) for row in out.read_text(encoding="utf-8").splitlines()]
        sessions = {row["session"] for row in written}
        assert len(sessions) == 1, f"una sesion de servidor, no {sessions}"
        assert [(r["tool"], r["arguments"]) for r in written] == [
            (c["tool"], c["arguments"]) for c in observed
        ], "el servidor registro otras llamadas que las que recibio el cliente"
        assert [r["links"] for r in written] == [c["links"] for c in observed]
        counts = load("link_follow").follow_counts([written])
        assert counts["inspect-risk"]["offered"] >= 1, counts


def test_without_the_variable_the_server_writes_nothing() -> None:
    import os

    assert "COGNICODE_LINK_TRACE" not in os.environ
    trace = load("link_trace")
    harness = load("harness_ab")
    binary = harness.default_binary()
    assert binary.exists(), f"missing release binary cognicode-mcp ({binary})"
    with tempfile.TemporaryDirectory() as tmp:
        project = Path(tmp) / "fixture"
        trace.build_fixture(project)
        trace.run_session(binary, project, follow=None, session="x")
        leaked = [p for p in Path(tmp).rglob("*.jsonl")]
        assert not leaked, f"el servidor escribio trazas sin que se pidieran: {leaked}"


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
    print(f"PASS — productor de trazas de enlaces ({len(checks)} comprobaciones)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
