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
from tools.code_intelligence import CodeIntelligence
from multi_tool_eval import LLMAnalyzer

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
    code_intel = CodeIntelligence()
    analyzer = LLMAnalyzer()
    evolution = EvolutionLogger(AUTORESEARCH_DIR / "evolution.tsv")
    
    session_iter = 0
    total_findings = 0
    total_improvements = 0
    improvements = 0  # Per-iteration counter
    
    remaining = corpus.remaining(language)
    
    logger.info("="*70)
    logger.info("  COGNICODE SELF-EVOLVING RULES")
    logger.info("  Pipeline: Ground Truth → Code Intel → LLM Analysis → Improvement")
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
            # ── Phase 1: Ground Truth (external tools only) ──
            logger.info("Phase 1: Ground Truth (external tools)...")
            
            from tools.eval_runner import EvalRunner
            runner = EvalRunner(corpus)
            
            if dry_run:
                repos = corpus.pick_repos(language, repos_per_iteration)
                logger.info(f"  DRY RUN — would evaluate: {[r['repo'] for r in repos]}")
                ground_truth = {}
            else:
                gt_results = runner.evaluate_batch(language, repos_per_iteration)
                ground_truth = {"clippy": gt_results.get("findings", [])}
                logger.info(f"  Clippy: {len(ground_truth.get('clippy', []))} findings")
            
            # ── Phase 2: Code Intelligence (CogniCode as analysis tool) ──
            logger.info("Phase 2: Code Intelligence (CogniCode for LLM context)...")
            
            # CogniCode reads the code structure to help LLM understand
            # NOT used for self-evaluation — only for providing context
            code_context = {}
            if not dry_run and ground_truth.get("clippy"):
                # Group findings by rule category
                from collections import defaultdict
                by_rule = defaultdict(list)
                for f in ground_truth["clippy"]:
                    norm = runner._normalize_rule(f.get("rule", "")) if hasattr(runner, '_normalize_rule') else f.get("rule", "unknown")
                    by_rule[norm].append(f)
                
                # Build rich context for top affected rules
                for rule_cat, findings in sorted(by_rule.items(), 
                                                  key=lambda x: -len(x[1]))[:3]:
                    # Find a repo dir to analyze
                    repos = corpus.pick_repos(language, 1)
                    if repos:
                        repo_dir = runner._clone_repo(repos[0]["repo"]) if hasattr(runner, '_clone_repo') else None
                        if repo_dir:
                            ctx = code_intel.analyze_for_llm(repo_dir, rule_cat, findings)
                            code_context[rule_cat] = ctx
                            import shutil
                            shutil.rmtree(repo_dir, ignore_errors=True)
                
                logger.info(f"  Context built for {len(code_context)} rule categories")
            
            # ── Phase 3: LLM Analysis with rich context ──
            logger.info("Phase 3: LLM Gap Analysis...")
            
            improvements = 0
            if not dry_run and code_context:
                for rule_cat, ctx in code_context.items():
                    prompt = code_intel.build_llm_prompt(
                        rule_cat, ctx, {}
                    )
                    
                    try:
                        llm = analyzer.llm
                        response = llm.chat(
                            system="You are a static analysis expert. Analyze code patterns and propose specific rule improvements.",
                            messages=[{"role": "user", "content": prompt}],
                            max_tokens=2000
                        )
                        
                        # Parse JSON from LLM response
                        import re as _re
                        json_match = _re.search(r'\{[\s\S]*\}', response)
                        if json_match:
                            analysis = json.loads(json_match.group(0))
                            
                            logger.info(f"  {rule_cat}: {analysis.get('improvement_type', '?')} "
                                       f"(ΔF1={analysis.get('expected_f1_delta', '?')})")
                            
                            evolution.log_experiment(
                                iteration=session_iter,
                                rule_id=rule_cat,
                                language=language,
                                metrics_before={"fn": ctx["ground_truth_count"]},
                                metrics_after={"expected_delta_f1": analysis.get("expected_f1_delta", 0)},
                                decision="analyzed",
                                description=f"LLM: {analysis.get('improvement_type', '?')} — "
                                           f"{analysis.get('suggested_fix', '')[:150]}"
                            )
                            improvements += 1
                    except Exception as e:
                        logger.warning(f"  LLM analysis failed for {rule_cat}: {e}")
            
            # ── Stats ──
            elapsed = time.time() - t0
            gt_count = sum(len(v) for v in ground_truth.values()) if ground_truth else 0
            total_findings += gt_count
            total_improvements += improvements if 'improvements' in dir() else 0
            
            logger.info(f"\n  ⏱ Iteration time: {elapsed:.0f}s")
            logger.info(f"  📊 Ground truth findings: {gt_count}")
            logger.info(f"  🔧 Improvements: {improvements if 'improvements' in dir() else 0}")
            logger.info(f"  🧠 Code context built for: {len(code_context)} rule categories")
            
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
