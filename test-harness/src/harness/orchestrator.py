"""Orquestador: prepare → plan → run → resume/status → aggregate.

Traduce el ciclo de vida de TH-S03 a operaciones con nombre. Dos invariantes que
se aplican siempre y que ningún atajo debe saltarse:

  * El recibo `RUNNING` se escribe **antes** de lanzar el proceso. Si el harness
    muere en medio, queda un recibo no terminal y `resume` lo trata como
    interrumpido, nunca como PASS.
  * `resume` recalcula el plan con el snapshot de **ahora**. Reutilizar el plan
    viejo sería afirmar que el código no ha cambiado mientras nadie lo miraba.
"""

from __future__ import annotations

import hashlib
import signal
import subprocess
import threading
from concurrent.futures import ThreadPoolExecutor, as_completed
from contextlib import contextmanager
from dataclasses import dataclass, field
from pathlib import Path

from .domain import (
    GateVerdict,
    Receipt,
    ReuseDecision,
    RunnerInfo,
    SourceSnapshot,
    digest_of,
)
from .gate import Coverage, aggregate
from .inventory import Inventory, load_inventory
from .obligations import Floor, load_floor
from .planner import PlanResult, PlannedSuite, build_plan, refutable_fallback, select
from .policy import certify_requires_fresh, reuse_allowed
from .validity import evaluate_reuse

from adapters import cargo_workspace, cr08_selector, git_repo, pipeline_catalog, process_runner
from adapters.git_repo import ChangeSet
from adapters.ledger_store import Ledger, now_iso, receipt_id_for, state_root
from adapters.pipeline_catalog import CatalogEntry

EXIT_SATISFIED = 0
EXIT_UNSATISFIED = 1
EXIT_HARNESS_ERROR = 2
EXIT_UNDETERMINED = 3
EXIT_INTERRUPTED = 130

FAIL_FAST_LANES = ("change",)
RUN_SCHEMA = "th.run/v1"


class HarnessError(RuntimeError):
    """Error de plan o de harness (exit 2), no un fallo de test (exit 1)."""


@dataclass
class Context:
    repo_root: Path
    repo_id: str
    ledger: Ledger
    toolchain: object
    changeset: object
    graph: object
    catalog: tuple
    inventory: Inventory = field(default_factory=lambda: Inventory(entries={}))
    obligation_floor: Floor = field(default_factory=lambda: Floor(entries={}))
    warnings: tuple[str, ...] = ()

    def catalog_for_lane(self, lane: str) -> tuple:
        return pipeline_catalog.catalog_for_lane(self.catalog, lane)


def prepare(
    repo_root: Path,
    *,
    state_dir: Path | None = None,
    base_ref: str | None,
    offline_metadata: bool = True,
) -> Context:
    """Fija identidad, catálogo, snapshot y ledger. Un solo punto de entrada."""
    from .planner import ToolchainInfo

    root = git_repo.repo_root(repo_root)
    identifier = git_repo.repo_id(root)
    ledger = Ledger(state_dir if state_dir is not None else state_root(identifier))
    toolchain = ToolchainInfo.read(root)
    changeset = git_repo.read_changeset(root, base_ref)
    graph = cargo_workspace.load_graph(root, offline=offline_metadata)
    catalog = pipeline_catalog.apply_suite_metadata(
        pipeline_catalog.read_catalog(root), root
    )
    inventory = load_inventory(root)
    obligation_floor = load_floor(root)
    warnings: list[str] = []
    if inventory.problem:
        warnings.append(f"inventario pinado ilegible: {inventory.problem}")
    if obligation_floor.problem:
        warnings.append(f"suelo de obligaciones ilegible: {obligation_floor.problem}")
    if graph.is_unknown:
        warnings.append(f"grafo de paquetes desconocido: {graph.reason}")
    if changeset.is_unknown:
        warnings.append(f"ChangeSet desconocido: {changeset.fallback_reason}")
    if not catalog:
        warnings.append(
            "el catálogo de obligaciones está vacío: no hay pipelines que leer en este checkout"
        )
    return Context(
        repo_root=root,
        repo_id=identifier,
        ledger=ledger,
        toolchain=toolchain,
        changeset=changeset,
        graph=graph,
        catalog=catalog,
        # MEDIDO 2026-10-16. Estas dos líneas faltaban: `prepare()` cargaba el
        # inventario y el suelo de obligaciones, avisaba si no se podían leer… y
        # construía el `Context` sin ellos, así que el valor por defecto (vacío)
        # viajaba a cada plan. Efecto real: el suelo de obligaciones críticas nunca
        # podía marcar un plan como indeterminado por perder una obligación, y los
        # mínimos pinados de `inventory.json` nunca se aplicaban, en **ningún**
        # camino de la CLI. Los tests que cubrían esos mecanismos llamaban a
        # `build_plan` con los objetos ya cargados a mano, así que probaban el
        # mecanismo y no el camino que el usuario ejecuta.
        inventory=inventory,
        obligation_floor=obligation_floor,
        warnings=tuple(warnings),
    )


