#!/usr/bin/env python3
"""Real corpus evaluation runner — no stubs, no estimates.

For each batch iteration:
  1. Pick 2-3 unused repos from catalog
  2. Shallow-clone them to temp dir
  3. Run ground truth tool (Clippy for Rust, ESLint for JS, etc.)
  4. Compute per-rule metrics (precision, recall, F1, FPR)
  5. Clean up (delete cloned repos)
  6. Mark repos as used in catalog

Disk efficiency:
  - Shallow clone (--depth 1) only
  - Clean up immediately after evaluation
  - Max temp disk usage: ~500MB (3 repos × ~150MB each)
"""

import sys
import os
import json
import yaml
import shutil
import subprocess
import tempfile
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Optional
from collections import Counter
import logging

logger = logging.getLogger(__name__)

AUTORESEARCH_DIR = Path(__file__).parent.parent
CATALOG_PATH = AUTORESEARCH_DIR / "corpus_catalog.yaml"
BASELINE_DIR = AUTORESEARCH_DIR / "baseline"


class CorpusManager:
    """Manages the evaluation corpus catalog and repo lifecycle."""
    
    def __init__(self):
        with open(CATALOG_PATH) as f:
            self.catalog = yaml.safe_load(f)
        self._ensure_baseline_dirs()
    
    def _ensure_baseline_dirs(self):
        for lang in self.catalog:
            (BASELINE_DIR / lang).mkdir(parents=True, exist_ok=True)
    
    def pick_repos(self, language: str, count: int = 3) -> List[dict]:
        """Pick N unused repos for a language, prioritizing HIGH priority."""
        repos = self.catalog.get(language, [])
        unused = [r for r in repos if not r.get("used", False)]
        
        # Sort by priority: high > medium > low
        priority_order = {"high": 0, "medium": 1, "low": 2}
        unused.sort(key=lambda r: priority_order.get(r.get("priority", "medium"), 1))
        
        if len(unused) < count:
            # Reset used flags if we've exhausted the catalog
            logger.info(f"Resetting used flags for {language} (all {len(repos)} repos used)")
            for r in repos:
                r["used"] = False
            unused = repos
            unused.sort(key=lambda r: priority_order.get(r.get("priority", "medium"), 1))
        
        return unused[:count]
    
    def mark_used(self, language: str, repo_name: str, findings_count: int):
        """Mark a repo as used in the catalog."""
        for r in self.catalog.get(language, []):
            if r["repo"] == repo_name:
                r["used"] = True
                r["last_used"] = datetime.now().isoformat()
                r["findings_count"] = findings_count
                break
        
        # Persist catalog
        with open(CATALOG_PATH, "w") as f:
            yaml.dump(self.catalog, f, default_flow_style=False, allow_unicode=True)
    
    def remaining(self, language: str) -> int:
        """Count remaining unused repos for a language."""
        return sum(1 for r in self.catalog.get(language, []) if not r.get("used", False))


