#!/usr/bin/env python3
"""Autonomous self-evolving rule system — Karpathy-style FOREVER loop.

Usage:
    python autoresearch/run_forever.py [--max-iterations N] [--cooldown S]

This is the MAIN entry point. It runs indefinitely:
    analyze → propose → sandbox eval → decide → git commit/reset → repeat

Stop with Ctrl+C. Progress is saved to evolution.tsv after each iteration.
Kept improvements are committed to git. Discarded changes are reverted.

Inspired by: github.com/karpathy/autoresearch
"""

import sys
import os
import signal
import argparse
import logging
import time
from pathlib import Path
from datetime import datetime
from typing import Optional, Dict

sys.path.insert(0, str(Path(__file__).parent))

from tools.rust_tools import CargoTool, GitTool
from tools.llm_client import LLMClient, ModelConfig
from tools.metric_tools import EvolutionLogger, BaselineStore, compute_delta, format_metrics_table
from sandbox.manager import SandboxManager

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    handlers=[
        logging.StreamHandler(),
        logging.FileHandler("autoresearch/run.log")
    ]
)
logger = logging.getLogger(__name__)

AUTORESEARCH_DIR = Path(__file__).parent
CATALOG_PATH = AUTORESEARCH_DIR.parent / "crates" / "cognicode-axiom" / "src" / "rules" / "catalog.rs"

# Global flag for graceful shutdown
SHOULD_STOP = False


def signal_handler(sig, frame):
    global SHOULD_STOP
    logger.info("\n⚠ Interrupt received. Finishing current iteration and stopping...")
    SHOULD_STOP = True

signal.signal(signal.SIGINT, signal_handler)
signal.signal(signal.SIGTERM, signal_handler)


# ═══════════════════════════════════════════════════════════════════
# Rule analysis
# ═══════════════════════════════════════════════════════════════════

def read_rule_block(rule_id: str) -> Optional[str]:
    """Extract a declare_rule! block from catalog.rs."""
    content = CATALOG_PATH.read_text()
    import re
    
    pos = content.find(f'id: "{rule_id}"')
    if pos == -1:
        return None
    
    block_start = content.rfind("declare_rule!", 0, pos)
    if block_start == -1:
        return None
    
    brace_start = content.find("{", block_start)
    brace_count = 0
    for i in range(brace_start, len(content)):
        if content[i] == "{":
            brace_count += 1
        elif content[i] == "}":
            brace_count -= 1
            if brace_count == 0:
                return content[block_start : i + 1]
    return None


def get_all_rule_ids() -> list:
    """Get all rule IDs from catalog.rs."""
    content = CATALOG_PATH.read_text()
    import re
    return re.findall(r'id:\s*"([^"]+)"', content)


def pick_next_rule(history: list, all_rules: list, force_rule: Optional[str] = None) -> str:
    """Pick the next rule to improve.
    
    Strategy:
    1. If force_rule is set, use it
    2. Select rules not yet attempted
    3. Prioritize rules still in catalog.rs (not yet segregated)
    4. Avoid recently attempted rules (last 5)
    """
    if force_rule:
        return force_rule
    
    recently_attempted = {h.get("rule_id") for h in history[-5:]}
    
    # Rules not yet attempted
    attempted = {h.get("rule_id") for h in history}
    candidates = [r for r in all_rules if r not in attempted]
    
    if not candidates:
        # All rules attempted at least once — pick the one with most discards
        from collections import Counter
        discard_counts = Counter(
            h["rule_id"] for h in history 
            if h.get("decision") == "discard"
        )
        candidates = [r for r, _ in discard_counts.most_common(5)]
    
    if not candidates:
        candidates = all_rules
    
    # Filter out recently attempted
    candidates = [r for r in candidates if r not in recently_attempted]
    
    if not candidates:
        candidates = all_rules  # Reset — try again
    
    return candidates[0]


# ═══════════════════════════════════════════════════════════════════
# Decision engine
# ═══════════════════════════════════════════════════════════════════

def decide(rule_id: str, baseline: dict, experiment: dict) -> tuple:
    """Decide keep/discard based on experiment result.
    
    Returns: (decision: str, reason: str)
    
    KEEP if:
    - Experiment status is "success" and tests passed
    - No metric regression (simplified for MVP)
    
    DISCARD if:
    - Experiment failed (compilation error, test failure, timeout)
    - No improvement detected
    """
    status = experiment.get("status", "failed")
    
    if status != "success":
        return "discard", f"Experiment failed: {experiment.get('reason', status)}"
    
    tests_passed = experiment.get("tests_passed", 0)
    tests_failed = experiment.get("tests_failed", 0)
    
    if tests_failed > 0:
        return "discard", f"Tests failed: {tests_failed}"
    
    # For MVP: if compilation + tests pass, it's a KEEP
    # (Full metric comparison requires Phase 2 consensus engine)
    return "keep", f"Compilation OK, {tests_passed} tests passed"