def new_run_id(lane: str, source_fingerprint: str) -> str:
    stamp = now_iso().replace(":", "").replace("-", "")
    digest = hashlib.sha256(f"{lane}\0{source_fingerprint}\0{stamp}".encode("utf-8")).hexdigest()[:8]
    return f"run-{stamp}-{lane}-{digest}"


def make_plan(
    context: Context,
    *,
    lane: str,
    allow_reuse: bool = False,
    allow_external: bool = False,
    oracle: str = "none",
    run_id: str | None = None,
) -> tuple[PlanResult, str]:
    """Plan de una lane. Con `oracle="cr08"` el plan nunca exige menos que el selector.

    El oráculo se consulta con los paths del ChangeSet real. Cuando el ChangeSet
    es desconocido, la lista va vacía y CR-08 responde `fallback` (todo), que es
    lo mismo que ya exige el fallback conservador: no hay contradicción posible.
    """
    identifier = run_id or new_run_id(lane, _provisional_fingerprint(context))
    decision = None
    if oracle == "cr08":
        paths = tuple(getattr(context.changeset, "all_paths", lambda: ())())
        decision = cr08_selector.run_selector(context.repo_root, paths)
    plan_result = build_plan(
        lane=lane,
        repo_id=context.repo_id,
        changeset=context.changeset,
        graph=context.graph,
        catalog=context.catalog_for_lane(lane),
        ledger=context.ledger,
        toolchain=context.toolchain,
        allow_reuse=allow_reuse,
        allow_external=allow_external,
        oracle=decision,
        inventory=context.inventory,
        obligation_floor=context.obligation_floor,
        repo_id_expected=context.repo_id,
        run_id=identifier,
        created_at=now_iso(),
    )
    return plan_result, identifier


def parity_corpus(context: Context, *, lane: str = "change") -> dict:
    """Mide la paridad con CR-08 sobre el corpus de clases de cambio.

    Cada entrada del corpus se planifica como si fuera el único cambio: un
    ChangeSet sintético con ese path. Es honesto porque lo que se está midiendo es
    el mapeo path → obligaciones, y el cambio real ya se planifica con su propio
    ChangeSet (el comando `plan`).
    """
    from . import parity as parity_module

    catalog = context.catalog_for_lane(lane)
    head = getattr(context.changeset, "head", "") or ""
    entries: list[dict] = []
    for path, why in parity_module.CORPUS:
        synthetic = ChangeSet(
            repo_root=context.repo_root,
            base_ref=None,
            merge_base=None,
            head=head,
            changed=(path,),
            dirty=True,
        )
        selection = select(
            lane=lane,
            changeset=synthetic,
            graph=context.graph,
            catalog=catalog,
            repo_root=context.repo_root,
        )
        decision = cr08_selector.run_selector(context.repo_root, (path,))
        report = parity_module.compare(
            decision,
            required=tuple(entry for entry, _ in selection.required),
            catalog=catalog,
            attributable=refutable_fallback(synthetic, context.graph),
        )
        entries.append(
            {
                "path": path,
                "why": why,
                "harness_required": len(selection.required),
                "report": report.to_json(),
            }
        )

    verdicts: dict[str, int] = {}
    for entry in entries:
        verdict = entry["report"]["verdict"]
        verdicts[verdict] = verdicts.get(verdict, 0) + 1
    weak_undeclared = [entry["path"] for entry in entries if entry["report"]["verdict"] == "HARNESS_WEAKER"]
    return {
        "lane": lane,
        "entries": entries,
        "summary": {
            "total": len(entries),
            "verdicts": verdicts,
            "weak_undeclared": weak_undeclared,
            "parity_ok": not weak_undeclared,
        },
    }


def _provisional_fingerprint(context: Context) -> str:
    return digest_of(
        {
            "head": getattr(context.changeset, "head", ""),
            "paths": list(getattr(context.changeset, "all_paths", lambda: ())()),
        }
    )


@dataclass
class RunOptions:
    lane: str
    allow_reuse: bool = False
    allow_external: bool = False
    fail_fast: bool | None = None
    timeout_s: float | None = None
    only: tuple[str, ...] = ()
    env_overrides: dict[str, str] = field(default_factory=dict)
    run_id: str | None = None
    max_workers: int = 1


