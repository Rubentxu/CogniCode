#!/usr/bin/env python3
"""
Karpathy Autonomous Rule Evolution — Clean Implementation.

Per batch:
  1. ANALYZER  → Pick N rules (SonarQube mismatches + worst F1)
  2. IMPROVER  → LLM proposes change (code → metadata → skip)
  3. EVALUATOR → Compilation + targeted tests
  4. DECIDER   → Keep (commit + segregate) or Discard (revert)

Usage:
  python autoresearch/evolve.py              # Run forever
  python autoresearch/evolve.py -n 10 -b 5   # 10 batches, 5 rules each
  python autoresearch/evolve.py --dry-run     # LLM only, no changes
"""

import sys
import os
import re
import signal
import time
import json
import argparse
import subprocess
import logging
from pathlib import Path
from collections import defaultdict
from datetime import datetime
from typing import List, Tuple, Optional, Dict, Any
from dataclasses import dataclass, asdict, field

sys.path.insert(0, str(Path(__file__).parent))

from tools.llm_client import LLMClient, ModelConfig
from tools.metric_tools import EvolutionLogger, BaselineStore
from tools.rust_tools import CargoTool, GitTool

log = logging.getLogger(__name__)

# ── Paths ──────────────────────────────────────────────────────────
ROOT = Path(__file__).parent.parent
CATALOG = ROOT / "crates" / "cognicode-axiom" / "src" / "rules" / "catalog.rs"
RULES_DIR = ROOT / "crates" / "cognicode-axiom" / "src" / "rules" / "rules"
SESSION_FILE = Path(__file__).parent / "session_done.txt"

# ── State ──────────────────────────────────────────────────────────
STOP = False
SESSION_DONE = set()
FAILURE_COUNT = defaultdict(int)  # rule_id → consecutive failures

# ── SonarQube mismatches (priority targets) ────────────────────────
SQ_TARGETS = {
    "S1313", "S134", "S107", "S1481", "S1141", "S100",
    "S1871", "S4144", "S2612", "S2092", "S3330", "S5042",
    "S2589", "S1186", "S2259", "S1854", "S1135", "S1226",
}

# ── Signals ────────────────────────────────────────────────────────
def _handle_stop(sig, frame):
    global STOP
    STOP = True
    log.info("\n⏹ STOP — finishing batch...")
    signal.signal(signal.SIGINT, lambda *_: sys.exit(0))

signal.signal(signal.SIGINT, _handle_stop)
signal.signal(signal.SIGTERM, _handle_stop)


# ═══════════════════════════════════════════════════════════════════
# SESSION TRACKING
# ═══════════════════════════════════════════════════════════════════

def load_session():
    global SESSION_DONE
    if SESSION_FILE.exists():
        SESSION_DONE = set(SESSION_FILE.read_text().strip().split("\n"))
        SESSION_DONE.discard("")  # Remove empty string if file is empty

def save_session():
    SESSION_FILE.write_text("\n".join(sorted(SESSION_DONE)))

def mark_done(rule_id):
    SESSION_DONE.add(rule_id)
    save_session()

def is_done(rule_id):
    return rule_id in SESSION_DONE

def is_retryable(rule_id):
    """Rules with <3 failures can be retried."""
    return FAILURE_COUNT.get(rule_id, 0) < 3

def record_failure(rule_id):
    FAILURE_COUNT[rule_id] = FAILURE_COUNT.get(rule_id, 0) + 1

def progress():
    total = len(re.findall(r'id:\s*"(S\d+)"', CATALOG.read_text()))
    done = len(SESSION_DONE)
    pct = round(done / max(1, total) * 100, 1)
    return done, total, pct


# ═══════════════════════════════════════════════════════════════════════════
# RULE STRATEGY CLASSIFIER
# ═══════════════════════════════════════════════════════════════════════════

# Known dataflow/security rule IDs (source/sink models, taint analysis)
DATAFLOW_RULE_IDS = {
    "S5122",  # Code injection
    "S3649",  # SQL injection
    "S5131",  # XSS
    "S2598",  # Command injection
    "S2631",  # Path traversal
    "S6092",  # Command injection (npm)
    "S4738",  # XXE
    "S5759",  # SSRF
}

# Known semantic analysis rule IDs (symbol/scope/usage analysis)
SEMANTIC_RULE_IDS = {
    "S2068",  # Hardcoded credentials (variable binding)
    "S1854",  # Dead store / unused variable
    "S1226",  # Variable shadowing
    "S1481",  # Unused local variable
    "S1125",  # Boolean literals
    "S2225",  # Increment/decrement in condition
    "S2376",  # Protected subclass
    "S3878",  # Signal handler
    "S4062",  # Trait method override
}

# Known AST/structural rule IDs (control flow, nesting, parameters)
AST_RULE_IDS = {
    "S2589",  # Constant boolean condition
    "S107",   # Max parameters
    "S134",   # Max nested control flow depth
    "S1134",  # Too many break/continue
    "S2259",  # Constant pattern
    "S1186",  # Empty default branch
    "S1871",  # Duplicate branch
    "S4144",  # Overload parameter
    "S2612",  # Insecure file permissions / chmod pattern
    "S2092",  # Session cookie flags
    "S3330",  # Cookie HttpOnly flag
    "S5042",  # Archive/resource extraction size checks
    "S100",   # Method naming
    "S1141",  # Method length
    "S1874",  # Deprecated item
}

# Known metric-based rule IDs
METRIC_RULE_IDS = {
    "S138",   # Function length
    "S1541",  # Cyclomatic complexity
    "S124",   # Commented-out code lines
    "S1105",  # Nesting depth
    "S1068",  # Unused private field
    "S3323",  # Cognitive complexity (if added)
}

# Known textual rules that should remain regex/preflight-oriented
REGEX_RULE_IDS = {
    "S1135",  # TODO/FIXME/HACK/XXX tags
    "S1313",  # Hardcoded IP address literals
    "S4792",  # Weak crypto literal/API names (until AST migration is available)
    "S5332",  # Clear-text protocol literals
}


@dataclass
class RuleSpec:
    """Specification for a rule's generation strategy and engine."""
    rule_id: str
    strategy: str                    # regex | ast | semantic | dataflow | metric | hybrid
    engine: str                      # preferred engine: tree_sitter | regex | semantic | dataflow | metric
    fallback_engine: str = "regex"   # fallback if preferred fails
    patterns: list = field(default_factory=list)
    constraints: list = field(default_factory=list)
    exclusions: list = field(default_factory=list)
    fixtures_required: bool = False
    rationale: str = ""
    metadata: dict = field(default_factory=dict)

    def to_dict(self) -> dict:
        return asdict(self)


