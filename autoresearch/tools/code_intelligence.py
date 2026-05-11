#!/usr/bin/env python3
"""CogniCode as Code Intelligence Layer for LLM — NOT as self-evaluator.

Problem: Using CogniCode to evaluate CogniCode is circular.
Solution: CogniCode provides DEEP CODE UNDERSTANDING to the LLM,
         while EXTERNAL tools (Clippy, SonarQube) provide ground truth.

Flow:
  1. External tools find real issues → GROUND TRUTH
  2. CogniCode analyzes code structure → CONTEXT for LLM
  3. LLM with rich context → UNDERSTANDS why rules fail
  4. LLM proposes specific fixes → IMPROVEMENT
"""

import sys
import json
import subprocess
from pathlib import Path
from typing import Dict, List, Optional
import logging

logger = logging.getLogger(__name__)

AUTORESEARCH_DIR = Path(__file__).parent.parent


class CodeIntelligence:
    """Uses CogniCode's deep analysis to give LLM rich code context.
    
    NOT for self-evaluation. For providing CODE UNDERSTANDING.
    """
    
    def __init__(self):
        self.binary = AUTORESEARCH_DIR.parent / "target" / "release" / "cognicode"
    
    def analyze_for_llm(self, repo_dir: Path, rule_id: str, 
                         ground_truth_findings: List[dict]) -> Dict:
        """Analyze code to help LLM understand WHY a rule is failing.
        
        Returns context the LLM can use:
        - Code snippets where ground truth found issues
        - AST patterns around those locations
        - Call graphs showing affected functions
        - Complexity metrics for affected files
        """
        
        context = {
            "rule_id": rule_id,
            "ground_truth_count": len(ground_truth_findings),
            "affected_files": [],
            "code_snippets": [],
            "patterns": [],
        }
        
        # For each ground truth finding, extract code context
        for finding in ground_truth_findings[:5]:  # Top 5 examples
            file_path = finding.get("file", "")
            line = finding.get("line", 0)
            
            if not file_path or not line:
                continue
            
            full_path = repo_dir / file_path if not Path(file_path).is_absolute() else Path(file_path)
            
            if not full_path.exists():
                continue
            
            # Read the code around the finding
            try:
                code = full_path.read_text()
                lines = code.split("\n")
                
                # Extract context (±5 lines around the finding)
                start = max(0, line - 6)
                end = min(len(lines), line + 5)
                snippet = "\n".join(f"{i+1}: {l}" for i, l in enumerate(lines[start:end], start))
                
                context["code_snippets"].append({
                    "file": str(file_path),
                    "line": line,
                    "snippet": snippet,
                    "message": finding.get("message", "")[:200],
                    "rule": finding.get("rule", ""),
                })
                
                if str(file_path) not in [f["path"] for f in context["affected_files"]]:
                    context["affected_files"].append({
                        "path": str(file_path),
                        "lines": len(lines),
                        "language": self._detect_language(str(file_path)),
                    })
                    
            except Exception as e:
                logger.debug(f"Could not read {file_path}: {e}")
        
        # Use CogniCode to get structural analysis
        if self.binary.exists():
            context["cognicode_analysis"] = self._run_cognicode_analysis(repo_dir)
        
        return context
    
    def _run_cognicode_analysis(self, repo_dir: Path) -> Dict:
        """Run CogniCode's deep analysis for structural context."""
        try:
            result = subprocess.run(
                [str(self.binary), "analyze", str(repo_dir)],
                capture_output=True, text=True, timeout=60
            )
            
            analysis = {"architecture_score": None, "cycles": 0, "complexity": []}
            
            for line in result.stdout.split("\n"):
                if "Score:" in line:
                    try:
                        analysis["architecture_score"] = float(line.split(":")[1].split("/")[0])
                    except: pass
                if "cycles detected" in line:
                    try:
                        analysis["cycles"] = int(line.split()[0])
                    except: pass
            
            return analysis
        except Exception:
            return {}
    
    def _detect_language(self, file_path: str) -> str:
        ext = Path(file_path).suffix.lower()
        return {
            ".rs": "rust", ".py": "python", ".js": "javascript",
            ".ts": "typescript", ".java": "java", ".go": "go",
        }.get(ext, "unknown")
    
    def build_llm_prompt(self, rule_id: str, context: Dict, 
                          comparison_metrics: Dict) -> str:
        """Build a rich LLM prompt using CogniCode's code intelligence.
        
        The LLM gets:
        1. Ground truth: what external tools found (real issues)
        2. Code context: snippets where issues were found
        3. Structural analysis: architecture, complexity from CogniCode
        4. Current rule: the CogniCode rule that should detect this
        
        The LLM reasons:
        "Given this code pattern (from CogniCode analysis),
         and these ground truth findings (from Clippy/SonarQube),
         why is CogniCode's rule missing these issues,
         and what specific change would fix it?"
        """
        
        snippets_text = ""
        for s in context.get("code_snippets", [])[:3]:
            snippets_text += f"""
### {s['file']}:{s['line']} — {s['rule']}
{s['message']}
```{context.get('language', 'rust')}
{s['snippet']}
```
"""
        
        arch = context.get("cognicode_analysis", {})
        arch_text = ""
        if arch:
            arch_text = f"""
## Code Structure (from CogniCode analysis)
- Architecture score: {arch.get('architecture_score', 'N/A')}/100
- Detected cycles: {arch.get('cycles', 'N/A')}
- Affected files: {len(context.get('affected_files', []))}
"""
        
        metrics = comparison_metrics.get(rule_id, {})
        
        prompt = f"""# Rule Improvement Analysis

## Ground Truth (External Tools)
- Rule: {rule_id}
- Real issues found: {context.get('ground_truth_count', 0)}
- Current CogniCode detection: TP={metrics.get('tp', 0)}, FN={metrics.get('fn', 0)}
{arch_text}

## Code Examples (where issues were found)
{snippets_text}

## Task
Analyze WHY CogniCode's rule for '{rule_id}' is missing these real issues.
Use the code structure analysis above to identify:
1. What pattern is the rule CURRENTLY looking for?
2. What pattern is it MISSING? (based on the code snippets)
3. What specific change would improve detection?

Return JSON:
{{"root_cause": "...", "missing_pattern": "...", 
  "suggested_fix": "specific change to regex/AST logic",
  "improvement_type": "regex_tighten|pattern_extend|threshold_tune|new_rule",
  "expected_f1_delta": 0.05, "confidence": 0.8}}"""
        
        return prompt


# ═══════════════════════════════════════════════════════════════════
# Integration: How this changes the evolve.py workflow
# ═══════════════════════════════════════════════════════════════════

"""
BEFORE (circular):
  Clippy → findings
  CogniCode → findings  
  Compare → metrics
  LLM → "improve rule" (with no code context)
  
AFTER (correct):
  Clippy + SonarQube → GROUND TRUTH (trusted external tools)
  CogniCode → CODE INTELLIGENCE (AST, call graphs, complexity)
  CodeIntelligence → RICH LLM PROMPT (snippets + structure)
  LLM → "Given this code pattern and these missed issues,
          add negative lookahead for X pattern to reduce FP"
  
CogniCode's role:
  ✅ Parse AST to show code structure around findings
  ✅ Show call graphs for affected functions
  ✅ Report complexity metrics for context
  ✅ Help LLM understand WHY a pattern is missed
  ❌ NOT: evaluate its own rules (circular!)
"""
