"""Ledger de evidencia: append-only, fuera de git, con escrituras atómicas.

TH-S02 pide un único ledger canónico con `fsync`+`rename` y locking
interproceso, y prohíbe que un índice mutuo sustituya al histórico. Aquí el
histórico son:

  * `ledger.jsonl` — eventos, solo se añade, nunca se reescribe.
  * `receipts/<receipt_id>.json` — inmutables; si el contenido difiere, es un
    error de integridad, no una actualización.
  * `runs/<run_id>.json` — estado del run, reescribible porque es una vista.

`index.json` es una vista derivada del directorio de recibos y se puede
reconstruir entera con `rebuild_index()`. Si desaparece, el ledger sigue siendo
correcto: solo se pierde velocidad.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import socket
import tempfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from harness.domain import Receipt, SourceSnapshot, RunnerInfo

SCHEMA_INDEX = "th.index/v1"
LEDGER_FILE = "ledger.jsonl"
RECEIPTS_DIR = "receipts"
RUNS_DIR = "runs"
LOGS_DIR = "logs"
LEASES_DIR = "leases"
INDEX_FILE = "index.json"

_SLUG_RE = re.compile(r"[^A-Za-z0-9._-]+")


def now_iso() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def state_root(repo_id: str, base: Path | None = None) -> Path:
    """`$XDG_STATE_HOME/cognicode-test-harness/<slug>-<hash>/` (TH-S02)."""
    if base is None:
        env = os.environ.get("XDG_STATE_HOME")
        base = Path(env) if env else Path.home() / ".local" / "state"
    slug = _SLUG_RE.sub("-", repo_id.rstrip("/").split("/")[-1].removesuffix(".git")) or "repo"
    stem = hashlib.sha256(repo_id.encode("utf-8")).hexdigest()[:8]
    return (base / "cognicode-test-harness" / f"{slug}-{stem}").resolve()


def receipt_id_for(run_id: str, suite_id: str, fingerprint: str, phase: str = "terminal") -> str:
    """Id del recibo. `phase` separa el marcador `RUNNING` del recibo final.

    Son dos hechos distintos y ambos deben existir: el marcador dice "aquí había
    algo corriendo" y el final es la evidencia. Compartir id obligaría a
    reescribir el primero, y los recibos son inmutables a propósito.
    """
    digest = hashlib.sha256(f"{run_id}\0{suite_id}\0{fingerprint}\0{phase}".encode("utf-8"))
    return f"rcpt-{digest.hexdigest()[:20]}"


def _atomic_write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    handle, tmp_name = tempfile.mkstemp(dir=str(path.parent), prefix=".tmp-")
    try:
        with os.fdopen(handle, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(tmp_name, path)
    except BaseException:
        try:
            os.unlink(tmp_name)
        except OSError:
            pass
        raise
    # fsync del directorio: sin esto el rename puede no ser durable tras un crash.
    try:
        dir_fd = os.open(str(path.parent), os.O_RDONLY)
        try:
            os.fsync(dir_fd)
        finally:
            os.close(dir_fd)
    except OSError:
        pass


def _atomic_write_text(path: Path, text: str) -> None:
    _atomic_write(path, text.encode("utf-8"))


@dataclass
class Lease:
    path: Path
    acquired: bool
    holder: dict | None = None
    reason: str = ""


class LeaseError(RuntimeError):
    pass


class Ledger:
    def __init__(self, root: Path):
        self.root = Path(root)
        self.root.mkdir(parents=True, exist_ok=True)

    # ---- rutas -----------------------------------------------------------
    @property
    def ledger_path(self) -> Path:
        return self.root / LEDGER_FILE

    @property
    def receipts_dir(self) -> Path:
        return self.root / RECEIPTS_DIR

    @property
    def runs_dir(self) -> Path:
        return self.root / RUNS_DIR

    @property
    def logs_dir(self) -> Path:
        return self.root / LOGS_DIR

    @property
    def leases_dir(self) -> Path:
        return self.root / LEASES_DIR

    def receipt_path(self, receipt_id: str) -> Path:
        return self.receipts_dir / f"{receipt_id}.json"

    def run_path(self, run_id: str) -> Path:
        return self.runs_dir / f"{run_id}.json"

    # ---- escritura -------------------------------------------------------
    def append_event(self, event: dict) -> None:
        """Añade un evento al histórico. Nunca reescribe, nunca trunca."""
        self.root.mkdir(parents=True, exist_ok=True)
        payload = {"at": now_iso(), **event}
        line = json.dumps(payload, sort_keys=True, ensure_ascii=False) + "\n"
        with self.ledger_path.open("a", encoding="utf-8") as stream:
            stream.write(line)
            stream.flush()
            os.fsync(stream.fileno())

    def write_receipt(self, receipt: Receipt) -> Path:
        """Escribe un recibo inmutable. Reintentar con el mismo contenido es inocuo."""
        path = self.receipt_path(receipt.receipt_id)
        data = json.dumps(receipt.to_json(), sort_keys=True, ensure_ascii=False, indent=2).encode("utf-8")
        if path.exists():
            existing = path.read_bytes()
            if existing == data:
                return path
            raise LeaseError(
                f"el recibo {receipt.receipt_id} ya existe con contenido distinto: "
                "los recibos son inmutables, y sobrescribirlos borraría evidencia"
            )
        _atomic_write(path, data)
        self.append_event(
            {
                "event": "receipt_written",
                "receipt_id": receipt.receipt_id,
                "run_id": receipt.run_id,
                "suite_id": receipt.suite_id,
                "status": receipt.status,
                "input_fingerprint": receipt.input_fingerprint,
                "log_sha256": receipt.log_sha256,
            }
        )
        return path

    def write_run(self, run_id: str, payload: dict) -> Path:
        path = self.run_path(run_id)
        _atomic_write_text(path, json.dumps(payload, sort_keys=True, ensure_ascii=False, indent=2))
        return path

    def read_run(self, run_id: str) -> dict | None:
        path = self.run_path(run_id)
        if not path.exists():
            return None
        return json.loads(path.read_text(encoding="utf-8"))

    def list_runs(self) -> tuple[dict, ...]:
        if not self.runs_dir.exists():
            return ()
        runs = []
        for path in sorted(self.runs_dir.glob("*.json")):
            try:
                runs.append(json.loads(path.read_text(encoding="utf-8")))
            except json.JSONDecodeError:
                continue
        return tuple(runs)

    def latest_run(self, lane: str | None = None) -> dict | None:
        runs = [r for r in self.list_runs() if lane is None or r.get("lane") == lane]
        if not runs:
            return None
        return max(runs, key=lambda r: r.get("started_at", ""))

    # ---- lectura ---------------------------------------------------------
    def read_receipt(self, receipt_id: str) -> tuple[Receipt | None, str | None]:
        """Devuelve (recibo, problema). Un recibo ilegible no es un recibo."""
        path = self.receipt_path(receipt_id)
        if not path.exists():
            return None, f"el recibo {receipt_id} no existe en el ledger"
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, UnicodeDecodeError) as exc:
            return None, f"el recibo {receipt_id} está corrupto: {exc}"
        return receipt_from_json(payload)

    def receipts_for_suite(self, suite_id: str) -> tuple[str, ...]:
        """Ids de recibo de una suite, del más reciente al más antiguo."""
        index = self._load_index()
        ids = list(index.get("by_suite", {}).get(suite_id, ()))
        return tuple(ids)

    def latest_receipt_for_suite(self, suite_id: str) -> tuple[Receipt | None, str | None]:
        for receipt_id in self.receipts_for_suite(suite_id):
            receipt, problem = self.read_receipt(receipt_id)
            if receipt is not None:
                return receipt, None
            return None, problem
        return None, None

    # ---- índice derivado --------------------------------------------------
    def _load_index(self) -> dict:
        """Índice por suite, reconstruido si el directorio de recibos cambió.

        El índice es una vista; la verdad es el directorio. Se compara el recuento
        de recibos con el que se anotó al construirlo, porque un proceso distinto
        puede haber escrito evidencia nueva: un índice rancio haría que la
        reutilización dijera "no hay recibo previo" con el recibo delante.
        """
        path = self.root / INDEX_FILE
        if path.exists():
            try:
                index = json.loads(path.read_text(encoding="utf-8"))
            except json.JSONDecodeError:
                index = None
            if index is not None:
                if index.get("receipt_count") == self._receipt_count():
                    return index
                return self.rebuild_index()
        return self.rebuild_index()

    def _receipt_count(self) -> int:
        if not self.receipts_dir.exists():
            return 0
        return sum(1 for _ in self.receipts_dir.glob("*.json"))

    def rebuild_index(self) -> dict:
        """Reconstruye el índice desde el directorio de recibos, que es la verdad."""
        by_suite: dict[str, list[tuple[str, str]]] = {}
        for path in sorted(self.receipts_dir.glob("*.json")) if self.receipts_dir.exists() else []:
            try:
                payload = json.loads(path.read_text(encoding="utf-8"))
            except json.JSONDecodeError:
                continue
            suite_id = payload.get("suite_id")
            finished = payload.get("finished_at") or ""
            if not suite_id:
                continue
            by_suite.setdefault(suite_id, []).append((finished, path.stem))
        ordered = {
            suite: [rid for _, rid in sorted(items, key=lambda item: item[0], reverse=True)]
            for suite, items in by_suite.items()
        }
        index = {
            "schema_version": SCHEMA_INDEX,
            "derived_at": now_iso(),
            "receipt_count": self._receipt_count(),
            "by_suite": ordered,
        }
        _atomic_write_text(self.root / INDEX_FILE, json.dumps(index, sort_keys=True, ensure_ascii=False, indent=2))
        return index

    # ---- lease -----------------------------------------------------------
    def acquire_lease(self, run_id: str, purpose: str) -> Lease:
        """Un solo escritor por run. Dos procesos ejecutando el mismo run es un bug."""
        self.leases_dir.mkdir(parents=True, exist_ok=True)
        path = self.leases_dir / f"{run_id}.lock"
        holder = {
            "pid": os.getpid(),
            "host": socket.gethostname(),
            "acquired_at": now_iso(),
            "purpose": purpose,
        }
        try:
            fd = os.open(str(path), os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o644)
        except FileExistsError:
            try:
                current = json.loads(path.read_text(encoding="utf-8"))
            except (json.JSONDecodeError, OSError):
                current = {"pid": None, "host": None, "acquired_at": None}
            return Lease(path, False, current, f"el run {run_id} ya tiene dueño: {current}")
        with os.fdopen(fd, "w", encoding="utf-8") as stream:
            json.dump(holder, stream, sort_keys=True)
            stream.flush()
            os.fsync(stream.fileno())
        return Lease(path, True, holder)

    def release_lease(self, lease: Lease) -> None:
        if lease.acquired and lease.path.exists():
            try:
                lease.path.unlink()
            except OSError:
                pass

    # ---- logs ------------------------------------------------------------
    def log_path(self, run_id: str, suite_id: str) -> Path:
        safe = _SLUG_RE.sub("-", suite_id)
        return self.logs_dir / run_id / f"{safe}.log"

    def write_log(self, run_id: str, suite_id: str, data: bytes) -> tuple[Path, str]:
        path = self.log_path(run_id, suite_id)
        _atomic_write(path, data)
        return path, hashlib.sha256(data).hexdigest()


def receipt_from_json(payload: dict) -> tuple[Receipt | None, str | None]:
    """Reconstruye un recibo verificando forma e integridad (TH-S02).

    Se comprueba lo que el esquema no puede comprobar: coherencia de estados y
    aritmética de casos. Un recibo que dice `PASSED` con fallos o con cuentas que
    no cuadran se rechaza aquí, no en el consumidor.
    """
    if not isinstance(payload, dict):
        return None, "el recibo no es un objeto JSON"
    if payload.get("schema_version") != "th.receipt/v1":
        return None, f"schema_version inesperado: {payload.get('schema_version')!r}"
    for field_name in (
        "receipt_id",
        "run_id",
        "repo_id",
        "suite_id",
        "lane",
        "status",
        "input_fingerprint",
        "source_snapshot",
        "runner",
        "log_sha256",
    ):
        if field_name not in payload:
            return None, f"falta el campo obligatorio '{field_name}'"
    fingerprint = payload["input_fingerprint"]
    if not isinstance(fingerprint, str) or not re.fullmatch(r"[a-f0-9]{64}", fingerprint):
        return None, "input_fingerprint no es un sha256 hexadecimal"
    try:
        snapshot = SourceSnapshot(**payload["source_snapshot"])
        runner = RunnerInfo(**{**payload["runner"], "argv": tuple(payload["runner"]["argv"]),
                               "features": tuple(payload["runner"]["features"])})
        receipt = Receipt(
            receipt_id=payload["receipt_id"],
            run_id=payload["run_id"],
            repo_id=payload["repo_id"],
            suite_id=payload["suite_id"],
            lane=payload["lane"],
            status=payload["status"],
            input_fingerprint=fingerprint,
            source_snapshot=snapshot,
            runner=runner,
            started_at=payload.get("started_at", ""),
            finished_at=payload.get("finished_at", ""),
            exit_code=int(payload.get("exit_code", -1)),
            discovered_cases=int(payload.get("discovered_cases", 0)),
            executed_cases=int(payload.get("executed_cases", 0)),
            passed_cases=int(payload.get("passed_cases", 0)),
            failed_cases=int(payload.get("failed_cases", 0)),
            skipped_cases=int(payload.get("skipped_cases", 0)),
            log_sha256=payload.get("log_sha256", ""),
        )
    except (TypeError, ValueError, KeyError) as exc:
        return None, f"el recibo no encaja con la forma esperada: {exc}"
    problem = check_receipt_arithmetic(receipt)
    if problem:
        return receipt, problem
    return receipt, None


def check_receipt_arithmetic(receipt: Receipt) -> str | None:
    total = receipt.passed_cases + receipt.failed_cases + receipt.skipped_cases
    if receipt.executed_cases != total:
        return (
            f"las cuentas no cuadran: ejecutados={receipt.executed_cases} y "
            f"pasados+fallidos+omitidos={total}"
        )
    if receipt.status == "PASSED" and receipt.exit_code != 0:
        return f"dice PASSED con exit_code={receipt.exit_code}"
    if receipt.status == "PASSED" and receipt.failed_cases != 0:
        return "dice PASSED con casos fallidos"
    if receipt.discovered_cases and receipt.executed_cases > receipt.discovered_cases:
        return "se ejecutaron más casos de los descubiertos"
    return None