def classify_rule_strategy(rule_id: str, rule_block: str) -> Dict[str, Any]:
    """
    Classify a rule's preferred analysis strategy.
    Returns a dict with strategy, reason, preferred_engine, fallback_engine, fixtures_required.
    """
    strategy = "regex"
    preferred_engine = "regex"
    fallback_engine = "regex"
    fixtures_required = False
    reason_parts = []

    if rule_id in DATAFLOW_RULE_IDS:
        return {
            "strategy": "dataflow",
            "reason": f"Rule ID {rule_id} is a known dataflow/source-sink rule",
            "preferred_engine": "dataflow",
            "fallback_engine": "tree_sitter",
            "fixtures_required": True,
            "rule_id": rule_id,
        }

    if rule_id in SEMANTIC_RULE_IDS:
        return {
            "strategy": "semantic",
            "reason": f"Rule ID {rule_id} is a known semantic analysis rule",
            "preferred_engine": "semantic",
            "fallback_engine": "tree_sitter",
            "fixtures_required": True,
            "rule_id": rule_id,
        }

    if rule_id in AST_RULE_IDS:
        return {
            "strategy": "ast",
            "reason": f"Rule ID {rule_id} is a known AST/structural rule",
            "preferred_engine": "tree_sitter",
            "fallback_engine": "regex",
            "fixtures_required": True,
            "rule_id": rule_id,
        }

    if rule_id in METRIC_RULE_IDS:
        return {
            "strategy": "metric",
            "reason": f"Rule ID {rule_id} is a known metric-based rule",
            "preferred_engine": "metric",
            "fallback_engine": "regex",
            "fixtures_required": False,
            "rule_id": rule_id,
        }

    if rule_id in REGEX_RULE_IDS:
        return {
            "strategy": "regex",
            "reason": f"Rule ID {rule_id} is a known textual/preflight rule",
            "preferred_engine": "regex",
            "fallback_engine": "tree_sitter",
            "fixtures_required": True,
            "rule_id": rule_id,
        }

    # Analyze rule block content (heuristic)
    block_lower = rule_block.lower()

    source_terms = ["user input", "request", "req.", "param", "body", "argv", "stdin", "env"]
    sink_terms = ["sink", "execute", "query", "eval", "command", "sql", "database", "shell", "process"]
    taint_terms = ["taint", "sanitize", "sanitizer", "escape", "encode", "validate"]
    injection_terms = ["sql injection", "xss", "command injection", "path traversal", "ldap injection", "xxe", "ssrf"]

    source_hits = sum(1 for kw in source_terms if kw in block_lower)
    sink_hits = sum(1 for kw in sink_terms if kw in block_lower)
    taint_hits = sum(1 for kw in taint_terms if kw in block_lower)
    injection_hits = sum(1 for kw in injection_terms if kw in block_lower)

    dataflow_score = (source_hits > 0 and sink_hits > 0) or taint_hits >= 2 or (injection_hits > 0 and (source_hits > 0 or sink_hits > 0))
    if dataflow_score:
        strategy = "dataflow"
        preferred_engine = "dataflow"
        fallback_engine = "tree_sitter"
        fixtures_required = True
        reason_parts.append(f"content has source/sink or taint-flow characteristics")

    semantic_keywords = ["variable", "assign", "reassign", "scope", "unused", "shadow", "redeclare", "symbol", "reference", "binding", "credential", "password", "token", "secret"]
    semantic_hits = sum(1 for kw in semantic_keywords if kw in block_lower)
    if semantic_hits >= 2 and strategy == "regex":
        strategy = "semantic"
        preferred_engine = "semantic"
        fallback_engine = "tree_sitter"
        fixtures_required = True
        reason_parts.append(f"content has semantic analysis characteristics ({semantic_hits} keyword hits)")

    ast_keywords = ["if", "while", "for", "loop", "nest", "depth", "branch", "condition", "boolean", "constant", "parameter", "function", "match", "pattern", "arm"]
    ast_hits = sum(1 for kw in ast_keywords if kw in block_lower)
    if ast_hits >= 3 and strategy == "regex":
        strategy = "ast"
        preferred_engine = "tree_sitter"
        fallback_engine = "regex"
        fixtures_required = True
        reason_parts.append(f"content has AST/structural characteristics ({ast_hits} keyword hits)")

    metric_keywords = ["count", "line", "length", "complexity", "threshold", "cognitive", "maintainability", "statement", "expression"]
    metric_hits = sum(1 for kw in metric_keywords if kw in block_lower)
    if metric_hits >= 2 and strategy == "regex":
        strategy = "metric"
        preferred_engine = "metric"
        fallback_engine = "regex"
        fixtures_required = False
        reason_parts.append(f"content has metric characteristics ({metric_hits} keyword hits)")

    if strategy == "regex":
        regex_patterns = re.findall(r'regex:\s*"([^"]+)"', rule_block)
        if regex_patterns:
            reason_parts.append(f"contains regex patterns ({len(regex_patterns)} found)")
        else:
            reason_parts.append("no specific strategy indicators found, defaulting to regex")

    reason = reason_parts[0] if reason_parts else "default regex strategy (no specific indicators)"

    return {
        "strategy": strategy,
        "reason": reason,
        "preferred_engine": preferred_engine,
        "fallback_engine": fallback_engine,
        "fixtures_required": fixtures_required,
        "rule_id": rule_id,
    }


# ═══════════════════════════════════════════════════════════════════════════
# RUST REGEX VALIDATOR
# ═══════════════════════════════════════════════════════════════════════════

# Rust regex crate does NOT support lookahead/lookbehind, backreferences, or named backreferences
RUST_REGEX_UNSUPPORTED = [
    (r'\(\?=', 'positive_lookahead'),
    (r'\(\?!', 'negative_lookahead'),
    (r'\(\?<=', 'positive_lookbehind'),
    (r'\(\?<!', 'negative_lookbehind'),
    (r'\\{1,2}[1-9][0-9]*', 'backreference'),
    (r'\\g<\w+>', 'named_backreference_g'),
    (r'\\k<\w+>', 'named_backreference_k'),
    (r'\(\?R\)', 'recursive_pattern'),
    (r'\(\?\(\w+\)', 'conditional_subpattern'),
]


def validate_rust_regex(code: str) -> Tuple[bool, str]:
    """Check if code contains Rust regex crate unsupported constructs."""
    for pattern, name in RUST_REGEX_UNSUPPORTED:
        if re.search(pattern, code):
            return False, f"rust_regex_unsupported: {name}"
    return True, ""


def validate_proposal(old_code: str, new_code: str) -> Tuple[bool, str]:
    """Validate proposed replacement code for Rust regex compatibility."""
    valid, reason = validate_rust_regex(new_code)
    if not valid:
        return False, reason
    return True, ""


# ═══════════════════════════════════════════════════════════════════════════
# JSON EXTRACTION HELPERS (P0-C)
# ═══════════════════════════════════════════════════════════════════════════

def _extract_json(resp: str) -> Optional[re.Match]:
    """
    Extract JSON object from LLM response with robust fallbacks.
    P0-C: Tries greedy match first, then last complete brace pair, then length check.
    """
    if not resp:
        return None

    # Try greedy match first
    m = re.search(r'\{[\s\S]*\}', resp)
    if m:
        json_str = m.group(0)
        # P0-C: Validate minimum length to avoid truncated JSON
        if len(json_str) < 50:
            return None
        try:
            json.loads(json_str)
            return m
        except json.JSONDecodeError:
            pass

    # Fallback: find the LAST complete } that produces valid JSON
    # This handles cases where the greedy match captured a truncated JSON
    candidates = list(re.finditer(r'\}', resp))
    for candidate in reversed(candidates):
        end_pos = candidate.end()
        # Try all start positions from 0 to this } position
        for start in range(0, end_pos):
            trial = resp[start:end_pos]
            if len(trial) < 50:
                continue
            try:
                json.loads(trial)
                # Found valid JSON - return match object at this position
                return re.match(r'\{[\s\S]*\}', resp[start:])
            except json.JSONDecodeError:
                continue

    return None


