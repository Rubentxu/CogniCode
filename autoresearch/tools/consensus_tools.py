"""Multi-tool consensus engine for ground truth classification.

Normalizes findings from multiple static analysis tools, matches them
by (file, line, rule_id), and classifies as TP/FP/FN/TN using
agreement-based consensus.
"""

import json
from pathlib import Path
from typing import Dict, List, Optional, Tuple
from dataclasses import dataclass, field
from collections import defaultdict
import logging

logger = logging.getLogger(__name__)


# ═══════════════════════════════════════════════════════════════════════
# Data Types
# ═══════════════════════════════════════════════════════════════════════

@dataclass
class Finding:
    tool: str
    file: str
    line: int
    column: Optional[int] = None
    rule_id: str = ""
    canonical_rule_id: str = ""
    severity: str = "P4"
    message: str = ""
    confidence: float = 0.0


@dataclass
class ClassifiedFinding:
    finding: Finding
    classification: str  # "TP", "FP", "FN", "TN", "UNCERTAIN"
    tools_agreeing: List[str] = field(default_factory=list)
    tools_disagreeing: List[str] = field(default_factory=list)
    confidence: float = 0.0
    consensus_level: str = "none"  # "strong", "likely", "weak", "none"


@dataclass
class RuleMetrics:
    rule_id: str
    language: str = "unknown"
    tp: int = 0
    fp: int = 0
    fn: int = 0
    tn: int = 0
    precision: Optional[float] = None
    recall: Optional[float] = None
    f1: Optional[float] = None
    fpr: Optional[float] = None
    execution_time_ms: float = 0.0
    issue_density: float = 0.0


# ═══════════════════════════════════════════════════════════════════════
# Rule ID Mapping
# ═══════════════════════════════════════════════════════════════════════

class RuleMapper:
    """Maps tool-specific rule IDs to canonical S{NUMBER} format."""
    
    def __init__(self, mapping_path: Optional[Path] = None):
        self.mapping: Dict[str, Dict[str, Optional[str]]] = {}
        self.reverse_mapping: Dict[str, Dict[str, str]] = defaultdict(dict)
        
        # Load built-in mappings
        self._load_defaults()
        
        # Load custom mappings if provided
        if mapping_path and mapping_path.exists():
            with open(mapping_path) as f:
                custom = json.load(f)
                self.mapping.update(custom)
    
    def _load_defaults(self):
        """Load default cross-tool rule mappings."""
        self.mapping = {
            "S2068": {
                "sonarqube": "S2068",
                "cognicode": "S2068",
                "eslint": "no-hardcoded-credentials",
                "clippy": None,
                "ruff": "S105",
            },
            "S134": {
                "sonarqube": "S134",
                "cognicode": "S134",
                "eslint": "max-depth",
                "clippy": None,
                "ruff": "PLR1702",
            },
            "S3776": {
                "sonarqube": "S3776",
                "cognicode": "S3776",
                "eslint": "complexity",
                "clippy": "cognitive_complexity",
                "ruff": "C901",
            },
            "S107": {
                "sonarqube": "S107",
                "cognicode": "S107",
                "eslint": "max-params",
                "clippy": "too_many_arguments",
                "ruff": "PLR0913",
            },
            "S5122": {
                "sonarqube": "S3649",
                "cognicode": "S5122",
                "eslint": "no-sql-injection",
                "clippy": None,
                "ruff": "S608",
            },
            "S138": {
                "sonarqube": "S138",
                "cognicode": "S138",
                "eslint": "max-lines-per-function",
                "clippy": "too_many_lines",
                "ruff": "PLR0915",
            },
            "S1481": {
                "sonarqube": "S1481",
                "cognicode": "S1481",
                "eslint": "no-unused-vars",
                "clippy": "unused_variables",
                "ruff": "F841",
            },
            "S1854": {
                "sonarqube": "S1854",
                "cognicode": "S1854",
                "eslint": "no-unused-vars",
                "clippy": "dead_code",
                "ruff": "F841",
            },
        }
    
    def to_canonical(self, tool: str, tool_rule_id: str) -> Optional[str]:
        """Convert a tool-specific rule ID to canonical format."""
        for canonical, tools in self.mapping.items():
            if tools.get(tool) == tool_rule_id:
                return canonical
        
        # Fuzzy match: try case-insensitive
        tool_rule_lower = tool_rule_id.lower()
        for canonical, tools in self.mapping.items():
            for t, rid in tools.items():
                if rid and rid.lower() == tool_rule_lower:
                    return canonical
        
        return None
    
    def to_tool(self, canonical: str, tool: str) -> Optional[str]:
        """Convert canonical rule ID to tool-specific format."""
        if canonical in self.mapping:
            return self.mapping[canonical].get(tool)
        return None


