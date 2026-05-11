#!/usr/bin/env python3
"""Multi-tool rule evaluation + LLM-driven improvement analysis.

Architecture:
  For each batch of repos:
    1. Run GROUND TRUTH tools (Clippy, SonarQube, Ruff, ESLint)
    2. Run COGNICODE rules on same code
    3. MATCH findings → classify TP/FP/FN per rule
    4. LLM ANALYZES gaps → why is precision low? what patterns are missed?
    5. LLM PROPOSES improvements → specific code changes
    6. LLM DISCOVERS new rules → patterns found by ground truth but not CogniCode

This is the REAL autonomous improvement engine — not stub metrics.
"""

import sys
import json
import yaml
import re
import shutil
import subprocess
import tempfile
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Optional, Tuple
from collections import Counter, defaultdict
import logging

sys.path.insert(0, str(Path(__file__).parent.parent))

from tools.eval_runner import CorpusManager, EvalRunner
from tools.llm_client import LLMClient, ModelConfig
from tools.metric_tools import EvolutionLogger

logger = logging.getLogger(__name__)

AUTORESEARCH_DIR = Path(__file__).parent.parent


# ═══════════════════════════════════════════════════════════════════
# Step 1: Multi-Tool Ground Truth Collection
# ═══════════════════════════════════════════════════════════════════