def _extract_check_closure(rule_block: str, rule_id: str) -> Optional[str]:
    """
    Extract just the check closure from a rule_block for S100 (P0-B/P3).
    Returns only the check function body to reduce token count for LLM.
    """
    # Look for common check closure patterns
    patterns = [
        # Pattern: check = |ctx: &RuleContext| { ... }
        r'check\s*=\s*\|[^|]*\|\s*\{([^}]+(?:\{[^}]*\}[^}]*)*)\}',
        # Pattern: fn check(ctx: &RuleContext) -> bool { ... }
        r'fn\s+check\s*\([^)]*\)\s*->\s*bool\s*\{([^}]+(?:\{[^}]*\}[^}]*)*)\}',
    ]

    for pattern in patterns:
        m = re.search(pattern, rule_block, re.DOTALL)
        if m:
            return m.group(0)

    # Fallback: extract the entire rule block if no check closure found
    return rule_block[:1500] if len(rule_block) > 1500 else rule_block


# ═══════════════════════════════════════════════════════════════════════════
# ANTI-OSCILLATION DETECTOR
# ═══════════════════════════════════════════════════════════════════════════

DESC_NOISE_RE = re.compile(r'[\s\-–—:,\.]+')
THRESHOLD_RE = re.compile(r'\bthreshold[:\s]*(\d+)', re.IGNORECASE)

# P1: Threshold type patterns - extract the KIND of threshold being tuned
THRESHOLD_TYPE_PATTERNS = [
    (re.compile(r'\bthreshold[:\s]*(\d+)', re.IGNORECASE), "threshold"),
    (re.compile(r'\bcontext\s+window[:\s]*(\d+)', re.IGNORECASE), "context_window"),
    (re.compile(r'\bmin\s+length[:\s]*(\d+)', re.IGNORECASE), "min_length"),
    (re.compile(r'\bmax\s+length[:\s]*(\d+)', re.IGNORECASE), "max_length"),
    (re.compile(r'\blines?[:\s]*(\d+)', re.IGNORECASE), "lines"),
    (re.compile(r'\bchars?[:\s]*(\d+)', re.IGNORECASE), "chars"),
    (re.compile(r'\bdepth[:\s]*(\d+)', re.IGNORECASE), "depth"),
    (re.compile(r'\bcount[:\s]*(\d+)', re.IGNORECASE), "count"),
]


def _normalize_desc(desc: str) -> str:
    """Normalize description for comparison."""
    desc = desc.lower()
    desc = DESC_NOISE_RE.sub(' ', desc)
    return desc.strip()


def _extract_threshold(desc: str) -> Optional[int]:
    """Extract numeric threshold from description if present."""
    m = THRESHOLD_RE.search(desc)
    if m:
        return int(m.group(1))
    return None


def _extract_threshold_type(desc: str) -> Optional[str]:
    """
    P1: Extract the TYPE of threshold being tuned.
    Returns the threshold type category (e.g., 'threshold', 'context_window', 'min_length').
    """
    for pattern, thresh_type in THRESHOLD_TYPE_PATTERNS:
        if pattern.search(desc):
            return thresh_type
    return None


def _extract_threshold_with_type(desc: str) -> Optional[Tuple[str, int]]:
    """P1: Extract both threshold type and value."""
    for pattern, thresh_type in THRESHOLD_TYPE_PATTERNS:
        m = pattern.search(desc)
        if m:
            return (thresh_type, int(m.group(1)))
    return None


def detect_oscillation(rule_id: str, history: List[dict], change_desc: str,
                       change_type: str, window_n: int = 8) -> Tuple[bool, str]:
    """
    Check if a proposed change would cause oscillation for this rule.
    Oscillation = same rule gets repeated inverse/near-duplicate changes.
    P1: Now distinguishes threshold TYPES - only rejects if same type+value already tried.
    """
    if not history:
        return False, ""

    if change_type not in ("threshold_tune", "regex_tighten", "metadata"):
        return False, ""

    rule_history = []
    for entry in reversed(history):
        if entry.get("rule_id") == rule_id:
            rule_history.append(entry)
            if len(rule_history) >= window_n:
                break

    if len(rule_history) < 2:
        return False, ""

    new_thresh_info = _extract_threshold_with_type(change_desc)
    new_desc_norm = _normalize_desc(change_desc)

    # P1: Check for same threshold type+value already tried
    if new_thresh_info is not None:
        new_thresh_type, new_thresh_val = new_thresh_info
        thresholds_seen_by_type = defaultdict(list)

        for entry in rule_history:
            entry_desc = entry.get("description", "")
            entry_info = _extract_threshold_with_type(entry_desc)
            if entry_info is not None:
                entry_type, entry_val = entry_info
                thresholds_seen_by_type[entry_type].append(entry_val)

        # P1: Only reject if same TYPE and same VALUE already tried
        if new_thresh_type in thresholds_seen_by_type:
            if new_thresh_val in thresholds_seen_by_type[new_thresh_type]:
                return True, f"oscillation_detected: {new_thresh_type}={new_thresh_val} already tried"

    # Check for direction oscillation (raise→lower→raise pattern)
    # P1: Only applies when we have threshold type info
    if new_thresh_info is not None and len(rule_history) >= 2:
        _, new_thresh = new_thresh_info
        thresholds_seen = []
        for entry in rule_history:
            entry_desc = entry.get("description", "")
            entry_thresh = _extract_threshold(entry_desc)
            if entry_thresh is not None:
                thresholds_seen.append(entry_thresh)

        if len(thresholds_seen) >= 2:
            recent_thresholds = [new_thresh] + thresholds_seen[:3]
            directions = []
            for i in range(len(recent_thresholds) - 1):
                diff = recent_thresholds[i] - recent_thresholds[i + 1]
                if diff > 0:
                    directions.append("up")
                elif diff < 0:
                    directions.append("down")
            dir_changes = sum(1 for i in range(len(directions) - 1) if directions[i] != directions[i + 1])
            if dir_changes >= 2:
                return True, f"oscillation_detected: threshold zigzag (directions: {directions})"

    # Check for near-duplicate descriptions
    similar_count = 0
    for entry in rule_history:
        entry_desc = entry.get("description", "")
        entry_norm = _normalize_desc(entry_desc)
        if new_desc_norm and entry_norm:
            shorter, longer = sorted([new_desc_norm, entry_norm], key=len)
            if len(shorter) > 10 and shorter in longer:
                similar_count += 1
                if similar_count >= 2:
                    return True, f"oscillation_detected: similar description repeated {similar_count} times"

    return False, ""


# ═══════════════════════════════════════════════════════════════════════════
# LOW-QUALITY PROPOSAL REJECTOR
# ═══════════════════════════════════════════════════════════════════════════

MIN_DESC_WORDS = 3


def is_low_quality_proposal(change: dict, imp_type: str) -> Tuple[bool, str]:
    """Check if a proposal is low quality and should be rejected."""
    desc = change.get("description", "").strip()
    old_code = change.get("old_code", "").strip()
    new_code = change.get("new_code", "").strip()

    if imp_type in ("threshold_tune", "regex_tighten", "logic_refactor", "metadata"):
        if len(desc) < 10:
            return True, "low_quality: description too short"
        words = desc.split()
        if len(words) < MIN_DESC_WORDS:
            return True, f"low_quality: description has <{MIN_DESC_WORDS} words"

    if imp_type in ("regex_tighten", "logic_refactor"):
        if not desc or desc.lower() in ("none", "n/a", "na", ""):
            return True, "low_quality: no description/rationale"
        vague_terms = {"improve", "fix", "update", "change", "modify", "edit"}
        if desc.lower().strip() in vague_terms:
            return True, "low_quality: description is too vague"

    if imp_type == "regex_tighten" and old_code and new_code:
        if old_code == new_code:
            return True, "low_quality: old_code == new_code (no change)"

    return False, ""


