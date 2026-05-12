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
    
    # Recent rules (last batch*3) — cooldown
    recent = set()
    for h in history[-batch * 3:]:
        rid = h.get("rule_id", "")
        if rid.startswith("S"):
            recent.add(rid)

    # Build F1 scores from history
    rule_f1 = defaultdict(list)
    for h in history:
        rid = h.get("rule_id", "")
        if re.match(r'^S\d+$', rid):
            try:
                rule_f1[rid].append(float(h.get("f1_after", 0) or 0))
            except (ValueError, TypeError):
                pass

    avg_f1 = {r: sum(s) / len(s) for r, s in rule_f1.items() if s}

    selected = []

    # Priority 1: SonarQube targets (up to 2)
    for r in SQ_TARGETS:
        if r in valid and r not in recent and r not in selected:
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
        ((r, a) for r, a in avg_f1.items() if r not in recent and r not in selected and r in valid),
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
        if r not in selected and r not in recent:
            if is_done(r) and not is_retryable(r):
                continue
            selected.append(r)
        if len(selected) >= batch:
            break

    return selected[:batch]


# ═══════════════════════════════════════════════════════════════════
# 2. IMPROVER
# ═══════════════════════════════════════════════════════════════════

def improver(rule_id):
    """LLM proposes change. Returns {success, type, description, confidence, level}."""
    llm = LLMClient()
    content = CATALOG.read_text()

    # Find rule block
    pos = content.find(f'id: "{rule_id}"')
    if pos == -1:
        return {"success": False, "error": "already segregated"}

    block_start = content.rfind("declare_rule!", 0, pos)
    brace_start = content.find("{", block_start)
    depth = 0
    for i in range(brace_start, len(content)):
        if content[i] == "{":
            depth += 1
        elif content[i] == "}":
            depth -= 1
            if depth == 0:
                rule_block = content[block_start:i + 1]
                break

    # Ask LLM
    system = (
        "You edit Rust static analysis rules. Propose ONE safe change to the "
        "detection logic. Prefer threshold adjustments (safest) over regex changes. "
        "Return ONLY valid JSON: "
        '{"improvement_type":"threshold_tune|regex_tighten|logic_refactor|metadata",'
        '"description":"what and why","old_code":"EXACT original code",'
        '"new_code":"EXACT replacement code","confidence":0.8}'
    )

    try:
        resp = llm.chat(
            system,
            [{"role": "user", "content": f"Rule {rule_id}:\n```rust\n{rule_block[:3000]}\n```\nPropose ONE safe change."}],
            max_tokens=1500
        )
        m = re.search(r'\{[\s\S]*\}', resp)
        if not m:
            return {"success": False, "error": "no JSON in response"}

        change = json.loads(m.group(0))
        imp_type = change.get("improvement_type", "none")
        if imp_type == "none":
            return {"success": False, "error": "no improvement needed"}

        old_code = change.get("old_code", "")
        new_code = change.get("new_code", "")
        description = change.get("description", "")
        confidence = change.get("confidence", 0.5)

        if not old_code or not new_code or old_code == new_code:
            return {"success": False, "error": "empty change"}

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
        }

    except Exception as e:
        return {"success": False, "error": str(e)}


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
    """Decide keep/discard based on compilation success and confidence."""
    tests_ok = current.get("tests_passed", 0) > 0
    confidence = change.get("confidence", 0)
    level = change.get("level", "code")

    if tests_ok and level == "code" and confidence > 0.70:
        return "keep", f"code conf={int(confidence*100)}%"
    if tests_ok and level == "metadata":
        return "keep", "metadata update"
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

def segregate(rule_id):
    """Extract rule from catalog.rs to its own file (SOLID/SRP)."""
    content = CATALOG.read_text()
    pos = content.find(f'id: "{rule_id}"')
    if pos == -1:
        return

    # Find declare_rule! block
    bs = content.rfind("declare_rule!", 0, pos)
    bc = content.find("{", bs)
    depth = 0
    for i in range(bc, len(content)):
        if content[i] == "{":
            depth += 1
        elif content[i] == "}":
            depth -= 1
            if depth == 0:
                block = content[bs:i + 1]
                break

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
                evolution.log_experiment(total_alltime, rid, "rust", {}, {}, "dry_run", "Analyzer selected")
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
                    "skipped", change.get("error", "?") if change else "?"
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
                    "failed", metrics["error"]
                )
                batch_results[rule_id] = ("failed", metrics["error"])
                record_failure(rule_id)
                fails += 1
                continue

            # 4. DECIDER
            decision, reason = decider(rule_id, baseline.get(rule_id, {}), metrics, change)

            if decision == "keep":
                # Segregate BEFORE commit (atomic)
                try:
                    segregate(rule_id)
                except Exception as e:
                    log.debug(f"Segregation skipped: {e}")

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
                f"{change.get('type', '?')}: {change.get('description', '')[:120]}"
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