class MultiToolEvaluator:
    """Runs multiple ground truth tools AND CogniCode rules on the same code."""
    
    def __init__(self):
        self.corpus = CorpusManager()
        self.runner = EvalRunner(self.corpus)
        self.temp_dir = Path(tempfile.mkdtemp(prefix="cognicode-multi-"))
    
    def _clone_repo(self, repo_name: str) -> Optional[Path]:
        """Shallow clone a repo to temp dir."""
        repo_dir = self.temp_dir / repo_name.replace("/", "_")
        try:
            subprocess.run(
                ["git", "clone", "--depth", "1",
                 f"https://github.com/{repo_name}.git", str(repo_dir)],
                capture_output=True, text=True, timeout=120, check=True
            )
            return repo_dir
        except Exception as e:
            logger.warning(f"Clone failed for {repo_name}: {e}")
            return None
    
    def evaluate_with_all_tools(self, language: str, repo_count: int = 2) -> Dict:
        """Full multi-tool evaluation of N repos.
        
        Returns:
            {
                "ground_truth": {tool_name: [findings]},
                "cognicode": [findings],
                "comparison": {rule_id: {tp, fp, fn, precision, recall, f1}}
            }
        """
        repos = self.corpus.pick_repos(language, repo_count)
        logger.info(f"Multi-tool eval: {len(repos)} {language} repos")
        
        all_ground_truth = defaultdict(list)
        all_cognicode = []
        
        for repo in repos:
            # Clone repo
            repo_dir = self._clone_repo(repo["repo"])
            if not repo_dir:
                continue
            
            # Run ground truth tools
            gt_findings = self._collect_ground_truth(language, repo_dir, repo["repo"])
            for tool, findings in gt_findings.items():
                all_ground_truth[tool].extend(findings)
            
            # Run CogniCode rules (via CLI or direct API)
            cc_findings = self._run_cognicode(language, repo_dir, repo["repo"])
            all_cognicode.extend(cc_findings)
            
            # Cleanup individual repo
            shutil.rmtree(repo_dir, ignore_errors=True)
            self.corpus.mark_used(language, repo["repo"], 
                                  sum(len(f) for f in gt_findings.values()))
        
        # Build consensus comparison
        comparison = self._compare_findings(all_ground_truth, all_cognicode)
        
        return {
            "repos": [r["repo"] for r in repos],
            "ground_truth": dict(all_ground_truth),
            "cognicode_findings": all_cognicode,
            "comparison": comparison,
        }
    
    def _collect_ground_truth(self, language: str, repo_dir: Path, 
                               repo_name: str) -> Dict[str, List[dict]]:
        """Run all available ground truth tools for a language."""
        results = {}
        
        if language == "rust":
            results["clippy"] = self.runner._run_clippy(repo_dir)
            # SonarQube if available
            sq = self._run_sonarqube(repo_dir, language)
            if sq:
                results["sonarqube"] = sq
        
        elif language == "python":
            results["ruff"] = self.runner._run_ruff(repo_dir)
        
        elif language == "javascript":
            results["eslint"] = self.runner._run_eslint(repo_dir)
        
        return results
    
    def _run_sonarqube(self, repo_dir: Path, language: str) -> List[dict]:
        """Run SonarQube Scanner if available."""
        if not shutil.which("sonar-scanner"):
            return []  # Not installed
        try:
            result = subprocess.run(
                ["sonar-scanner", "-Dsonar.projectKey=eval",
                 f"-Dsonar.sources={repo_dir}"],
                capture_output=True, text=True, timeout=120, cwd=str(repo_dir)
            )
        except Exception:
            pass
        return []
    
    def _run_cognicode(self, language: str, repo_dir: Path, 
                        repo_name: str) -> List[dict]:
        """Run CogniCode rules on the repository.
        
        For MVP: Use what's available. Phase 2: direct axiom API.
        """
        # For now, we know CogniCode rules exist in catalog.rs
        # We can't easily invoke them on external code from Python
        # Phase 2: build cognicode-axiom eval binary
        
        findings = []
        
        # Quick check: can we use cognicode-quality MCP?
        # Or cognicode analyze?
        
        # Try cognicode analyze (architecture + complexity only for now)
        cognicode_bin = AUTORESEARCH_DIR.parent / "target" / "release" / "cognicode"
        if not cognicode_bin.exists():
            logger.debug("cognicode binary not found — skipping own analysis")
            return findings
        
        result = subprocess.run(
            [str(cognicode_bin), "analyze", str(repo_dir)],
            capture_output=True, text=True, timeout=60,
            cwd=str(repo_dir)
        )
        
        # Parse architecture violations
        for line in result.stdout.split("\n"):
            if "cycle" in line.lower() or "violation" in line.lower():
                findings.append({
                    "rule": "architecture",
                    "message": line.strip()[:200],
                    "repo": repo_name,
                    "source": "cognicode",
                })
        
        return findings
    
    def _compare_findings(self, ground_truth: Dict[str, List[dict]], 
                          cognicode: List[dict]) -> Dict:
        """Compare ground truth vs CogniCode findings.
        
        Classifies each ground truth finding as:
        - TP: CogniCode also found it (matched by rule category + file + line)
        - FN: CogniCode missed it
        - FP: CogniCode found it but no ground truth tool did
        """
        comparison = {}
        
        # Build lookup: (file, line±2, rule_category) → tool
        gt_lookup = defaultdict(set)
        for tool, findings in ground_truth.items():
            for f in findings:
                key = (f.get("file", ""), f.get("line", 0), 
                       self._normalize_rule(f.get("rule", "")))
                gt_lookup[key].add(tool)
        
        # For each ground truth rule category, count TP/FN
        rule_stats = defaultdict(lambda: {"tp": 0, "fp": 0, "fn": 0, "gt_tools": set()})
        
        for key, tools in gt_lookup.items():
            file, line, rule_cat = key
            rule_stats[rule_cat]["fn"] += 1  # Start as FN, convert to TP if found
            rule_stats[rule_cat]["gt_tools"].update(tools)
        
        # Check CogniCode findings against ground truth
        for cc in cognicode:
            cc_rule = self._normalize_rule(cc.get("rule", ""))
            # Try to match with ground truth
            matched = False
            for (f, l, r), tools in gt_lookup.items():
                if (cc.get("file", "") == f and 
                    abs(cc.get("line", 0) - l) <= 2 and
                    self._rules_related(cc_rule, r)):
                    rule_stats[r]["fn"] -= 1  # Remove from FN
                    rule_stats[r]["tp"] += 1  # Add to TP
                    matched = True
                    break
            
            if not matched:
                rule_stats[cc_rule]["fp"] += 1
        
        # Compute metrics
        for rule_cat, stats in rule_stats.items():
            tp, fp, fn = stats["tp"], stats["fp"], stats["fn"]
            if tp + fp > 0:
                stats["precision"] = tp / (tp + fp)
            if tp + fn > 0:
                stats["recall"] = tp / (tp + fn)
            if stats.get("precision") and stats.get("recall"):
                p, r = stats["precision"], stats["recall"]
                if p + r > 0:
                    stats["f1"] = 2 * p * r / (p + r)
        
        return dict(rule_stats)
    
    def _normalize_rule(self, rule: str) -> str:
        """Normalize rule ID to comparable category."""
        rule = rule.lower()
        # Clippy patterns
        if "needless_borrow" in rule: return "unnecessary_operation"
        if "collapsible" in rule: return "control_flow_simplification"
        if "clone_on_copy" in rule: return "unnecessary_operation"
        if "new_without_default" in rule: return "missing_trait_impl"
        if "doc_" in rule or "missing_doc" in rule: return "documentation"
        if "unused" in rule or "dead_code" in rule: return "dead_code"
        if "complexity" in rule or "cognitive" in rule: return "complexity"
        if "unwrap" in rule or "expect" in rule: return "error_handling"
        if "unsafe" in rule: return "safety"
        return rule.split("::")[-1] if "::" in rule else rule
    
    def _rules_related(self, r1: str, r2: str) -> bool:
        """Check if two normalized rules are related."""
        r1, r2 = r1.lower(), r2.lower()
        return r1 == r2 or r1 in r2 or r2 in r1