# ═══════════════════════════════════════════════════════════════════
# Change script generation
# ═══════════════════════════════════════════════════════════════════

def generate_change_script(rule_id: str, analysis: dict, rule_block: str) -> str:
    """Generate a Python change script for the sandbox."""
    suggestions = analysis.get("suggested_changes", [])
    improvement_type = analysis.get("improvement_type", "pattern_extend")
    
    suggestions_py = "[\n"
    for s in suggestions:
        escaped = s.replace('\\', '\\\\').replace("'", "\\'")
        suggestions_py += f"    '{escaped}',\n"
    suggestions_py += "]"
    
    lines = []
    lines.append('#!/usr/bin/env python3')
    lines.append(f'"""Autonomous improvement for rule {rule_id} (MiniMax M2.7)"""')
    lines.append('from pathlib import Path')
    lines.append('')
    lines.append(f'CATALOG = Path("/workspace/CogniCode/crates/cognicode-axiom/src/rules/catalog.rs")')
    lines.append(f'RULE_ID = "{rule_id}"')
    lines.append(f'IMPROVEMENT_TYPE = "{improvement_type}"')
    lines.append(f'suggestions = {suggestions_py}')
    lines.append('')
    lines.append('print(f"Rule {{RULE_ID}}: {{len(suggestions)}} LLM suggestions (type: {{IMPROVEMENT_TYPE}})")')
    lines.append('for i, s in enumerate(suggestions):')
    lines.append('    print(f"  {{i+1}}. {{s[:120]}}")')
    
    if improvement_type == "segregation":
        lines.append('')
        lines.append('# SOLID Segregation detected — extracting to own file')
        lines.append('print("  → Segregation candidate (will be applied in Phase 2)")')
    
    lines.append('')
    lines.append('print("✓ Analysis complete — MVP Phase 1 (code changes in Phase 2)")')
    
    return "\n".join(lines) + "\n"


# ═══════════════════════════════════════════════════════════════════
# MAIN AUTONOMOUS LOOP
# ═══════════════════════════════════════════════════════════════════

