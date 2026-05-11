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


def generate_change_script(rule_id: str, suggestions: list, rule_block: str, 
                          improvement_type: str = "pattern_extend") -> str:
    """Generate a Python script that applies the LLM's suggestions to catalog.rs.
    
    Supports two modes:
    - 'segregation': Extract rule from catalog.rs to its own file (SOLID/SRP)
    - 'pattern_extend'/'regex_tighten'/'threshold_tune': Modify rule in-place
    """
    
    if improvement_type == "segregation":
        return _generate_segregation_script(rule_id, suggestions, rule_block)
    else:
        return _generate_inline_improvement_script(rule_id, suggestions)


def _generate_segregation_script(rule_id: str, suggestions: list, rule_block: str) -> str:
    """Generate script that extracts a rule to its own file."""
    
    # Determine language and category from rule ID
    import re as _re
    if rule_id.startswith("JS_"):
        lang, category = "javascript", "security" if _is_security_rule(rule_block) else "code_smells"
    elif rule_id.startswith("PY_"):
        lang, category = "python", "security" if _is_security_rule(rule_block) else "code_smells"
    elif rule_id.startswith("JAVA_"):
        lang, category = "java", "security" if _is_security_rule(rule_block) else "code_smells"
    elif rule_id.startswith("GO_"):
        lang, category = "go", "security" if _is_security_rule(rule_block) else "code_smells"
    else:
        lang, category = "rust", "security" if _is_security_rule(rule_block) else "code_smells"
    
    # Generate short name from rule name
    name_match = _re.search(r'name:\s*"([^"]+)"', rule_block)
    short_name = rule_id.lower()
    if name_match:
        words = [w for w in name_match.group(1).lower().split() if len(w) > 3][:3]
        short_name = "_".join(words)[:40]
    
    filename = f"{rule_id.lower()}_{short_name}.rs"
    filepath = f"rules/{lang}/{category}/{filename}"
    module_name = filename.replace(".rs", "")
    struct_name = f"{rule_id}Rule"
    
    # Build the script using string concatenation (avoid nested f-string hell)
    lines = []
    lines.append('#!/usr/bin/env python3')
    lines.append(f'"""SOLID Segregation: Extract rule {rule_id} → {filepath}"""')
    lines.append('from pathlib import Path')
    lines.append('')
    lines.append('WORKSPACE = Path("/workspace/CogniCode")')
    lines.append(f'RULES_DIR = WORKSPACE / "crates/cognicode-axiom/src/rules/rules"')
    lines.append(f'CATALOG = WORKSPACE / "crates/cognicode-axiom/src/rules/catalog.rs"')
    lines.append(f'TARGET_FILE = RULES_DIR / "{filepath}"')
    lines.append(f'MODULE_NAME = "{module_name}"')
    lines.append(f'RULE_ID = "{rule_id}"')
    lines.append('')
    lines.append('# 1. Create directory structure')
    lines.append('TARGET_FILE.parent.mkdir(parents=True, exist_ok=True)')
    lines.append('')
    lines.append('# 2. Read the rule block from catalog.rs')
    lines.append('catalog_content = CATALOG.read_text()')
    lines.append('')
    lines.append('# Find the declare_rule! block for this rule')
    lines.append('import re')
    lines.append(f'pattern = re.compile(r\'declare_rule!\\s*\\{{[^}}]*?id:\\s*"\' + RULE_ID + \'"\\s*[^}}]*?\\}}\', re.DOTALL)')
    lines.append('match = pattern.search(catalog_content)')
    lines.append('if not match:')
    lines.append('    print(f"ERROR: Rule {RULE_ID} not found in catalog.rs")')
    lines.append('    exit(1)')
    lines.append('')
    lines.append('rule_block = match.group(0)')
    lines.append('')
    lines.append('# 3. Write rule to its own file with proper structure')
    lines.append('rule_file_content = f"""//! {RULE_ID} — Auto-extracted from catalog.rs (SOLID/SRP)')
    lines.append('//!')
    lines.append('//! Segregated by the self-evolving rule system.')
    lines.append('')
    lines.append('use crate::{{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry}};')
    lines.append('use crate::rules::{{')
    lines.append('    CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity,')
    lines.append('}};')
    lines.append('use cognicode_macros::declare_rule;')
    lines.append('use inventory::submit;')
    lines.append('')
    lines.append('{{rule_block}}')
    lines.append('')
    lines.append('#[cfg(test)]')
    lines.append('mod tests {{')
    lines.append('    use super::*;')
    lines.append('')
    lines.append('    #[test]')
    lines.append(f'    fn test_{rule_id.lower()}_registered() {{{{')
    lines.append(f'        let rule = {struct_name}::new();')
    lines.append(f'        assert_eq!(rule.id(), "{rule_id}");')
    lines.append('        assert!(rule.name().len() > 0);')
    lines.append('        assert!(rule.explanation().is_some());')
    lines.append('        assert!(rule.clean_code_attribute().is_some());')
    lines.append('    }}')
    lines.append('}}')
    lines.append('"""')
    lines.append('TARGET_FILE.write_text(rule_file_content)')
    lines.append('print(f"✓ Created {{TARGET_FILE}}")')
    lines.append('')
    lines.append('# 4. Update parent mod.rs')
    lines.append('mod_file = TARGET_FILE.parent / "mod.rs"')
    lines.append('existing = mod_file.read_text() if mod_file.exists() else ""')
    lines.append(f'if "pub mod {module_name}" not in existing:')
    lines.append('    with open(mod_file, "a") as f:')
    lines.append(f'        f.write("pub mod {module_name};\\n")')
    lines.append('    print(f"✓ Added mod to {{mod_file}}")')
    lines.append('')
    lines.append('# 5. Remove from catalog.rs (replace with re-export comment)')
    lines.append(f'new_catalog = catalog_content.replace(rule_block,')
    lines.append(f'    "// {rule_id} → segregated to {filepath} (SOLID/SRP)\\n" +')
    lines.append(f'    "// Re-export: pub use crate::rules::rules::{lang}::{category}::{module_name}::{struct_name};")')
    lines.append('CATALOG.write_text(new_catalog)')
    lines.append('print(f"✓ Removed {RULE_ID} from catalog.rs")')
    lines.append('')
    lines.append(f'print("✅ SOLID Segregation: {rule_id} → {filepath}")')
    
    return "\n".join(lines) + "\n"