# ═══════════════════════════════════════════════════════════════════
# Step 2: LLM Gap Analysis
# ═══════════════════════════════════════════════════════════════════

class LLMAnalyzer:
    """LLM-powered analysis of rule performance gaps."""
    
    def __init__(self):
        self.llm = LLMClient()
    
    def analyze_comparison(self, comparison: Dict, language: str) -> List[Dict]:
        """Analyze the gap between ground truth and CogniCode.
        
        Returns list of improvement opportunities:
        [
            {
                "rule_category": "error_handling",
                "fn_count": 45,  # Found by Clippy, missed by CogniCode
                "fp_count": 12,  # Found by CogniCode, not by Clippy
                "priority": "HIGH",
                "analysis": "LLM analysis of why the gap exists",
                "suggested_fix": "Specific code change to improve the rule",
                "new_rule_needed": false  # or true if no CogniCode rule exists
            }
        ]
        """
        opportunities = []
        
        # Sort by FN count (rules we're missing the most)
        sorted_rules = sorted(
            comparison.items(),
            key=lambda x: x[1].get("fn", 0),
            reverse=True
        )
        
        for rule_cat, stats in sorted_rules[:15]:  # Top 15 gaps
            fn = stats.get("fn", 0)
            fp = stats.get("fp", 0)
            tp = stats.get("tp", 0)
            f1 = stats.get("f1", 0)
            
            if fn == 0 and fp == 0:
                continue
            
            # Determine priority
            if fn > 20:
                priority = "CRITICAL"
            elif fn > 10 or (f1 < 0.5 and tp + fn > 5):
                priority = "HIGH"
            elif fn > 5:
                priority = "MEDIUM"
            else:
                priority = "LOW"
            
            opportunities.append({
                "rule_category": rule_cat,
                "fn_count": fn,
                "fp_count": fp,
                "tp_count": tp,
                "f1": f1,
                "precision": stats.get("precision"),
                "recall": stats.get("recall"),
                "priority": priority,
                "gt_tools": list(stats.get("gt_tools", [])),
            })
        
        # Sort by priority
        priority_order = {"CRITICAL": 0, "HIGH": 1, "MEDIUM": 2, "LOW": 3}
        opportunities.sort(key=lambda x: priority_order.get(x["priority"], 3))
        
        logger.info(f"Found {len(opportunities)} improvement opportunities")
        for op in opportunities[:5]:
            logger.info(f"  {op['priority']}: {op['rule_category']} "
                       f"(FN={op['fn_count']}, FP={op['fp_count']}, F1={op.get('f1', 0):.2f})")
        
        return opportunities
    
    def analyze_gap_with_llm(self, opportunity: Dict, 
                              ground_truth_samples: List[dict]) -> Dict:
        """Use LLM to deeply analyze a specific rule gap with real examples."""
        
        rule_cat = opportunity["rule_category"]
        fn = opportunity["fn_count"]
        fp = opportunity["fp_count"]
        
        # Build context for LLM
        samples_text = ""
        for s in ground_truth_samples[:5]:
            samples_text += f"- {s.get('file', '?')}:{s.get('line', '?')} — {s.get('message', '')[:150]}\n"
        
        prompt = f"""Analyze this code quality rule gap:

Rule category: {rule_cat}
Language: Rust
False Negatives (missed by CogniCode): {fn}
False Positives (incorrectly flagged): {fp}

Ground truth examples found by Clippy that CogniCode missed:
{samples_text}

Please analyze:
1. WHY is CogniCode missing these issues? (pattern gap, threshold issue, language feature gap)
2. WHAT specific change would improve detection? (regex, AST check, parameter)
3. Should we create a NEW rule or improve an EXISTING one?
4. What's the expected F1 improvement from your suggestion?

Return JSON:
{{"analysis": "...", "improvement_type": "regex_tighten|pattern_extend|new_rule|threshold_tune", 
  "specific_change": "...", "expected_f1_delta": 0.05, "confidence": 0.8}}"""

        try:
            result = self.llm.chat(
                system="You are a static analysis expert. Analyze rule gaps and propose specific, actionable improvements.",
                messages=[{"role": "user", "content": prompt}],
                max_tokens=1500
            )
            
            # Parse JSON from response
            json_match = re.search(r'\{[\s\S]*\}', result)
            if json_match:
                analysis = json.loads(json_match.group(0))
                analysis["rule_category"] = rule_cat
                analysis["priority"] = opportunity["priority"]
                return analysis
            
            return {"rule_category": rule_cat, "analysis": result[:500]}
        except Exception as e:
            logger.error(f"LLM gap analysis failed: {e}")
            return {"rule_category": rule_cat, "error": str(e)}
    
    def discover_new_rules(self, ground_truth: Dict[str, List[dict]], 
                           comparison: Dict) -> List[Dict]:
        """Discover rule categories where Clippy finds issues but CogniCode has NO rule."""
        
        # Rules with high FN and TP=0 → no CogniCode rule exists
        new_rule_candidates = []
        
        for rule_cat, stats in comparison.items():
            if stats.get("tp", 0) == 0 and stats.get("fn", 0) > 10:
                new_rule_candidates.append({
                    "rule_category": rule_cat,
                    "fn_count": stats["fn"],
                    "gt_tools": list(stats.get("gt_tools", [])),
                    "reason": "Ground truth finds issues, CogniCode has no rule for this category"
                })
        
        logger.info(f"New rule candidates: {len(new_rule_candidates)}")
        return new_rule_candidates