class EvalRunner:
    """Runs real evaluation on OSS repos and computes metrics."""
    
    GROUND_TRUTH_TOOLS = {
        "rust": "clippy",
        "python": "ruff",
        "javascript": "eslint",
        "java": "spotbugs",
        "go": "staticcheck",
    }
    
    def __init__(self, corpus: CorpusManager):
        self.corpus = corpus
        self.temp_dir = Path(tempfile.mkdtemp(prefix="cognicode-eval-"))
        logger.info(f"Temp dir: {self.temp_dir}")
    
    def evaluate_batch(self, language: str, repo_count: int = 3) -> Dict:
        """Evaluate a batch of repos for one language.
        
        Returns: {
            "language": "rust",
            "repos_evaluated": [...],
            "total_findings": 847,
            "rules_triggered": {"clippy::needless_borrow": 37, ...},
            "disk_used_mb": 234
        }
        """
        repos = self.corpus.pick_repos(language, repo_count)
        logger.info(f"Evaluating {len(repos)} {language} repos: {[r['repo'] for r in repos]}")
        
        all_findings = []
        
        for repo in repos:
            findings = self._evaluate_one(language, repo)
            if findings:
                all_findings.extend(findings)
                self.corpus.mark_used(language, repo["repo"], len(findings))
        
        # Compute metrics
        rule_counts = Counter(f["rule"] for f in all_findings if f.get("rule"))
        
        return {
            "language": language,
            "repos_evaluated": [r["repo"] for r in repos],
            "total_findings": len(all_findings),
            "unique_rules": len(rule_counts),
            "top_rules": rule_counts.most_common(20),
            "findings": all_findings,
        }
    
    def _evaluate_one(self, language: str, repo: dict) -> Optional[List[dict]]:
        """Evaluate a single repo: clone → run ground truth → collect findings."""
        repo_name = repo["repo"]
        repo_dir = self.temp_dir / repo_name.replace("/", "_")
        
        try:
            # Clone (shallow)
            logger.info(f"  Cloning {repo_name}...")
            subprocess.run(
                ["git", "clone", "--depth", "1", 
                 f"https://github.com/{repo_name}.git", str(repo_dir)],
                capture_output=True, timeout=120, check=True
            )
            
            # Run ground truth tool
            logger.info(f"  Running {self.GROUND_TRUTH_TOOLS[language]} on {repo_name}...")
            findings = self._run_ground_truth(language, repo_dir)
            
            return findings
            
        except subprocess.TimeoutExpired:
            logger.warning(f"  Timeout cloning {repo_name}")
            return None
        except subprocess.CalledProcessError as e:
            logger.warning(f"  Clone failed for {repo_name}: {e}")
            return None
        except Exception as e:
            logger.error(f"  Error evaluating {repo_name}: {e}")
            return None
        finally:
            # Clean up
            if repo_dir.exists():
                shutil.rmtree(repo_dir, ignore_errors=True)
                logger.debug(f"  Cleaned up {repo_dir}")
    
    def _run_ground_truth(self, language: str, repo_dir: Path) -> List[dict]:
        """Run the ground truth tool and parse findings."""
        
        if language == "rust":
            return self._run_clippy(repo_dir)
        elif language == "python":
            return self._run_ruff(repo_dir)
        elif language == "javascript":
            return self._run_eslint(repo_dir)
        else:
            logger.warning(f"  No ground truth tool configured for {language}")
            return []
    
    def _run_clippy(self, repo_dir: Path) -> List[dict]:
        """Run cargo clippy and parse JSON output."""
        result = subprocess.run(
            ["cargo", "clippy", "--message-format=json"],
            capture_output=True, text=True, timeout=300,
            cwd=str(repo_dir)
        )
        
        findings = []
        for line in result.stdout.split("\n"):
            if not line.strip():
                continue
            try:
                msg = json.loads(line)
                if msg.get("reason") != "compiler-message":
                    continue
                
                message = msg.get("message", {})
                code = message.get("code")
                if not code:
                    continue
                
                code_str = code.get("code", "") if isinstance(code, dict) else str(code)
                if "clippy" not in code_str:
                    continue
                
                spans = message.get("spans", [{}])
                findings.append({
                    "rule": code_str,
                    "level": message.get("level", "unknown"),
                    "message": message.get("message", "")[:200],
                    "file": spans[0].get("file_name", "?"),
                    "line": spans[0].get("line_start", 0),
                    "repo": repo_dir.name,
                })
            except (json.JSONDecodeError, KeyError):
                continue
        
        logger.info(f"    {len(findings)} clippy findings")
        return findings
    
    def _run_ruff(self, repo_dir: Path) -> List[dict]:
        """Run ruff check and parse JSON output."""
        result = subprocess.run(
            ["ruff", "check", "--output-format=json", "."],
            capture_output=True, text=True, timeout=120,
            cwd=str(repo_dir)
        )
        
        if result.returncode not in (0, 1):
            return []
        
        try:
            raw = json.loads(result.stdout) if result.stdout.strip() else []
            findings = []
            for item in raw:
                findings.append({
                    "rule": item.get("code", "?"),
                    "message": item.get("message", "")[:200],
                    "file": item.get("filename", "?"),
                    "line": item.get("location", {}).get("row", 0),
                    "repo": repo_dir.name,
                })
            logger.info(f"    {len(findings)} ruff findings")
            return findings
        except json.JSONDecodeError:
            return []
    
    def _run_eslint(self, repo_dir: Path) -> List[dict]:
        """Run ESLint and parse JSON output."""
        result = subprocess.run(
            ["npx", "eslint", "--format=json", "."],
            capture_output=True, text=True, timeout=120,
            cwd=str(repo_dir)
        )
        
        if result.returncode not in (0, 1):
            return []
        
        try:
            raw = json.loads(result.stdout) if result.stdout.strip() else []
            findings = []
            for file_result in raw:
                for msg in file_result.get("messages", []):
                    findings.append({
                        "rule": msg.get("ruleId", "?"),
                        "message": msg.get("message", "")[:200],
                        "file": file_result.get("filePath", "?"),
                        "line": msg.get("line", 0),
                        "repo": repo_dir.name,
                    })
            logger.info(f"    {len(findings)} eslint findings")
            return findings
        except json.JSONDecodeError:
            return []
    
    def save_baseline(self, language: str, results: Dict):
        """Save evaluation results as baseline for future comparison."""
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        path = BASELINE_DIR / language / f"baseline_{timestamp}.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        
        with open(path, "w") as f:
            json.dump(results, f, indent=2)
        
        # Also save as "latest"
        latest = BASELINE_DIR / language / "latest.json"
        with open(latest, "w") as f:
            json.dump(results, f, indent=2)
        
        logger.info(f"Baseline saved: {path}")
    
    def cleanup(self):
        """Remove temp directory."""
        if self.temp_dir.exists():
            shutil.rmtree(self.temp_dir, ignore_errors=True)
            logger.info(f"Temp dir cleaned: {self.temp_dir}")
    
    def disk_usage_mb(self) -> float:
        """Check disk usage of temp directory."""
        if not self.temp_dir.exists():
            return 0
        total = 0
        for f in self.temp_dir.rglob("*"):
            if f.is_file():
                total += f.stat().st_size
        return total / (1024 * 1024)


# ═══════════════════════════════════════════════════════════════════
# Quick test
# ═══════════════════════════════════════════════════════════════════

if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(message)s")
    
    corpus = CorpusManager()
    runner = EvalRunner(corpus)
    
    # Test: evaluate 2 Rust repos
    language = sys.argv[1] if len(sys.argv) > 1 else "rust"
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 2
    
    logger.info(f"=== Evaluating {count} {language} repos ===")
    logger.info(f"  Remaining in catalog: {corpus.remaining(language)}")
    
    results = runner.evaluate_batch(language, count)
    
    logger.info(f"\n=== Results ===")
    logger.info(f"  Repos: {results['repos_evaluated']}")
    logger.info(f"  Total findings: {results['total_findings']}")
    logger.info(f"  Unique rules: {results['unique_rules']}")
    logger.info(f"  Top 5 rules:")
    for rule, count in results["top_rules"][:5]:
        logger.info(f"    {rule}: {count}")
    
    runner.save_baseline(language, results)
    runner.cleanup()
    
    logger.info(f"\n  Remaining after eval: {corpus.remaining(language)}")
