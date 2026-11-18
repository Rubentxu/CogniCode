#!/usr/bin/env python3
"""Mide cuanto cuesta llegar a lo que una tarea necesita desde un `pack`.

Abre sesiones MCP reales (`--profile full`) sobre un fixture escrito aqui y,
para dos tareas con verdad terreno (una de arquitectura y un fallo), compara
tres brazos:

* `pack_only`: una llamada a `pack` y lo que trae.
* `pack_links`: `pack` y despues SOLO los enlaces `_links` que la respuesta
  ofrece para el nivel que la tarea necesita, con los argumentos que traen.
* `files`: lo que haria hoy un agente sin grafo: `git grep -l -w` por el nombre
  y leer entero cada fichero que casa.

Por brazo y tarea: llamadas, bytes que lee el agente y exito contra la verdad
terreno. Ademas: la autoridad de cada enlace que se siguio (ninguno ensancha
permisos si todos son `read`) y el consejo de parada que dio el `pack`.

Alcance, dicho sin adornos: la politica de seguimiento es determinista (no es
un agente real), el fixture es pequeno, los bytes no se convierten en tokens y
NO es el baseline de recuperacion real de la validacion del Context Pack: es
una comparacion dentro de un mismo binario y un mismo arbol.

Uso: pack_levels_walk.py --binary <cognicode-mcp> --out informe.json
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent

# Los hechos que el fixture fija: `hub` tiene estos llamantes, repartidos en
# ficheros distintos, y `leaf` solo uno.
CALLERS = [f"caller_{i}" for i in range(12)]

# Cuantas veces se sigue como maximo un enlace por tarea: una oferta que
# apunta a otra no puede encadenar sin fin.
MAX_FOLLOWS = 3


def _harness():
    spec = importlib.util.spec_from_file_location("harness_ab", HERE / "harness_ab.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules["harness_ab"] = module
    spec.loader.exec_module(module)
    return module


def build_fixture(project: Path) -> None:
    """`hub` con doce llamantes en cuatro ficheros y mas codigo que no lo toca."""
    src = project / "src"
    src.mkdir(parents=True, exist_ok=True)
    (src / "lib.rs").write_text(
        "pub mod a;\npub mod b;\npub mod c;\npub mod d;\npub mod noise;\n\n"
        "/// Scales a reading; the fault of the bug task is here: it must add.\n"
        "pub fn hub(x: i32) -> i32 {\n    x - 1\n}\n",
        encoding="utf-8",
    )
    for index, module in enumerate("abcd"):
        body = "use crate::hub;\n\n" + "".join(
            f"pub fn caller_{3 * index + n}() -> i32 {{\n    hub({n})\n}}\n\n" for n in range(3)
        )
        (src / f"{module}.rs").write_text(body, encoding="utf-8")
    (src / "noise.rs").write_text(
        "".join(f"pub fn unrelated_{n}(v: i32) -> i32 {{\n    v * {n} + 1\n}}\n\n" for n in range(40)),
        encoding="utf-8",
    )
    (project / "Cargo.toml").write_text(
        '[package]\nname = "levelsfix"\nversion = "0.1.0"\nedition = "2021"\n', encoding="utf-8"
    )
    for command in (
        ["git", "init", "-q"],
        ["git", "add", "-A"],
        ["git", "-c", "user.name=m", "-c", "user.email=m@example.com", "commit", "-qm", "chore: fixture"],
    ):
        subprocess.run(command, cwd=project, check=True, capture_output=True)


TASKS = [
    {
        "name": "architecture",
        "question": "who depends on `hub`",
        "pack": {"symbols": ["hub"], "mode": "architecture", "max_tokens": 2000},
        "follow": ["trace-consumers"],
        "grep": "hub",
        "truth": CALLERS,
    },
    {
        "name": "bug",
        "question": "`hub` returns the wrong value: read the code that is at fault",
        "pack": {"symbols": ["hub"], "mode": "diagnose", "max_tokens": 2000},
        "follow": ["read-range"],
        "grep": "hub",
        "truth": ["x - 1"],
    },
]


def reached(truth: list[str], text: str) -> bool:
    """Every fact of the ground truth appears as a whole word (`caller_1` is
    not found inside `caller_10`)."""
    return all(re.search(rf"(?<![\w]){re.escape(item)}(?![\w])", text) for item in truth)


def found(truth: list[str], text: str) -> int:
    return sum(
        1 for item in truth if re.search(rf"(?<![\w]){re.escape(item)}(?![\w])", text)
    )


def pack_only(client: Any, task: dict[str, Any]) -> dict[str, Any]:
    reply = client.call("pack", task["pack"])
    text = json.dumps(reply["result"].get("items", []))
    return {
        "calls": 1,
        "bytes": reply["bytes"],
        "success": reached(task["truth"], text),
        "facts_found": f"{found(task['truth'], text)}/{len(task['truth'])}",
        "state": reply["state"],
        "stop_verdict": (reply["result"].get("navigation") or {}).get("advisory", {}).get("verdict"),
        "links_offered": [link["rel"] for link in reply["links"]],
    }


def pack_links(client: Any, task: dict[str, Any]) -> dict[str, Any]:
    reply = client.call("pack", task["pack"])
    calls, size, text = 1, reply["bytes"], json.dumps(reply["result"].get("items", []))
    followed: list[dict[str, str]] = []
    authorities: set[str] = set()
    for link in reply["links"]:
        if link["rel"] not in task["follow"] or len(followed) >= MAX_FOLLOWS:
            continue
        answer = client.call(link["tool"], dict(link["required_args"]))
        calls += 1
        size += answer["bytes"]
        text += json.dumps(answer["result"])
        authorities.add(link["authority"])
        followed.append({"rel": link["rel"], "tool": link["tool"], "authority": link["authority"]})
        if answer["is_error"]:
            followed[-1]["error"] = "the link answered with an error"
    return {
        "calls": calls,
        "bytes": size,
        "success": reached(task["truth"], text),
        "facts_found": f"{found(task['truth'], text)}/{len(task['truth'])}",
        "followed": followed,
        "authorities_followed": sorted(authorities),
        "widened": sorted(authorities - {"read"}),
        "stop_verdict": (reply["result"].get("navigation") or {}).get("advisory", {}).get("verdict"),
    }


def files(project: Path, task: dict[str, Any]) -> dict[str, Any]:
    listing = subprocess.run(
        ["git", "grep", "-l", "-w", task["grep"]], cwd=project, capture_output=True, text=True
    )
    paths = [line for line in listing.stdout.splitlines() if line]
    text = "".join((project / path).read_text(encoding="utf-8") for path in paths)
    return {
        "calls": 1 + len(paths),
        "bytes": len(listing.stdout.encode()) + len(text.encode()),
        "success": reached(task["truth"], text),
        "facts_found": f"{found(task['truth'], text)}/{len(task['truth'])}",
        "files_read": len(paths),
    }


def measure(binary: Path) -> dict[str, Any]:
    harness = _harness()
    report: dict[str, Any] = {
        "binary": str(binary),
        "load_before": os.getloadavg()[0],
        "cores": os.cpu_count(),
        "tasks": {},
    }
    with tempfile.TemporaryDirectory() as tmp:
        project = Path(tmp) / "fixture"
        build_fixture(project)
        cache = Path(tmp) / "cache"
        cache.mkdir()
        os.environ["XDG_CACHE_HOME"] = str(cache)
        client = harness.McpClient(binary, project, "full", 180.0)
        try:
            client.call("build_graph", {"directory": str(project)})
            for task in TASKS:
                report["tasks"][task["name"]] = {
                    "question": task["question"],
                    "pack_only": pack_only(client, task),
                    "pack_links": pack_links(client, task),
                    "files": files(project, task),
                }
        finally:
            client.close()
    report["load_after"] = os.getloadavg()[0]
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args(argv)
    if not args.binary.exists():
        print(f"missing cognicode-mcp: {args.binary}", file=sys.stderr)
        return 2
    report = measure(args.binary)
    text = json.dumps(report, indent=2)
    if args.out:
        args.out.write_text(text + "\n", encoding="utf-8")
    print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
