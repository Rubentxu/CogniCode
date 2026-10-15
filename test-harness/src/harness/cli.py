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
    prepare,
    resume,
    run_plan,
    status,
)
from .policy import POLICY_VERSION, reuse_allowed

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
    plan.add_argument("--shadow", action="store_true", help="modo observación: no ejecuta ni decide el gate")
    plan.add_argument("--run-id", default=None)

    run = sub.add_parser("run", help="ejecuta el plan y escribe recibos")
    run.add_argument("--lane", default="change")
    run.add_argument("--base", default=DEFAULT_BASE)
    run.add_argument("--allow-reuse", action="store_true")
    run.add_argument("--allow-external", action="store_true")
    run.add_argument("--selection", default=None, help="plan-id que se pretende ejecutar")
    run.add_argument("--only", action="append", default=[], help="limita la ejecución a estas suites")
    run.add_argument("--timeout", type=float, default=None, help="segundos por suite")
    run.add_argument("--max-workers", type=int, default=1)
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
    for warning in (*context.warnings, *plan_result.warnings):
        lines.append(f"  aviso: {warning}")
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
                run_id=args.run_id,
            )
            # El documento del plan cumple el contrato tal cual; los extras de la
            # CLI (exclusiones con motivo, avisos) viajan al lado para no romper
            # `additionalProperties: false` en el esquema.
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
            _emit(payload, args, _plan_text(plan_result, run_id, args.lane, context))
            if args.shadow:
                return 0
            if plan_result.plan.status == "UNKNOWN_INPUT":
                return EXIT_UNDETERMINED
            if plan_result.plan.status == "BLOCKED":
                return EXIT_UNDETERMINED
            return 0

        if args.command == "run":
            if args.max_workers != 1:
                raise HarnessError(
                    "--max-workers > 1 no está implementado: la ejecución en paralelo exige "
                    "locks por recurso y separación de suites seriales (W5 del roadmap). "
                    "Ejecutar en paralelo sin eso sería compartir estado sin saberlo"
                )
            context = prepare(Path(args.repo), state_dir=state_dir, base_ref=args.base)
            plan_result, run_id = make_plan(
                context, lane=args.lane, allow_reuse=args.allow_reuse or args.lane == "change",
                allow_external=args.allow_external,
            )
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
            )
            outcome = run_plan(context, plan_result, run_id=run_id, options=options)
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

        if args.command == "resume":
            if args.max_workers != 1:
                raise HarnessError("--max-workers > 1 no está implementado (ver W5)")
            context = prepare(Path(args.repo), state_dir=state_dir, base_ref=args.base)
            outcome = resume(
                context,
                run_id=args.run_id,
                lane=args.lane,
                allow_external=args.allow_external,
                mode=args.mode,
                only=tuple(args.only),
                timeout_s=args.timeout,
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
            if args.max_workers != 1:
                raise HarnessError("--max-workers > 1 no está implementado (ver W5)")
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
