#!/usr/bin/env python3
"""Produce la traza que `link_follow.py` cuenta, desde una sesion MCP real.

Abre UN servidor `cognicode-mcp` contra un proyecto fixture, encadena llamadas
(`build_graph`, `analyze_impact`) y, por cada enlace de `_links` que el
servidor ofrece, aplica una politica determinista de seguimiento: llamar a la
tool del enlace con exactamente sus `required_args`.

Alcance: mide el contrato de cable (que el servidor ofrece enlaces utilizables)
y la politica elegida. NO mide el comportamiento de un agente: con la politica
«seguir todo» la tasa es 1 por construccion. Para decidir retirar un enlace
hacen falta trazas de agentes reales.

Uso: link_trace.py --out traza.jsonl [--follow all|none|rel,rel] [--binary ruta]
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent

# Cuantas llamadas de seguimiento como maximo: una oferta que apunta a otra
# oferta no puede encadenar sin fin.
MAX_FOLLOWS = 8

# Cuantos llamantes tiene `hub` en el fixture (el umbral de fan-in del servidor
# es 8; el riesgo se calcula sobre el grafo construido).
FIXTURE_CALLERS = 12


def _harness():
    spec = importlib.util.spec_from_file_location("harness_ab", HERE / "harness_ab.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules["harness_ab"] = module
    spec.loader.exec_module(module)
    return module


def build_fixture(project: Path) -> None:
    """Un crate con una funcion `hub` de la que dependen muchas."""
    (project / "src").mkdir(parents=True, exist_ok=True)
    lines = ["pub fn hub() -> i32 { 1 }"]
    lines += [f"pub fn caller_{i}() -> i32 {{ hub() + {i} }}" for i in range(FIXTURE_CALLERS)]
    (project / "src" / "lib.rs").write_text("\n".join(lines) + "\n", encoding="utf-8")
    (project / "Cargo.toml").write_text(
        '[package]\nname = "linkfix"\nversion = "0.1.0"\nedition = "2021"\n', encoding="utf-8"
    )


def _wanted(offer: dict[str, Any], follow: frozenset[str] | set[str] | None) -> bool:
    return follow is None or offer["rel"] in follow


def run_session(
    binary: Path,
    project: Path,
    *,
    follow: frozenset[str] | set[str] | None,
    session: str = "0",
    profile: str = "legacy",
    timeout_s: float = 120.0,
    script: list[tuple[str, dict[str, Any]]] | None = None,
) -> list[dict[str, Any]]:
    """La traza de una sesion. `follow=None` sigue todo; un conjunto vacio, nada.

    `script` son las llamadas encadenadas; por defecto construye el grafo y
    pregunta por el impacto de `hub`. Una oferta repetida se sigue una vez."""
    client = _harness().McpClient(binary, project, profile, timeout_s)
    trace: list[dict[str, Any]] = []
    sent: set[str] = set()
    try:

        def make(tool: str, arguments: dict[str, Any]) -> list[dict[str, Any]]:
            reply = client.call(tool, arguments)
            trace.append(
                {"tool": tool, "arguments": arguments, "links": reply["links"], "session": session}
            )
            sent.add(json.dumps([tool, arguments], sort_keys=True))
            return reply["links"]

        script = script or [
            ("build_graph", {"directory": str(project)}),
            ("analyze_impact", {"symbol_name": "hub"}),
        ]
        pending: list[dict[str, Any]] = []
        followed = 0
        for tool, arguments in script:
            pending += [o for o in make(tool, arguments) if _wanted(o, follow)]
            while pending and followed < MAX_FOLLOWS:
                offer = pending.pop(0)
                key = json.dumps([offer["tool"], offer["required_args"]], sort_keys=True)
                if key in sent:
                    continue
                followed += 1
                pending += [
                    o for o in make(offer["tool"], dict(offer["required_args"])) if _wanted(o, follow)
                ]
    finally:
        client.close()
    return trace


def parse_follow(text: str) -> frozenset[str] | None:
    if text == "all":
        return None
    if text == "none":
        return frozenset()
    return frozenset(part for part in text.split(",") if part)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--follow", default="all", help="all | none | rel,rel")
    parser.add_argument("--binary", type=Path)
    args = parser.parse_args(argv)

    binary = args.binary or _harness().default_binary()
    if not binary.exists():
        print(f"missing release binary cognicode-mcp: {binary}", file=sys.stderr)
        return 2
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        project = Path(tmp) / "fixture"
        build_fixture(project)
        trace = run_session(binary, project, follow=parse_follow(args.follow))
    args.out.write_text("".join(json.dumps(call) + "\n" for call in trace), encoding="utf-8")
    print(f"{len(trace)} llamadas -> {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