# ═══════════════════════════════════════════════════════════════════════
# Severity Normalization
# ═══════════════════════════════════════════════════════════════════════

class SeverityNormalizer:
    """Normalizes severity levels across tools to P0-P4 scale."""
    
    P0 = "P0"  # Blocker
    P1 = "P1"  # Critical/High
    P2 = "P2"  # Major/Medium
    P3 = "P3"  # Minor/Low
    P4 = "P4"  # Info
    
    MAP = {
        "sonarqube":  {"BLOCKER": "P0", "CRITICAL": "P1", "MAJOR": "P2", "MINOR": "P3", "INFO": "P4"},
        "cognicode":  {"CRITICAL": "P0", "HIGH": "P1", "MEDIUM": "P2", "LOW": "P3", "INFO": "P4"},
        "eslint":     {"error": "P1", "warn": "P2", "info": "P4"},
        "clippy":     {"error": "P1", "warning": "P2", "note": "P3", "help": "P4"},
        "ruff":       {"error": "P1", "warn": "P2", "info": "P4"},
        "spotbugs":   {"HIGH": "P1", "MEDIUM": "P2", "LOW": "P3"},
        "staticcheck":{"error": "P1", "warning": "P2"},
    }
    
    def normalize(self, tool: str, raw_severity: str) -> str:
        """Convert a tool-specific severity to P0-P4."""
        tool_map = self.MAP.get(tool, {})
        raw_upper = raw_severity.upper()
        
        if raw_upper in tool_map:
            return tool_map[raw_upper]
        
        # Fallback heuristics
        if any(word in raw_upper for word in ["BLOCKER", "CRITICAL", "FATAL"]):
            return "P0"
        if any(word in raw_upper for word in ["ERROR", "HIGH"]):
            return "P1"
        if any(word in raw_upper for word in ["WARN", "MEDIUM", "MAJOR"]):
            return "P2"
        if any(word in raw_upper for word in ["MINOR", "LOW", "NOTE"]):
            return "P3"
        
        return "P4"


# ═══════════════════════════════════════════════════════════════════════
# Consensus Engine
# ═══════════════════════════════════════════════════════════════════════

