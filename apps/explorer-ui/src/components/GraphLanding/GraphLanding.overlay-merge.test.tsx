/**
 * `GraphLanding.overlay-merge` tests — C4 overlay class application.
 *
 * Tests that verify:
 * - Node with both hotspot + drift → hotspot class wins
 * - Drift-only node → drift class applied
 * - Hotspot-only node → hotspot class applied
 * - Neither → no overlay class
 */
import { describe, it, expect } from "vitest";

import {
  normalizeContainerName,
  matchDriftToNodes,
  applyOverlayClass,
} from "./GraphLanding";
import type { DriftReport, GraphNode, DriftKind } from "../../api/types";
import type { C4HotspotData } from "../../hooks/useC4Hotspots";

describe("C4 Overlay utilities", () => {
  describe("normalizeContainerName", () => {
    it("strips hyphens, underscores, and spaces", () => {
      expect(normalizeContainerName("my-container")).toBe("mycontainer");
      expect(normalizeContainerName("my_container")).toBe("mycontainer");
      expect(normalizeContainerName("my container")).toBe("mycontainer");
      expect(normalizeContainerName("My-Container_Name")).toBe("mycontainername");
    });

    it("handles mixed separators", () => {
      expect(normalizeContainerName("foo-bar_baz qux")).toBe("foobarbazqux");
    });

    it("returns empty string for empty input", () => {
      expect(normalizeContainerName("")).toBe("");
    });
  });

  describe("matchDriftToNodes", () => {
    const mockNodes: GraphNode[] = [
      { id: "n1", label: "api-service", kind: "container", style_class: "node-container" },
      { id: "n2", label: "web-frontend", kind: "container", style_class: "node-container" },
      { id: "n3", label: "backend-db", kind: "container", style_class: "node-container" },
    ];

    it("returns empty map when driftReport is undefined", () => {
      const result = matchDriftToNodes(undefined, mockNodes);
      expect(result.size).toBe(0);
    });

    it("returns empty map when driftReport has no findings", () => {
      const report: DriftReport = { findings: [], summary: "", missing_containers: 0, extra_containers: 0, wrong_sub_kinds: 0, boundary_violations: 0 };
      const result = matchDriftToNodes(report, mockNodes);
      expect(result.size).toBe(0);
    });

    it("matches findings to nodes by normalized name", () => {
      const report: DriftReport = {
        findings: [
          { kind: "missing", expected: "api-service", actual: "—", severity: "high", detail: "" },
          { kind: "extra", expected: "—", actual: "unknown-service", severity: "medium", detail: "" },
        ],
        summary: "",
        missing_containers: 1,
        extra_containers: 1,
        wrong_sub_kinds: 0,
        boundary_violations: 0,
      };
      const result = matchDriftToNodes(report, mockNodes);
      expect(result.get("n1")).toBe("missing");
      // "unknown-service" doesn't match any node
      expect(result.size).toBe(1);
    });

    it("handles names with different separators", () => {
      const report: DriftReport = {
        findings: [
          { kind: "wrong_sub_kind", expected: "api_service", actual: "api-service", severity: "medium", detail: "" },
        ],
        summary: "",
        missing_containers: 0,
        extra_containers: 0,
        wrong_sub_kinds: 1,
        boundary_violations: 0,
      };
      const result = matchDriftToNodes(report, mockNodes);
      expect(result.get("n1")).toBe("wrong_sub_kind");
    });
  });

  describe("applyOverlayClass", () => {
    const emptyDrift = new Map<string, DriftKind>();
    const emptyHotspot = new Map<string, C4HotspotData>();

    it("returns node unchanged when no overlays", () => {
      const node: GraphNode = { id: "n1", label: "test", kind: "container", style_class: "node-container" };
      const result = applyOverlayClass(node, emptyDrift, emptyHotspot);
      expect(result.style_class).toBe("node-container");
    });

    it("applies hotspot-high when present (priority over drift)", () => {
      const node: GraphNode = { id: "n1", label: "test", kind: "container", style_class: "node-container" };
      const hotspotMap = new Map<string, C4HotspotData>([["n1", { score: 0.9, kind: "high" }]]);
      const driftMap = new Map<string, DriftKind>([["n1", "missing"]]);

      const result = applyOverlayClass(node, driftMap, hotspotMap);
      expect(result.style_class).toBe("hotspot-high");
    });

    it("applies hotspot-med when present (priority over drift)", () => {
      const node: GraphNode = { id: "n1", label: "test", kind: "container", style_class: "node-container" };
      const hotspotMap = new Map<string, C4HotspotData>([["n1", { score: 0.5, kind: "med" }]]);
      const driftMap = new Map<string, DriftKind>([["n1", "extra"]]);

      const result = applyOverlayClass(node, driftMap, hotspotMap);
      expect(result.style_class).toBe("hotspot-med");
    });

    it("applies drift-missing when no hotspot", () => {
      const node: GraphNode = { id: "n1", label: "test", kind: "container", style_class: "node-container" };
      const driftMap = new Map<string, DriftKind>([["n1", "missing"]]);

      const result = applyOverlayClass(node, driftMap, emptyHotspot);
      expect(result.style_class).toBe("drift-missing");
    });

    it("applies drift-extra when no hotspot", () => {
      const node: GraphNode = { id: "n1", label: "test", kind: "container", style_class: "node-container" };
      const driftMap = new Map<string, DriftKind>([["n1", "extra"]]);

      const result = applyOverlayClass(node, driftMap, emptyHotspot);
      expect(result.style_class).toBe("drift-extra");
    });

    it("applies drift-wrong-kind when no hotspot", () => {
      const node: GraphNode = { id: "n1", label: "test", kind: "container", style_class: "node-container" };
      const driftMap = new Map<string, DriftKind>([["n1", "wrong_sub_kind"]]);

      const result = applyOverlayClass(node, driftMap, emptyHotspot);
      expect(result.style_class).toBe("drift-wrong-kind");
    });

    it("applies hotspot-only when both maps have entry", () => {
      const node: GraphNode = { id: "n1", label: "test", kind: "container", style_class: "node-container" };
      const hotspotMap = new Map<string, C4HotspotData>([["n1", { score: 0.9, kind: "high" }]]);
      const driftMap = new Map<string, DriftKind>([["n1", "extra"]]);

      const result = applyOverlayClass(node, driftMap, hotspotMap);
      expect(result.style_class).toBe("hotspot-high");
    });

    it("node without any overlay entry keeps original style_class", () => {
      const node: GraphNode = { id: "n1", label: "test", kind: "container", style_class: "node-component" };
      const hotspotMap = new Map<string, C4HotspotData>([["other-node", { score: 0.9, kind: "high" }]]);
      const driftMap = new Map<string, DriftKind>([["other-node", "missing"]]);

      const result = applyOverlayClass(node, driftMap, hotspotMap);
      expect(result.style_class).toBe("node-component");
    });

    it("returns a new object, not mutating the original", () => {
      const node: GraphNode = { id: "n1", label: "test", kind: "container", style_class: "node-container" };
      const hotspotMap = new Map<string, C4HotspotData>([["n1", { score: 0.9, kind: "high" }]]);

      const result = applyOverlayClass(node, emptyDrift, hotspotMap);
      expect(result).not.toBe(node);
      expect(node.style_class).toBe("node-container");
      expect(result.style_class).toBe("hotspot-high");
    });
  });
});
