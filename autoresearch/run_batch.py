#!/usr/bin/env python3
"""Batch autonomous rule improvement — Direct mode (no sandbox).

Usage:
    python autoresearch/run_batch.py --batch-size 20 --max-iterations 5

Strategy:
    1. Analyzer picks N rules (SQ targets + worst F1 + round-robin)
    2. For each rule: improver → evaluator → decider → P5/P6/P9
    3. All processing done directly on local repo (no containers)
    4. git checkout restores catalog.rs on failure
    5. Results logged to evolution.tsv

Reuses evolve.py functions: analyzer, improver, evaluator, decider,
segregate, revert_segregation, validate_with_cognicode, check_quality_gate.
"""

import sys
import argparse
import logging
import time
import subprocess
from pathlib import Path
from collections import defaultdict

sys.path.insert(0, str(Path(__file__).parent))

# Import all pipeline functions from evolve.py
from evolve import (
    analyzer, improver, evaluator, decider, segregate,
    revert_segregation, validate_with_cognicode, check_quality_gate,
    commit_msg, mark_done, record_failure, progress,
    load_session, is_done, is_retryable,
    FAILURE_COUNT, SESSION_DONE,
    CATALOG, ROOT,
    EvolutionLogger, BaselineStore, GitTool, ModelConfig,
)

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    handlers=[logging.StreamHandler(), logging.FileHandler("autoresearch/run.log")]
)
logger = logging.getLogger(__name__)

AUTORESEARCH_DIR = Path(__file__).parent