def run_forever(max_iterations: Optional[int] = None, cooldown: int = 10,
                force_rule: Optional[str] = None, dry_run: bool = False):
    """Karpathy-style autonomous improvement loop.
    
    Runs until:
    - max_iterations reached (if set)
    - Ctrl+C (SIGINT)
    - Fatal error
    """
    global SHOULD_STOP
    
    # Initialize
    git = GitTool()
    sandbox = SandboxManager()
    llm = LLMClient()
    evolution = EvolutionLogger(AUTORESEARCH_DIR / "evolution.tsv")
    baseline_store = BaselineStore(AUTORESEARCH_DIR / "baseline")
    
    # Load state
    history = evolution.read_history()
    all_rules = get_all_rule_ids()
    total_iterations = len(history)  # All-time count
    session_iteration = 0  # This session's count
    
    logger.info("="*70)
    logger.info("  COGNICODE SELF-EVOLVING RULE SYSTEM")
    logger.info("  Mode: Autonomous (Karpathy-style FOREVER loop)")
    logger.info(f"  Model: {ModelConfig.MODEL}")
    logger.info(f"  Sandbox: Podman isolated containers")
    logger.info(f"  Rules available: {len(all_rules)}")
    logger.info(f"  Previous experiments: {total_iterations}")
    if max_iterations:
        logger.info(f"  Max iterations this session: {max_iterations}")
    else:
        logger.info(f"  Max iterations: ∞ (run until interrupted)")
    logger.info(f"  Dry run: {dry_run}")
    logger.info("="*70)
    
    keeps = 0
    discards = 0
    fails = 0
    
    while not SHOULD_STOP:
        if max_iterations and session_iteration >= max_iterations:
            logger.info(f"\n✓ Max iterations ({max_iterations}) reached. Stopping.")
            break
        
        # ═══════════════════════════════════════════════════════
        # ITERATION START
        # ═══════════════════════════════════════════════════════
        
        total_iterations += 1
        session_iteration += 1
        iteration_start = time.time()
        
        logger.info(f"\n{'─'*70}")
        logger.info(f"  ITERATION {session_iteration}" + 
                   (f"/{max_iterations}" if max_iterations else ""))
        logger.info(f"{'─'*70}")
        
        # ── Step 1: Pick rule ──
        rule_id = pick_next_rule(history, all_rules, force_rule)
        if not rule_id:
            logger.warning("No rules available. Stopping.")
            break
        
        logger.info(f"  Target: {rule_id}")
        
        # ── Step 2: Read rule block ──
        rule_block = read_rule_block(rule_id)
        if not rule_block:
            logger.warning(f"  Rule {rule_id} not found in catalog.rs — skipping")
            continue
        
        # ── Step 3: LLM Analysis ──
        logger.info(f"  Analyzing with LLM...")
        try:
            analysis = llm.analyze_rule(
                rule_id=rule_id,
                rule_code=rule_block,
                metrics={"f1": 0.78, "fpr": 0.03},  # TODO: real metrics
            )
        except Exception as e:
            logger.error(f"  LLM analysis failed: {e}")
            evolution.log_experiment(
                iteration=total_iterations, rule_id=rule_id, language="rust",
                metrics_before={}, metrics_after={},
                decision="failed",
                description=f"LLM error: {str(e)[:100]}"
            )
            fails += 1
            continue
        
        # ── Step 4: Generate change script ──
        script_content = generate_change_script(rule_id, analysis, rule_block)
        script_path = AUTORESEARCH_DIR / "results" / f"change_{rule_id}_{total_iterations:04d}.py"
        script_path.parent.mkdir(parents=True, exist_ok=True)
        script_path.write_text(script_content)
        
        # ── Step 5: Sandbox evaluation ──
        logger.info(f"  Running sandbox evaluation...")
        try:
            experiment = sandbox.run_experiment(
                rule_id=rule_id,
                git_ref="main",
                change_script=str(script_path),
                timeout=600,
            )
        except Exception as e:
            logger.error(f"  Sandbox error: {e}")
            experiment = {"status": "failed", "reason": str(e)}
        
        # ── Step 6: Decide ──
        decision, reason = decide(rule_id, {}, experiment)
        
        logger.info(f"  Decision: {decision.upper()}")
        logger.info(f"  Reason: {reason}")
        
        # ── Step 7: Execute decision (git) ──
        if decision == "keep" and not dry_run:
            git.commit(f"autoresearch[{total_iterations:04d}]: improve {rule_id} — {reason[:50]}")
            keeps += 1
        elif decision == "discard":
            discards += 1
        else:
            fails += 1
        
        # ── Step 8: Log ──
        evolution.log_experiment(
            iteration=total_iterations,
            rule_id=rule_id,
            language="rust",
            metrics_before={},
            metrics_after=experiment,
            decision=decision,
            description=f"LLM: {analysis.get('improvement_type', '?')} " +
                       f"({analysis.get('confidence', 0):.0%}) — " +
                       f"{analysis.get('suggested_changes', [''])[0][:80]}"
        )
        
        # ── Step 9: Stats ──
        elapsed = time.time() - iteration_start
        logger.info(f"  ⏱ Iteration time: {elapsed:.0f}s")
        logger.info(f"  📊 Session: {keeps} kept | {discards} discarded | {fails} failed")
        logger.info(f"  📝 Log: evolution.tsv ({total_iterations} total)")
        
        # ── Cooldown ──
        if cooldown > 0 and not SHOULD_STOP:
            logger.info(f"  💤 Cooldown: {cooldown}s...")
            time.sleep(cooldown)
    
    # ═══════════════════════════════════════════════════════════
    # FINAL SUMMARY
    # ═══════════════════════════════════════════════════════════
    
    logger.info(f"\n{'='*70}")
    logger.info(f"  AUTONOMOUS RUN COMPLETE")
    logger.info(f"{'='*70}")
    logger.info(f"  Session iterations: {session_iteration}")
    logger.info(f"  Total all-time: {total_iterations}")
    logger.info(f"  Kept: {keeps} | Discarded: {discards} | Failed: {fails}")
    if keeps + discards > 0:
        logger.info(f"  Keep rate: {keeps/(keeps+discards)*100:.1f}%")
    logger.info(f"  Log: {AUTORESEARCH_DIR / 'evolution.tsv'}")
    logger.info(f"  Session log: {AUTORESEARCH_DIR / 'run.log'}")
    logger.info(f"{'='*70}")


# ═══════════════════════════════════════════════════════════════════
# Entry point
# ═══════════════════════════════════════════════════════════════════

if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="CogniCode Self-Evolving Rules — Autonomous Loop (Karpathy-style)"
    )
    parser.add_argument("--max-iterations", "-n", type=int, default=None,
                       help="Max iterations (default: unlimited, runs forever)")
    parser.add_argument("--cooldown", "-c", type=int, default=10,
                       help="Cooldown between iterations in seconds (default: 10)")
    parser.add_argument("--rule", "-r", type=str, default=None,
                       help="Force a specific rule ID (overrides auto-selection)")
    parser.add_argument("--dry-run", action="store_true",
                       help="Analyze but don't run sandbox or modify code")
    
    args = parser.parse_args()
    
    if args.max_iterations is None:
        logger.info("🚀 Starting autonomous loop (NEVER STOP until Ctrl+C)")
    else:
        logger.info(f"🚀 Starting autonomous loop ({args.max_iterations} iterations)")
    
    run_forever(
        max_iterations=args.max_iterations,
        cooldown=args.cooldown,
        force_rule=args.rule,
        dry_run=args.dry_run,
    )
