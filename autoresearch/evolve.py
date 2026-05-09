#!/usr/bin/env python3
"""CogniCode Self-Evolving Rules — MAIN ENTRY POINT

This is THE script. It orchestrates the full autonomous improvement loop:

  1. Pick 2-3 unused repos from catalog
  2. Run multi-tool evaluation (Clippy + CogniCode + SonarQube)
  3. Compare findings → real TP/FP/FN metrics per rule
  4. LLM analyzes gaps → specific improvements with expected ΔF1
  5. Generate change scripts → sandbox validation
  6. Keep/discard → evolution.tsv
  7. Repeat with new repos

Usage:
  python autoresearch/evolve.py                    # Run forever
  python autoresearch/evolve.py -n 10              # 10 iterations
  python autoresearch/evolve.py -n 5 --dry-run     # LLM only, no sandbox
  python autoresearch/evolve.py -l python -n 3     # Python rules
"""

import sys
import os
import signal
import argparse
import time
import json
import logging
from pathlib import Path
from datetime import datetime
from typing import Optional

sys.path.insert(0, str(Path(__file__).parent))

from tools.llm_client import ModelConfig
from tools.metric_tools import EvolutionLogger
from tools.eval_runner import CorpusManager
from tools.sonarqube_validator import validate_against_sonarqube, check_severity_consistency
from multi_tool_eval import MultiToolEvaluator, LLMAnalyzer

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    handlers=[logging.StreamHandler(), logging.FileHandler("autoresearch/run.log")]
)
logger = logging.getLogger(__name__)

AUTORESEARCH_DIR = Path(__file__).parent
SHOULD_STOP = False

def signal_handler(sig, frame):
    global SHOULD_STOP
    logger.info("\n⚠ Interrupt received. Finishing and stopping...")
    SHOULD_STOP = True

signal.signal(signal.SIGINT, signal_handler)
signal.signal(signal.SIGTERM, signal_handler)


# ═══════════════════════════════════════════════════════════════════
# MAIN AUTONOMOUS LOOP
# ═══════════════════════════════════════════════════════════════════