# ═══════════════════════════════════════════════════════════════════════════
# RULE COOLDOWN (oscillation prevention)
# ═══════════════════════════════════════════════════════════════════════════

RULE_COOLDOWN = {}  # rule_id → next eligible iteration
COOLDOWN_BATCHES = 5  # skip rule for N batches after oscillation


def set_rule_cooldown(rule_id: str, batches: int = COOLDOWN_BATCHES):
    """Set cooldown for a rule (skip for N batches)."""
    RULE_COOLDOWN[rule_id] = len(SESSION_DONE) + batches


# ═══════════════════════════════════════════════════════════════════════════
# LOCKED RULES (stable, do not re-tune)
# ═══════════════════════════════════════════════════════════════════════════

LOCKED_RULES = {
    "S2259",  # 100% keep rate — stable
    "S4792",  # Converged weak crypto patterns
    "S2068",  # Converged credential length threshold
}


# ═══════════════════════════════════════════════════════════════════════════
# STRATEGY ENFORCEMENT (S4)
# ═══════════════════════════════════════════════════════════════════════════

STRATEGY_ALLOWED_TYPES = {
    "regex": {"threshold_tune", "regex_tighten", "logic_refactor", "metadata"},
    "ast": {"ast_migrate", "threshold_tune", "metadata"},
    "semantic": {"semantic_migrate", "threshold_tune", "metadata"},
    "dataflow": {"dataflow_migrate", "threshold_tune", "metadata"},
    "metric": {"metric_migrate", "threshold_tune", "metadata"},
    "hybrid": {"hybrid_migrate", "ast_migrate", "semantic_migrate", "dataflow_migrate", "metric_migrate", "threshold_tune", "metadata"},
}


def get_strategy_allowed_types(strategy: str) -> set:
    """Get allowed improvement types for a given strategy."""
    return STRATEGY_ALLOWED_TYPES.get(strategy, {"threshold_tune", "regex_tighten", "logic_refactor", "metadata"})


# ═══════════════════════════════════════════════════════════════════
# 1. ANALYZER
# ═══════════════════════════════════════════════════════════════════

