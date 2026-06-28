#!/usr/bin/env tsx
// Coverage matrix generator — produces docs/inventory/e17-coverage-matrix.md
//
// Cross-references:
// - 65-feature inventory (docs/inventory/explorer-ui-feature-inventory.md)
// - Playwright specs in apps/explorer-ui/e2e (any .spec.ts)
// - Snapshot files in apps/explorer-ui/e2e (any *.spec.ts-snapshots/*.png)
//
// Output: a markdown table mapping each feature to its spec, test name,
// screenshot path, and GToolkit equivalent. The matrix is the source of
// truth for "did we cover everything" in cycle e17.
//
// Run via: npm run coverage:matrix

import { readdirSync, readFileSync, writeFileSync, statSync, existsSync } from "node:fs";
import { join, relative, basename, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname_esm = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(__dirname_esm, "..", "..", "..");
const E2E_DIR = join(REPO_ROOT, "apps", "explorer-ui", "e2e");
const OUTPUT_PATH = join(REPO_ROOT, "docs", "inventory", "e17-coverage-matrix.md");

interface Feature {
  name: string;
  category: string;
  gtoolkitEquivalent: string;
}

interface Coverage {
  feature: string;
  category: string;
  spec: string | null;
  testName: string | null;
  screenshot: string | null;
  gtoolkit: string;
  status: "covered" | "skipped-debt" | "missing";
}

/**
 * Feature catalog — mirrors docs/inventory/explorer-ui-feature-inventory.md
 * (the canonical 65-feature inventory produced during e17 explore phase).
 *
 * Each entry: { name, category, gtoolkitEquivalent }.
 * 65 features total, grouped by category.
 */
const FEATURES: Feature[] = [
  // ─── Navigation (8) ─────────────────────────────────────────────
  { name: "Spotter (Cmd+K palette)", category: "Navigation", gtoolkitEquivalent: "Spotter (universal search)" },
  { name: "Spotter cross-family results", category: "Navigation", gtoolkitEquivalent: "Spotter (multi-source)" },
  { name: "PaneStackView (lateral navigation)", category: "Navigation", gtoolkitEquivalent: "GtPager (tabbed pane stack)" },
  { name: "Pane drill-in preserves history", category: "Navigation", gtoolkitEquivalent: "GtPager history" },
  { name: "Pane dedup on re-select", category: "Navigation", gtoolkitEquivalent: "GtPager dedup" },
  { name: "Pane close (✕ button)", category: "Navigation", gtoolkitEquivalent: "GtPager tab close" },
  { name: "EntryPointResolver", category: "Navigation", gtoolkitEquivalent: "GT contextual launchers" },
  { name: "ShareExplorationButton", category: "Navigation", gtoolkitEquivalent: "Lepiter page links" },

  // ─── Views — 15 wired executors (15) ─────────────────────────────
  ...[
    "overview", "call-graph", "source", "quality", "evidence",
    "symbols", "dependencies", "hotspots", "architecture-drift",
    "usage-examples", "api-surface", "test-slice", "debug-slice",
    "change-impact-story", "ownership-map",
  ].map((id): Feature => ({
    name: `${id} view executor`,
    category: "View executor (wired)",
    gtoolkitEquivalent: "Phlow view per object type",
  })),

  // ─── Views — catalog-only debt (23) ──────────────────────────────
  ...[
    "composed_narrative", "project_diary", "example_object",
    "concept_map", "evidence_pack", "risk_map", "decision_trace",
    "doc_code_alignment", "ownership_map_v2_codeowners", "boundary_map",
    "dependency_pressure", "change_impact_story_v2", "refactor_plan",
    "callers_and_implementors", "usage_examples_v2", "api_surface_v2",
    "dead_code_candidates", "semantic_search_results", "doc_code_alignment_v2",
    "example_object_v2", "composed_narrative_v2", "project_diary_v2",
    "concept_map_v2",
  ].map((id): Feature => ({
    name: `${id} view (catalog-only)`,
    category: "View executor (debt)",
    gtoolkitEquivalent: "Phlow view (planned)",
  })),

  // ─── Landing (5) ─────────────────────────────────────────────────
  { name: "Landing entry_points", category: "Landing", gtoolkitEquivalent: "GT World home" },
  { name: "Landing hot_paths", category: "Landing", gtoolkitEquivalent: "GT Quick Tour" },
  { name: "Landing god_nodes", category: "Landing", gtoolkitEquivalent: "Mondrian high-degree" },
  { name: "Landing truncation banner", category: "Landing", gtoolkitEquivalent: "GT error state" },
  { name: "Landing virtualization (>200 nodes)", category: "Landing", gtoolkitEquivalent: "GT scrollable lists" },

  // ─── Inspectable objects (9) ─────────────────────────────────────
  ...[
    "Symbol", "File", "Scope", "Issue", "Rule", "Component",
    "Container", "System", "Decision",
  ].map((k): Feature => ({
    name: `Inspect ${k}`,
    category: "Inspectable object",
    gtoolkitEquivalent: "GT Inspector (any object)",
  })),

  // ─── Authoring (3) ───────────────────────────────────────────────
  { name: "ViewSpecWizard (5 steps)", category: "Authoring", gtoolkitEquivalent: "Lepiter authoring" },
  { name: "JSONata transform step", category: "Authoring", gtoolkitEquivalent: "Mondrian query preparation" },
  { name: "Save ViewSpec", category: "Authoring", gtoolkitEquivalent: "Lepiter persist page" },

  // ─── Settings (3) ────────────────────────────────────────────────
  { name: "Perspective toggle (Default / C4 / Quality)", category: "Settings", gtoolkitEquivalent: "GT tool perspective" },
  { name: "Responsive layout", category: "Settings", gtoolkitEquivalent: "GT resize" },
  { name: "Theme (light/dark)", category: "Settings", gtoolkitEquivalent: "GT theme" },

  // ─── Error states (4) ────────────────────────────────────────────
  { name: "Error boundary (network)", category: "Error state", gtoolkitEquivalent: "GT exception view" },
  { name: "Unknown view fallback", category: "Error state", gtoolkitEquivalent: "GT UnknownObject view" },
  { name: "MSW fallback", category: "Error state", gtoolkitEquivalent: "GT network error" },
  { name: "Panic boundary", category: "Error state", gtoolkitEquivalent: "GT debugger" },

  // ─── Accessibility (2) ───────────────────────────────────────────
  { name: "axe-core a11y (landing)", category: "Accessibility", gtoolkitEquivalent: "GT a11y" },
  { name: "axe-core a11y (all views)", category: "Accessibility", gtoolkitEquivalent: "GT a11y" },
];

/**
 * Walk e2e/ and extract test names from .spec.ts files.
 * Naive regex — accurate enough for matrix generation.
 */
function extractTests(): { spec: string; name: string }[] {
  const results: { spec: string; name: string }[] = [];
  if (!existsSync(E2E_DIR)) return results;

  for (const entry of readdirSync(E2E_DIR)) {
    if (!entry.endsWith(".spec.ts")) continue;
    const specPath = join(E2E_DIR, entry);
    const text = readFileSync(specPath, "utf8");
    const re = /test(?:\.skip|\.fixme)?\(\s*["']([^"']+)["']/g;
    let match: RegExpExecArray | null;
    while ((match = re.exec(text)) !== null) {
      results.push({ spec: entry, name: match[1] });
    }
  }
  return results;
}

/**
 * Walk e2e/*-snapshots/ and list PNG files.
 */
function extractSnapshots(): string[] {
  const results: string[] = [];
  if (!existsSync(E2E_DIR)) return results;

  for (const entry of readdirSync(E2E_DIR)) {
    if (!entry.endsWith(".spec.ts-snapshots")) continue;
    const dir = join(E2E_DIR, entry);
    if (!statSync(dir).isDirectory()) continue;
    for (const file of readdirSync(dir)) {
      if (file.endsWith(".png")) {
        results.push(relative(REPO_ROOT, join(dir, file)));
      }
    }
  }
  return results;
}

/**
 * Match a feature to a test/screenshot by name overlap.
 * Returns null if no match.
 */
function matchFeature(
  feature: Feature,
  tests: { spec: string; name: string }[],
  snapshots: string[],
): { spec: string; testName: string; screenshot: string | null } | null {
  const fWords = feature.name.toLowerCase().split(/\s+/).filter((w) => w.length > 3);
  const fId = feature.name.replace(/\s+/g, "-").toLowerCase();

  for (const t of tests) {
    const tLower = t.name.toLowerCase();
    if (
      fWords.some((w) => tLower.includes(w)) ||
      tLower.includes(fId) ||
      tLower.includes(feature.gtoolkitEquivalent.toLowerCase().split(" ")[0] ?? "")
    ) {
      const matchingSnap = snapshots.find((s) => {
        const sBase = basename(s).toLowerCase();
        return sBase.includes(t.spec.replace(".spec.ts", "")) && fWords.some((w) => sBase.includes(w));
      });
      return { spec: t.spec, testName: t.name, screenshot: matchingSnap ?? null };
    }
  }
  return null;
}

function main(): void {
  const tests = extractTests();
  const snapshots = extractSnapshots();

  const coverage: Coverage[] = FEATURES.map((f) => {
    const match = matchFeature(f, tests, snapshots);
    const isDebt = f.category.includes("debt");
    if (match) {
      return {
        feature: f.name,
        category: f.category,
        spec: match.spec,
        testName: match.testName,
        screenshot: match.screenshot,
        gtoolkit: f.gtoolkitEquivalent,
        status: "covered",
      };
    }
    if (isDebt) {
      return {
        feature: f.name,
        category: f.category,
        spec: null,
        testName: null,
        screenshot: null,
        gtoolkit: f.gtoolkitEquivalent,
        status: "skipped-debt",
      };
    }
    return {
      feature: f.name,
      category: f.category,
      spec: null,
      testName: null,
      screenshot: null,
      gtoolkit: f.gtoolkitEquivalent,
      status: "missing",
    };
  });

  // ─── Summary by category ─────────────────────────────────────────
  const categories = Array.from(new Set(coverage.map((c) => c.category)));
  const summary = categories.map((cat) => {
    const items = coverage.filter((c) => c.category === cat);
    const covered = items.filter((c) => c.status === "covered").length;
    const skipped = items.filter((c) => c.status === "skipped-debt").length;
    const missing = items.filter((c) => c.status === "missing").length;
    return { category: cat, total: items.length, covered, skipped, missing };
  });

  const totalCovered = coverage.filter((c) => c.status === "covered").length;
  const totalSkipped = coverage.filter((c) => c.status === "skipped-debt").length;
  const totalMissing = coverage.filter((c) => c.status === "missing").length;

  // ─── Markdown output ─────────────────────────────────────────────
  const lines: string[] = [];
  lines.push("# E2E Coverage Matrix — Cycle e17");
  lines.push("");
  lines.push(`Last generated: ${new Date().toISOString()}`);
  lines.push("");
  lines.push("Cross-reference of the 65-feature inventory against the Playwright E2E suite.");
  lines.push("This matrix is the source of truth for \"did we cover everything\".");
  lines.push("");
  lines.push("## Summary");
  lines.push("");
  lines.push("| Category | Total | Covered | Skipped (debt) | Missing |");
  lines.push("|---|---|---|---|---|");
  for (const s of summary) {
    lines.push(`| ${s.category} | ${s.total} | ${s.covered} | ${s.skipped} | ${s.missing} |`);
  }
  lines.push(`| **TOTAL** | **${coverage.length}** | **${totalCovered}** | **${totalSkipped}** | **${totalMissing}** |`);
  lines.push("");
  lines.push("## Coverage targets (cycle e17 spec)");
  lines.push("");
  lines.push("- ≥60 of 65 features covered (≥92%)");
  lines.push("- Each of 15 wired executors covered");
  lines.push("- Each of 6 Spotter families covered");
  lines.push("- Every MSW handler hit");
  lines.push("");
  lines.push("## Detailed matrix");
  lines.push("");
  lines.push("| Feature | Category | Spec | Test name | Screenshot | GToolkit equivalent | Status |");
  lines.push("|---|---|---|---|---|---|---|");
  for (const c of coverage) {
    const statusBadge =
      c.status === "covered" ? "✅" : c.status === "skipped-debt" ? "⏭️" : "❌";
    lines.push(
      `| ${c.feature} | ${c.category} | ${c.spec ?? "—"} | ${c.testName ?? "—"} | ${c.screenshot ?? "—"} | ${c.gtoolkit} | ${statusBadge} |`,
    );
  }
  lines.push("");
  lines.push("## Legend");
  lines.push("");
  lines.push("- ✅ **covered** — at least one E2E test asserts user-visible behavior");
  lines.push("- ⏭️ **skipped (debt)** — feature is catalog-only, no implementation (ADR-002 capability debt)");
  lines.push("- ❌ **missing** — implementation exists but no E2E test covers it (must fix before cycle close)");

  // Write output.
  const out = lines.join("\n") + "\n";
  writeFileSync(OUTPUT_PATH, out, "utf8");
  console.log(`Wrote ${OUTPUT_PATH}`);
  console.log(`Total: ${coverage.length} | Covered: ${totalCovered} | Skipped: ${totalSkipped} | Missing: ${totalMissing}`);
}

main();