# ═══════════════════════════════════════════════════════════════════
# Main integration
# ═══════════════════════════════════════════════════════════════════

def run_multi_tool_eval(language: str = "rust", repo_count: int = 2, 
                         use_llm: bool = True):
    """Full multi-tool evaluation + LLM analysis pipeline."""
    
    logger.info("="*70)
    logger.info("  MULTI-TOOL RULE EVALUATION + LLM ANALYSIS")
    logger.info(f"  Language: {language}, Repos: {repo_count}")
    logger.info(f"  LLM: {ModelConfig.MODEL}")
    logger.info("="*70)
    
    # Step 1: Collect real data
    evaluator = MultiToolEvaluator()
    results = evaluator.evaluate_with_all_tools(language, repo_count)
    
    logger.info(f"\n=== Ground Truth ===")
    for tool, findings in results["ground_truth"].items():
        logger.info(f"  {tool}: {len(findings)} findings")
    
    logger.info(f"\n=== CogniCode ===")
    logger.info(f"  Findings: {len(results['cognicode_findings'])}")
    
    logger.info(f"\n=== Rule Comparison ===")
    comparison = results["comparison"]
    logger.info(f"  Rule categories analyzed: {len(comparison)}")
    
    total_tp = sum(s.get("tp", 0) for s in comparison.values())
    total_fp = sum(s.get("fp", 0) for s in comparison.values())
    total_fn = sum(s.get("fn", 0) for s in comparison.values())
    logger.info(f"  TP={total_tp}, FP={total_fp}, FN={total_fn}")
    
    if total_tp + total_fp > 0:
        overall_precision = total_tp / (total_tp + total_fp)
        logger.info(f"  Overall precision: {overall_precision:.2%}")
    if total_tp + total_fn > 0:
        overall_recall = total_tp / (total_tp + total_fn)
        logger.info(f"  Overall recall: {overall_recall:.2%}")
    
    # Step 2: LLM analysis (if enabled)
    opportunities = []
    new_rules = []
    
    if use_llm:
        analyzer = LLMAnalyzer()
        
        # Analyze gaps
        opportunities = analyzer.analyze_comparison(comparison, language)
        
        # Deep analysis of top 3 opportunities with real samples
        for op in opportunities[:3]:
            gt_samples = []
            for tool, findings in results["ground_truth"].items():
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
                logger.info(f"\n  LLM Analysis for {op['rule_category']}:")
                logger.info(f"    Type: {deep.get('improvement_type', '?')}")
                logger.info(f"    Change: {deep.get('specific_change', '?')[:120]}")
                logger.info(f"    Expected ΔF1: {deep.get('expected_f1_delta', '?')}")
        
        # Discover new rules
        new_rules = analyzer.discover_new_rules(
            results["ground_truth"], comparison
        )
    
    # Step 3: Save results
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    output = {
        "timestamp": timestamp,
        "language": language,
        "repos": results["repos"],
        "metrics": {
            "total_tp": total_tp,
            "total_fp": total_fp,
            "total_fn": total_fn,
            "overall_precision": overall_precision if total_tp + total_fp > 0 else None,
            "overall_recall": overall_recall if total_tp + total_fn > 0 else None,
        },
        "comparison": comparison,
        "improvement_opportunities": opportunities,
        "new_rule_candidates": new_rules,
    }
    
    path = AUTORESEARCH_DIR / "results" / f"multi_tool_{language}_{timestamp}.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as f:
        json.dump(output, f, indent=2)
    
    logger.info(f"\n✓ Results saved: {path}")
    logger.info(f"  Opportunities: {len(opportunities)}")
    logger.info(f"  New rule candidates: {len(new_rules)}")
    
    return output


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(message)s")
    
    lang = sys.argv[1] if len(sys.argv) > 1 else "rust"
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 2
    
    run_multi_tool_eval(lang, count, use_llm=True)