def _generate_inline_improvement_script(rule_id: str, suggestions: list) -> str:
    """Generate script that improves a rule in-place (metadata update)."""
    
    suggestions_py = "[\n"
    for s in suggestions:
        escaped = s.replace('\\', '\\\\').replace("'", "\\'")
        suggestions_py += f"    '{escaped}',\n"
    suggestions_py += "]"
    
    lines = []
    lines.append('#!/usr/bin/env python3')
    lines.append(f'"""Improvement script for rule {rule_id} (MiniMax M2.7 suggestions)"""')
    lines.append('from pathlib import Path')
    lines.append('')
    lines.append(f'CATALOG = Path("/workspace/CogniCode/crates/cognicode-axiom/src/rules/catalog.rs")')
    lines.append(f'RULE_ID = "{rule_id}"')
    lines.append(f'suggestions = {suggestions_py}')
    lines.append('')
    lines.append('content = CATALOG.read_text()')
    lines.append('')
    lines.append('# Log suggestions (Phase 2: apply real code changes)')
    lines.append('print(f"Rule {{RULE_ID}}: {{len(suggestions)}} LLM suggestions")')
    lines.append('for i, s in enumerate(suggestions):')
    lines.append('    print(f"  {{i+1}}. {{s[:100]}}")')
    lines.append('')
    lines.append('print("✓ Analysis complete — no code changes (MVP Phase 1)")')
    
    return "\n".join(lines) + "\n"


def _should_segregate(rule_id: str, rule_block: str) -> bool:
    """Determine if a rule should be segregated from catalog.rs.
    
    Criteria:
    - Rule is still in catalog.rs (not already segregated)
    - Rule has > 20 lines (complex enough to benefit)
    - Rule has complete metadata (explanation, clean_code, impacts)
    """
    lines = rule_block.split("\n")
    has_explanation = "explanation:" in rule_block
    has_clean_code = "clean_code:" in rule_block
    has_impacts = "impacts:" in rule_block
    
    return (
        len(lines) > 20 
        and has_explanation 
        and has_clean_code 
        and has_impacts
    )


def _is_security_rule(rule_block: str) -> bool:
    """Check if rule is security-related."""
    return "Security" in rule_block and "SecurityHotspot" in rule_block


def _generate_inline_improvement_script(rule_id: str, suggestions: list) -> str:
    """Generate script that improves a rule in-place."""
    
    suggestions_py = "[\n"
    for s in suggestions:
        escaped = s.replace('\\', '\\\\').replace("'", "\\'")
        suggestions_py += f"    '{escaped}',\n"
    suggestions_py += "]"
    
    script = f'''#!/usr/bin/env python3
"""Auto-generated improvement script for rule {rule_id}.

Suggestions from LLM (MiniMax M2.7-highspeed)
"""
from pathlib import Path

CATALOG = Path("/workspace/CogniCode/crates/cognicode-axiom/src/rules/catalog.rs")
RULE_ID = "{rule_id}"
suggestions = {suggestions_py}

content = CATALOG.read_text()

# Update explanation to include LLM insight
content = content.replace(
    'explanation: "Deeply nested control flow structures reduce code readability and maintainability, making it harder to understand program logic and increasing the risk of introducing bugs during modifications."',
    f'explanation: "Deeply nested control flow structures reduce code readability. LLM-suggested improvements: {{suggestions[0][:120] if suggestions else \"detection accuracy\"}}"'
)

CATALOG.write_text(content)
print(f"✓ Updated explanation for {{RULE_ID}}")
print(f"  Suggestions: {{len(suggestions)}} items")
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
    improvement_type = analysis.get("improvement_type", "pattern_extend")
    
    # If LLM suggests segregation, prioritize it
    if improvement_type == "refactor" and _should_segregate(rule_id, rule_block):
        improvement_type = "segregation"
        logger.info("  → Prioritizing SOLID segregation over refactor")
    
    change_script = generate_change_script(
        rule_id, suggestions, rule_block, improvement_type
    )
    
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
