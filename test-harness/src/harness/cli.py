"""CLI del harness: plan, run, status, resume, explain, certify.

Códigos de salida (TH-S03): `0` gate satisfecho, `1` obligaciones incumplidas,
`2` error de plan o de harness, `3` evidencia indeterminada o caducada, `130`
interrupción. La CLI no reinterpreta veredictos: los imprime.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .domain import canonical_json
from .gc import apply_gc, plan_gc
from .inventory import Inventory, inventory_from_receipts, write_inventory
from .orchestrator import (
    EXIT_HARNESS_ERROR,
    EXIT_INTERRUPTED,
    EXIT_UNDETERMINED,
    HarnessError,
    RunOptions,
    certify,
    exit_code_for,
    explain,
    make_plan,
    parity_corpus,
    prepare,
    resume,
    run_plan,
    status,
)
from .policy import POLICY_VERSION, reuse_allowed
from .report import (
    budget_plan,
    coverage_map,
    harness_timings,
    read_gate_profile,
    read_shadow_observations,
    shadow_summary,
    write_baseline,
)

DEFAULT_BASE = "origin/main"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="harness",
        description=(
            "Selecciona, ejecuta y reutiliza pruebas con evidencia verificable. "
            "PipelineK sigue siendo la autoridad del gate: esto no lo sustituye."
        ),
    )
    parser.add_argument("--repo", default=".", help="raíz del repositorio (por defecto: cwd)")
    parser.add_argument(
        "--state-dir",
        default=None,
        help="directorio del ledger (por defecto: $XDG_STATE_HOME/cognicode-test-harness/...)",
    )
    parser.add_argument("--format", choices=("text", "json"), default="text")
    parser.add_argument("--quiet", action="store_true", help="solo lo esencial en formato texto")

    sub = parser.add_subparsers(dest="command", required=True)

    plan = sub.add_parser("plan", help="calcula el plan y explica cada decisión")
    plan.add_argument("--lane", default="change", help="change | merge | integration | ...")
    plan.add_argument("--base", default=DEFAULT_BASE, help="gitref base para el merge-base")
    plan.add_argument("--allow-reuse", action="store_true", help="permite reutilizar fuera de la lane change")
    plan.add_argument("--allow-external", action="store_true", help="incluye suites que requieren sandbox/servicios")
    plan.add_argument(
        "--oracle",
        choices=("none", "cr08"),
        default="none",
        help="compara con el selector vigente y nunca exige menos que él (cr08)",
    )
    plan.add_argument(
        "--profile",
        choices=("gate", "dev"),
        default="gate",
        help="perfil de selección; 'dev' sólo en la lane change y nunca exige menos que el oráculo",
    )
    plan.add_argument("--shadow", action="store_true", help="modo observación: no ejecuta ni decide el gate")
    plan.add_argument("--run-id", default=None)

    run = sub.add_parser("run", help="ejecuta el plan y escribe recibos")
    run.add_argument("--lane", default="change")
    run.add_argument("--base", default=DEFAULT_BASE)
    run.add_argument("--allow-reuse", action="store_true")
    run.add_argument("--allow-external", action="store_true")
    run.add_argument(
        "--profile",
        choices=("gate", "dev"),
        default="gate",
        help="perfil de selección; 'dev' sólo en la lane change y nunca exige menos que el oráculo",
    )
    run.add_argument(
        "--oracle",
        choices=("none", "cr08"),
        default="none",
        help="igual que en `plan`: el plan que se ejecuta nunca exige menos que el selector (cr08)",
    )
    run.add_argument(
        "--plan-only",
        dest="plan_only",
        action="store_true",
        help="calcula y muestra el plan de esta ejecución sin ejecutar ninguna suite",
    )
    run.add_argument("--selection", default=None, help="plan-id que se pretende ejecutar")
    run.add_argument("--only", action="append", default=[], help="limita la ejecución a estas suites")
    run.add_argument("--timeout", type=float, default=None, help="segundos por suite")
    run.add_argument("--max-workers", type=int, default=1)
    run.add_argument("--run-id", default=None, help="id de run explícito (por defecto: derivado)")
    run.add_argument(
        "--fail-fast",
        dest="fail_fast",
        action="store_true",
        default=None,
        help="detiene el scheduling al primer fallo (por defecto: según lane)",
    )
    run.add_argument("--fail-collect", dest="fail_fast", action="store_false", help="sigue tras un fallo")

    resume_parser = sub.add_parser("resume", help="reanuda un run recalculando el plan")
    resume_parser.add_argument("--run-id", required=True)
    resume_parser.add_argument("--lane", default=None)
    resume_parser.add_argument("--mode", choices=("continue", "failed-only"), default="continue")
    resume_parser.add_argument("--base", default=DEFAULT_BASE)
    resume_parser.add_argument("--allow-external", action="store_true")
    resume_parser.add_argument("--only", action="append", default=[])
    resume_parser.add_argument("--timeout", type=float, default=None)
    resume_parser.add_argument("--max-workers", type=int, default=1)
    resume_parser.add_argument(
        "--oracle",
        choices=("none", "cr08"),
        default="none",
        help="igual que en `plan`/`run`: la reanudación no exige menos que el selector (cr08)",
    )

    status_parser = sub.add_parser("status", help="estado de un run")
    status_parser.add_argument("--run-id", default=None)

    explain_parser = sub.add_parser("explain", help="por qué una suite está o no en el plan")
    explain_parser.add_argument("--suite", required=True)
    explain_parser.add_argument("--lane", default="change")
    explain_parser.add_argument("--base", default=DEFAULT_BASE)
    explain_parser.add_argument("--run-id", default=None)
    explain_parser.add_argument("--allow-reuse", action="store_true")
    explain_parser.add_argument("--allow-external", action="store_true")

    certify_parser = sub.add_parser("certify", help="certifica con evidencia fresca")
    certify_parser.add_argument("--lane", default="certification")
    certify_parser.add_argument("--base", default=DEFAULT_BASE)
    certify_parser.add_argument("--candidate", default=None, help="sha de commit o digest de artefacto")
    certify_parser.add_argument("--allow-external", action="store_true")
    certify_parser.add_argument("--only", action="append", default=[])
    certify_parser.add_argument("--timeout", type=float, default=None)
    certify_parser.add_argument("--max-workers", type=int, default=1)

    inventory_parser = sub.add_parser(
        "inventory", help="pin del inventario de casos por suite (TH-S02)"
    )
    inventory_parser.add_argument("--write", action="store_true", help="reescribe el pin con lo observado")
    inventory_parser.add_argument("--check", action="store_true", help="compara el pin con los recibos")
    inventory_parser.add_argument("--from-run", default=None, help="run del que tomar los recuentos")
    inventory_parser.add_argument("--base", default=DEFAULT_BASE)

    report_parser = sub.add_parser(
        "report", help="informe reproducible de tiempos del gate y mapa de cobertura"
    )
    report_parser.add_argument("--lane", default="merge")
    report_parser.add_argument(
        "--pipelinek-db",
        default=".pipelinek/merge-gate.db",
        help="base durable de PipelineK de la que leer las duraciones (solo lectura)",
    )
    report_parser.add_argument(
        "--budget",
        type=float,
        default=None,
        help="presupuesto en segundos para el que calcular qué habría que dejar de ejecutar",
    )
    report_parser.add_argument(
        "--write-baseline",
        default=None,
        help="escribe el informe como documento versionado en esa ruta",
    )
    report_parser.add_argument(
        "--protect",
        action="append",
        default=[],
        help="etapa que no se puede tocar (contratos estructurales, lint, seguridad); repetible",
    )

    gc_parser = sub.add_parser("gc", help="borra logs de runs antiguos y conserva la evidencia")
    gc_parser.add_argument("--keep", type=int, default=20, help="runs cuyos logs se conservan")
    gc_parser.add_argument("--dry-run", action="store_true", help="dice qué borraría, sin borrar")
    gc_parser.add_argument("--base", default=None)

    parity_parser = sub.add_parser(
        "parity", help="compara la selección con el selector vigente (CR-08)"
    )
    parity_parser.add_argument("--lane", default="change")
    parity_parser.add_argument("--base", default=DEFAULT_BASE)
    parity_parser.add_argument(
        "--corpus",
        action="store_true",
        help="mide las clases de cambio del corpus en vez del cambio actual",
    )

    return parser


def _emit(payload: dict, args: argparse.Namespace, text: str) -> None:
    if args.format == "json":
        print(canonical_json(payload))
    else:
        print(text)


def _plan_text(plan_result, run_id: str, lane: str, context) -> str:
    plan = plan_result.plan
    counts = plan.counts()
    lines = [
        f"PLAN {plan.plan_id}  lane={lane}  estado={plan.status}  política={POLICY_VERSION}",
        f"  run_id={run_id}  repo={plan.repo_id}",
        f"  head={getattr(context.changeset, 'head', '')[:12]}  "
        f"base={getattr(context.changeset, 'base_ref', None)}  "
        f"merge_base={(getattr(context.changeset, 'merge_base', None) or '-')[:12]}  "
        f"dirty={getattr(context.changeset, 'dirty', False)}",
        f"  source_fingerprint={plan.source_fingerprint}",
        f"  obligaciones: {len(plan.required_suites)} requeridas "
        f"(EXECUTE {counts.get('EXECUTE', 0)}, REUSE {counts.get('REUSE', 0)}, "
        f"BLOCK {counts.get('BLOCK', 0)}), {len(plan_result.selection.excluded)} excluidas "
        f"de {len(context.catalog_for_lane(lane))} catalogadas",
    ]
    for suite in plan_result.suites:
        lines.append(f"  {suite.action:<7} {suite.entry.suite_id}")
        lines.append(f"          motivo: {suite.reason}")
        lines.append(f"          decisión: {suite.action_reason}")
        lines.append(f"          huella: {suite.fingerprint.digest()[:16]}")
    if plan_result.selection.excluded and not getattr(plan_result, "quiet", False):
        lines.append("  excluidas (motivo único por suite):")
        for entry, reason in plan_result.selection.excluded[:8]:
            lines.append(f"    - {entry.suite_id}: {reason}")
        remaining = len(plan_result.selection.excluded) - 8
        if remaining > 0:
            lines.append(f"    ... y {remaining} más (usa --format json para el detalle completo)")
    if plan.unknown_reasons:
        lines.append("  indeterminación (el plan cae al fallback conservador):")
        for reason in plan.unknown_reasons:
            lines.append(f"    - {reason}")
    if plan_result.parity is not None:
        parity = plan_result.parity
        lines.append(f"  paridad CR-08: {parity.verdict} (oráculo {parity.decision.strategy})")
        if parity.missing:
            lines.append(f"    scopes que el oráculo pedía y el plan no cubría: {', '.join(parity.missing)}")
        for deviation in parity.deviations:
            lines.append(f"    desviación declarada: {deviation[:160]}")
    for warning in (*context.warnings, *plan_result.warnings):
        lines.append(f"  aviso: {warning}")
    return "\n".join(lines)


def _shadow_text(plan_result, context, lane: str) -> str:
    """Resumen de observación: qué seleccionaría el harness, sin ejecutar nada.

    Va a un log de gate, así que dice lo que el lector necesita para decidir la
    adopción: cuántas obligaciones hay en la lane, cuántas exige este cambio, qué
    queda bloqueado por depender de estado externo y qué dijo el oráculo. Y dice,
    en la primera línea, que no decide nada: un log que se lee como un gate es
    peligroso.
    """
    catalog = context.catalog_for_lane(lane)
    plan = plan_result.plan
    counts = plan.counts()
    required = len(plan.required_suites)
    total = len(catalog)
    skipped = max(0, total - required)
    share = f"{skipped * 100 // total}%" if total else "n/a"
    lines = [
        "SHADOW: observación, no decide el gate. No se ha ejecutado nada y el gate "
        "sigue ejecutando sus etapas completas.",
        f"  lane={lane}  estado={plan.status}  head={getattr(context.changeset, 'head', '')[:12]}  "
        f"base={getattr(context.changeset, 'base_ref', None)}",
        f"  obligaciones de la lane: {total} catalogadas, {required} requeridas por impacto "
        f"(EXECUTE {counts.get('EXECUTE', 0)}, REUSE {counts.get('REUSE', 0)}, "
        f"BLOCK {counts.get('BLOCK', 0)}); {skipped} sin tocar ({share})",
        f"  source_fingerprint={plan.source_fingerprint}",
    ]
    blocked = [suite.entry.suite_id for suite in plan_result.suites if suite.blocked]
    if blocked:
        lines.append(f"  bloqueadas por estado externo: {', '.join(blocked)}")
    if plan_result.parity is not None:
        parity = plan_result.parity
        lines.append(f"  paridad CR-08: {parity.verdict} (oráculo {parity.decision.strategy}: {parity.decision.reason})")
        for deviation in parity.deviations:
            lines.append(f"  desviación declarada: {deviation[:160]}")
    for warning in (*context.warnings, *plan_result.warnings):
        lines.append(f"  aviso: {warning}")
    lines.append(
        "  adopción: esta observación no cambia ninguna etapa; decidir qué etapas "
        "pasarían a selección exige su propio ADR con la paridad delante"
    )
    return "\n".join(lines)


def _outcome_text(outcome) -> str:
    verdict = outcome.verdict
    lines = [
        f"RUN {outcome.run_id}  lane={verdict.lane}  veredicto={verdict.verdict}  política={verdict.policy_version}",
        f"  ejecutadas={len(outcome.executed)} reutilizadas={len(outcome.reused)} "
        f"pendientes={len(outcome.pending)} bloqueadas={len(verdict.gaps)}",
    ]
    for note in outcome.notes:
        lines.append(f"  nota: {note}")
    for receipt in outcome.executed:
        lines.append(
            f"  {receipt.status:<12} {receipt.suite_id}  casos={receipt.passed_cases} pasados/"
            f"{receipt.failed_cases} fallidos/{receipt.skipped_cases} omitidos  "
            f"recibo={receipt.receipt_id}"
        )
    for receipt in outcome.reused:
        lines.append(f"  REUSED       {receipt.suite_id}  recibo={receipt.receipt_id} (huella idéntica)")
    if outcome.pending:
        lines.append("  pendientes (no se ejecutaron, y el gate lo sabe):")
        for suite_id in outcome.pending:
            lines.append(f"    - {suite_id}")
    if verdict.gaps:
        lines.append("  huecos:")
        for gap in verdict.gaps:
            lines.append(f"    - {gap['suite_id']}: {gap['reason']}")
    if verdict.verdict == "SATISFIED":
        lines.append("  prueba: cada obligación tiene un PASS verificado con su huella")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    state_dir = Path(args.state_dir) if args.state_dir else None

    try:
        if args.command == "plan":
            context = prepare(Path(args.repo), state_dir=state_dir, base_ref=args.base)
            plan_result, run_id = make_plan(
                context,
                lane=args.lane,
                allow_reuse=args.allow_reuse,
                allow_external=args.allow_external,
                oracle=args.oracle,
                run_id=args.run_id,
                profile=args.profile,
            )
            # El documento del plan cumple el contrato tal cual; los extras de la
            # CLI (exclusiones con motivo, avisos y paridad) viajan al lado para no
            # romper `additionalProperties: false` en el esquema.
            payload = {
                "plan": plan_result.plan.to_json(),
                "run_id": run_id,
                "shadow": bool(args.shadow),
                "excluded": [
                    {"suite_id": entry.suite_id, "reason": reason}
                    for entry, reason in plan_result.selection.excluded
                ],
                "warnings": [*context.warnings, *plan_result.warnings],
            }
            if plan_result.parity is not None:
                payload["parity"] = plan_result.parity.to_json()
            text = (
                _shadow_text(plan_result, context, args.lane)
                if args.shadow
                else _plan_text(plan_result, run_id, args.lane, context)
            )
            if args.shadow:
                # La observación se guarda como evento append-only: es el dato con
                # el que se decide la adopción, y un log de gate no se puede
                # agregar. Un `plan` normal no escribe nada.
                counts = plan_result.plan.counts()
                catalog = context.catalog_for_lane(args.lane)
                context.ledger.append_event(
                    {
                        "event": "plan_observed",
                        "run_id": run_id,
                        "lane": args.lane,
                        "head": getattr(context.changeset, "head", ""),
                        "base": getattr(context.changeset, "base_ref", None),
                        "catalog": len(catalog),
                        "required": len(plan_result.plan.required_suites),
                        "execute": counts.get("EXECUTE", 0),
                        "reuse": counts.get("REUSE", 0),
                        "block": counts.get("BLOCK", 0),
                        "parity": plan_result.parity.verdict if plan_result.parity else None,
                        "parity_missing": list(plan_result.parity.missing) if plan_result.parity else [],
                        "fingerprint": plan_result.plan.source_fingerprint,
                    }
                )
            _emit(payload, args, text)
            if args.shadow:
                return 0
            if plan_result.plan.status == "UNKNOWN_INPUT":
                return EXIT_UNDETERMINED
            if plan_result.plan.status == "BLOCKED":
                return EXIT_UNDETERMINED
            return 0

        if args.command == "run":
            context = prepare(Path(args.repo), state_dir=state_dir, base_ref=args.base)
            plan_result, run_id = make_plan(
                context,
                lane=args.lane,
                allow_reuse=args.allow_reuse or args.lane == "change",
                allow_external=args.allow_external,
                run_id=args.run_id,
                oracle=args.oracle,
                profile=args.profile,
            )
            if args.plan_only:
                # El camino de ejecución se puede inspeccionar sin ejecutar. Sin
                # esto, la única forma de comprobar que el argv que construye un
                # consumidor (p. ej. scripts/test-fast.sh) es aceptable era
                # correr el plan entero, así que en la práctica no se comprobaba
                # nada: `run --oracle cr08` estuvo roto y nadie lo vio.
                payload = {
                    "plan": plan_result.plan.to_json(),
                    "parity": plan_result.parity.to_json() if plan_result.parity else None,
                    "run_id": run_id,
                    "plan_only": True,
                    "would_execute": [
                        suite.entry.suite_id
                        for suite in plan_result.suites
                        if suite.action == "EXECUTE"
                    ],
                }
                _emit(payload, args, f"PLAN-ONLY run={run_id} lane={args.lane}")
                if plan_result.plan.status == "UNKNOWN_INPUT":
                    return EXIT_UNDETERMINED
                if plan_result.plan.status == "BLOCKED":
                    return EXIT_UNDETERMINED
                return 0
            if args.selection and args.selection != plan_result.plan.plan_id:
                raise HarnessError(
                    f"el plan calculado ahora es {plan_result.plan.plan_id} y se pidió ejecutar "
                    f"{args.selection}: el código cambió entre plan y run, así que se replanifica"
                )
            options = RunOptions(
                lane=args.lane,
                allow_reuse=args.allow_reuse or args.lane == "change",
                allow_external=args.allow_external,
                fail_fast=args.fail_fast,
                timeout_s=args.timeout,
                only=tuple(args.only),
                run_id=run_id,
                max_workers=max(1, args.max_workers),
            )
            outcome = run_plan(context, plan_result, run_id=run_id, options=options)
            payload = {
                "run_id": outcome.run_id,
                "verdict": outcome.verdict.to_json(),
                "executed": [r.to_json() for r in outcome.executed],
                "reused": [
                    {"suite_id": r.suite_id, "receipt_id": r.receipt_id} for r in outcome.reused
                ],
                # El veredicto INCONCLUSIVE llega con su motivo: quien solo llama a
                # `run` (el wrapper fast, por ejemplo) tiene que poder decir por qué
                # el resultado no es determinable sin pedir el plan otra vez.
                "unknown_reasons": list(plan_result.plan.unknown_reasons),
                "pending": list(outcome.pending),
            }
            _emit(payload, args, _outcome_text(outcome))
            return exit_code_for(outcome)

        if args.command == "resume":
            context = prepare(Path(args.repo), state_dir=state_dir, base_ref=args.base)
            outcome = resume(
                context,
                run_id=args.run_id,
                lane=args.lane,
                allow_external=args.allow_external,
                mode=args.mode,
                only=tuple(args.only),
                timeout_s=args.timeout,
                max_workers=max(1, args.max_workers),
                oracle=args.oracle,
            )
            payload = {
                "run_id": outcome.run_id,
                "verdict": outcome.verdict.to_json(),
                "executed": [r.to_json() for r in outcome.executed],
                "reused": [
                    {"suite_id": r.suite_id, "receipt_id": r.receipt_id} for r in outcome.reused
                ],
                "pending": list(outcome.pending),
            }
            _emit(payload, args, _outcome_text(outcome))
            return exit_code_for(outcome)

        if args.command == "status":
            context = prepare(Path(args.repo), state_dir=state_dir, base_ref=None)
            run = status(context, args.run_id)
            counts = run.get("counts") or {}
            text = "\n".join(
                [
                    f"RUN {run.get('run_id')}  lane={run.get('lane')}  estado={run.get('status')}",
                    f"  plan={run.get('plan_id')}  iniciado={run.get('started_at')}  "
                    f"terminado={run.get('finished_at') or '-'}",
                    f"  ejecutadas={counts.get('executed', 0)} reutilizadas={counts.get('reused', 0)} "
                    f"pendientes={counts.get('pending', 0)} bloqueadas={counts.get('blocked', 0)}",
                    f"  veredicto={(run.get('verdict') or {}).get('verdict', 'sin agregar')}",
                    f"  ledger={context.ledger.root}",
                ]
                + ([f"  nota: {run['stop_reason']}"] if run.get("stop_reason") else [])
            )
            _emit(run, args, text)
            verdict = (run.get("verdict") or {}).get("verdict")
            if verdict == "SATISFIED":
                return 0
            if verdict == "UNSATISFIED":
                return 1
            return EXIT_UNDETERMINED

        if args.command == "explain":
            context = prepare(Path(args.repo), state_dir=state_dir, base_ref=args.base)
            payload = explain(
                context,
                args.suite,
                lane=args.lane,
                allow_reuse=args.allow_reuse,
                allow_external=args.allow_external,
            )
            decision = payload.get("decision", {})
            lines = [
                f"SUITE {payload['suite_id']}  lane={payload['lane']}  acción={decision.get('action')}",
                f"  motivo de selección: {decision.get('selection_reason', decision.get('reason'))}",
            ]
            if "input_fingerprint" in decision:
                lines.append(f"  huella={decision['input_fingerprint']}")
                lines.append(f"  argv={' '.join(decision.get('argv', []))}")
                lines.append(f"  definida en {decision.get('origin')} (lanes: {','.join(decision.get('lanes', []))})")
            if decision.get("reason") and decision.get("action") in ("NOT_REQUIRED", "UNKNOWN"):
                lines.append(f"  detalle: {decision['reason']}")
            if payload.get("reuse"):
                lines.append(
                    f"  reutilización: {payload['reuse']['kind']} — {payload['reuse']['reason']}"
                )
            if payload.get("last_receipt_problem"):
                lines.append(f"  problema de integridad: {payload['last_receipt_problem']}")
            if payload.get("receipts"):
                lines.append(f"  recibos históricos: {len(payload['receipts'])} (más reciente {payload['receipts'][0]})")
            _emit(payload, args, "\n".join(lines))
            return 0

        if args.command == "certify":
            context = prepare(Path(args.repo), state_dir=state_dir, base_ref=args.base)
            candidate_digest = None
            if args.candidate:
                if len(args.candidate) == 64 and all(c in "0123456789abcdef" for c in args.candidate):
                    candidate_digest = args.candidate
                else:
                    from adapters.git_repo import current_head

                    head = current_head(context.repo_root)
                    if not head.startswith(args.candidate) and not args.candidate.startswith(head):
                        raise HarnessError(
                            f"el candidato {args.candidate} no es el HEAD de este checkout ({head[:12]}): "
                            "certificar otro commit exige tenerlo materializado"
                        )
            options = RunOptions(
                lane=args.lane,
                allow_reuse=False,
                allow_external=args.allow_external,
                timeout_s=args.timeout,
                only=tuple(args.only),
                max_workers=max(1, args.max_workers),
            )
            outcome = certify(context, lane=args.lane, options=options)
            payload = {
                "run_id": outcome.run_id,
                "verdict": outcome.verdict.to_json(),
                "candidate_digest": candidate_digest,
                "notes": list(outcome.notes),
            }
            _emit(payload, args, _outcome_text(outcome))
            return exit_code_for(outcome)

        if args.command == "parity":
            context = prepare(Path(args.repo), state_dir=state_dir, base_ref=args.base)
            if args.corpus:
                report = parity_corpus(context, lane=args.lane)
                summary = report["summary"]
                lines = [
                    f"PARIDAD con CR-08  lane={args.lane}  clases={summary['total']}  "
                    f"ok={summary['parity_ok']}",
                ]
                for entry in report["entries"]:
                    verdict = entry["report"]["verdict"]
                    lines.append(
                        f"  {verdict:<17} {entry['path']}"
                        f"  (harness exige {entry['harness_required']}; oráculo "
                        f"{entry['report']['oracle']['strategy']}:{entry['report']['oracle']['reason']})"
                    )
                    for row in entry["report"]["rows"]:
                        if row["verdict"] != "AGREE":
                            lines.append(
                                f"        scope {row['scope']}: oráculo="
                                f"{'sí' if row['demanded_by_oracle'] else 'no'} "
                                f"harness={row['harness_how']} -> {row['verdict']}"
                            )
                    for note in entry["report"]["deviations"]:
                        lines.append(f"        desviación declarada: {note[:150]}")
                lines.append(f"  resumen de veredictos: {summary['verdicts']}")
                if summary["weak_undeclared"]:
                    lines.append(
                        "  SIN DESVIACIÓN DECLARADA (esto es un fallo, no un matiz): "
                        + ", ".join(summary["weak_undeclared"])
                    )
                _emit(report, args, "\n".join(lines))
                return 0 if summary["parity_ok"] else EXIT_UNDETERMINED

            plan_result, run_id = make_plan(context, lane=args.lane, oracle="cr08")
            report = plan_result.parity.to_json() if plan_result.parity else None
            the_plan = plan_result.plan.to_json()
            payload = {
                "plan": the_plan,
                "run_id": run_id,
                "parity": report,
                "excluded": [
                    {"suite_id": entry.suite_id, "reason": reason}
                    for entry, reason in plan_result.selection.excluded
                ],
            }
            if report is None:
                text = (
                    f"PARIDAD  lane={args.lane}  sin oráculo: {plan_result.oracle.to_json() if plan_result.oracle else 'no consultado'}"
                )
                _emit(payload, args, text)
                return EXIT_UNDETERMINED
            lines = [
                f"PARIDAD con CR-08  lane={args.lane}  veredicto={report['verdict']}",
                f"  oráculo: {report['oracle']['strategy']} ({report['oracle']['reason']})",
                f"  obligaciones exigidas por el harness: {len(plan_result.suites)}",
                f"  scopes que el harness añade sobre el oráculo: {report['extra_scopes'] or 'ninguno'}",
                f"  scopes que le faltarían: {report['missing_scopes'] or 'ninguno'}",
            ]
            for row in report["rows"]:
                if row["verdict"] != "AGREE":
                    lines.append(
                        f"  scope {row['scope']}: oráculo={row['demanded_by_oracle']} "
                        f"harness={row['harness_how']} -> {row['verdict']}"
                    )
            for deviation in report["deviations"]:
                lines.append(f"  desviación declarada: {deviation}")
            for note in report["notes"]:
                lines.append(f"  nota: {note}")
            _emit(payload, args, "\n".join(lines))
            if report["verdict"] == "HARNESS_WEAKER":
                return EXIT_UNDETERMINED
            if report["verdict"] == "ORACLE_UNKNOWN":
                return EXIT_UNDETERMINED
            return 0

        if args.command == "inventory":
            context = prepare(Path(args.repo), state_dir=state_dir, base_ref=args.base)
            receipts = []
            if args.from_run:
                run_state = context.ledger.read_run(args.from_run)
                if run_state is None:
                    raise HarnessError(f"no existe el run {args.from_run} en {context.ledger.root}")
                for suite_id in (run_state.get("suites") or {}):
                    receipt, _ = context.ledger.latest_receipt_for_suite(suite_id)
                    if receipt is not None:
                        receipts.append(receipt)
            else:
                for path in sorted(context.ledger.receipts_dir.glob("*.json")):
                    receipt, _ = context.ledger.read_receipt(path.stem)
                    if receipt is not None:
                        receipts.append(receipt)
            observed = inventory_from_receipts(receipts)
            current = context.inventory
            lines = [
                f"INVENTARIO  recibos leídos={len(receipts)}  pin actual={
                    len(current.entries)} suites  observado={len(observed.entries)} suites",
            ]
            for suite_id in sorted(set(current.entries) | set(observed.entries)):
                pinned = current.entries.get(suite_id)
                seen = observed.entries.get(suite_id)
                flag = ""
                if pinned is not None and seen is not None and seen < pinned:
                    flag = "  <-- por debajo del pin"
                lines.append(f"  {suite_id}: pin={pinned} observado={seen}{flag}")
            if current.problem:
                lines.append(f"  aviso: {current.problem}")
            under = [
                suite_id
                for suite_id in current.entries
                if suite_id in observed.entries and observed.entries[suite_id] < current.entries[suite_id]
            ]
            payload = {
                "pinned": current.to_json(),
                "observed": observed.to_json(),
                "under_pin": under,
                "problem": current.problem,
            }
            if args.write:
                written = write_inventory(context.repo_root, observed)
                lines.append(f"  pin reescrito en {written.relative_to(context.repo_root)}")
                payload["written"] = str(written.relative_to(context.repo_root))
            if args.check and under:
                _emit(payload, args, "\n".join(lines))
                return EXIT_UNDETERMINED
            _emit(payload, args, "\n".join(lines))
            return 0

        if args.command == "report":
            context = prepare(Path(args.repo), state_dir=state_dir, base_ref=None)
            catalog = context.catalog_for_lane(args.lane)
            coverage = coverage_map(catalog, args.lane)
            profile = read_gate_profile(Path(args.repo) / args.pipelinek_db)
            timings = harness_timings(context.ledger, args.lane)
            observations = read_shadow_observations(context.ledger)
            shadows = shadow_summary(observations, lane=args.lane)
            payload = {
                "lane": args.lane,
                "coverage": coverage,
                "gate": profile.to_json(),
                "harness_timings": timings.to_json(),
                "shadow": shadows,
            }
            lines = [
                f"INFORME lane={args.lane}  obligaciones catalogadas: {coverage['total']} "
                f"{coverage['by_kind']}",
                "  perfiles: " + ", ".join(
                    f"{name}={count}" for name, count in sorted(coverage["by_origin"].items())
                ),
            ]
            if profile.problem:
                lines.append(f"  perfil del gate: sin datos ({profile.problem})")
            else:
                lines.append(
                    f"  perfil del gate (PipelineK, run {(profile.run_id or '')[:8]}): "
                    f"{len(profile.stages)} etapas, {profile.total_seconds:.1f} s"
                )
                for stage in sorted(profile.stages, key=lambda s: -s.seconds)[:8]:
                    lines.append(f"    {stage.seconds:7.1f}s  {stage.name}")
            if timings.by_suite:
                lines.append(
                    f"  duraciones medidas por el harness (run {(timings.run_id or '')[:8]}): "
                    + ", ".join(
                        f"{suite}={seconds:.0f}s"
                        for suite, seconds in sorted(
                            timings.by_suite.items(), key=lambda item: -item[1]
                        )[:6]
                    )
                )
            if args.budget is not None and not profile.problem:
                plan = budget_plan(
                    profile, args.budget, protected=tuple(args.protect)
                )
                payload["budget"] = plan
                lines.append(
                    f"  presupuesto {args.budget:.0f}s: actual {plan['current_seconds']:.0f}s → "
                    f"{plan['after_seconds']:.0f}s dejando de ejecutar "
                    f"{len(plan['skip_candidates'])} etapa(s); "
                    + ("cabe" if plan["within_budget"] else f"faltan {plan['shortfall_seconds']:.0f}s")
                )
                for stage in plan["skip_candidates"][:8]:
                    lines.append(f"    dejar de ejecutar: {stage['stage']} ({stage['seconds']:.0f}s)")
                if plan["protected"]:
                    lines.append("    protegidas: " + ", ".join(plan["protected"]))
            if shadows["observations"]:
                lines.append(
                    f"  observaciones de sombra (lane {args.lane}): {shadows['observations']} "
                    f"planes, {shadows['avg_required_share']:.0%} de las obligaciones exigidas "
                    f"de media, bloqueadas {shadows['blocked_total']}; paridad {shadows['parity_verdicts']}"
                )
            else:
                lines.append(
                    "  observaciones de sombra: ninguna todavía (la etapa "
                    "harness-shadow-plan las guarda en cada merge)"
                )
            lines.append(
                "  aviso: el informe no decide nada; la adopción de la selección exige su ADR"
            )
            if args.write_baseline:
                target = Path(args.repo) / args.write_baseline
                written = write_baseline(
                    target,
                    payload,
                    commit=getattr(context.changeset, "head", ""),
                )
                lines.append(f"  baseline escrito en {target}")
                payload["baseline_path"] = str(written)
            _emit(payload, args, "\n".join(lines))
            return 0

        if args.command == "gc":
            context = prepare(Path(args.repo), state_dir=state_dir, base_ref=args.base)
            plan = plan_gc(context.ledger, args.keep)
            if not args.dry_run:
                plan = apply_gc(context.ledger, plan)
            lines = [
                f"GC  ledger={context.ledger.root}  runs conservados={len(plan.kept)}  "
                f"logs a borrar={len(plan.logs_to_delete)}  bytes={plan.bytes_to_free}"
                + ("  (dry-run: no se ha borrado nada)" if args.dry_run else ""),
            ]
            for note in plan.notes:
                lines.append(f"  nota: {note}")
            payload = plan.to_json()
            payload["dry_run"] = bool(args.dry_run)
            _emit(payload, args, "\n".join(lines))
            return 0

        raise HarnessError(f"comando desconocido: {args.command}")
    except HarnessError as exc:
        print(f"harness: error de plan/harness: {exc}", file=sys.stderr)
        return EXIT_HARNESS_ERROR
    except KeyboardInterrupt:
        print("harness: interrumpido", file=sys.stderr)
        return EXIT_INTERRUPTED


def policy_summary() -> str:
    lanes = ("change", "merge", "integration", "certification", "release-candidate", "release")
    return json.dumps(
        {
            "policy_version": POLICY_VERSION,
            "reuse": {lane: reuse_allowed(lane)[0] for lane in lanes},
        },
        sort_keys=True,
    )


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