def run_batch(
    batch_size: int = 20,
    max_iterations: int = 5,
    auto_commit: bool = False,
):
    """Main batch loop — direct mode, no sandbox."""

    evolution = EvolutionLogger(AUTORESEARCH_DIR / "evolution.tsv")
    baseline_store = BaselineStore(AUTORESEARCH_DIR / "baseline")
    baseline = baseline_store.load()
    git = GitTool()

    load_session()
    history = evolution.read_history()

    total_alltime = len(history)
    session_iter = 0
    keeps = discards = fails = 0

    done, total_rules, pct = progress()

    logger.info("=" * 70)
    logger.info("  COGNICODE BATCH SELF-EVOLVING RULES (Direct Mode)")
    logger.info(f"  Batch size: {batch_size} rules/iteration")
    logger.info(f"  Model: {ModelConfig.MODEL}")
    logger.info(f"  Progress: {done}/{total_rules} rules ({pct}%)")
    logger.info("=" * 70)

    while session_iter < max_iterations:
        session_iter += 1
        t0 = time.time()
        batch_results = {}

        # ── ANALYZER: Pick rules ──
        targets = analyzer(history, force=None, batch=batch_size)

        if not targets:
            logger.warning("No rules available — catalog exhausted")
            break

        logger.info(f"\n{'─' * 70}")
        logger.info(f"  BATCH {session_iter}/{max_iterations}: {targets}")
        logger.info(f"{'─' * 70}")

        # ── Process each rule sequentially (direct mode) ──
        for rule_id in targets:
            total_alltime += 1

            # Check if already segregated
            import re
            if rule_id not in re.findall(r'id:\s*"(S\d+)"', CATALOG.read_text()):
                logger.info(f"  {rule_id} already segregated — skipping")
                mark_done(rule_id)
                batch_results[rule_id] = ("skipped", "already segregated")
                fails += 1
                continue

            # Previous F1
            f1_before = baseline.get(rule_id, {}).get("f1", 0) or 0

            # ── IMPROVER (3-tier) ──
            change = None
            for tier in range(3):
                change = improver(rule_id)
                if change.get("success") and change.get("level") == "code":
                    break
                err = change.get("error", "") if change else ""
                if any(kw in err for kw in ["no such group", "Invalid", "Expecting", "no JSON"]):
                    break

            if not change or not change.get("success"):
                record_failure(rule_id)
                reason = change.get("error", "?") if change else "?"
                if FAILURE_COUNT[rule_id] >= 3:
                    mark_done(rule_id)
                    logger.info(f"  {rule_id} — 3 failures, permanently skipped")
                else:
                    logger.info(f"  {rule_id} — retry #{FAILURE_COUNT[rule_id]}/3")

                evolution.log_experiment(
                    total_alltime, rule_id, "rust", {"f1": f1_before}, {},
                    "skipped", reason,
                    strategy=change.get("strategy", "") if change else ""
                )
                batch_results[rule_id] = ("skipped", reason)
                fails += 1
                continue

            # ── EVALUATOR ──
            metrics = evaluator(rule_id)
            if "error" in metrics:
                git.checkout(str(CATALOG))
                evolution.log_experiment(
                    total_alltime, rule_id, "rust", {"f1": f1_before}, {},
                    "failed", metrics["error"],
                    strategy=change.get("strategy", "")
                )
                batch_results[rule_id] = ("failed", metrics["error"])
                record_failure(rule_id)
                fails += 1
                continue

            # ── DECIDER ──
            decision, reason = decider(rule_id, baseline.get(rule_id, {}), metrics, change)

            if decision == "keep":
                # ── SEGREGATE ──
                seg_path = None
                try:
                    seg_path = segregate(rule_id)
                except Exception as e:
                    logger.error(f"  Segregation failed: {e}")
                    git.checkout(str(CATALOG))
                    evolution.log_experiment(
                        total_alltime, rule_id, "rust", {"f1": f1_before}, {},
                        "discard", f"segregation error: {e}",
                        strategy=change.get("strategy", "")
                    )
                    batch_results[rule_id] = ("discard", f"segregation error: {e}")
                    discards += 1
                    mark_done(rule_id)
                    continue

                if not seg_path:
                    logger.warning("  Segregation returned no path — discarding")
                    git.checkout(str(CATALOG))
                    evolution.log_experiment(
                        total_alltime, rule_id, "rust", {"f1": f1_before}, {},
                        "discard", "segregation failed",
                        strategy=change.get("strategy", "")
                    )
                    batch_results[rule_id] = ("discard", "segregation failed")
                    discards += 1
                    mark_done(rule_id)
                    continue

                # P5: Validate compilation after segregation
                from evolve import validate_segregation
                if not validate_segregation(rule_id, seg_path):
                    logger.warning(f"  P5: Validation failed — reverting {rule_id}")
                    revert_segregation(rule_id, seg_path)
                    evolution.log_experiment(
                        total_alltime, rule_id, "rust", {"f1": f1_before}, {},
                        "discard", "compilation failed after segregation (P5)",
                        strategy=change.get("strategy", "")
                    )
                    batch_results[rule_id] = ("discard", "P5 failed")
                    discards += 1
                    mark_done(rule_id)
                    continue

                # P6: Quality validation
                quality_ok, quality_msg = validate_with_cognicode(seg_path, rule_id)
                if not quality_ok:
                    logger.warning(f"  P6: Quality failed for {rule_id} — {quality_msg}")
                    revert_segregation(rule_id, seg_path)
                    evolution.log_experiment(
                        total_alltime, rule_id, "rust", {"f1": f1_before}, {},
                        "discard", f"P6: {quality_msg}",
                        strategy=change.get("strategy", "")
                    )
                    batch_results[rule_id] = ("discard", f"P6: {quality_msg}")
                    discards += 1
                    mark_done(rule_id)
                    continue

                # P9: Quality gate
                gate_ok, gate_msg = check_quality_gate(rule_id, seg_path)
                if not gate_ok:
                    logger.warning(f"  P9: Quality gate failed for {rule_id} — {gate_msg}")

                # ── COMMIT (optional) ──
                if auto_commit:
                    r = subprocess.run(
                        ["git", "add", "-f",
                         "crates/cognicode-axiom/src/rules/catalog.rs",
                         "crates/cognicode-axiom/src/rules/rules/"],
                        capture_output=True, cwd=str(ROOT)
                    )
                    if r.returncode == 0:
                        git.commit(commit_msg(rule_id, change))
                        baseline[rule_id] = metrics
                        baseline_store.save(baseline)
                        keeps += 1
                        mark_done(rule_id)
                    else:
                        logger.warning("git add failed — reverting")
                        git.checkout(str(CATALOG))
                        discards += 1
                else:
                    logger.info(f"  {rule_id} — kept in working tree (no auto-commit)")
                    keeps += 1
                    mark_done(rule_id)
            else:
                git.checkout(str(CATALOG))
                discards += 1
                mark_done(rule_id)

            # ── Log ──
            desc = f"{change.get('type', '?')}: {change.get('description', '')[:120]}"
            evolution.log_experiment(
                total_alltime, rule_id, "rust", {"f1": f1_before}, metrics,
                decision, desc,
                strategy=change.get("strategy", "")
            )
            batch_results[rule_id] = (decision, desc)
            logger.info(f"  {rule_id} → {decision.upper()}: {reason}")

        # ── Batch report ──
        elapsed = int(time.time() - t0)
        total_batch = keeps + discards + fails
        keep_rate = 0 if keeps + discards == 0 else int(keeps / (keeps + discards) * 100)
        done, total_rules, pct = progress()

        logger.info(f"\n  ┌{'─' * 60}")
        logger.info(f"  │ Batch {session_iter}: {len(targets)} rules in {elapsed}s")
        logger.info(f"  │ {keeps}✅ kept | {discards}❌ discarded | {fails}⚠️ failed — rate {keep_rate}%")
        for rid in targets:
            if rid in batch_results:
                dec, desc = batch_results[rid]
                desc = (desc or "")[:55]
                icon = "✅" if dec == "keep" else ("❌" if dec == "discard" else "⚠️")
                logger.info(f"  │  {icon} {rid:<7} {dec:<8} {desc}")
        logger.info(f"  └{'─' * 60}")
        logger.info(f"  📋 Progress: {done}/{total_rules} rules ({pct}%)")

        # Self-check every 10 batches
        if session_iter % 10 == 0:
            logger.info("  🛡️ Self-check: running full test suite...")
            r = subprocess.run(
                ["cargo", "test", "-p", "cognicode-axiom", "--lib"],
                capture_output=True, text=True, timeout=120, cwd=str(ROOT)
            )
            if "test result: ok" in (r.stdout + r.stderr):
                logger.info("  ✅ Tests OK")
            else:
                logger.error("  ❌ Tests FAILED — stopping for safety")
                break

        time.sleep(2)  # Brief cooldown

    # ── Final summary ──
    done, total_rules, pct = progress()
    logger.info(f"\n{'=' * 60}")
    logger.info(f"  BATCH RUN COMPLETE — {session_iter} batches")
    logger.info(f"  Kept: {keeps} | Discarded: {discards} | Failed: {fails}")
    if keeps + discards > 0:
        logger.info(f"  Keep rate: {keep_rate}%")
    logger.info(f"  Rules processed: {done}/{total_rules} ({pct}%)")
    logger.info(f"{'=' * 60}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Batch autonomous rule improvement (direct mode)")
    parser.add_argument("--batch-size", "-b", type=int, default=20)
    parser.add_argument("--max-iterations", "-n", type=int, default=5)
    parser.add_argument("--commit", action="store_true", help="Auto-commit kept changes")
    args = parser.parse_args()

    run_batch(
        batch_size=args.batch_size,
        max_iterations=args.max_iterations,
        auto_commit=args.commit,
    )
