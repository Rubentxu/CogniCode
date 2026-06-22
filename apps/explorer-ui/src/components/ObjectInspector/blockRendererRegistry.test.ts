/**
 * `blockRendererRegistry.test.ts` — E1.4 block registry exhaustiveness.
 *
 * Phase 3 acceptance criteria:
 * 1. Every known block id (from ViewBlocks/types.ts) has a registered renderer.
 * 2. Unknown block ids have no entry (fallback is UnknownBlockView component).
 * 3. BlockRendererEntry shape: { component, displayName }
 */
import { describe, it, expect } from "vitest";

// Import ViewBlock first to trigger side-effect block registrations.
// ViewBlock.tsx imports blockRendererRegistry and then imports all
// ViewBlocks/*.tsx files which call registerBlockRenderer at module load.
import "./ViewBlock";
import { blockRendererRegistry } from "./blockRendererRegistry";
import type { BlockRendererEntry } from "./blockRendererRegistry";

// The 29 known block ids — matches KNOWN_IDS in ViewBlocks/types.ts
const EXPECTED_BLOCKS = [
  "identity",
  "call_metrics",
  "signature",
  "callers",
  "callees",
  "source_slice",
  "symbol_quality_identity",
  "symbol_quality_issues",
  "file_quality_identity",
  "file_quality_issues",
  "file_quality_gate",
  "scope_quality_identity",
  "scope_quality_gate",
  "scope_quality_issues",
  "issue_identity",
  "issue_location",
  "issue_message",
  "rule_identity",
  "rule_related",
  "file_identity",
  "kinds",
  "symbols",
  "scope_identity",
  "scope_kinds",
  "scope_files",
  "cross_scope",
  "hotspots",
  "quality_summary",
  "quality_issue_detail",
] as const;

describe("blockRendererRegistry — exhaustiveness", () => {
  it("all 29 expected block ids are registered", () => {
    const missing: string[] = [];
    for (const id of EXPECTED_BLOCKS) {
      const entry = blockRendererRegistry.get(id);
      if (!entry) missing.push(id);
    }
    expect(missing).toEqual([]);
  });

  it("no entry for unknown block ids", () => {
    const entry = blockRendererRegistry.get("not_a_real_block_id_xyz");
    expect(entry).toBeUndefined();
  });

  it("every registered entry has component and displayName", () => {
    for (const id of EXPECTED_BLOCKS) {
      const entry = blockRendererRegistry.get(id);
      expect(entry).toMatchObject({
        component: expect.any(Function),
        displayName: expect.any(String),
      } satisfies Partial<BlockRendererEntry<unknown>>);
    }
  });

  it("all 29 entries have non-empty displayNames", () => {
    for (const id of EXPECTED_BLOCKS) {
      const entry = blockRendererRegistry.get(id);
      expect(entry?.displayName?.trim().length).toBeGreaterThan(0);
    }
  });
});

describe("blockRendererRegistry — size", () => {
  it("registry has at least 29 entries (all expected blocks registered)", () => {
    expect(blockRendererRegistry.size).toBeGreaterThanOrEqual(EXPECTED_BLOCKS.length);
  });
});
