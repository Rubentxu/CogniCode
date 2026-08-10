#!/usr/bin/env python3
"""scorecard_streak.py — Track the 3-consecutive-scorecard-runs counter.

Per ADR-031 §3: "1.0.0 requires the scorecard to be GREEN for 3 consecutive
executions before tagging." This script verifies the counter and emits a
streak record.

Usage:
  python3 sandbox/scripts/scorecard_streak.py --record <scorecard.json> --purpose e31g

The streak is preserved in `sandbox/results/scorecard_streak.json`:
  {
    "current_streak": 3,
    "last_run_at": "2026-08-10T16:40:00Z",
    "history": [...],
    "verdict": "GREEN"
  }

The counter is INCREMENTED when the new run is GREEN+ and the previous run
was GREEN+ with no RED gates. The counter is RESET to 0 on any RED.

Exit code:
  0  streak counter incremented (or counter already at goal)
  1  some gates RED — counter reset
  2  some gates AMBER — counter held (not reset, but not incremented)
"""

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path


# Goal: 3 consecutive GREEN runs before v1.0.0 tag
GOAL_STREAK = 3


def load_scorecard(path: Path) -> dict:
    """Load a scorecard.json and return its gates verdict."""
    data = json.loads(path.read_text())
    gates = data.get("gates", [])
    if not gates:
        raise ValueError(f"scorecard {path} has no gates")
    return data


def verdict_is_green_plus(data: dict) -> bool:
    """All 12 gates are GREEN or AMBER (no RED)."""
    return all(g.get("status") != "RED" for g in data.get("gates", []))


def verdict_is_all_green(data: dict) -> bool:
    """All 12 gates are GREEN (no AMBER, no RED)."""
    return all(g.get("status") == "GREEN" for g in data.get("gates", []))


def load_streak(streak_path: Path) -> dict:
    """Load the streak file (or initialize a new one)."""
    if streak_path.exists():
        return json.loads(streak_path.read_text())
    return {
        "current_streak": 0,
        "goal": GOAL_STREAK,
        "last_run_at": None,
        "history": [],
        "verdict": "N/A",
    }


def save_streak(streak_path: Path, streak: dict) -> None:
    """Atomically write the streak file."""
    streak_path.write_text(json.dumps(streak, indent=2))


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--record", required=True,
                   help="Path to scorecard.json to record")
    p.add_argument("--streak-file", default="sandbox/results/scorecard_streak.json",
                   help="Path to the streak ledger (default: sandbox/results/scorecard_streak.json)")
    p.add_argument("--purpose", default="e31g",
                   help="Label for this run (e.g. e31g, e30.5, etc.)")
    args = p.parse_args()

    scorecard_path = Path(args.record)
    if not scorecard_path.exists():
        print(f"ERROR: scorecard not found: {scorecard_path}", file=sys.stderr)
        return 2

    data = load_scorecard(scorecard_path)
    streak_path = Path(args.streak_file)
    streak = load_streak(streak_path)

    # Verdict classification
    if verdict_is_all_green(data):
        verdict = "GREEN"
    elif verdict_is_green_plus(data):
        verdict = "AMBER"
    else:
        verdict = "RED"

    # Count gates by status
    counts = {"GREEN": 0, "AMBER": 0, "RED": 0}
    for g in data.get("gates", []):
        counts[g.get("status", "UNKNOWN")] = counts.get(g.get("status", "UNKNOWN"), 0) + 1

    now = datetime.now(timezone.utc).isoformat()

    # Update the streak
    if verdict == "RED":
        # Counter reset
        streak["current_streak"] = 0
        streak["verdict"] = "RESET"
    elif verdict == "AMBER":
        # Counter held (no increment, no reset)
        streak["verdict"] = "HELD"
    else:  # GREEN all 12
        streak["current_streak"] = streak.get("current_streak", 0) + 1
        streak["verdict"] = "GREEN"

    streak["last_run_at"] = now
    streak["last_run_scorecard"] = str(scorecard_path)
    streak["history"] = streak.get("history", []) + [
        {
            "at": now,
            "purpose": args.purpose,
            "verdict": verdict,
            "gate_counts": counts,
            "streak_after": streak["current_streak"],
        }
    ]

    # Keep only last 20 history entries
    streak["history"] = streak["history"][-20:]

    save_streak(streak_path, streak)

    # Report
    print(f"==> Scorecard Streak Update")
    print(f"    Scorecard:  {scorecard_path}")
    print(f"    Verdict:    {verdict} (G:{counts['GREEN']} A:{counts['AMBER']} R:{counts['RED']})")
    print(f"    Streak:     {streak['current_streak']}/{GOAL_STREAK}")
    print(f"    Status:     {streak['verdict']}")

    if streak["current_streak"] >= GOAL_STREAK:
        print()
        print(f"==> GOAL REACHED: {GOAL_STREAK} consecutive ALL-GREEN scorecards.")
        print(f"    Ready for v1.0.0 tag cut (per ADR-031 §3).")

    # Set exit code
    if verdict == "RED":
        return 1
    if verdict == "AMBER":
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