class ConsensusEngine:
    """Classifies findings via multi-tool agreement."""
    
    def __init__(self, mapper: Optional[RuleMapper] = None, 
                 normalizer: Optional[SeverityNormalizer] = None,
                 min_agreement: int = 2,
                 strong_threshold: int = 3,
                 location_tolerance: int = 1):
        self.mapper = mapper or RuleMapper()
        self.normalizer = normalizer or SeverityNormalizer()
        self.min_agreement = min_agreement
        self.strong_threshold = strong_threshold
        self.location_tolerance = location_tolerance
    
    def classify(self, findings: List[Finding]) -> List[ClassifiedFinding]:
        """Classify findings via multi-tool consensus.
        
        Steps:
        1. Normalize rule IDs and severities
        2. Group by (file, line±tolerance, canonical_rule_id)
        3. Count agreeing/disagreeing tools
        4. Classify based on agreement thresholds
        """
        # Step 1: Normalize
        for finding in findings:
            finding.canonical_rule_id = self.mapper.to_canonical(
                finding.tool, finding.rule_id
            ) or f"UNKNOWN_{finding.tool}_{finding.rule_id}"
            finding.severity = self.normalizer.normalize(
                finding.tool, finding.severity
            )
        
        # Step 2: Group by (file, line, rule)
        match_groups: Dict[Tuple[str, int, str], List[Finding]] = defaultdict(list)
        
        for finding in findings:
            # Add to exact line match
            key = (finding.file, finding.line, finding.canonical_rule_id)
            match_groups[key].append(finding)
            
            # Also add to neighboring lines (±tolerance)
            for delta in range(1, self.location_tolerance + 1):
                for neighbor in [finding.line + delta, finding.line - delta]:
                    if neighbor > 0:
                        neighbor_key = (finding.file, neighbor, finding.canonical_rule_id)
                        match_groups[neighbor_key].append(finding)
        
        # Step 3 & 4: Classify each group
        classified = []
        seen_finding_ids = set()
        
        for key, group_findings in match_groups.items():
            file, line, canonical_rule = key
            
            # Count unique tools agreeing
            tools_agreeing = list(set(f.tool for f in group_findings))
            tools_disagreeing = []
            n_tools = len(tools_agreeing)
            
            # Determine consensus level
            if n_tools >= self.strong_threshold:
                consensus = "strong"
                classification = "TP"
                confidence = 0.95
            elif n_tools >= self.min_agreement:
                consensus = "likely"
                classification = "TP"
                confidence = 0.75
            elif n_tools == 1:
                consensus = "weak"
                classification = "UNCERTAIN"
                confidence = 0.40
            else:
                consensus = "none"
                classification = "FP"
                confidence = 0.10
            
            # Create classified finding for the primary finding
            primary = group_findings[0]
            
            classified.append(ClassifiedFinding(
                finding=primary,
                classification=classification,
                tools_agreeing=tools_agreeing,
                tools_disagreeing=tools_disagreeing,
                confidence=confidence,
                consensus_level=consensus,
            ))
        
        return classified
    
    def compute_rule_metrics(self, classified: List[ClassifiedFinding], 
                             rule_id: str) -> RuleMetrics:
        """Compute precision, recall, F1, FPR from classified findings."""
        tp = sum(1 for c in classified 
                if c.finding.canonical_rule_id == rule_id and c.classification == "TP")
        fp = sum(1 for c in classified 
                if c.finding.canonical_rule_id == rule_id and c.classification == "FP")
        fn = sum(1 for c in classified 
                if c.finding.canonical_rule_id == rule_id and c.classification == "FN")
        tn = sum(1 for c in classified 
                if c.finding.canonical_rule_id == rule_id and c.classification == "TN")
        
        metrics = RuleMetrics(rule_id=rule_id)
        metrics.tp = tp
        metrics.fp = fp
        metrics.fn = fn
        metrics.tn = tn
        
        # Precision
        if tp + fp > 0:
            metrics.precision = tp / (tp + fp)
        
        # Recall
        if tp + fn > 0:
            metrics.recall = tp / (tp + fn)
        
        # F1
        if metrics.precision is not None and metrics.recall is not None:
            if metrics.precision + metrics.recall > 0:
                metrics.f1 = 2 * (metrics.precision * metrics.recall) / (metrics.precision + metrics.recall)
        
        # FPR
        if fp + tn > 0:
            metrics.fpr = fp / (fp + tn)
        
        return metrics
    
    def compute_health_score(self, metrics: RuleMetrics, 
                             weights: Optional[Dict[str, float]] = None) -> float:
        """Compute composite Health Score for a rule.
        
        Health Score = w_f1×F1 + w_snr×SNR + w_res×RES + w_dar×DAR - w_cost×cost
        
        For Phase 1 MVP: simplified version with only F1 and FPR.
        """
        if weights is None:
            weights = {"f1": 0.50, "fpr_penalty": 0.50}
        
        score = 0.0
        
        # F1 contribution (0 to 1)
        if metrics.f1 is not None:
            score += weights["f1"] * metrics.f1
        
        # FPR penalty (lower is better, 0 FPR = no penalty)
        if metrics.fpr is not None:
            fpr_penalty = max(0, 1.0 - metrics.fpr * 10)  # FPR 0.10 → penalty 1.0
            score += weights["fpr_penalty"] * fpr_penalty
        
        return max(0.0, min(1.0, score))