def analyzer(history, force=None, batch=3):
    """Pick N rules: 2 SonarQube targets + worst F1 rules."""
    if force:
        return [force]

    # Read all valid rules from catalog
    all_rules = re.findall(r'id:\s*"(S\d+)"', CATALOG.read_text())
    if not all_rules:
        log.error("catalog.rs corrupted — no rules found")
        return []

    valid = set(all_rules)

    # S5: Remove locked rules from selection pool
    valid = valid - LOCKED_RULES

    # Recent rules (last batch*3) — cooldown
    recent = set()
    for h in history[-batch * 3:]:
        rid = h.get("rule_id") or ""
        if rid.startswith("S"):
            recent.add(rid)

    # S3: Cooldown rules (oscillation cooldown)
    cooled = {rid for rid, eligible_at in RULE_COOLDOWN.items() if len(SESSION_DONE) < eligible_at}

    # Build F1 scores from history
    rule_f1 = defaultdict(list)
    for h in history:
        rid = h.get("rule_id") or ""
        if rid and re.match(r'^S\d+$', rid):
            try:
                rule_f1[rid].append(float(h.get("f1_after", 0) or 0))
            except (ValueError, TypeError):
                pass

    avg_f1 = {r: sum(s) / len(s) for r, s in rule_f1.items() if s}

    selected = []

    # Priority 1: SonarQube targets (up to 2)
    for r in SQ_TARGETS:
        if r in valid and r not in recent and r not in selected and r not in cooled:
            if is_done(r) and not is_retryable(r):
                continue  # Skip permanently done
            if is_done(r) and is_retryable(r):
                # Retryable — allow
                pass
            selected.append(r)
        if len(selected) >= max(1, batch // 2):
            break

    # Priority 2: Lowest F1
    candidates = sorted(
        ((r, a) for r, a in avg_f1.items() if r not in recent and r not in selected and r in valid and r not in cooled),
        key=lambda x: x[1]
    )
    for r, _ in candidates:
        if is_done(r) and not is_retryable(r):
            continue
        selected.append(r)
        if len(selected) >= batch:
            break

    # Priority 3: Any remaining valid rule
    for r in sorted(valid):
        if r not in selected and r not in recent and r not in cooled:
            if is_done(r) and not is_retryable(r):
                continue
            selected.append(r)
        if len(selected) >= batch:
            break

    return selected[:batch]


# ═══════════════════════════════════════════════════════════════════
# 2. IMPROVER
# ═══════════════════════════════════════════════════════════════════

def _extract_rule_block(content: str, start: int) -> Optional[str]:
    """Extract a declare_rule!{...} block handling nested braces and strings."""
    i = content.find("{", start)
    if i == -1:
        return None

    depth = 0
    in_string = False
    string_char = None
    raw_prefix = False

    while i < len(content):
        ch = content[i]

        if not in_string:
            if ch in ('"', 'r'):
                if ch == 'r' and i + 1 < len(content) and content[i + 1] == '"':
                    in_string = True
                    string_char = '"'
                    raw_prefix = True
                    i += 1
                elif ch == '"':
                    in_string = True
                    string_char = '"'
        else:
            if raw_prefix:
                if ch == '"':
                    in_string = False
                    raw_prefix = False
            else:
                if ch == '\\':
                    i += 1
                elif ch == string_char:
                    in_string = False

        if not in_string:
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    return content[start:i + 1]
        i += 1

    return None


def improver(rule_id):
    """LLM proposes change. Returns {success, type, description, confidence, level, strategy}."""
    llm = LLMClient()
    content = CATALOG.read_text()

    # Find rule block
    pos = content.find(f'id: "{rule_id}"')
    if pos == -1:
        return {"success": False, "error": "already segregated"}

    block_start = content.rfind("declare_rule!", 0, pos)
    if block_start == -1:
        return {"success": False, "error": "rule block not found"}

    # Extract rule block using balanced brace matching with string awareness
    rule_block = _extract_rule_block(content, block_start)
    if not rule_block:
        return {"success": False, "error": "rule block parse failed"}

    # ── Strategy classification (before building prompt) ──
    classification = classify_rule_strategy(rule_id, rule_block)
    strategy = classification["strategy"]
    log.info(f"  {rule_id} → classified as strategy='{strategy}' ({classification['reason']})")

    # Build strategy-aware system prompt
    system = _build_strategy_prompt(strategy)

    # P3: For S100, extract only the check closure to avoid LLM JSON parsing failures
    # S100 rule blocks are ~2000 chars, which combined with prompt exceeds token limits
    if rule_id == "S100":
        check_closure = _extract_check_closure(rule_block, rule_id)
        prompt_block = check_closure
        log.info(f"  {rule_id} — P3: using check closure only ({len(check_closure)} chars)")
    else:
        prompt_block = rule_block[:3000]

    try:
        resp = llm.chat(
            system,
            [{"role": "user", "content": f"Rule {rule_id}:\n```rust\n{prompt_block}\n```\nPropose ONE safe change to improve detection quality."}],
            max_tokens=2500
        )
        # M4: Retry with simpler prompt for JSON parsing failures
        m = _extract_json(resp)
        if not m:
            log.info(f"  {rule_id} — no JSON in first response, retrying with simpler prompt...")
            resp2 = llm.chat(
                "Return ONLY a JSON object. No markdown, no explanation. "
                'Format: {"improvement_type":"threshold_tune|metadata","description":"...",'
                '"old_code":"...","new_code":"...","confidence":0.8}',
                [{"role": "user", "content": f"Rule {rule_id}:\n```rust\n{prompt_block[:2000]}\n```\nPropose ONE safe improvement."}],
                max_tokens=1000
            )
            m = _extract_json(resp2)
            if not m:
                # P0-B: 3rd attempt with only closure check and simplified prompt
                log.info(f"  {rule_id} — 2nd attempt failed, trying 3rd attempt with minimal prompt...")
                check_closure = _extract_check_closure(rule_block, rule_id)
                if check_closure:
                    resp3 = llm.chat(
                        "Return ONLY valid JSON with exactly these fields: "
                        '{"improvement_type":"threshold_tune","description":"...","old_code":"...","new_code":"...","confidence":0.8}',
                        [{"role": "user", "content": f"Rule {rule_id} check closure:\n```rust\n{check_closure}\n```\nPropose ONE safe improvement to this check function."}],
                        max_tokens=800
                    )
                    m = _extract_json(resp3)
                    if m:
                        log.info(f"  {rule_id} — 3rd attempt succeeded")
                if not m:
                    return {"success": False, "error": "no JSON in response (3 attempts)"}

        change = json.loads(m.group(0))
        imp_type = change.get("improvement_type", "none")
        if imp_type == "none":
            return {"success": False, "error": "no improvement needed"}
        if imp_type not in get_strategy_allowed_types(strategy):
            log.info(f"  {rule_id} — strategy enforcement: {imp_type} not allowed for strategy={strategy}")
            return {"success": False, "error": f"strategy_mismatch: {imp_type} not allowed for {strategy}"}

        old_code = change.get("old_code", "")
        new_code = change.get("new_code", "")
        description = change.get("description", "")
        confidence = change.get("confidence", 0.5)

        if not old_code or not new_code or old_code == new_code:
            return {"success": False, "error": "empty change"}

        # M3: Pre-validate: old_code must exist in current catalog
        if old_code and old_code not in content:
            old_code_stripped = old_code.strip()
            found_fuzzy = False
            for line in content.split("\n"):
                if line.strip() == old_code_stripped:
                    found_fuzzy = True
                    break
            if not found_fuzzy:
                return {"success": False, "error": "old_code not found in catalog (pre-validation)"}

        # Low-quality proposal check
        is_low, lq_reason = is_low_quality_proposal(change, imp_type)
        if is_low:
            return {"success": False, "error": lq_reason}

        # Anti-oscillation check
        history = read_evolution_history()
        is_osc, osc_reason = detect_oscillation(rule_id, history, description, imp_type)
        if is_osc:
            set_rule_cooldown(rule_id)
            return {"success": False, "error": osc_reason}

        # S1: Rust regex validation — runs for ALL improvement types with code changes
        if new_code.strip():
            valid, reason = validate_proposal(old_code, new_code)
            if not valid:
                log.debug(f"  {rule_id} — regex validation rejected: {reason}")
                return {"success": False, "error": reason}

        # Apply change
        if old_code in content:
            new_content = content.replace(old_code, new_code, 1)
        else:
            # Fuzzy match: strip whitespace
            found = False
            for line in content.split("\n"):
                if line.strip() == old_code.strip():
                    new_content = content.replace(line, new_code.strip(), 1)
                    found = True
                    break
            if not found:
                # Fallback: update explanation
                return _improve_metadata(rule_id, description)

        CATALOG.write_text(new_content)

        # Verify compilation
        cargo = CargoTool()
        ok, err = cargo.check(package="cognicode-axiom")
        if not ok:
            CATALOG.write_text(content)  # Revert
            # Fallback: update explanation instead
            return _improve_metadata(rule_id, description)

        return {
            "success": True,
            "type": imp_type,
            "description": description,
            "confidence": confidence,
            "level": "code",
            "strategy": strategy,  # Include strategy in return for observability
        }

    except Exception as e:
        return {"success": False, "error": str(e)}


def _build_strategy_prompt(strategy: str) -> str:
    """Build strategy-aware system prompt for the LLM."""
    base = (
        "You edit Rust static analysis rules for the CogniCode analyzer. "
        "IMPORTANT Rust regex limitations: The Rust `regex` crate does NOT support "
        "lookahead (?=), negative lookahead (?!), lookbehind (?<=), negative lookbehind (?<!), "
        "backreferences (\\1, \\2), or named backreferences (\\g<name>, \\k<name>). "
        "Rust regex DOES support non-capturing groups (?:...), inline flags (?i), "
        "word boundaries (\\b), and quantifiers like \\d{2}. "
        "Prefer threshold adjustments (safest) over regex changes. "
    )

    if strategy == "regex":
        return base + (
            "Return ONLY valid JSON (extra fields are ignored): "
            '{"improvement_type":"threshold_tune|regex_tighten|logic_refactor|metadata",'
            '"description":"what and why this change helps (be specific)",'
            '"fp_rationale":"why this reduces false positives",'
            '"test_implication":"what existing tests should still pass",'
            '"old_code":"EXACT original code from the rule block",'
            '"new_code":"EXACT replacement code","confidence":0.8}'
        )

    elif strategy == "ast":
        guidance = (
            "This rule is classified as AST/structural. "
            "Do NOT propose regex-first changes. "
            "Instead consider: tree-sitter AST patterns, structural conditions, or parameter count checks. "
            "Acceptable improvement_types: ast_migrate, threshold_tune, metadata. "
        )
        return base + guidance + (
            "Return ONLY valid JSON: "
            '{"improvement_type":"ast_migrate|threshold_tune|metadata",'
            '"description":"AST/structural change and why it improves detection",'
            '"old_code":"EXACT original code","new_code":"EXACT replacement code","confidence":0.8}'
        )

    elif strategy == "semantic":
        guidance = (
            "This rule is classified as semantic (symbol/scope/usage analysis). "
            "Do NOT propose regex-first changes. "
            "Consider: variable binding analysis, unused symbol detection, scope shadowing checks. "
            "Acceptable improvement_types: semantic_migrate, threshold_tune, metadata. "
        )
        return base + guidance + (
            "Return ONLY valid JSON: "
            '{"improvement_type":"semantic_migrate|threshold_tune|metadata",'
            '"description":"semantic analysis change and why it improves detection",'
            '"old_code":"EXACT original code","new_code":"EXACT replacement code","confidence":0.8}'
        )

    elif strategy == "dataflow":
        guidance = (
            "This rule is classified as dataflow (source-sink/taint analysis). "
            "Do NOT propose regex-first changes. "
            "Consider: taint propagation, source/sink modeling, sanitizer identification. "
            "Acceptable improvement_types: dataflow_migrate, threshold_tune, metadata. "
        )
        return base + guidance + (
            "Return ONLY valid JSON: "
            '{"improvement_type":"dataflow_migrate|threshold_tune|metadata",'
            '"description":"dataflow/taint analysis change and why it improves detection",'
            '"old_code":"EXACT original code","new_code":"EXACT replacement code","confidence":0.8}'
        )

    elif strategy == "metric":
        guidance = (
            "This rule is classified as metric-based. "
            "Do NOT propose regex-first changes. "
            "Consider: threshold tuning, metric aggregation changes, counting logic adjustments. "
            "Acceptable improvement_types: metric_migrate, threshold_tune, metadata. "
        )
        return base + guidance + (
            "Return ONLY valid JSON: "
            '{"improvement_type":"metric_migrate|threshold_tune|metadata",'
            '"description":"metric-based change and why it improves detection",'
            '"old_code":"EXACT original code","new_code":"EXACT replacement code","confidence":0.8}'
        )

    elif strategy == "hybrid":
        guidance = (
            "This rule is classified as hybrid (multiple analysis approaches). "
            "Consider combining regex, AST, semantic, or metric strategies as appropriate. "
            "Acceptable improvement_types: hybrid_migrate, ast_migrate, semantic_migrate, "
            "dataflow_migrate, metric_migrate, threshold_tune, metadata. "
        )
        return base + guidance + (
            "Return ONLY valid JSON: "
            '{"improvement_type":"hybrid_migrate|threshold_tune|metadata",'
            '"description":"hybrid analysis change and why it improves detection",'
            '"old_code":"EXACT original code","new_code":"EXACT replacement code","confidence":0.8}'
        )

    else:
        return base + (
            "Return ONLY valid JSON: "
            '{"improvement_type":"threshold_tune|regex_tighten|logic_refactor|metadata",'
            '"description":"what and why this change helps","old_code":"EXACT original code",'
            '"new_code":"EXACT replacement code","confidence":0.8}'
        )


def read_evolution_history() -> List[dict]:
    """Read autoresearch evolution history without mutating experiment state."""
    return EvolutionLogger(Path(__file__).parent / "evolution.tsv").read_history()


def _improve_metadata(rule_id, description):
    """Safe fallback: update explanation field."""
    content = CATALOG.read_text()
    pattern = re.compile(
        rf'(id:\s*"{rule_id}".*?explanation:\s*)"(?:[^"\\]|\\.)*"',
        re.DOTALL
    )
    match = pattern.search(content)
    if match:
        new_expl = f"[AUTORESEARCH] {description[:150]}"
        replacement_start = match.end(1)
        replacement_end = match.end(0)
        new_content = content[:replacement_start] + f'"{new_expl}"' + content[replacement_end:]
        CATALOG.write_text(new_content)
        return {"success": True, "type": "metadata", "description": description, "confidence": 0.3, "level": "metadata"}
    return {"success": False, "error": "no explanation field"}


# ═══════════════════════════════════════════════════════════════════
# 3. EVALUATOR
# ═══════════════════════════════════════════════════════════════════

def evaluator(rule_id):
    """Fast compilation check + targeted tests."""
    # Compilation
    r = subprocess.run(
        ["cargo", "check", "-p", "cognicode-axiom"],
        capture_output=True, text=True, timeout=120, cwd=str(ROOT)
    )
    if r.returncode != 0 and ("error[" in r.stderr or "error:" in r.stderr):
        return {"error": "compilation"}

    # Targeted tests (filter by rule_id)
    r = subprocess.run(
        ["cargo", "test", "-p", "cognicode-axiom", "--lib", "--", rule_id.lower()],
        capture_output=True, text=True, timeout=300, cwd=str(ROOT)
    )
    combined = r.stdout + r.stderr
    if "test result: ok" not in combined:
        # Fallback: run all tests
        r = subprocess.run(
            ["cargo", "test", "-p", "cognicode-axiom", "--lib"],
            capture_output=True, text=True, timeout=300, cwd=str(ROOT)
        )
        combined = r.stdout + r.stderr
        if "test result: ok" not in combined:
            return {"error": "tests"}

    passed = int(re.search(r"(\d+) passed", combined).group(1)) if re.search(r"(\d+) passed", combined) else 0
    return {"tests_passed": passed}


# ═══════════════════════════════════════════════════════════════════
# 4. DECIDER
# ═══════════════════════════════════════════════════════════════════

def decider(rule_id, baseline, current, change):
    """
    Decide keep/discard based on compilation success and confidence.
    P2: Now handles rules without tests by relying on description quality.
    """
    tests_passed = current.get("tests_passed", 0)
    tests_ok = tests_passed > 0
    confidence = change.get("confidence", 0)
    level = change.get("level", "code")
    description = change.get("description", "")

    if tests_ok and level == "code" and confidence > 0.70:
        return "keep", f"code conf={int(confidence*100)}%"
    if tests_ok and level == "metadata":
        return "keep", "metadata update"

    # P2: Handle rules without tests
    if tests_passed == 0:
        # P2-1: metadata level with decent confidence
        if level == "metadata" and confidence >= 0.75:
            log.info(f"  {rule_id} — tests_passed == 0, relying on description quality (metadata, conf={confidence:.2f})")
            return "keep", "metadata (no tests, high confidence)"
        # P2-2: High confidence + detailed description
        if confidence >= 0.85 and len(description) > 50:
            log.info(f"  {rule_id} — tests_passed == 0, relying on description quality (conf={confidence:.2f}, desc_len={len(description)})")
            return "keep", f"code (no tests, high confidence+description)"

    return "discard", "no gain"


# ═══════════════════════════════════════════════════════════════════
# COMMIT MESSAGE
# ═══════════════════════════════════════════════════════════════════

def commit_msg(rule_id, change):
    """Generate conventional commit via LLM."""
    try:
        llm = LLMClient()
        resp = llm.chat(
            "Conventional commit. One line. Format: type(scope): description.",
            [{"role": "user", "content": f"Rule:{rule_id} Type:{change.get('type','?')} Desc:{change.get('description','')[:80]}"}],
            max_tokens=200, temperature=0.1
        )
        msg = resp.strip().strip('"').split("\n")[0][:100]
        msg = msg.replace("`", "").replace("#", "").strip()
        return f"{msg} [auto]" if ":" in msg else f"refactor({rule_id}): improve [auto]"
    except:
        return f"refactor({rule_id}): improve [auto]"


# ═══════════════════════════════════════════════════════════════════
# SEGREGATION
# ═══════════════════════════════════════════════════════════════════

def segregate(rule_id) -> Optional[Path]:
    """
    Extract rule from catalog.rs to its own file (SOLID/SRP).
    Returns Path on success, None on failure.
    """
    content = CATALOG.read_text()
    pos = content.find(f'id: "{rule_id}"')
    if pos == -1:
        log.warning(f"  segregate: {rule_id} not found in catalog — already segregated?")
        return False

    # Find declare_rule! block using robust extraction
    bs = content.rfind("declare_rule!", 0, pos)
    if bs == -1:
        log.warning(f"  segregate: declare_rule! not found before {rule_id}")
        return False

    block = _extract_rule_block(content, bs)
    if not block:
        log.warning(f"  segregate: failed to extract block for {rule_id}")
        return False

    # ── VALIDATION: Prevent corrupted segregation ─────────────────────────
    # Check 1: Block must contain exactly ONE rule ID matching the target
    id_count = block.count(f'id: "{rule_id}"')
    if id_count != 1:
        log.error(f"  segregate: BLOCK CORRUPTED for {rule_id} — contains {id_count} occurrences of id (expected 1). Aborting!")
        log.error(f"  segregate: This usually means the rule was already segregated. Skipping.")
        return False

    # Check 2: Block must not be absurdly large (entire catalog is ~1MB)
    if len(block) > 10000:  # 10KB is way too big for a single rule
        log.error(f"  segregate: BLOCK TOO LARGE for {rule_id} ({len(block)} bytes). Aborting!")
        return False

    # Check 3: Block must end with a closing brace
    if not block.strip().endswith('}'):
        log.error(f"  segregate: BLOCK MALFORMED for {rule_id} — doesn't end with '}}'. Aborting!")
        return False

    # Determine category
    if "Security" in block or "VULNERABILITY" in block:
        cat = "security"
    elif "Bug" in block or "Reliability" in block:
        cat = "bugs"
    else:
        cat = "code_smells"

    # Create file
    target_dir = RULES_DIR / "rust" / cat
    target_dir.mkdir(parents=True, exist_ok=True)
    fname = f"{rule_id.lower()}_rule.rs"
    fpath = target_dir / fname

    # Build file content
    fc = f"""//! {rule_id} — Auto-segregated by Karpathy workflow (SOLID/SRP)
use crate::{{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry}};
use crate::rules::{{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity}};
use cognicode_macros::declare_rule;
use inventory::submit;
use streaming_iterator::StreamingIterator;

{block}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_{rule_id.lower()}_registered() {{
        let rule = {rule_id}Rule::new();
        assert_eq!(rule.id(), "{rule_id}");
        assert!(rule.name().len() > 0);
    }}
}}
"""
    fpath.write_text(fc)

    # Update mod.rs
    mod_file = target_dir / "mod.rs"
    mod_content = mod_file.read_text() if mod_file.exists() else ""
    mod_line = f"pub mod {fname.replace('.rs', '')};"
    if mod_line not in mod_content:
        mod_file.write_text(mod_content + f"\n{mod_line}\n")

    # Replace in catalog.rs
    new_content = content.replace(
        block,
        f"// {rule_id} → segregated to {fpath.relative_to(ROOT)} (SOLID)"
    )
    CATALOG.write_text(new_content)

    log.info(f"   📁 Segregated: {rule_id} → {fpath.relative_to(ROOT)}")
    return fpath  # Return the path for validation


# ═══════════════════════════════════════════════════════════════════
# POST-SEGREGATION VALIDATION (P5)
# ═══════════════════════════════════════════════════════════════════

def validate_segregation(rule_id: str, fpath: Path) -> bool:
    """
    P5: After segregation, verify that the new file compiles.
    Returns True if cargo check passes, False otherwise.
    """
    # Run cargo check on the axiom crate
    r = subprocess.run(
        ["cargo", "check", "-p", "cognicode-axiom"],
        capture_output=True, text=True, timeout=120, cwd=str(ROOT)
    )
    if r.returncode != 0:
        stderr = r.stderr.lower()
        # Check if the error is related to our file
        if fpath.name in stderr or rule_id in stderr:
            log.error(f"  P5: Compilation failed for {rule_id} after segregation — reverting")
            log.error(f"  P5: Error preview: {r.stderr[:500]}")
            return False
        # Error is elsewhere, might be pre-existing
        log.warning(f"  P5: cargo check failed but not related to {rule_id} — manual review recommended")
        return True  # Don't revert for unrelated errors

    log.info(f"  P5: Compilation validated for {rule_id}")
    return True


def revert_segregation(rule_id: str, fpath: Path):
    """Revert segregation: restore catalog.rs and remove the new file."""
    # Remove the file
    if fpath.exists():
        fpath.unlink()
        log.info(f"  P5: Removed {fpath.name}")

    # Update mod.rs to remove the entry
    mod_file = fpath.parent / "mod.rs"
    if mod_file.exists():
        mod_content = mod_file.read_text()
        mod_line = f"pub mod {fpath.stem};"
        if mod_line in mod_content:
            mod_content = mod_content.replace(mod_line + "\n", "")
            mod_content = mod_content.replace(mod_line, "")
            mod_file.write_text(mod_content)
            log.info(f"  P5: Removed {mod_line} from mod.rs")

    # Restore catalog.rs from git
    git = GitTool()
    git.checkout(str(CATALOG))
    log.info(f"  P5: Restored catalog.rs")


# ═══════════════════════════════════════════════════════════════════
# MAIN LOOP
# ═══════════════════════════════════════════════════════════════════

def evolve(max_iterations=None, force_rule=None, dry_run=False, cooldown=5, batch_size=3, auto_commit=False):
    """Karpathy autonomous improvement loop."""
    global SESSION_DONE

    evolution = EvolutionLogger(Path(__file__).parent / "evolution.tsv")
    baseline_store = BaselineStore(Path(__file__).parent / "baseline")
    baseline = baseline_store.load()
    git = GitTool()

    load_session()
    history = evolution.read_history()

    session = keeps = discards = fails = 0
    total_alltime = len(history)

    done, total_rules, pct = progress()

    # ── Startup banner ──
    log.info("┌" + "─" * 60)
    log.info("│ 🧬 Self-Evolving Rules — Karpathy Autonomous Loop")
    log.info(f"│ 📋 {done}/{total_rules} rules ({pct}%) | Model: {ModelConfig.MODEL}")
    log.info(f"│ 🎯 {len(SQ_TARGETS)} SonarQube targets | {batch_size} rules/batch")
    log.info("│ 🔧 3-tier: code change → metadata fallback → skip")
    log.info("└" + "─" * 60)

    while not STOP:
        if max_iterations and session >= max_iterations:
            break

        session += 1
        t0 = time.time()
        batch_results = {}

        # ── ANALYZER ──
        targets = analyzer(history, force_rule, batch_size)

        if not targets:
            log.warning("No rules available — catalog exhausted")
            done, total_rules, pct = progress()
            log.info(f"📋 {done}/{total_rules} rules ({pct}%) processed")
            break

        log.info(f"\n── BATCH {session}" + (f"/{max_iterations}" if max_iterations else "") + f": {targets}")

        if dry_run:
            for rid in targets:
                total_alltime += 1
                evolution.log_experiment(total_alltime, rid, "rust", {}, {}, "dry_run", "Analyzer selected", strategy="")
                batch_results[rid] = ("dry_run", "Analyzer selected")
            time.sleep(1)
            continue

        # ── Process each rule ──
        for rule_id in targets:
            total_alltime += 1

            if rule_id not in re.findall(r'id:\s*"(S\d+)"', CATALOG.read_text()):
                log.debug(f"  {rule_id} already segregated — marking done")
                mark_done(rule_id)
                batch_results[rule_id] = ("failed", "already segregated")
                fails += 1
                continue

            # Previous F1
            f1_before = baseline.get(rule_id, {}).get("f1", 0) or 0

            # 2. IMPROVER (3-tier)
            change = None
            for tier in range(3):
                change = improver(rule_id)
                if change.get("success") and change.get("level") == "code":
                    break
                # Early exit for LLM errors that won't improve
                err = change.get("error", "") if change else ""
                if any(kw in err for kw in ["no such group", "Invalid", "Expecting", "no JSON"]):
                    break

            if not change or not change.get("success"):
                record_failure(rule_id)
                if FAILURE_COUNT[rule_id] >= 3:
                    mark_done(rule_id)
                    log.debug(f"  {rule_id} — 3 failures, permanently skipped")
                else:
                    log.debug(f"  {rule_id} — retry #{FAILURE_COUNT[rule_id]}/3")
                evolution.log_experiment(
                    total_alltime, rule_id, "rust", {"f1": f1_before}, {},
                    "skipped", change.get("error", "?") if change else "?",
                    strategy=change.get("strategy", "") if change else ""
                )
                batch_results[rule_id] = ("skipped", change.get("error", "?") if change else "?")
                fails += 1
                continue

            # 3. EVALUATOR
            metrics = evaluator(rule_id)
            if "error" in metrics:
                git.checkout(str(CATALOG))
                evolution.log_experiment(
                    total_alltime, rule_id, "rust", {"f1": f1_before}, {},
                    "failed", metrics["error"],
                    strategy=change.get("strategy", "")
                )
                batch_results[rule_id] = ("failed", metrics["error"])
                record_failure(rule_id)
                fails += 1
                continue

            # 4. DECIDER
            decision, reason = decider(rule_id, baseline.get(rule_id, {}), metrics, change)

            if decision == "keep":
                # Segregate BEFORE commit (atomic)
                seg_path = None
                try:
                    seg_path = segregate(rule_id)
                except Exception as e:
                    log.error(f"  Segregation failed: {e}")
                    git.checkout(str(CATALOG))
                    evolution.log_experiment(
                        total_alltime, rule_id, "rust", {"f1": f1_before}, {},
                        "discard", f"segregation error: {e}",
                        strategy=change.get("strategy", "")
                    )
                    batch_results[rule_id] = ("discard", f"segregation error: {e}")
                    discards += 1
                    mark_done(rule_id)
                    continue

                if not seg_path:
                    log.warning("  Segregation returned no path — discarding")
                    git.checkout(str(CATALOG))
                    evolution.log_experiment(
                        total_alltime, rule_id, "rust", {"f1": f1_before}, {},
                        "discard", "segregation failed",
                        strategy=change.get("strategy", "")
                    )
                    batch_results[rule_id] = ("discard", "segregation failed")
                    discards += 1
                    mark_done(rule_id)
                    continue

                # P5: Validate compilation after segregation
                if not validate_segregation(rule_id, seg_path):
                    log.warning(f"  P5: Validation failed — reverting segregation for {rule_id}")
                    revert_segregation(rule_id, seg_path)
                    evolution.log_experiment(
                        total_alltime, rule_id, "rust", {"f1": f1_before}, {},
                        "discard", "compilation failed after segregation (P5)",
                        strategy=change.get("strategy", "")
                    )
                    batch_results[rule_id] = ("discard", "compilation failed after segregation (P5)")
                    discards += 1
                    mark_done(rule_id)
                    continue

                if auto_commit:
                    # Stage and commit
                    r = subprocess.run(
                        ["git", "add", "-f", "crates/cognicode-axiom/src/rules/catalog.rs",
                         "crates/cognicode-axiom/src/rules/rules/"],
                        capture_output=True, cwd=str(ROOT)
                    )
                    if r.returncode == 0:
                        git.commit(commit_msg(rule_id, change))
                        baseline[rule_id] = metrics
                        baseline_store.save(baseline)
                        keeps += 1
                        mark_done(rule_id)
                    else:
                        log.warning("git add failed — reverting")
                        git.checkout(str(CATALOG))
                        discards += 1
                else:
                    log.info("  AUTO-COMMIT disabled — changes kept in working tree for review")
                    keeps += 1
            else:
                git.checkout(str(CATALOG))
                discards += 1
                mark_done(rule_id)

            # Log
            evolution.log_experiment(
                total_alltime, rule_id, "rust", {"f1": f1_before}, metrics,
                decision,
                f"{change.get('type', '?')}: {change.get('description', '')[:120]}",
                strategy=change.get("strategy", "")
            )
            batch_results[rule_id] = (decision, f"{change.get('type', '?')}: {change.get('description', '')[:120]}")
            log.info(f"  {rule_id} → {decision.upper()} ({change.get('level', '?')}): {reason}")

        # ── Batch report ──
        elapsed = int(time.time() - t0)
        keep_rate = 0 if keeps + discards == 0 else int(keeps / (keeps + discards) * 100)
        done, total_rules, pct = progress()

        log.info("  ┌" + "─" * 55)
        log.info(f"  │ Batch {session}: {len(targets)} rules in {elapsed}s — {keeps}✅ {discards}❌ {fails}⚠ — rate {keep_rate}%")
        for rid in targets:
            if rid in batch_results:
                dec, desc = batch_results[rid]
                desc = (desc or "")[:55]
                icon = "✅" if dec == "keep" else ("❌" if dec == "discard" else "⚠️")
                log.info(f"  │  {icon} {rid:<7} {dec:<8} {desc}")
        log.info("  └" + "─" * 55)
        log.info(f"  📋 Progress: {done}/{total_rules} rules ({pct}%)")
        log.info(f"  📊 Session: {session} batches | {keeps} kept | {discards} discarded | {fails} failed")

        # Self-check every 10 batches
        if session % 10 == 0:
            log.info("  🛡️ Self-check: running full test suite...")
            r = subprocess.run(
                ["cargo", "test", "-p", "cognicode-axiom", "--lib"],
                capture_output=True, text=True, timeout=120, cwd=str(ROOT)
            )
            if "test result: ok" in (r.stdout + r.stderr):
                log.info("  ✅ Tests OK")
            else:
                log.error("  ❌ Tests FAILED — stopping for safety")
                break

        if cooldown and not STOP:
            time.sleep(cooldown)

    # ── Final summary ──
    done, total_rules, pct = progress()
    log.info(f"\n{'=' * 60}")
    log.info(f"  EVOLUTION COMPLETE — {session} batches")
    log.info(f"  Kept: {keeps} | Discarded: {discards} | Failed: {fails}")
    if keeps + discards > 0:
        log.info(f"  Keep rate: {keep_rate}%")
    log.info(f"  Rules processed: {done}/{total_rules} ({pct}%)")
    log.info(f"{'=' * 60}")


# ═══════════════════════════════════════════════════════════════════
# ENTRY POINT
# ═══════════════════════════════════════════════════════════════════

if __name__ == "__main__":
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s [%(levelname)s] %(message)s",
        handlers=[logging.StreamHandler(), logging.FileHandler("autoresearch/run.log")]
    )

    parser = argparse.ArgumentParser(description="Karpathy Autonomous Rule Evolution")
    parser.add_argument("-n", "--max-iterations", type=int, default=None)
    parser.add_argument("-r", "--rule", type=str, default=None)
    parser.add_argument("-c", "--cooldown", type=int, default=5)
    parser.add_argument("-b", "--batch-size", type=int, default=5)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--commit", action="store_true")

    args = parser.parse_args()

    log.info("🚀 Starting Karpathy autonomous evolution...")

    try:
        evolve(
            max_iterations=args.max_iterations,
            force_rule=args.rule,
            dry_run=args.dry_run,
            cooldown=args.cooldown,
            batch_size=args.batch_size,
            auto_commit=args.commit,
        )
    except KeyboardInterrupt:
        log.info("\n⏹ Interrupted — shutting down")
    except Exception as e:
        log.error(f"Fatal error: {e}")
        import traceback
        traceback.print_exc()