MAX_WORKERS_LIMIT = 8

# Recurso compartido implícito de toda suite de Cargo: el directorio de
# compilación del workspace. Dos `cargo test` concurrentes se serializan de todas
# formas (cargo bloquea el target dir), así que declararlo evita el ruido de
# «Blocking waiting for file lock» y deja claro de dónde sale —o no— el
# paralelismo real.
CARGO_TARGET_LOCK = "cargo-target"


class ResourceGate:
    """Un cerrojo por recurso, y barrera exclusiva para las suites seriales.

    TH-S03 prohíbe paralelizar por defecto y exige exclusión para las suites que
    mutan el entorno. Una suite marcada `serial` no solo no se solapa con otra
    serial: **nadie** corre mientras ella corre, así que la barrera es de
    escritor/lector. La primera versión solo bloqueaba a las otras seriales, y eso
    dejaba a una suite normal corriendo en paralelo con una que muta el entorno —
    exactamente lo que la marca existe para impedir.
    """

    def __init__(self, lock_names: tuple[str, ...] = ()) -> None:
        names = set(lock_names) | {CARGO_TARGET_LOCK}
        self._locks = {name: threading.Lock() for name in sorted(names)}
        self._condition = threading.Condition()
        self._running = 0
        self._serial_running = 0
        self._serial_waiting = 0

    @contextmanager
    def hold(self, entry: CatalogEntry):
        if getattr(entry, "serial", False):
            self._enter_serial()
            try:
                yield
            finally:
                self._leave_serial()
            return
        self._enter_shared()
        acquired: list[threading.Lock] = []
        try:
            for name in sorted(set(getattr(entry, "locks", ()))):
                lock = self._locks.get(name)
                if lock is not None:
                    lock.acquire()
                    acquired.append(lock)
            yield
        finally:
            for lock in reversed(acquired):
                lock.release()
            self._leave_shared()

    def _enter_shared(self) -> None:
        with self._condition:
            # Si hay una serial esperando, las normales esperan: sin esto, una
            # racha de suites normales podría posponer indefinidamente la serial.
            while self._serial_running or self._serial_waiting:
                self._condition.wait()
            self._running += 1

    def _leave_shared(self) -> None:
        with self._condition:
            self._running -= 1
            self._condition.notify_all()

    def _enter_serial(self) -> None:
        with self._condition:
            self._serial_waiting += 1
            try:
                while self._serial_running or self._running:
                    self._condition.wait()
            finally:
                self._serial_waiting -= 1
            self._serial_running += 1

    def _leave_serial(self) -> None:
        with self._condition:
            self._serial_running -= 1
            self._condition.notify_all()

    def lock_names(self) -> tuple[str, ...]:
        return tuple(sorted(self._locks))

    def close(self) -> None:
        return None


@dataclass
class RunOutcome:
    run_id: str
    plan_result: PlanResult
    verdict: GateVerdict
    executed: tuple[Receipt, ...] = ()
    reused: tuple[Receipt, ...] = ()
    pending: tuple[str, ...] = ()
    interrupted: bool = False
    notes: tuple[str, ...] = ()


def _snapshot(context: Context) -> SourceSnapshot:
    changeset = context.changeset
    return SourceSnapshot(
        git_head=getattr(changeset, "head", "") or "no-head",
        effective_content_digest=changeset.digest_for(changeset.all_paths()),
        dirty=bool(getattr(changeset, "dirty", False)),
        worktree_id=git_repo.worktree_id(context.repo_root, getattr(changeset, "head", "") or ""),
    )


def _runner_info(suite: PlannedSuite) -> RunnerInfo:
    is_cargo = suite.argv[:2] == ("cargo", "test")
    return RunnerInfo(
        name="cargo-test" if is_cargo else "script",
        version=suite.fingerprint.runner_version,
        argv=suite.argv,
        features=suite.fingerprint.features,
        target_triple=suite.fingerprint.target_triple,
    )


