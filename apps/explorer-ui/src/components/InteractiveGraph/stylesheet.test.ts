/**
 * Tests for the cytoscape stylesheet builder in
 * `stylesheet.ts`. T18 — multimodal buckets.
 *
 * RED gate: the test scans the rendered stylesheet for a one-per-
 * class regex match of the 4 new node styles (decision / doc /
 * issue / evidence) and the 4 new edge styles (cites / justifies /
 * resolves / corroborated). The console-warn fallback for an
 * unknown bucket must still fire.
 */
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";

import { buildStylesheet, KNOWN_NODE_CLASSES, KNOWN_EDGE_CLASSES } from "./stylesheet";

describe("buildStylesheet — T18 multimodal node blocks", () => {
  it("stylesheet_has_decision_style", () => {
    const styles = buildStylesheet();
    const has = styles.some(
      (s) => typeof s === "object" && "selector" in s &&
        s.selector === 'node[style_class = "node-decision"]',
    );
    expect(has).toBe(true);
  });

  it("stylesheet_has_doc_style", () => {
    const styles = buildStylesheet();
    const has = styles.some(
      (s) => typeof s === "object" && "selector" in s &&
        s.selector === 'node[style_class = "node-doc"]',
    );
    expect(has).toBe(true);
  });

  it("stylesheet_has_issue_style", () => {
    const styles = buildStylesheet();
    const has = styles.some(
      (s) => typeof s === "object" && "selector" in s &&
        s.selector === 'node[style_class = "node-issue"]',
    );
    expect(has).toBe(true);
  });

  it("stylesheet_has_evidence_style", () => {
    const styles = buildStylesheet();
    const has = styles.some(
      (s) => typeof s === "object" && "selector" in s &&
        s.selector === 'node[style_class = "node-evidence"]',
    );
    expect(has).toBe(true);
  });
});

describe("buildStylesheet — T18 multimodal edge blocks", () => {
  it("stylesheet_has_cites_edge_style", () => {
    const styles = buildStylesheet();
    const has = styles.some(
      (s) => typeof s === "object" && "selector" in s &&
        s.selector === 'edge[style_class = "edge-cites"]',
    );
    expect(has).toBe(true);
  });

  it("stylesheet_has_justifies_edge_style", () => {
    const styles = buildStylesheet();
    const has = styles.some(
      (s) => typeof s === "object" && "selector" in s &&
        s.selector === 'edge[style_class = "edge-justifies"]',
    );
    expect(has).toBe(true);
  });

  it("stylesheet_has_resolves_edge_style", () => {
    const styles = buildStylesheet();
    const has = styles.some(
      (s) => typeof s === "object" && "selector" in s &&
        s.selector === 'edge[style_class = "edge-resolves"]',
    );
    expect(has).toBe(true);
  });

  it("stylesheet_has_corroborated_edge_style", () => {
    const styles = buildStylesheet();
    const has = styles.some(
      (s) => typeof s === "object" && "selector" in s &&
        s.selector === 'edge[style_class = "edge-corroborated"]',
    );
    expect(has).toBe(true);
  });
});

describe("KNOWN_*_CLASSES — T18 mirrors the 3+3 → 7+7 expansion", () => {
  it("includes the 4 multimodal node classes", () => {
    for (const c of ["node-decision", "node-doc", "node-issue", "node-evidence"]) {
      expect(KNOWN_NODE_CLASSES.has(c)).toBe(true);
    }
  });

  it("includes the 4 multimodal edge classes", () => {
    for (const c of [
      "edge-cites",
      "edge-justifies",
      "edge-resolves",
      "edge-corroborated",
    ]) {
      expect(KNOWN_EDGE_CLASSES.has(c)).toBe(true);
    }
  });

  it("still includes the 3 legacy node classes (regression)", () => {
    for (const c of ["function", "module", "external"]) {
      expect(KNOWN_NODE_CLASSES.has(c)).toBe(true);
    }
  });

  it("still includes the 3 legacy edge classes (regression)", () => {
    for (const c of ["edge.calls", "edge.implements", "edge.uses"]) {
      expect(KNOWN_EDGE_CLASSES.has(c)).toBe(true);
    }
  });
});

describe("resolveNodeStyleClass — T18 console.warn regression", () => {
  let warnSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
  });

  afterEach(() => {
    warnSpy.mockRestore();
  });

  it("falls back to function and warns once for an unknown bucket", async () => {
    // Re-import so the module-level warnedBuckets Set is reset by
    // vitest's per-test module cache. (We can't reset it from
    // outside the module; using a unique key per test is the
    // documented workaround.)
    const { resolveNodeStyleClass } = await import("./stylesheet");
    const out = resolveNodeStyleClass("node-bogus-test-1");
    expect(out).toBe("function");
    expect(warnSpy).toHaveBeenCalled();
    const msg = warnSpy.mock.calls[0]?.[0] as string;
    expect(msg).toContain("node-bogus-test-1");
  });
});
