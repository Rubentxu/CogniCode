"""Metric computation and evolution log tools."""

import csv
import json
from pathlib import Path
from typing import Dict, List, Optional
from datetime import datetime
import logging

logger = logging.getLogger(__name__)


class EvolutionLogger:
    """Manages the evolution.tsv experiment log."""
    
    COLUMNS = [
        "timestamp", "iteration", "rule_id", "language",
        "f1_before", "f1_after", "fpr_before", "fpr_after",
        "precision", "recall", "execution_ms",
        "health_before", "health_after",
        "decision", "description"
    ]
    
    def __init__(self, log_path: Path):
        self.log_path = log_path
        self._ensure_header()
    
    def _ensure_header(self):
        if not self.log_path.exists():
            with open(self.log_path, "w", newline="") as f:
                writer = csv.writer(f, delimiter="\t")
                writer.writerow(self.COLUMNS)
    
    def log_experiment(self, iteration: int, rule_id: str, language: str,
                       metrics_before: dict, metrics_after: dict,
                       decision: str, description: str):
        """Append an experiment result to the log."""
        with open(self.log_path, "a", newline="") as f:
            writer = csv.writer(f, delimiter="\t")
            writer.writerow([
                datetime.now().isoformat(),
                iteration,
                rule_id,
                language,
                metrics_before.get("f1", ""),
                metrics_after.get("f1", ""),
                metrics_before.get("fpr", ""),
                metrics_after.get("fpr", ""),
                metrics_after.get("precision", ""),
                metrics_after.get("recall", ""),
                metrics_after.get("execution_ms", ""),
                metrics_before.get("health", ""),
                metrics_after.get("health", ""),
                decision,
                description,
            ])
    
    def read_history(self) -> List[dict]:
        """Read all experiments from the log."""
        if not self.log_path.exists():
            return []
        
        with open(self.log_path, newline="") as f:
            reader = csv.DictReader(f, delimiter="\t")
            return list(reader)
    
    def recently_attempted_rules(self, n: int = 5) -> List[str]:
        """Get rule IDs attempted in the last N iterations."""
        history = self.read_history()
        recent = history[-n:] if len(history) >= n else history
        return [row["rule_id"] for row in recent if row.get("rule_id")]


class BaselineStore:
    """Manages baseline metrics for rules."""
    
    def __init__(self, baseline_dir: Path):
        self.baseline_dir = baseline_dir
        self.baseline_dir.mkdir(parents=True, exist_ok=True)
    
    def save(self, rule_metrics: dict):
        """Save baseline metrics for all rules."""
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        path = self.baseline_dir / f"baseline_{timestamp}.json"
        
        with open(path, "w") as f:
            json.dump(rule_metrics, f, indent=2)
        
        # Also save as "latest" for quick access
        latest_path = self.baseline_dir / "latest.json"
        with open(latest_path, "w") as f:
            json.dump(rule_metrics, f, indent=2)
        
        logger.info(f"Baseline saved to {path}")
    
    def load(self) -> dict:
        """Load the latest baseline metrics."""
        latest = self.baseline_dir / "latest.json"
        if not latest.exists():
            return {}
        
        with open(latest) as f:
            return json.load(f)


def compute_delta(before: dict, after: dict) -> dict:
    """Compute metric deltas between before and after states."""
    delta = {}
    
    for key in ["f1", "precision", "recall", "fpr"]:
        v_before = before.get(key)
        v_after = after.get(key)
        if v_before is not None and v_after is not None:
            delta[f"delta_{key}"] = v_after - v_before
    
    return delta


def format_metrics_table(rule_id: str, before: dict, after: dict, delta: dict) -> str:
    """Format metrics as a human-readable table."""
    lines = []
    lines.append(f"\n{'='*60}")
    lines.append(f"  Rule: {rule_id}")
    lines.append(f"{'='*60}")
    lines.append(f"  {'Metric':<15} {'Before':>10} {'After':>10} {'Delta':>10}")
    lines.append(f"  {'-'*45}")
    
    for key, label in [("f1", "F1 Score"), ("precision", "Precision"), 
                        ("recall", "Recall"), ("fpr", "FPR")]:
        b = before.get(key)
        a = after.get(key)
        d = delta.get(f"delta_{key}")
        
        b_str = f"{b:.4f}" if b is not None else "N/A"
        a_str = f"{a:.4f}" if a is not None else "N/A"
        d_str = f"{d:+.4f}" if d is not None else "N/A"
        
        lines.append(f"  {label:<15} {b_str:>10} {a_str:>10} {d_str:>10}")
    
    lines.append(f"{'='*60}")
    return "\n".join(lines)