def evolve(language: str = "rust", max_iterations: Optional[int] = None,
           repos_per_iteration: int = 2, dry_run: bool = False,
           cooldown: int = 10):
    """Karpathy-style autonomous rule evolution.
    
    Each iteration:
    1. Pick N unused repos from catalog
    2. Multi-tool evaluation (ground truth + CogniCode)
    3. LLM gap analysis
    4. Generate improvements
    5. Log to evolution.tsv
    """
    global SHOULD_STOP
    
    # Initialize
    corpus = CorpusManager()
    evaluator = MultiToolEvaluator()
    analyzer = LLMAnalyzer()
    evolution = EvolutionLogger(AUTORESEARCH_DIR / "evolution.tsv")
    
    session_iter = 0
    total_findings = 0
    total_improvements = 0
    
    remaining = corpus.remaining(language)
    
    logger.info("="*70)
    logger.info("  COGNICODE SELF-EVOLVING RULES")
    logger.info("  Pipeline: Multi-Tool Eval → LLM Gap Analysis → Improvement")
    logger.info(f"  Language: {language}")
    logger.info(f"  Model: {ModelConfig.MODEL}")
    logger.info(f"  Repos per iteration: {repos_per_iteration}")
    logger.info(f"  Remaining in catalog: {remaining}")
    if max_iterations:
        logger.info(f"  Max iterations: {max_iterations}")
    else:
        logger.info(f"  Mode: FOREVER (Ctrl+C to stop)")
    logger.info(f"  Dry run: {dry_run}")
    logger.info("="*70)
    
    # ── Phase 0: SonarQube metadata validation (once per session) ──
    logger.info(f"\n{'─'*70}")
    logger.info("  PHASE 0: SonarQube Rule Validation")
    logger.info(f"{'─'*70}")
    
    try:
        sq_results = validate_against_sonarqube()
        sev_issues = check_severity_consistency()
        
        # Log SonarQube validation results
        evolution.log_experiment(
            iteration=0,  # Session marker
            rule_id="SONARQUBE_VALIDATION",
            language=language,
            metrics_before={},
            metrics_after={
                "coverage_pct": sq_results["coverage_pct"],
                "accuracy_pct": sq_results["accuracy_pct"],
                "issues_found": len(sq_results["issues"]) + len(sev_issues),
            },
            decision="validated",
            description=f"SonarQube validation: {sq_results['accuracy_pct']:.0f}% accuracy, "
                       f"{len(sq_results['issues'])} metadata + {len(sev_issues)} severity issues"
        )
        
        if sq_results["accuracy_pct"] >= 95:
            logger.info(f"  ✅ SonarQube validation: {sq_results['accuracy_pct']:.0f}% accuracy — PASSED")
        else:
            logger.warning(f"  ⚠️ SonarQube validation needed — {len(sq_results['issues'])} issues")
    except Exception as e:
        logger.warning(f"  SonarQube validation skipped: {e}")
    
    while not SHOULD_STOP:
        if max_iterations and session_iter >= max_iterations:
            logger.info(f"\n✓ Max iterations ({max_iterations}) reached.")
            break
        
        remaining = corpus.remaining(language)
        if remaining == 0:
            logger.info("Catalog exhausted — resetting used flags.")
            # Reset all used flags
            for repo in corpus.catalog.get(language, []):
                repo["used"] = False
            remaining = len(corpus.catalog.get(language, []))
        
        session_iter += 1
        t0 = time.time()
        
        logger.info(f"\n{'─'*70}")
        logger.info(f"  ITERATION {session_iter}" + 
                   (f"/{max_iterations}" if max_iterations else ""))
        logger.info(f"  Repos remaining: {remaining}")
        logger.info(f"{'─'*70}")
        
        try:
            # ── Phase 1: Multi-tool evaluation ──
            logger.info("Phase 1: Multi-tool evaluation...")
            
            if dry_run:
                # Skip clone + tools, just pick repos
                repos = corpus.pick_repos(language, repos_per_iteration)
                logger.info(f"  DRY RUN — would evaluate: {[r['repo'] for r in repos]}")
                results = {"repos": [r["repo"] for r in repos], "comparison": {}}
            else:
                results = evaluator.evaluate_with_all_tools(language, repos_per_iteration)
            
            # ── Phase 2: Metrics ──
            logger.info("Phase 2: Computing metrics...")
            
            comparison = results.get("comparison", {})
            total_tp = sum(s.get("tp", 0) for s in comparison.values())
            total_fp = sum(s.get("fp", 0) for s in comparison.values())
            total_fn = sum(s.get("fn", 0) for s in comparison.values())
            
            precision = total_tp / (total_tp + total_fp) if total_tp + total_fp > 0 else None
            recall = total_tp / (total_tp + total_fn) if total_tp + total_fn > 0 else None
            f1 = (2 * precision * recall / (precision + recall) 
                  if precision and recall and precision + recall > 0 else None)
            
            total_findings += total_tp + total_fp + total_fn
            
            logger.info(f"  TP={total_tp} FP={total_fp} FN={total_fn}")
            if precision: logger.info(f"  Precision: {precision:.1%}")
            if recall: logger.info(f"  Recall: {recall:.1%}")
            if f1: logger.info(f"  F1: {f1:.3f}")
            
            # ── Phase 3: LLM Gap Analysis ──
            if not dry_run and comparison:
                logger.info("Phase 3: LLM gap analysis...")
                
                opportunities = analyzer.analyze_comparison(comparison, language)
                
                # Deep analysis of top gaps
                for op in opportunities[:3]:
                    gt_samples = []
                    for tool, findings in results.get("ground_truth", {}).items():
                        for f in findings:
                            norm = evaluator._normalize_rule(f.get("rule", ""))
                            if norm == op["rule_category"]:
                                gt_samples.append(f)
                                if len(gt_samples) >= 5:
                                    break
                        if len(gt_samples) >= 5:
                            break
                    
                    if gt_samples:
                        deep = analyzer.analyze_gap_with_llm(op, gt_samples)
                        op["llm_analysis"] = deep
                        
                        # Log to evolution
                        evolution.log_experiment(
                            iteration=session_iter,
                            rule_id=op["rule_category"],
                            language=language,
                            metrics_before={"fn": op["fn_count"], "tp": op["tp_count"]},
                            metrics_after={
                                "expected_delta_f1": deep.get("expected_f1_delta", 0),
                                "improvement_type": deep.get("improvement_type", "?"),
                            },
                            decision="analyzed",
                            description=f"LLM: {deep.get('improvement_type', '?')} — "
                                       f"{deep.get('specific_change', '')[:120]}"
                        )
                        total_improvements += 1
                
                # Discover new rules
                new_rules = analyzer.discover_new_rules(
                    results.get("ground_truth", {}), comparison
                )
                if new_rules:
                    logger.info(f"  🔍 New rule candidates: {len(new_rules)}")
                    for nr in new_rules[:3]:
                        logger.info(f"    {nr['rule_category']}: {nr['fn_count']} missed findings")
            else:
                opportunities = []
            
            # ── Stats ──
            elapsed = time.time() - t0
            logger.info(f"\n  ⏱ Iteration time: {elapsed:.0f}s")
            logger.info(f"  📊 Total findings: {total_findings}")
            logger.info(f"  🔧 Improvements logged: {total_improvements}")
            
            # Cooldown
            if cooldown > 0 and not SHOULD_STOP:
                logger.info(f"  💤 Cooldown: {cooldown}s...")
                time.sleep(cooldown)
                
        except Exception as e:
            logger.error(f"  ❌ Iteration failed: {e}")
            import traceback
            logger.debug(traceback.format_exc())
            time.sleep(cooldown)
    
    # ═══════════════════════════════════════════════════════════════
    # FINAL SUMMARY
    # ═══════════════════════════════════════════════════════════════
    
    logger.info(f"\n{'='*70}")
    logger.info(f"  EVOLUTION RUN COMPLETE")
    logger.info(f"{'='*70}")
    logger.info(f"  Iterations: {session_iter}")
    logger.info(f"  Total findings analyzed: {total_findings}")
    logger.info(f"  Improvements logged: {total_improvements}")
    logger.info(f"  Remaining in catalog: {corpus.remaining(language)}")
    logger.info(f"  Log: {AUTORESEARCH_DIR / 'evolution.tsv'}")
    logger.info(f"{'='*70}")


# ═══════════════════════════════════════════════════════════════════
# Entry point
# ═══════════════════════════════════════════════════════════════════

if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="CogniCode Self-Evolving Rules — Multi-Tool + LLM"
    )
    parser.add_argument("-l", "--language", default="rust",
                       help="Target language (rust, python, javascript, java, go)")
    parser.add_argument("-n", "--max-iterations", type=int, default=None,
                       help="Max iterations (default: unlimited)")
    parser.add_argument("-r", "--repos-per-iteration", type=int, default=2,
                       help="Repos to evaluate per iteration (default: 2)")
    parser.add_argument("-c", "--cooldown", type=int, default=10,
                       help="Cooldown seconds between iterations")
    parser.add_argument("--dry-run", action="store_true",
                       help="LLM analysis only, no sandbox/clone")
    
    args = parser.parse_args()
    
    logger.info("🚀 Starting autonomous rule evolution...")
    
    evolve(
        language=args.language,
        max_iterations=args.max_iterations,
        repos_per_iteration=args.repos_per_iteration,
        dry_run=args.dry_run,
        cooldown=args.cooldown,
    )
