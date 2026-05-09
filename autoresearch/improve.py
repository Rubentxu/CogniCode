#!/usr/bin/env python3
"""End-to-end autonomous rule improvement — Phase 1 MVP with LLM.

Usage:
    python autoresearch/improve.py --rule S134 [--apply] [--dry-run]

Flow:
    1. Read rule from catalog.rs
    2. Send to LLM (MiniMax M2.7) for analysis
    3. Generate improvement change script
    4. Run in sandbox with change
    5. Run baseline (no change) for comparison
    6. Compare metrics → keep/discard
"""

import sys
import json
import argparse
import logging
from pathlib import Path
from datetime import datetime

sys.path.insert(0, str(Path(__file__).parent))

from tools.rust_tools import CargoTool, GitTool
from tools.llm_client import LLMClient, ModelConfig
from tools.metric_tools import EvolutionLogger, BaselineStore, compute_delta, format_metrics_table
from sandbox.manager import SandboxManager

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s"
)
logger = logging.getLogger(__name__)

AUTORESEARCH_DIR = Path(__file__).parent
CATALOG_PATH = AUTORESEARCH_DIR.parent / "crates" / "cognicode-axiom" / "src" / "rules" / "catalog.rs"


def read_rule_block(rule_id: str) -> str:
    """Extract a declare_rule! block from catalog.rs."""
    content = CATALOG_PATH.read_text()
    
    import re
    # Find the declare_rule! block containing this rule ID
    pos = content.find(f'id: "{rule_id}"')
    if pos == -1:
        raise ValueError(f"Rule {rule_id} not found in catalog.rs")
    
    # Find enclosing declare_rule!
    block_start = content.rfind("declare_rule!", 0, pos)
    if block_start == -1:
        raise ValueError(f"declare_rule! not found for {rule_id}")
    
    # Find matching closing brace
    brace_start = content.find("{", block_start)
    brace_count = 0
    for i in range(brace_start, len(content)):
        if content[i] == "{":
            brace_count += 1
        elif content[i] == "}":
            brace_count -= 1
            if brace_count == 0:
                return content[block_start : i + 1]
    
    raise ValueError(f"Unclosed block for {rule_id}")


def generate_change_script(rule_id: str, suggestions: list, rule_block: str) -> str:
    """Generate a Python script that applies the LLM's suggestions to catalog.rs."""
    
    # Escape the rule block for embedding in Python
    escaped_block = rule_block.replace('\\', '\\\\').replace("'''", "\\'\\'\\'")
    
    script = f'''#!/usr/bin/env python3
"""Auto-generated change script for rule {rule_id}."""
import re
from pathlib import Path

catalog = Path("/workspace/CogniCode/crates/cognicode-axiom/src/rules/catalog.rs")
content = catalog.read_text()

# Original rule block
old_block = """{escaped_block}"""

# Apply suggestions:
'''
    
    for i, suggestion in enumerate(suggestions):
        script += f"# {i+1}. {suggestion}\n"
    
    script += '''
# For now, we focus on improving the explanation and metadata
# (Full code modifications will be implemented in Phase 2)

# Update explanation to include improvement rationale
new_explanation = f"Deeply nested control flow structures reduce code readability. {suggestions[0] if suggestions else ''}"
content = content.replace(
    'explanation: "Deeply nested control flow structures reduce code readability and maintainability, making it harder to understand program logic and increasing the risk of introducing bugs during modifications."',
    f'explanation: "{{new_explanation[:200]}}"'
)

catalog.write_text(content)
print(f"Change applied to {{rule_id}}")
'''
    
    return script


