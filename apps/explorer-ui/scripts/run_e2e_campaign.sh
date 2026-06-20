#!/usr/bin/env bash
# run_e2e_campaign.sh — Explorer E2E campaign runner
# Usage: bash apps/explorer-ui/scripts/run_e2e_campaign.sh [repeat-count]
#
# Creates an isolated run directory under apps/explorer-ui/e2e-runs/<run-id>/
# For each repeat:
#   - runs `npx playwright test --reporter=json` capturing output to run-N.json
#   - captures the exit code
# Builds summary.json and (if repeat > 1) stability.json
# Generates report.html by calling generate_e2e_report.py
# Exits non-zero if any repeat fails.

set -euo pipefail

REPEAT="${1:-1}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
E2E_DIR="$(dirname "$SCRIPT_DIR")"
RUNS_DIR="$E2E_DIR/e2e-runs"
RUN_ID="$(date +%Y%m%dT%H%M%S)"
RUN_DIR="$RUNS_DIR/$RUN_ID"

mkdir -p "$RUN_DIR"

echo "🎭 Explorer E2E campaign — run_id=$RUN_ID repeat=$REPEAT"
echo "📁 $RUN_DIR"

FAILED=0
cd "$E2E_DIR"

for i in $(seq 1 "$REPEAT"); do
    echo "▶  Run $i/$REPEAT..."
    EXIT_CODE=0
    npx playwright test --reporter=json 2>/dev/null > "$RUN_DIR/run-$i.json" || EXIT_CODE=$?
    echo "   exit=$EXIT_CODE  →  $RUN_DIR/run-$i.json"
    if [ "$EXIT_CODE" -ne 0 ]; then
        FAILED=1
    fi
done

# Aggregate summary.json
echo ""
echo "📊 Building summary.json..."
node -e "
const fs = require('fs');
const repeat = $REPEAT;
const runIds = Array.from({length: repeat}, (_, i) => i + 1);
const runs = runIds.map(id => JSON.parse(fs.readFileSync('$RUN_DIR/run-' + id + '.json', 'utf8')));

const totalExpected = runs.reduce((s, r) => s + (r.stats.expected || 0), 0);
const totalUnexpected = runs.reduce((s, r) => s + (r.stats.unexpected || 0), 0);
const totalSkipped = runs.reduce((s, r) => s + (r.stats.skipped || 0), 0);
const totalFlaky = runs.reduce((s, r) => s + (r.stats.flaky || 0), 0);
const totalDuration = runs.reduce((s, r) => s + (r.stats.duration || 0), 0);
const allPassed = totalUnexpected === 0;

const summary = {
  runId: '$RUN_ID',
  repeat,
  allPassed,
  stats: {
    expected: totalExpected,
    unexpected: totalUnexpected,
    skipped: totalSkipped,
    flaky: totalFlaky,
    duration: totalDuration
  }
};
fs.writeFileSync('$RUN_DIR/summary.json', JSON.stringify(summary, null, 2));
console.log('  expected:', totalExpected, ' unexpected:', totalUnexpected,
            ' skipped:', totalSkipped, ' flaky:', totalFlaky,
            ' duration_ms:', totalDuration.toFixed(1));
console.log('  allPassed:', allPassed);
"

# Stability analysis for repeat > 1
if [ "$REPEAT" -gt 1 ]; then
    echo ""
    echo "🔄 Computing stability.json..."
    node -e "
const fs = require('fs');
const repeat = $REPEAT;
const runIds = Array.from({length: repeat}, (_, i) => i + 1);
const runs = runIds.map(id => JSON.parse(fs.readFileSync('$RUN_DIR/run-' + id + '.json', 'utf8')));

// Walk all specs across all runs
const testMap = {};
runs.forEach((run, runIdx) => {
  function walk(suite) {
    (suite.suites || []).forEach(walk);
    (suite.specs || []).forEach(spec => {
      (spec.tests || []).forEach(test => {
        const key = spec.title + '::' + test.title;
        if (!testMap[key]) {
          testMap[key] = { spec: spec.title, title: test.title, outcomes: [], durations: [] };
        }
        testMap[key].outcomes.push(test.status);
        testMap[key].durations.push(test.duration || 0);
      });
    });
  }
  (run.suites || []).forEach(walk);
});

const flaky = [];
const stable = [];
Object.values(testMap).forEach(t => {
  const passCount = t.outcomes.filter(o => o === 'expected').length;
  const passRate = passCount / t.outcomes.length;
  const meanDuration = t.durations.reduce((a, b) => a + b, 0) / t.durations.length;
  const entry = { spec: t.spec, title: t.title, passRate, passCount, total: t.outcomes.length, meanDurationMs: meanDuration };
  if (passRate < 1.0) flaky.push(entry); else stable.push(entry);
});

flaky.sort((a, b) => a.passRate - b.passRate);

const stability = { runId: '$RUN_ID', repeat, totalTests: Object.keys(testMap).length, flaky, stable };
fs.writeFileSync('$RUN_DIR/stability.json', JSON.stringify(stability, null, 2));

console.log('  total unique tests:', Object.keys(testMap).length);
console.log('  flaky:', flaky.length, '  stable:', stable.length);
if (flaky.length > 0) {
  console.log('  Top flaky:');
  flaky.slice(0, 5).forEach(t => console.log('   ', t.spec + '::' + t.title, 'passRate=' + t.passRate.toFixed(2)));
}
"
fi

# Generate HTML report
echo ""
echo "📄 Generating report.html..."
python3 "$SCRIPT_DIR/generate_e2e_report.py" --results-dir "$RUN_DIR" --output "$RUN_DIR/report.html"

echo ""
echo "✅ Campaign complete: $RUN_DIR"
echo "   report: $RUN_DIR/report.html"
echo "   summary: $RUN_DIR/summary.json"
if [ "$REPEAT" -gt 1 ]; then echo "   stability: $RUN_DIR/stability.json"; fi

if [ "$FAILED" -ne 0 ]; then
    echo ""
    echo "⚠️  One or more repeats had failures (FAILED=$FAILED)"
fi

exit "$FAILED"