def _running_receipt(
    *,
    run_id: str,
    context: Context,
    suite: PlannedSuite,
    lane: str,
    snapshot: SourceSnapshot,
    started_at: str,
) -> Receipt:
    return Receipt(
        # Un intento = un marcador. El id incluye el instante de arranque porque
        # reanudar un run vuelve a lanzar la misma suite y los recibos son
        # inmutables: reutilizar el id obligaría a reescribir evidencia.
        receipt_id=receipt_id_for(
            run_id, suite.entry.suite_id, suite.fingerprint.digest(), f"running-{started_at}"
        ),
        run_id=run_id,
        repo_id=context.repo_id,
        suite_id=suite.entry.suite_id,
        lane=lane,
        status="RUNNING",
        input_fingerprint=suite.fingerprint.digest(),
        source_snapshot=snapshot,
        runner=_runner_info(suite),
        started_at=started_at,
        finished_at="",
        exit_code=-1,
        discovered_cases=0,
        executed_cases=0,
        passed_cases=0,
        failed_cases=0,
        skipped_cases=0,
        log_sha256="",
    )


TARGET_DIR_SCRIPT = "scripts/ci/target-dir.sh"


def resolve_lane_environment(repo_root: Path) -> tuple[dict[str, str], dict]:
    """Entorno que la lane da por hecho, resuelto con las herramientas del repo.

    MEDIDO 2026-10-16. `merge-gate.pipeline.kts` exporta
    `CARGO_TARGET_DIR="$(scripts/ci/target-dir.sh)"` para las etapas que necesitan
    binarios construidos (líneas 441-442 y 476-477). El harness ejecutaba las
    suites con el entorno ambiente, así que `prf_h06_adversarial_e2e` —que busca
    el binario en `$CARGO_TARGET_DIR/{release,debug}`— fallaba con «cognicode-mcp
    binary not found» mientras el gate lo encontraba. El target no se inventa: se
    pregunta al script del repositorio, que es la autoridad que el gate ya usa.
    """
    script = repo_root / TARGET_DIR_SCRIPT
    evidence: dict = {"script": TARGET_DIR_SCRIPT}
    if not script.is_file():
        evidence["problem"] = "no existe el script del repositorio; no se resuelve el target dir"
        return {}, evidence
    try:
        proc = subprocess.run(
            ["bash", TARGET_DIR_SCRIPT],
            cwd=str(repo_root),
            capture_output=True,
            text=True,
            timeout=60,
            check=False,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        evidence["problem"] = f"no se pudo ejecutar {TARGET_DIR_SCRIPT}: {exc}"
        return {}, evidence
    resolved = proc.stdout.strip().splitlines()[-1].strip() if proc.stdout.strip() else ""
    if proc.returncode != 0 or not resolved:
        evidence["problem"] = (
            f"{TARGET_DIR_SCRIPT} salió con {proc.returncode} y no dio un directorio"
        )
        return {}, evidence
    evidence["resolved"] = resolved
    return {"CARGO_TARGET_DIR": resolved}, evidence


def run_plan(
    context: Context,
    plan_result: PlanResult,
    *,
    run_id: str,
    options: RunOptions,
) -> RunOutcome:
    """Ejecuta el plan. Escribe evidencia a medida que ocurre, nunca al final."""
    lane = options.lane
    snapshot = _snapshot(context)
    fail_fast = options.fail_fast if options.fail_fast is not None else lane in FAIL_FAST_LANES
    env_overrides = dict(options.env_overrides or {})
    environment_evidence: dict = {"source": "opciones explícitas"}
    if not env_overrides:
        env_overrides, environment_evidence = resolve_lane_environment(context.repo_root)
        environment_evidence["source"] = TARGET_DIR_SCRIPT
    lease = context.ledger.acquire_lease(run_id, purpose=f"run:{lane}")
    if not lease.acquired:
        raise HarnessError(
            f"otro proceso tiene el run {run_id}: {lease.reason}. Dos ejecuciones del "
            "mismo run duplicarían trabajo y podrían cruzar evidencia"
        )

    started_at = now_iso()
    state: dict = {
        "schema_version": RUN_SCHEMA,
        "run_id": run_id,
        "lane": lane,
        "repo_id": context.repo_id,
        "plan_id": plan_result.plan.plan_id,
        "source_fingerprint": plan_result.plan.source_fingerprint,
        "base_ref": getattr(context.changeset, "base_ref", None),
        "merge_base": getattr(context.changeset, "merge_base", None),
        "status": "RUNNING",
        "started_at": started_at,
        "finished_at": "",
        "suites": {},
        "pending": [],
        # El entorno con el que corrieron las suites es parte de la evidencia: un
        # resultado que depende del target dir no se puede leer sin saber cuál era.
        "environment": environment_evidence,
    }
    context.ledger.write_run(run_id, state)
    context.ledger.append_event(
        {"event": "run_started", "run_id": run_id, "lane": lane, "plan_id": plan_result.plan.plan_id}
    )

    executed: list[Receipt] = []
    reused: list[Receipt] = []
    coverage: list[Coverage] = []
    pending: list[str] = []
    interrupted = False
    verdict: GateVerdict | None = None
    state_lock = threading.Lock()
    stop = threading.Event()
    workers = max(1, min(options.max_workers, MAX_WORKERS_LIMIT))
    declared_locks = tuple(
        sorted({lock for suite in plan_result.suites for lock in getattr(suite.entry, "locks", ())})
    )
    gate = ResourceGate(declared_locks)

    def _record(suite_id: str, payload: dict) -> None:
        with state_lock:
            state["suites"][suite_id] = payload

    def _prepare(suite: PlannedSuite) -> str:
        """Resuelve lo que no ejecuta: filtro, BLOCK y REUSE. Devuelve el veredicto."""
        suite_id = suite.entry.suite_id
        if options.only and suite_id not in options.only:
            pending.append(suite_id)
            _record(suite_id, {"action": suite.action, "status": "SKIPPED_BY_FILTER"})
            return "filtered"
        if suite.action in ("BLOCK", "UNKNOWN"):
            # `UNKNOWN` no se ejecuta. El dispatch antiguo trataba cualquier
            # acción distinta de REUSE como «ejecutar», así que una suite que el
            # planificador declaraba indeterminada (por una variable de pipeline
            # sin expandir, por ejemplo) habría corrido igual: el veredicto
            # indeterminado se convertía en un intento de ejecución con un argv
            # que no era un comando.
            _record(
                suite_id,
                {"action": "BLOCK", "status": "BLOCKED", "reason": suite.action_reason},
            )
            return "blocked"
        if suite.action != "REUSE":
            return "execute"
        receipt, problem = context.ledger.latest_receipt_for_suite(suite_id)
        if receipt is None or problem:
            # La decisión se tomó con el mismo ledger hace un momento; si ya no
            # cuadra, se ejecuta en vez de inventar una prueba.
            suite.action = "EXECUTE"
            suite.action_reason += " (el recibo desapareció o no verifica: se ejecuta)"
            return "execute"
        reused.append(receipt)
        coverage.append(
            Coverage(
                suite_id=suite_id,
                receipt_id=receipt.receipt_id,
                fingerprint=receipt.input_fingerprint,
                how="REUSE",
            )
        )
        _record(
            suite_id, {"action": "REUSE", "status": "REUSED", "receipt_id": receipt.receipt_id}
        )
        return "reused"

    def _execute(suite: PlannedSuite) -> tuple[Receipt, bool]:
        """Ejecuta una suite con sus cerrojos. Escribe evidencia antes y después."""
        suite_id = suite.entry.suite_id
        with gate.hold(suite.entry):
            running = _running_receipt(
                run_id=run_id,
                context=context,
                suite=suite,
                lane=lane,
                snapshot=snapshot,
                started_at=now_iso(),
            )
            context.ledger.write_receipt(running)
            context.ledger.append_event(
                {
                    "event": "suite_running",
                    "run_id": run_id,
                    "suite_id": suite_id,
                    "receipt_id": running.receipt_id,
                    "serial": bool(getattr(suite.entry, "serial", False)),
                }
            )
            outcome, text = process_runner.execute(
                suite.argv,
                cwd=context.repo_root,
                env_overrides=env_overrides or None,
                timeout_s=options.timeout_s,
            )
        _, log_digest = context.ledger.write_log(run_id, suite_id, text.encode("utf-8"))
        terminal = Receipt(
            receipt_id=receipt_id_for(run_id, suite_id, suite.fingerprint.digest(), "terminal"),
            run_id=run_id,
            repo_id=context.repo_id,
            suite_id=suite_id,
            lane=lane,
            status=outcome.status,
            input_fingerprint=suite.fingerprint.digest(),
            source_snapshot=snapshot,
            runner=_runner_info(suite),
            started_at=running.started_at,
            finished_at=now_iso(),
            exit_code=outcome.exit_code,
            discovered_cases=outcome.counts.discovered,
            executed_cases=outcome.counts.executed,
            passed_cases=outcome.counts.passed,
            failed_cases=outcome.counts.failed,
            skipped_cases=outcome.counts.skipped,
            log_sha256=log_digest,
        )
        context.ledger.write_receipt(terminal)
        context.ledger.append_event(
            {
                "event": "suite_finished",
                "run_id": run_id,
                "suite_id": suite_id,
                "receipt_id": terminal.receipt_id,
                "status": terminal.status,
                "classification": outcome.classification,
                "duration_s": round(outcome.duration_s, 3),
            }
        )
        with state_lock:
            executed.append(terminal)
        _record(
            suite_id,
            {
                "action": "EXECUTE",
                "status": terminal.status,
                "classification": outcome.classification,
                "receipt_id": terminal.receipt_id,
                "duration_s": round(outcome.duration_s, 3),
                "serial": bool(getattr(suite.entry, "serial", False)),
            },
        )
        well_formed = terminal.is_well_formed_pass()
        if well_formed:
            with state_lock:
                coverage.append(
                    Coverage(
                        suite_id=suite_id,
                        receipt_id=terminal.receipt_id,
                        fingerprint=terminal.input_fingerprint,
                        how="EXECUTE",
                    )
                )
        elif fail_fast:
            stop.set()
        return terminal, well_formed

    def _execute_guarded(suite: PlannedSuite) -> tuple[Receipt | None, bool]:
        """Ejecuta y, si el supervisor falla, lo convierte en un hueco con nombre."""
        try:
            return _execute(suite)
        except Exception as exc:  # noqa: BLE001 - un fallo del supervisor no es un verde
            stop.set()
            _record(
                suite.entry.suite_id,
                {
                    "action": "EXECUTE",
                    "status": "INFRA_ERROR",
                    "reason": f"el supervisor falló: {exc}",
                },
            )
            context.ledger.append_event(
                {
                    "event": "suite_supervisor_error",
                    "run_id": run_id,
                    "suite_id": suite.entry.suite_id,
                    "error": str(exc)[:300],
                }
            )
            return None, False

    def _remaining() -> list[str]:
        with state_lock:
            seen = set(state["suites"])
        return [
            s.entry.suite_id
            for s in plan_result.suites
            if s.entry.suite_id not in seen and s.action == "EXECUTE"
        ]

    try:
        to_run: list[PlannedSuite] = []
        for suite in plan_result.suites:
            if _prepare(suite) == "execute":
                to_run.append(suite)

        if workers == 1:
            for suite in to_run:
                if stop.is_set():
                    break
                terminal, ok = _execute_guarded(suite)
                if fail_fast and not ok:
                    state["stop_reason"] = (
                        "modo fail-fast: "
                        + (terminal.suite_id if terminal else suite.entry.suite_id)
                        + " no terminó en verde"
                    )
                    break
        else:
            with ThreadPoolExecutor(max_workers=workers) as pool:
                futures = {}
                for suite in to_run:
                    if stop.is_set():
                        break
                    futures[pool.submit(_execute_guarded, suite)] = suite
                for future in as_completed(futures):
                    suite = futures[future]
                    terminal, ok = future.result()
                    if fail_fast and not ok:
                        state["stop_reason"] = (
                            "modo fail-fast: "
                            + (terminal.suite_id if terminal else suite.entry.suite_id)
                            + " no terminó en verde"
                        )
                        stop.set()
        if stop.is_set():
            remaining = _remaining()
            pending.extend(remaining)
            state.setdefault(
                "stop_reason",
                f"detenido tras un fallo; quedan {len(remaining)} obligaciones sin ejecutar",
            )
    except KeyboardInterrupt:
        interrupted = True
        stop.set()
        pending.extend(_remaining())
        state["stop_reason"] = "interrumpido por señal"
    finally:
        gate.close()
        receipts_by_suite = {receipt.suite_id: receipt for receipt in (*reused, *executed)}
        verdict = aggregate(
            lane=lane,
            repo_id=context.repo_id,
            source_fingerprint=plan_result.plan.source_fingerprint,
            required=plan_result.plan.required_suites,
            actions=plan_result.plan.actions,
            receipts=receipts_by_suite,
            coverage=tuple(coverage),
            minimums=plan_result.minimums(),
            unknown_reasons=plan_result.plan.unknown_reasons,
        )
        state["status"] = (
            "INTERRUPTED"
            if interrupted
            else ("FINISHED" if verdict.verdict == "SATISFIED" else "FAILED")
        )
        state["finished_at"] = now_iso()
        state["pending"] = pending
        state["verdict"] = verdict.to_json()
        state["counts"] = {
            "executed": len(executed),
            "reused": len(reused),
            "pending": len(pending),
            "blocked": sum(1 for s in state["suites"].values() if s.get("action") == "BLOCK"),
        }
        state["concurrency"] = {
            "requested": options.max_workers,
            "effective": workers,
            "locks": list(gate.lock_names()),
            "serial_suites": sorted(
                s.entry.suite_id for s in plan_result.suites if getattr(s.entry, "serial", False)
            ),
        }
        context.ledger.write_run(run_id, state)
        context.ledger.append_event(
            {
                "event": "run_finished",
                "run_id": run_id,
                "lane": lane,
                "verdict": verdict.verdict,
                "gaps": len(verdict.gaps),
                "max_workers": workers,
            }
        )
        context.ledger.release_lease(lease)

    assert verdict is not None
    return RunOutcome(
        run_id=run_id,
        plan_result=plan_result,
        verdict=verdict,
        executed=tuple(executed),
        reused=tuple(reused),
        pending=tuple(pending),
        interrupted=interrupted,
    )


def resume(
    context: Context,
    *,
    run_id: str,
    lane: str | None = None,
    allow_external: bool = False,
    mode: str = "continue",
    only: tuple[str, ...] = (),
    timeout_s: float | None = None,
    fail_fast: bool | None = None,
    max_workers: int = 1,
    oracle: str = "none",
) -> RunOutcome:
    """Reanuda un run recalculando el plan con el snapshot de ahora."""
    previous = context.ledger.read_run(run_id)
    if previous is None:
        raise HarnessError(f"no existe el run {run_id} en {context.ledger.root}")
    if previous.get("status") == "RUNNING":
        lease = context.ledger.acquire_lease(run_id, purpose="resume:probe")
        if not lease.acquired:
            raise HarnessError(
                f"el run {run_id} sigue vivo según su lease: {lease.reason}"
            )
        context.ledger.release_lease(lease)
        context.ledger.append_event(
            {
                "event": "run_interrupted_detected",
                "run_id": run_id,
                "note": "estado RUNNING sin dueño vivo: se reconcilia como interrumpido",
            }
        )
    effective_lane = lane or previous.get("lane") or "change"
    allow_reuse = reuse_allowed(effective_lane, False)[0]
    plan_result, _ = make_plan(
        context,
        lane=effective_lane,
        allow_reuse=allow_reuse,
        allow_external=allow_external,
        run_id=run_id,
        oracle=oracle,
    )
    if mode == "failed-only":
        executed_before = {
            suite_id
            for suite_id, info in (previous.get("suites") or {}).items()
            if (info or {}).get("status") == "PASSED"
        }
        still_required = []
        for suite in plan_result.suites:
            if suite.entry.suite_id in executed_before:
                shared = evaluate_reuse(
                    _find_receipt(context, suite.entry.suite_id),
                    suite.fingerprint,
                    lane_reuse_allowed=True,
                    lane_reason="resume --mode failed-only: se conserva lo que pasó y su huella sigue siendo la misma",
                )
                if shared.kind == "Reusable":
                    continue
            still_required.append(suite)
        plan_result.suites = tuple(still_required)
    options = RunOptions(
        lane=effective_lane,
        allow_reuse=allow_reuse,
        allow_external=allow_external,
        fail_fast=fail_fast,
        timeout_s=timeout_s,
        only=only,
        run_id=run_id,
        max_workers=max_workers,
    )
    return run_plan(context, plan_result, run_id=run_id, options=options)


def _unstable_suites(context: Context, required: tuple[str, ...]) -> dict[str, set[str]]:
    """Suites que han pasado y no pasado con la **misma** huella de entradas.

    Una suite así no es estable: el resultado depende de algo que la huella no
    captura. No se bloquea su reutilización (la política no cambia), pero tampoco
    se certifica que sea reproducible, que es lo que AAT-27 exige.
    """
    unstable: dict[str, set[str]] = {}
    for suite_id in required:
        by_fingerprint: dict[str, set[str]] = {}
        for receipt_id in context.ledger.receipts_for_suite(suite_id):
            receipt, problem = context.ledger.read_receipt(receipt_id)
            if receipt is None or problem or not receipt.is_terminal():
                continue
            by_fingerprint.setdefault(receipt.input_fingerprint, set()).add(receipt.status)
        if any({"PASSED"} < statuses and statuses != {"PASSED"} for statuses in by_fingerprint.values()):
            statuses = set().union(*by_fingerprint.values())
            unstable[suite_id] = statuses
    return unstable


def _find_receipt(context: Context, suite_id: str) -> Receipt | None:
    receipt, _ = context.ledger.latest_receipt_for_suite(suite_id)
    return receipt


def status(context: Context, run_id: str | None = None) -> dict:
    """Estado de un run: qué pasó, qué falta y por qué."""
    run = context.ledger.read_run(run_id) if run_id else context.ledger.latest_run()
    if run is None:
        raise HarnessError(
            f"no hay ningún run registrado en {context.ledger.root}"
            + (f" con id {run_id}" if run_id else "")
        )
    return run


def explain(context: Context, suite_id: str, *, lane: str, allow_reuse: bool = False,
            allow_external: bool = False) -> dict:
    """Por qué una suite está (o no está) en el plan de una lane."""
    plan_result, run_id = make_plan(
        context, lane=lane, allow_reuse=allow_reuse, allow_external=allow_external
    )
    payload: dict = {
        "suite_id": suite_id,
        "lane": lane,
        "run_id": run_id,
        "planned_suites": [suite.entry.suite_id for suite in plan_result.suites],
        "known_suites": [entry.suite_id for entry in context.catalog_for_lane(lane)],
    }
    for suite in plan_result.suites:
        if suite.entry.suite_id == suite_id:
            payload["decision"] = {
                "action": suite.action,
                "reason": suite.action_reason,
                "selection_reason": suite.reason,
                "input_fingerprint": suite.fingerprint.digest(),
                "fingerprint_inputs": suite.fingerprint.canonical(),
                "lanes": list(suite.entry.lanes),
                "argv": list(suite.argv),
                "origin": suite.entry.origin,
            }
            history = context.ledger.receipts_for_suite(suite_id)
            payload["receipts"] = list(history[:10])
            if history:
                receipt, problem = context.ledger.read_receipt(history[0])
                if receipt is None:
                    payload["last_receipt_problem"] = problem
                else:
                    decision: ReuseDecision = evaluate_reuse(
                        receipt,
                        suite.fingerprint,
                        lane_reuse_allowed=reuse_allowed(lane, allow_reuse)[0],
                        lane_reason=reuse_allowed(lane, allow_reuse)[1],
                        integrity_ok=problem is None,
                    )
                    payload["reuse"] = decision.to_json()
            return payload
    for entry, reason in plan_result.selection.excluded:
        if entry.suite_id == suite_id:
            payload["decision"] = {"action": "NOT_REQUIRED", "reason": reason}
            return payload
    payload["decision"] = {
        "action": "UNKNOWN",
        "reason": f"{suite_id} no está en el catálogo de la lane {lane}",
    }
    return payload


def certify(
    context: Context,
    *,
    lane: str,
    options: RunOptions,
) -> RunOutcome:
    """Certifica con evidencia fresca: aquí no se reutiliza nada, en ninguna lane."""
    allowed, reason = certify_requires_fresh(lane)
    if allowed:  # pragma: no cover - la política actual siempre exige fresco
        raise HarnessError("la política permitió reutilizar en certify, lo cual no está previsto")
    plan_result, run_id = make_plan(
        context, lane=lane, allow_reuse=False, allow_external=options.allow_external
    )
    outcome = run_plan(context, plan_result, run_id=options.run_id or run_id, options=options)
    unstable = _unstable_suites(context, plan_result.plan.required_suites)
    if unstable:
        outcome.verdict.gaps = outcome.verdict.gaps + tuple(
            {
                "suite_id": suite_id,
                "reason": (
                    "la suite pasó y falló con la misma huella de entradas: no se puede "
                    "certificar estabilidad sobre un resultado que se repite distinto "
                    f"({len(statuses)} ejecuciones divergentes con la misma huella)"
                ),
            }
            for suite_id, statuses in sorted(unstable.items())
        )
        outcome.verdict.verdict = "UNSATISFIED"
        outcome.notes = (
            *outcome.notes,
            "certify no certifica estabilidad con suites que divergen bajo la misma huella",
        )
    if outcome.reused:
        # No debería ocurrir, pero si ocurriera, la certificación no vale.
        outcome.verdict.gaps = outcome.verdict.gaps + (
            {
                "suite_id": "policy",
                "reason": "certify reutilizó recibos, y eso no certifica",
            },
        )
        outcome.verdict.verdict = "UNSATISFIED"
    outcome.notes = (f"certify {lane}: {reason}",)
    return outcome


def exit_code_for(outcome: RunOutcome) -> int:
    if outcome.interrupted:
        return EXIT_INTERRUPTED
    return outcome.verdict.exit_code()


def signal_exit_code(signum: int) -> int:  # pragma: no cover - usado por la CLI
    return EXIT_INTERRUPTED if signum in (signal.SIGINT, signal.SIGTERM) else EXIT_HARNESS_ERROR


__all__ = [
    "Context",
    "HarnessError",
    "RunOptions",
    "RunOutcome",
    "certify",
    "exit_code_for",
    "explain",
    "make_plan",
    "new_run_id",
    "prepare",
    "resume",
    "run_plan",
    "status",
    "EXIT_HARNESS_ERROR",
    "EXIT_INTERRUPTED",
    "EXIT_SATISFIED",
    "EXIT_UNDETERMINED",
    "EXIT_UNSATISFIED",
]
