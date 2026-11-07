#!/usr/bin/env python3
"""Harness A/B: lo que le cuesta a un agente entender y usar la superficie.

Mide, sobre los repos de referencia y con los dos brazos (`v2`: los goals;
`legacy`: las tools), los bytes de descubrimiento, las llamadas, los bytes
leídos, el coste estimado, la latencia y la exactitud juzgada. Después compara
las cifras con `slo-budget.toml`: una meta superada sale con rc 1 y nombra la
meta y la cifra.

Es un comando a demanda. No está en la suite ni en el gate, y por eso su
lógica de decisión (el juez, el veredicto, las extracciones) la vigila
`scripts/ci/test_harness_ab.py`.

    just harness-ab                # mide y juzga
    just harness-ab -- --publish   # además escribe product/measurements/

rc 0  todas las metas se cumplen
rc 1  una meta se supera, o una cifra con meta no se pudo medir
rc 2  el comando no pudo medir (falta el binario, configuración inválida)
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CONFIG_PATH = REPO_ROOT / "scripts" / "measure" / "harness.toml"
SLO_PATH = REPO_ROOT / "slo-budget.toml"
MEASUREMENTS = REPO_ROOT / "product" / "measurements"


# --- las metas y el veredicto -------------------------------------------------


@dataclass(frozen=True)
class Goal:
    """Una meta. `ceiling` limita la cifra; `not_below` la compara con otro brazo."""

    name: str
    metric: str
    ceiling: float | None = None
    note: str = ""
    kind: str = "ceiling"
    arm: str = "v2"
    baseline: str = "legacy"


def load_goals(path: Path) -> list[Goal]:
    with path.open("rb") as handle:
        document = tomllib.load(handle)
    goals = []
    for entry in document.get("goal", []):
        goals.append(
            Goal(
                name=entry["name"],
                metric=entry["metric"],
                ceiling=entry.get("ceiling"),
                note=entry.get("note", ""),
                kind=entry.get("kind", "ceiling"),
                arm=entry.get("arm", "v2"),
                baseline=entry.get("baseline", "legacy"),
            )
        )
    return goals


def check_goals(measured: dict[str, dict[str, Any]], goals: list[Goal]) -> list[dict[str, Any]]:
    """Las metas superadas. Una cifra que no se midió no es una violación aquí:
    `unmeasured` la nombra aparte, para que no se confunda con «dentro»."""
    violations: list[dict[str, Any]] = []
    for goal in goals:
        if goal.kind == "not_below":
            value = measured.get(goal.arm, {}).get(goal.metric)
            floor = measured.get(goal.baseline, {}).get(goal.metric)
            if value is None or floor is None:
                continue
            if value < floor:
                violations.append(
                    {"goal": goal.name, "measured": value, "ceiling": floor, "kind": goal.kind}
                )
            continue
        value = measured.get(goal.arm, {}).get(goal.metric)
        if value is None or goal.ceiling is None:
            continue
        if value > goal.ceiling:
            violations.append(
                {"goal": goal.name, "measured": value, "ceiling": goal.ceiling, "kind": goal.kind}
            )
    return violations


def unmeasured(measured: dict[str, dict[str, Any]], goals: list[Goal]) -> list[str]:
    """Las metas cuya cifra no existe: un veredicto verde sobre nada."""
    missing = []
    for goal in goals:
        if measured.get(goal.arm, {}).get(goal.metric) is None:
            missing.append(goal.name)
        elif goal.kind == "not_below" and measured.get(goal.baseline, {}).get(goal.metric) is None:
            missing.append(goal.name)
    return missing


def violation_message(violation: dict[str, Any]) -> str:
    if violation.get("kind") == "not_below":
        return (
            f"goal `{violation['goal']}` not met: measured {violation['measured']} "
            f"is below the baseline {violation['ceiling']}"
        )
    return (
        f"goal `{violation['goal']}` not met: measured {violation['measured']} "
        f"exceeds the ceiling {violation['ceiling']}"
    )


# --- el juez ------------------------------------------------------------------


def _pattern(template: str, name: str) -> re.Pattern[str]:
    return re.compile(template.replace("{name}", re.escape(name)))


def declares(source: str, name: str, patterns: list[str]) -> bool:
    """¿El texto declara el símbolo? Un nombre con puntos (`Plain.Companion`)
    se declara si alguno de sus segmentos se declara: el compañero sin nombre
    de Kotlin no aparece escrito, pero su dueño sí."""
    segments = [name, *name.split(".")] if "." in name else [name]
    return any(
        _pattern(template, segment).search(source)
        for segment in segments
        for template in patterns
    )


def calls(source: str, name: str, call_pattern: str) -> bool:
    return _pattern(call_pattern, name).search(source) is not None


# --- las extracciones: las dos superficies no tienen la misma forma ----------


def _result(response: dict[str, Any]) -> dict[str, Any]:
    result = response.get("result")
    return result if isinstance(result, dict) else {}


def extract_trace_files(response: dict[str, Any], arm: str) -> list[str]:
    key = "entries" if arm == "v2" else "calls"
    return [
        item["file"]
        for item in _result(response).get(key, [])
        if isinstance(item, dict) and item.get("file")
    ]


def extract_locations(response: dict[str, Any]) -> list[str]:
    return [
        item["file"]
        for item in _result(response).get("locations", [])
        if isinstance(item, dict) and item.get("file")
    ]


def extract_symbol_names(response: dict[str, Any]) -> list[str]:
    return [
        item["name"]
        for item in _result(response).get("symbols", [])
        if isinstance(item, dict) and item.get("name")
    ]


def judge(
    intent: str,
    response: dict[str, Any],
    arm: str,
    question: dict[str, Any],
    language: dict[str, Any],
) -> bool:
    if response.get("is_error"):
        return False
    symbol = question["symbol"]
    if intent == "inspect":
        return symbol in extract_symbol_names(response)
    if intent == "find":
        files = extract_locations(response)
        patterns = language["declaration"]
        return any(
            declares(_read(Path(path)), symbol, patterns) for path in files if Path(path).is_file()
        )
    if intent == "trace":
        files = extract_trace_files(response, arm)
        return any(
            calls(_read(Path(path)), symbol, language["call"])
            for path in files
            if Path(path).is_file()
        )
    raise ValueError(f"no judge for the intent `{intent}`")


def _read(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


# --- estadística --------------------------------------------------------------


def percentile(samples: list[float], fraction: float) -> float | None:
    """Rango más cercano: sin interpolar, para que la cifra sea una medida."""
    if not samples:
        return None
    ordered = sorted(samples)
    index = max(0, math.ceil(fraction * len(ordered)) - 1)
    return round(ordered[index], 3)


# --- el cliente MCP -----------------------------------------------------------


class McpClient:
    """Un servidor `cognicode-mcp` por stdio. Una respuesta sin plazo es un
    cuelgue del comando, así que la lectura va en un hilo con su deadline."""

    memory_max = "3G"

    def __init__(self, binary: Path, cwd: Path, profile: str, timeout_s: float):
        self.timeout_s = timeout_s
        self.next_id = 0
        # Un techo de memoria por servidor (cgroup): un servidor que crece sin
        # límite lo mata el kernel a él y no a sus vecinos. `ulimit -v` no vale,
        # porque la base de grafos reserva espacio de direcciones de sobra.
        cap = shutil.which("systemd-run")
        if cap is None:
            raise RuntimeError("systemd-run is required to cap the server memory; refusing to run unbounded")
        self.proc = subprocess.Popen(
            [cap, "--user", "--scope", "--quiet", "-p", f"MemoryMax={self.memory_max}",
             "-p", "MemorySwapMax=0",
             str(binary), "--cwd", str(cwd), "--profile", profile],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        self._request(
            "initialize",
            {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "harness-ab", "version": "1"},
            },
        )
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}})

    def _send(self, message: dict[str, Any]) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write(json.dumps(message) + "\n")
        self.proc.stdin.flush()

    def _read_line(self) -> str:
        import threading

        box: dict[str, str] = {}

        def reader() -> None:
            assert self.proc.stdout is not None
            box["line"] = self.proc.stdout.readline()

        thread = threading.Thread(target=reader, daemon=True)
        thread.start()
        thread.join(self.timeout_s)
        if thread.is_alive() or not box.get("line"):
            self.proc.kill()
            raise TimeoutError(f"no response within {self.timeout_s}s")
        return box["line"]

    def _request(self, method: str, params: dict[str, Any]) -> tuple[dict[str, Any], int, float]:
        self.next_id += 1
        started = time.perf_counter()
        self._send({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params})
        while True:
            line = self._read_line()
            message = json.loads(line)
            if message.get("id") == self.next_id:
                return message, len(line.encode("utf-8")), (time.perf_counter() - started) * 1000

    def discovery_bytes(self) -> int:
        """Los bytes del `tools/list` entero, siguiendo la paginación."""
        total, cursor = 0, None
        while True:
            params = {"cursor": cursor} if cursor else {}
            message, size, _ = self._request("tools/list", params)
            total += size
            cursor = message.get("result", {}).get("nextCursor")
            if not cursor:
                return total

    def call(self, tool: str, arguments: dict[str, Any]) -> dict[str, Any]:
        message, size, elapsed_ms = self._request(
            "tools/call", {"name": tool, "arguments": arguments}
        )
        result = message.get("result", {})
        payload: Any = (result.get("structuredContent") or {}).get("result")
        if not isinstance(payload, dict):
            payload = _first_text_json(result)
        state = (result.get("structuredContent") or {}).get("state")
        if state is None and isinstance(payload, dict):
            state = (payload.get("coverage") or {}).get("status")
        return {
            "result": payload if isinstance(payload, dict) else {},
            "state": state or "unknown",
            "is_error": bool(result.get("isError")) or "error" in message,
            "bytes": size,
            "elapsed_ms": elapsed_ms,
        }

    def close(self) -> None:
        self.proc.kill()
        self.proc.wait()


def _first_text_json(result: dict[str, Any]) -> Any:
    for block in result.get("content", []):
        if block.get("type") == "text":
            try:
                return json.loads(block["text"])
            except (json.JSONDecodeError, KeyError):
                return None
    return None


# --- la medición --------------------------------------------------------------


def _fill(template: dict[str, Any], question: dict[str, Any]) -> dict[str, Any]:
    out = {}
    for key, value in template.items():
        if isinstance(value, str):
            value = value.replace("{symbol}", question.get("symbol", "")).replace(
                "{file}", question.get("file", "")
            )
        out[key] = value
    return out


def _timed_out(timeout_s: float) -> dict[str, Any]:
    return {
        "result": {},
        "state": "timeout",
        "is_error": True,
        "bytes": 0,
        "elapsed_ms": timeout_s * 1000,
    }


def measure_arm(
    arm: dict[str, Any], config: dict[str, Any], binary: Path
) -> dict[str, Any]:
    repeats = config["run"]["repeats"]
    timeout = config["run"]["call_timeout_s"]
    price = config["price"]
    repos = {repo["name"]: repo for repo in config["repo"]}
    discovery = 0
    sessions = 0
    response_bytes = 0
    calls_made = 0
    questions: list[dict[str, Any]] = []
    latencies: dict[str, list[float]] = {}
    states: dict[str, int] = {}

    for repo_name in dict.fromkeys(q["repo"] for q in config["question"]):
        repo = repos[repo_name]
        root = (REPO_ROOT / repo["path"]).resolve()
        language = config["language"][repo["language"]]
        client = McpClient(binary, root, arm["profile"], timeout)
        try:
            discovery = max(discovery, client.discovery_bytes())
            sessions += 1
            for question in (q for q in config["question"] if q["repo"] == repo_name):
                spec = arm["call"][question["intent"]]
                arguments = _fill(spec["args"], question)
                try:
                    warm = client.call(spec["tool"], arguments)
                    samples = [client.call(spec["tool"], arguments) for _ in range(repeats)]
                except TimeoutError:
                    # Una llamada que no vuelve a tiempo es una respuesta fallida
                    # con la latencia del plazo; el servidor se relanza porque
                    # `_read_line` lo mató, y las repeticiones no se reintentan.
                    client.close()
                    client = McpClient(binary, root, arm["profile"], timeout)
                    warm = _timed_out(timeout)
                    samples = [warm]
                correct = judge(question["intent"], warm, arm["name"], question, language)
                for sample in [warm, *samples]:
                    response_bytes += sample["bytes"]
                    calls_made += 1
                    states[sample["state"]] = states.get(sample["state"], 0) + 1
                latencies.setdefault(question["intent"], []).extend(
                    s["elapsed_ms"] for s in samples
                )
                questions.append(
                    {
                        "repo": repo_name,
                        "intent": question["intent"],
                        "symbol": question["symbol"],
                        "tool": spec["tool"],
                        "correct": correct,
                        "state": warm["state"],
                        "bytes": warm["bytes"],
                        "warmup_ms": round(warm["elapsed_ms"], 3),
                    }
                )
        finally:
            client.close()

    tokens = (discovery * sessions + response_bytes) / price["bytes_per_token"]
    metrics: dict[str, Any] = {
        "profile": arm["profile"],
        "questions": questions,
        "discovery_bytes": discovery,
        "response_bytes": response_bytes,
        "calls": calls_made,
        "tokens_estimate": round(tokens),
        "cost_estimate_usd": round(tokens / 1_000_000 * price["usd_per_million_input_tokens"], 6),
        "accuracy": round(sum(q["correct"] for q in questions) / len(questions), 4),
        "states": states,
    }
    for intent, samples in latencies.items():
        metrics[f"{intent}_p50_ms"] = percentile(samples, 0.50)
        metrics[f"{intent}_p95_ms"] = percentile(samples, 0.95)
    return metrics


def measure_latencies(config: dict[str, Any], binary: Path) -> dict[str, Any]:
    """`status`, `context` y el reindexado de un fichero, en el brazo v2."""
    repeats = config["run"]["repeats"]
    timeout = config["run"]["call_timeout_s"]
    repos = {repo["name"]: repo for repo in config["repo"]}
    out: dict[str, Any] = {}

    reference = repos["kotlin"]
    root = (REPO_ROOT / reference["path"]).resolve()
    client = McpClient(binary, root, "default", timeout)
    try:
        for probe in config.get("latency", []):
            client.call(probe["tool"], probe["args"])
            samples = [client.call(probe["tool"], probe["args"])["elapsed_ms"] for _ in range(repeats)]
            out[f"{probe['metric']}_p95_ms"] = percentile(samples, 0.95)
    finally:
        client.close()

    spec = config.get("reparse")
    if spec:
        source = (REPO_ROOT / repos[spec["repo"]]["path"]).resolve()
        with tempfile.TemporaryDirectory(prefix="harness-ab-") as scratch:
            copy = Path(scratch) / "ws"
            shutil.copytree(source, copy)
            target = copy / spec["file"]
            client = McpClient(binary, copy, "default", timeout)
            try:
                samples = []
                for index in range(repeats + 1):
                    with target.open("a", encoding="utf-8") as handle:
                        handle.write(f"\n// harness-ab edit {index}\n")
                    reply = client.call("reparse_on_edit", {"file_paths": [str(target)]})
                    if reply["is_error"]:
                        samples = []
                        break
                    if index > 0:
                        samples.append(reply["elapsed_ms"])
                out["reparse_p95_ms"] = percentile(samples, 0.95)
            finally:
                client.close()
    return out


# --- la publicación -----------------------------------------------------------


def _git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=REPO_ROOT, capture_output=True, text=True, check=True
    ).stdout.strip()


def binary_version(binary: Path) -> str:
    output = subprocess.run([str(binary), "--version"], capture_output=True, text=True)
    return output.stdout.strip().split()[-1] if output.returncode == 0 and output.stdout else ""


def publish(arms: dict[str, Any], binary: Path, config: dict[str, Any]) -> Path:
    commit = _git("rev-parse", "HEAD")
    version = binary_version(binary)
    if not version:
        raise RuntimeError("the binary does not report a version; nothing to publish against")
    document = {
        "schema_version": "cognicode.measurement/v1",
        "version": version,
        "commit": commit,
        "dirty": bool(_git("status", "--porcelain", "--untracked-files=no")),
        "measured_on": dt.date.today().isoformat(),
        "binary": str(binary),
        "price": config["price"],
        "arms": arms,
    }
    MEASUREMENTS.mkdir(parents=True, exist_ok=True)
    path = MEASUREMENTS / f"{version}-{commit[:12]}.json"
    path.write_text(json.dumps(document, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return path


# --- el comando ---------------------------------------------------------------


def default_binary() -> Path:
    target = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
    )
    base = Path(json.loads(target.stdout)["target_directory"]) if target.returncode == 0 else REPO_ROOT / "target"
    return base / "release" / "cognicode-mcp"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--binary", type=Path, help="cognicode-mcp release binary")
    parser.add_argument("--publish", action="store_true", help="write product/measurements/")
    args = parser.parse_args(argv)

    binary = args.binary or default_binary()
    if not binary.is_file():
        print(
            f"missing binary {binary}: run `cargo build --release --bin cognicode-mcp` first",
            file=sys.stderr,
        )
        return 2

    with CONFIG_PATH.open("rb") as handle:
        config = tomllib.load(handle)
    goals = load_goals(SLO_PATH)

    arms: dict[str, Any] = {}
    for arm in config["arm"]:
        print(f"measuring arm `{arm['name']}` ...", file=sys.stderr)
        arms[arm["name"]] = measure_arm(arm, config, binary)
    print("measuring the latencies ...", file=sys.stderr)
    arms["v2"].update(measure_latencies(config, binary))
    arms["v2"]["tools_list_bytes"] = arms["v2"]["discovery_bytes"]

    for name, metrics in arms.items():
        print(
            f"{name:7} discovery={metrics['discovery_bytes']}B calls={metrics['calls']} "
            f"tokens~{metrics['tokens_estimate']} cost~${metrics['cost_estimate_usd']} "
            f"accuracy={metrics['accuracy']}"
        )

    if args.publish:
        print(f"published {publish(arms, binary, config)}")

    violations = check_goals(arms, goals)
    missing = unmeasured(arms, goals)
    for violation in violations:
        print(violation_message(violation), file=sys.stderr)
    for name in missing:
        print(f"goal `{name}` could not be measured", file=sys.stderr)
    return 1 if violations or missing else 0


if __name__ == "__main__":
    sys.exit(main())