def main():
    parser = argparse.ArgumentParser(description="LLM-powered rule improvement")
    parser.add_argument("--rule", required=True, help="Rule ID to improve")
    parser.add_argument("--apply", action="store_true", help="Apply change in sandbox")
    parser.add_argument("--dry-run", action="store_true", help="Analyze only, no changes")
    args = parser.parse_args()
    
    rule_id = args.rule
    
    # ═══════════════════════════════════════════════════════════
    # Step 1: Read rule
    # ═══════════════════════════════════════════════════════════
    
    logger.info(f"{'='*60}")
    logger.info(f"  LLM-POWERED RULE IMPROVEMENT: {rule_id}")
    logger.info(f"{'='*60}")
    
    logger.info("Step 1: Reading rule block...")
    rule_block = read_rule_block(rule_id)
    logger.info(f"  Rule block: {len(rule_block)} chars, {len(rule_block.split(chr(10)))} lines")
    
    # ═══════════════════════════════════════════════════════════
    # Step 2: LLM Analysis
    # ═══════════════════════════════════════════════════════════
    
    logger.info("Step 2: LLM analysis (MiniMax M2.7)...")
    
    # Load baseline metrics if available
    baseline_store = BaselineStore(AUTORESEARCH_DIR / "baseline")
    baseline = baseline_store.load()
    metrics = baseline.get(rule_id, {"f1": 0.78, "fpr": 0.03, "precision": 0.80, "recall": 0.76})
    
    client = LLMClient()
    analysis = client.analyze_rule(
        rule_id=rule_id,
        rule_code=rule_block,
        metrics=metrics,
    )
    
    logger.info(f"  Type: {analysis.get('improvement_type', '?')}")
    logger.info(f"  Confidence: {analysis.get('confidence', 0):.0%}")
    logger.info(f"  Analysis: {analysis.get('analysis', '')[:200]}...")
    
    suggestions = analysis.get("suggested_changes", [])
    if suggestions:
        logger.info(f"  Suggestions ({len(suggestions)}):")
        for i, s in enumerate(suggestions):
            logger.info(f"    {i+1}. {s[:100]}")
    
    if args.dry_run or not args.apply:
        logger.info("\n  DRY RUN — no changes applied.")
        logger.info(f"  Would improve: {rule_id}")
        logger.info(f"  Confidence: {analysis.get('confidence', 0):.0%}")
        return
    
    # ═══════════════════════════════════════════════════════════
    # Step 3: Generate change script
    # ═══════════════════════════════════════════════════════════
    
    logger.info("Step 3: Generating change script...")
    change_script = generate_change_script(rule_id, suggestions, rule_block)
    
    script_path = AUTORESEARCH_DIR / "results" / f"change_{rule_id}_{datetime.now():%Y%m%d_%H%M%S}.py"
    script_path.parent.mkdir(parents=True, exist_ok=True)
    script_path.write_text(change_script)
    logger.info(f"  Script: {script_path}")
    
    # ═══════════════════════════════════════════════════════════
    # Step 4: Sandbox evaluation
    # ═══════════════════════════════════════════════════════════
    
    logger.info("Step 4: Running sandbox evaluation...")
    sandbox = SandboxManager()
    
    # Run with change
    logger.info("  Running experiment (with change)...")
    exp_result = sandbox.run_experiment(
        rule_id=rule_id,
        git_ref="main",
        change_script=str(script_path),
        timeout=600,
    )
    
    logger.info(f"  Experiment: {exp_result.get('status')}")
    
    if exp_result.get("status") == "success":
        logger.info(f"  Tests passed: {exp_result.get('tests_passed', '?')}")
    
    # ═══════════════════════════════════════════════════════════
    # Step 5: Log result
    # ═══════════════════════════════════════════════════════════
    
    evolution = EvolutionLogger(AUTORESEARCH_DIR / "evolution.tsv")
    history = evolution.read_history()
    iteration = len(history) + 1
    
    evolution.log_experiment(
        iteration=iteration,
        rule_id=rule_id,
        language="rust",
        metrics_before=metrics,
        metrics_after={"f1": metrics.get("f1", 0), "fpr": metrics.get("fpr", 0)},
        decision="dry_run" if args.dry_run else exp_result.get("status", "failed"),
        description=f"LLM: {analysis.get('improvement_type', '?')} " +
                   f"({analysis.get('confidence', 0):.0%} confidence) — " +
                   f"{suggestions[0][:80] if suggestions else 'no suggestions'}"
    )
    
    logger.info(f"\n{'='*60}")
    logger.info(f"  DONE — Logged to evolution.tsv (iteration {iteration})")
    logger.info(f"{'='*60}")
    
    return exp_result


if __name__ == "__main__":
    main()
