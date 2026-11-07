#!/usr/bin/env python3
"""Harness A/B: lo que le cuesta a un agente entender y usar la superficie.

Mide, sobre los repos de referencia y con los dos brazos (`v2`: los goals;
`legacy`: las tools), los bytes de descubrimiento, las llamadas, los bytes
leídos, el coste estimado, la latencia y la exactitud juzgada. Después compara
las cifras con `slo-budget.toml`: una meta superada sale con rc 1 y nombra la
meta y la cifra.

Es un comando a demanda. No está en la suite ni en el gate, y por eso su
lógica de decisión (el juez, el veredicto, las extracciones) la vigila
`scripts/ci/test_script_drift_and_recall.py`.

    just harness-ab run --repos kotlin,jenkins-library,csharp   # los pequeños
    just harness-ab run --repos cognicode                       # el grande
    just harness-ab report [--publish]                          # juzga y publica

Cada llamada es un proceso aparte (`worker`) con plazo y techo de memoria, y deja
una fila en `product/measurements/task-93/calls.csv` en cuanto termina: una
llamada que se cuelga o se pasa de memoria queda registrada como cortada y la
corrida sigue con la siguiente.

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
import uuid
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

    def __init__(self, binary: Path, cwd: Path, profile: str, timeout_s: float):
        self.timeout_s = timeout_s
        self.next_id = 0
        self.proc = subprocess.Popen(
            [str(binary), "--cwd", str(cwd), "--profile", profile],
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
            "links": (result.get("structuredContent") or {}).get("_links") or [],
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


# --- el aislamiento: una llamada, un proceso ---------------------------------

QUESTION_INTENTS = ("find", "inspect", "trace")
PROBE_INTENTS = ("status", "context", "reparse")
FIELDS = [
    "version", "repo", "question", "seconds", "max_rss_kb", "exit_code", "cut",
    "intent", "repeat", "call_ms", "correct", "state", "bytes", "discovery_bytes",
]
ROWS_PATH = MEASUREMENTS / "task-93" / "calls.csv"
TIME = "/usr/bin/time"


def parse_time_report(text: str) -> dict[str, Any]:
    """Lo que `/usr/bin/time -v` dice del proceso medido."""
    seconds = rss = exit_code = None
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("Elapsed (wall clock) time"):
            total = 0.0
            for part in line.rsplit(": ", 1)[1].split(":"):
                total = total * 60 + float(part)
            seconds = round(total, 3)
        elif line.startswith("Maximum resident set size"):
            rss = int(line.rsplit(": ", 1)[1])
        elif line.startswith("Exit status:"):
            exit_code = int(line.rsplit(": ", 1)[1])
        elif line.startswith("Command terminated by signal"):
            exit_code = 128 + int(line.rsplit(" ", 1)[1])
    return {"seconds": seconds, "max_rss_kb": rss, "exit_code": exit_code}


def classify_cut(exit_code: int | None) -> str:
    """124 es el plazo de `timeout`; 137 es SIGKILL, que aquí sólo lo manda el
    kernel cuando el cgroup pasa de `MemoryMax`."""
    if exit_code == 124:
        return "timeout"
    if exit_code == 137:
        return "memory"
    return ""


def isolated_run(argv: list[str], *, timeout_s: float, memory_max: str) -> dict[str, Any]:
    """Ejecuta `argv` como un proceso aparte, con plazo, techo de memoria y
    prioridad mínima de CPU y disco, y devuelve lo medido. Nunca lanza por culpa
    del proceso: uno que falla, se cuelga o se pasa de memoria es una fila."""
    if shutil.which("systemd-run") is None or not Path(TIME).is_file():
        raise RuntimeError("systemd-run and /usr/bin/time are required; refusing to run unbounded")
    unit = f"harness-ab-{uuid.uuid4().hex[:12]}"
    with tempfile.NamedTemporaryFile("r", suffix=".time") as report:
        command = [
            TIME, "-v", "-o", report.name,
            "timeout", "--kill-after=5", f"{timeout_s}s",
            "systemd-run", "--user", "--scope", "--quiet", "--unit", unit,
            "-p", f"MemoryMax={memory_max}", "-p", "MemorySwapMax=0",
            "nice", "-n", "19", "ionice", "-c3",
            *argv,
        ]
        done = subprocess.run(command, capture_output=True, text=True)
        measured = parse_time_report(Path(report.name).read_text())
    # Lo que el plazo dejó vivo (el servidor hijo de un worker cortado) muere con el scope.
    subprocess.run(["systemctl", "--user", "stop", f"{unit}.scope"], capture_output=True)
    exit_code = measured["exit_code"] if measured["exit_code"] is not None else done.returncode
    payload: dict[str, Any] = {}
    for line in reversed(done.stdout.splitlines()):
        try:
            candidate = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(candidate, dict):
            payload = candidate
            break
    return {
        "seconds": measured["seconds"],
        "max_rss_kb": measured["max_rss_kb"],
        "exit_code": exit_code,
        "cut": classify_cut(exit_code),
        "payload": payload,
    }


def make_call(meta: dict[str, Any], argv: list[str], timeout_s: float, memory_max: str) -> dict[str, Any]:
    return {"meta": meta, "argv": argv, "timeout_s": timeout_s, "memory_max": memory_max}


def build_row(call: dict[str, Any], result: dict[str, Any]) -> dict[str, Any]:
    payload = result["payload"]
    row = {name: "" for name in FIELDS}
    row.update(call["meta"])
    row.update(
        seconds=result["seconds"],
        max_rss_kb=result["max_rss_kb"],
        exit_code=result["exit_code"],
        cut=result["cut"],
        call_ms=payload.get("call_ms", ""),
        correct=payload.get("correct", ""),
        state=payload.get("state") or result["cut"] or "no-response",
        bytes=payload.get("bytes", ""),
        discovery_bytes=payload.get("discovery_bytes", ""),
    )
    return row


def run_plan(plan: list[dict[str, Any]], sink, run_one=isolated_run) -> list[dict[str, Any]]:
    """Corre las llamadas una a una. Una cortada queda registrada y la corrida
    sigue; las repeticiones de la misma pregunta se omiten, porque repetir lo
    que ya agotó el plazo sólo gasta el plazo otra vez."""
    rows: list[dict[str, Any]] = []
    cut_keys: set[tuple[str, str, str]] = set()
    for call in plan:
        meta = call["meta"]
        key = (meta["version"], meta["repo"], meta["question"])
        if key in cut_keys:
            continue
        result = run_one(call["argv"], timeout_s=call["timeout_s"], memory_max=call["memory_max"])
        row = build_row(call, result)
        sink(row)
        rows.append(row)
        if row["cut"]:
            cut_keys.add(key)
    return rows


class CsvSink:
    """Añade cada fila en cuanto llega: una corrida que muere no pierde lo medido."""

    def __init__(self, path: Path):
        self.path = path

    def __call__(self, row: dict[str, Any]) -> None:
        import csv

        self.path.parent.mkdir(parents=True, exist_ok=True)
        fresh = not self.path.exists() or self.path.stat().st_size == 0
        with self.path.open("a", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=FIELDS)
            if fresh:
                writer.writeheader()
            writer.writerow({name: row.get(name, "") for name in FIELDS})


def read_rows(path: Path) -> list[dict[str, Any]]:
    import csv

    if not path.is_file():
        return []
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


# --- el trabajador: una llamada dentro de su proceso --------------------------


def run_worker(spec: dict[str, Any]) -> dict[str, Any]:
    scratch = None
    cwd = Path(spec["cwd"])
    args = dict(spec["args"])
    edit = spec.get("edit_copy")
    if edit:
        scratch = tempfile.mkdtemp(prefix="harness-ab-")
        cwd = Path(scratch) / "ws"
        shutil.copytree(edit["source"], cwd)
        args = {"file_paths": [str(cwd / edit["file"])]}
    client = McpClient(Path(spec["binary"]), cwd, spec["profile"], 3600)
    try:
        discovery = client.discovery_bytes() if spec.get("measure_discovery") else None
        if edit:
            with (cwd / edit["file"]).open("a", encoding="utf-8") as handle:
                handle.write("\n// harness-ab edit\n")
        reply = client.call(spec["tool"], args)
    finally:
        client.close()
        if scratch:
            shutil.rmtree(scratch, ignore_errors=True)
    correct = ""
    if spec.get("question"):
        correct = judge(spec["intent"], reply, spec["arm"], spec["question"], spec["language"])
    return {
        "call_ms": round(reply["elapsed_ms"], 3),
        "correct": correct,
        "state": reply["state"],
        "bytes": reply["bytes"],
        "discovery_bytes": discovery if discovery is not None else "",
    }


# --- el plan -----------------------------------------------------------------


def _fill(template: dict[str, Any], question: dict[str, Any]) -> dict[str, Any]:
    out = {}
    for key, value in template.items():
        if isinstance(value, str):
            value = value.replace("{symbol}", question.get("symbol", "")).replace(
                "{file}", question.get("file", "")
            )
        out[key] = value
    return out


def _worker_argv(spec: dict[str, Any]) -> list[str]:
    return [sys.executable, "-I", str(Path(__file__).resolve()), "worker", "--spec", json.dumps(spec)]


def build_plan(
    config: dict[str, Any], binary: Path, repos: set[str] | None, arms: set[str] | None
) -> list[dict[str, Any]]:
    run = config["run"]
    plan: list[dict[str, Any]] = []
    by_name = {repo["name"]: repo for repo in config["repo"]}

    def selected(repo: dict[str, Any]) -> bool:
        if not repos:
            return True
        size = "large" if repo.get("large") else "small"
        return repo["name"] in repos or size in repos

    def limits() -> tuple[float, str]:
        return run["call_timeout_s"], run["memory_max"]

    for repo in sorted(config["repo"], key=lambda r: bool(r.get("large"))):
        if not selected(repo):
            continue
        root = (REPO_ROOT / repo["path"]).resolve()
        language = config["language"][repo["language"]]
        repeats = run["large_repeats"] if repo.get("large") else run["repeats"]
        for arm in config["arm"]:
            if arms and arm["name"] not in arms:
                continue
            for question in (q for q in config["question"] if q["repo"] == repo["name"]):
                call = arm["call"][question["intent"]]
                label = f"{question['intent']}:{question.get('file') or question['symbol']}"
                for repeat in range(repeats):
                    spec = {
                        "binary": str(binary), "cwd": str(root), "profile": arm["profile"],
                        "arm": arm["name"], "tool": call["tool"],
                        "args": _fill(call["args"], question), "intent": question["intent"],
                        "question": question, "language": language,
                        "measure_discovery": repeat == 0,
                    }
                    meta = {"version": arm["name"], "repo": repo["name"], "question": label,
                            "intent": question["intent"], "repeat": repeat}
                    plan.append(make_call(meta, _worker_argv(spec), *limits()))

    v2 = next((a for a in config["arm"] if a["name"] == "v2"), None)
    if v2 and (not arms or "v2" in arms):
        kotlin = by_name["kotlin"]
        if selected(kotlin):
            for probe in config.get("latency", []):
                for repeat in range(run["repeats"]):
                    spec = {"binary": str(binary), "cwd": str((REPO_ROOT / kotlin["path"]).resolve()),
                            "profile": "default", "arm": "v2", "tool": probe["tool"],
                            "args": probe["args"], "intent": probe["metric"],
                            "measure_discovery": False}
                    meta = {"version": "v2", "repo": "kotlin", "question": probe["metric"],
                            "intent": probe["metric"], "repeat": repeat}
                    plan.append(make_call(meta, _worker_argv(spec), *limits()))
        reparse = config.get("reparse")
        if reparse and selected(by_name[reparse["repo"]]):
            source = str((REPO_ROOT / by_name[reparse["repo"]]["path"]).resolve())
            for repeat in range(run["repeats"]):
                spec = {"binary": str(binary), "cwd": source, "profile": "default", "arm": "v2",
                        "tool": "reparse_on_edit", "args": {}, "intent": "reparse",
                        "edit_copy": {"source": source, "file": reparse["file"]},
                        "measure_discovery": False}
                meta = {"version": "v2", "repo": reparse["repo"], "question": "reparse",
                        "intent": "reparse", "repeat": repeat}
                plan.append(make_call(meta, _worker_argv(spec), *limits()))
    return plan


# --- de las filas a las cifras ------------------------------------------------


def _latency_ms(row: dict[str, Any]) -> float | None:
    if row["call_ms"] not in ("", None):
        return float(row["call_ms"])
    if row["seconds"] not in ("", None):
        return float(row["seconds"]) * 1000
    return None


def aggregate(rows: list[dict[str, Any]], config: dict[str, Any]) -> dict[str, Any]:
    """Las filas de un brazo → las cifras que juzgan las metas. Una llamada
    cortada cuenta como incorrecta y como latencia de su plazo: una respuesta
    que no llega es la peor respuesta, no una ausencia."""
    price = config["price"]
    arms: dict[str, Any] = {}
    for name in dict.fromkeys(r["version"] for r in rows):
        mine = [r for r in rows if r["version"] == name]
        asked = [r for r in mine if r["intent"] in QUESTION_INTENTS]
        first: dict[tuple[str, str], dict[str, Any]] = {}
        for r in asked:
            first.setdefault((r["repo"], r["question"]), r)
        per_repo_discovery: dict[str, int] = {}
        for r in mine:
            if r["discovery_bytes"] not in ("", None):
                per_repo_discovery[r["repo"]] = max(
                    per_repo_discovery.get(r["repo"], 0), int(float(r["discovery_bytes"]))
                )
        discovery = max(per_repo_discovery.values(), default=None)
        response_bytes = sum(int(float(r["bytes"])) for r in mine if r["bytes"] not in ("", None))
        tokens = (sum(per_repo_discovery.values()) + response_bytes) / price["bytes_per_token"]
        states: dict[str, int] = {}
        for r in mine:
            states[r["state"]] = states.get(r["state"], 0) + 1
        metrics: dict[str, Any] = {
            "profile": next(a["profile"] for a in config["arm"] if a["name"] == name),
            "discovery_bytes": discovery,
            "response_bytes": response_bytes,
            "calls": len(mine),
            "cut_calls": sum(1 for r in mine if r["cut"]),
            "tokens_estimate": round(tokens),
            "cost_estimate_usd": round(tokens / 1_000_000 * price["usd_per_million_input_tokens"], 6),
            "states": states,
            "questions": [
                {
                    "repo": r["repo"], "intent": r["intent"], "question": r["question"],
                    "correct": str(r["correct"]) == "True", "state": r["state"],
                    "cut": r["cut"], "bytes": r["bytes"], "cold_s": r["seconds"],
                    "max_rss_kb": r["max_rss_kb"],
                }
                for r in first.values()
            ],
        }
        if first:
            metrics["accuracy"] = round(
                sum(str(r["correct"]) == "True" for r in first.values()) / len(first), 4
            )
        for intent in (*QUESTION_INTENTS, *PROBE_INTENTS):
            samples = [v for r in mine if r["intent"] == intent for v in [_latency_ms(r)] if v is not None]
            if samples:
                metrics[f"{intent}_p50_ms"] = percentile(samples, 0.50)
                metrics[f"{intent}_p95_ms"] = percentile(samples, 0.95)
        if name == "v2":
            metrics["tools_list_bytes"] = discovery
        arms[name] = metrics
    return arms


# --- la publicación -----------------------------------------------------------


def _git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=REPO_ROOT, capture_output=True, text=True, check=True
    ).stdout.strip()


def binary_version(binary: Path) -> str:
    output = subprocess.run([str(binary), "--version"], capture_output=True, text=True)
    return output.stdout.strip().split()[-1] if output.returncode == 0 and output.stdout else ""


def publish(arms: dict[str, Any], binary: Path, config: dict[str, Any]) -> Path:
    # El producto medido es el de la base en `main`: ese SHA sobrevive al squash
    # de la rama; el de la rama, no.
    commit = _git("merge-base", "HEAD", "origin/main")
    harness_commit = _git("rev-parse", "HEAD")
    version = binary_version(binary)
    if not version:
        raise RuntimeError("the binary does not report a version; nothing to publish against")
    document = {
        "schema_version": "cognicode.measurement/v1",
        "version": version,
        "commit": commit,
        "harness_commit": harness_commit,
        "dirty": bool(_git("status", "--porcelain", "--untracked-files=no")),
        "measured_on": dt.date.today().isoformat(),
        "method": "one process per call; cold server per call; timeout and MemoryMax per call",
        "limits": {k: config["run"][k] for k in ("call_timeout_s", "memory_max", "repeats", "large_repeats")},
        "calls_file": str(ROWS_PATH.relative_to(REPO_ROOT)),
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
        cwd=REPO_ROOT, capture_output=True, text=True,
    )
    base = (Path(json.loads(target.stdout)["target_directory"])
            if target.returncode == 0 else REPO_ROOT / "target")
    return base / "release" / "cognicode-mcp"


def _csv_set(value: str | None) -> set[str] | None:
    return {item for item in value.split(",") if item} if value else None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run", help="measure, one process per call")
    run.add_argument("--repos", help="names, or `small` / `large`")
    run.add_argument("--arms", help="v2,legacy")
    run.add_argument("--binary", type=Path)
    run.add_argument("--rows", type=Path, default=ROWS_PATH)
    report = sub.add_parser("report", help="judge the rows against slo-budget.toml")
    report.add_argument("--binary", type=Path)
    report.add_argument("--rows", type=Path, default=ROWS_PATH)
    report.add_argument("--publish", action="store_true")
    worker = sub.add_parser("worker", help="(internal) one call")
    worker.add_argument("--spec", required=True)
    args = parser.parse_args(argv)

    if args.command == "worker":
        print(json.dumps(run_worker(json.loads(args.spec))))
        return 0

    binary = args.binary or default_binary()
    with CONFIG_PATH.open("rb") as handle:
        config = tomllib.load(handle)

    if args.command == "run":
        if not binary.is_file():
            print(f"missing binary {binary}: run `cargo build --release --bin cognicode-mcp`", file=sys.stderr)
            return 2
        plan = build_plan(config, binary, _csv_set(args.repos), _csv_set(args.arms))
        if not plan:
            print("the selection matches no call", file=sys.stderr)
            return 2
        print(f"{len(plan)} calls, one process each; rows -> {args.rows}", file=sys.stderr)
        rows = run_plan(plan, CsvSink(args.rows))
        cut = [r for r in rows if r["cut"]]
        print(f"ran {len(rows)} calls; {len(cut)} cut")
        for r in cut:
            print(f"  cut ({r['cut']}): {r['version']} {r['repo']} {r['question']}")
        return 0

    rows = read_rows(args.rows)
    if not rows:
        print(f"no rows in {args.rows}: run `run` first", file=sys.stderr)
        return 2
    arms = aggregate(rows, config)
    for name, metrics in arms.items():
        print(f"{name:7} discovery={metrics['discovery_bytes']}B calls={metrics['calls']} "
              f"cut={metrics['cut_calls']} tokens~{metrics['tokens_estimate']} "
              f"cost~${metrics['cost_estimate_usd']} accuracy={metrics.get('accuracy')}")
    if args.publish:
        print(f"published {publish(arms, binary, config)}")
    goals = load_goals(SLO_PATH)
    violations = check_goals(arms, goals)
    missing = unmeasured(arms, goals)
    for violation in violations:
        print(violation_message(violation), file=sys.stderr)
    for name in missing:
        print(f"goal `{name}` could not be measured", file=sys.stderr)
    return 1 if violations or missing else 0


if __name__ == "__main__":
    sys.exit(main())


