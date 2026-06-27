/**
 * Tests for the Zod schemas in `src/api/schemas.ts`.
 *
 * Coverage:
 * - All top-level DTOs accept a minimal valid fixture.
 * - The discriminated union accepts every block id in the union.
 * - The fallback schema accepts any unknown id and any body.
 * - Invalid data is rejected (status, missing fields, wrong types).
 */
import { describe, expect, it } from "vitest";

import {
  contextualViewSchema,
  decisionArtifactSummarySchema,
  designFindingSchema,
  generateArtifactRequestSchema,
  healthResponseSchema,
  inspectableObjectSummarySchema,
  inspectableObjectTypeSchema,
  lensDescriptorSchema,
  lensResultSchema,
  openWorkspaceRequestSchema,
  qualityIssueItemSchema,
  spotterResultSchema,
  spotterSearchResultSchema,
  viewBlockAnySchema,
  viewBlockSchema,
  viewDescriptorSchema,
  viewSpecSummarySchema,
  workspaceSummarySchema,
  unknownViewBlockSchema,
} from "./schemas";
import {
  contextualViewFixture,
  decisionArtifactFixture,
  inspectableObjectFixture,
  lensDescriptorsFixture,
  lensResultFixture,
  spotterResultsFixture,
  workspaceSummaryFixture,
} from "../mocks/fixtures";

// ============================================================================
// Helpers
// ============================================================================

/**
 * Build a minimal-but-valid `qualityIssueItem` for tests that need
 * one. The shared fixture already includes one, but the helper makes
 * the input shape obvious at every call site.
 */
const baseIssueItem = {
  id: 1,
  rule_id: "rust:S100",
  severity: "warning",
  category: "naming",
  file: "src/lib.rs",
  line: 10,
  message: "naming convention",
  status: "open",
  object_id: "issue:1",
};

// ============================================================================
// Top-level DTOs
// ============================================================================

describe("workspaceSummarySchema", () => {
  it("accepts a valid summary", () => {
    expect(() => workspaceSummarySchema.parse(workspaceSummaryFixture)).not.toThrow();
  });

  it("rejects a missing root_path", () => {
    const broken = { ...workspaceSummaryFixture, root_path: undefined };
    expect(() => workspaceSummarySchema.parse(broken)).toThrow();
  });

  it("rejects an unknown graph_status", () => {
    const broken = { ...workspaceSummaryFixture, graph_status: "wat" };
    expect(() => workspaceSummarySchema.parse(broken)).toThrow();
  });
});

describe("inspectableObjectSummarySchema", () => {
  it("accepts a valid object", () => {
    expect(() =>
      inspectableObjectSummarySchema.parse(inspectableObjectFixture),
    ).not.toThrow();
  });

  it("rejects an unknown object_type", () => {
    const broken = { ...inspectableObjectFixture, object_type: "alien" };
    expect(() => inspectableObjectSummarySchema.parse(broken)).toThrow();
  });
});

describe("spotterResultSchema", () => {
  it("accepts a list of results", () => {
    expect(() => spotterResultSchema.parse(spotterResultsFixture[0])).not.toThrow();
  });

  it("rejects a result with a non-numeric score", () => {
    const broken = { ...spotterResultsFixture[0], score: "high" };
    expect(() => spotterResultSchema.parse(broken)).toThrow();
  });
});

/**
 * Regression test for e13-wave-1.1: the backend `SpotterSearchResult`
 * enum has 6 families (symbol, file, viewspec, saved_exploration,
 * quality_issue, rule). The frontend schema must accept all of them or
 * Zod silently drops the unrecognised variants from the parsed array.
 *
 * Before the fix: only "symbol" and "viewspec" were in the union.
 * After the fix: all 6 families are accepted.
 */
describe("spotterSearchResultSchema", () => {
  // Minimal valid SpotterResult payload shared by symbol/file/saved_exploration/
  // quality_issue/rule variants.
  const baseResult = {
    object: inspectableObjectFixture,
    score: 0.9,
    match_type: "name_exact",
  };

  // Minimal valid ViewSpecSummary for the viewspec variant.
  const baseViewSpec = {
    id: "550e8400-e29b-41d4-a716-446655440000",
    title: "Overview",
    view_kind: "vertical_slice",
    applies_to: "symbol",
    owner: "system",
    updated_at: "2026-06-27T00:00:00Z",
  };

  it("accepts symbol variant", () => {
    expect(
      spotterSearchResultSchema.parse({ kind: "symbol", result: baseResult }),
    ).toMatchObject({ kind: "symbol", result: baseResult });
  });

  it("accepts file variant", () => {
    expect(
      spotterSearchResultSchema.parse({ kind: "file", result: baseResult }),
    ).toMatchObject({ kind: "file", result: baseResult });
  });

  it("accepts viewspec variant", () => {
    expect(
      spotterSearchResultSchema.parse({ kind: "viewspec", result: baseViewSpec }),
    ).toMatchObject({ kind: "viewspec", result: baseViewSpec });
  });

  it("accepts saved_exploration variant", () => {
    expect(
      spotterSearchResultSchema.parse({
        kind: "saved_exploration",
        result: baseResult,
      }),
    ).toMatchObject({ kind: "saved_exploration", result: baseResult });
  });

  it("accepts quality_issue variant", () => {
    expect(
      spotterSearchResultSchema.parse({
        kind: "quality_issue",
        result: baseResult,
      }),
    ).toMatchObject({ kind: "quality_issue", result: baseResult });
  });

  it("accepts rule variant", () => {
    expect(
      spotterSearchResultSchema.parse({ kind: "rule", result: baseResult }),
    ).toMatchObject({ kind: "rule", result: baseResult });
  });

  it("accepts an array of mixed families", () => {
    const wire = [
      { kind: "symbol", result: baseResult },
      { kind: "file", result: baseResult },
      { kind: "viewspec", result: baseViewSpec },
      { kind: "saved_exploration", result: baseResult },
      { kind: "quality_issue", result: baseResult },
      { kind: "rule", result: baseResult },
    ];
    expect(() => spotterSearchResultSchema.array().parse(wire)).not.toThrow();
    const parsed = spotterSearchResultSchema.array().parse(wire);
    expect(parsed).toHaveLength(6);
  });

  it("rejects unknown kind", () => {
    const broken = { kind: "alien", result: baseResult };
    expect(() => spotterSearchResultSchema.parse(broken)).toThrow();
  });

  it("rejects missing kind", () => {
    const broken = { result: baseResult };
    expect(() => spotterSearchResultSchema.parse(broken)).toThrow();
  });
});

describe("viewDescriptorSchema", () => {
  it("accepts a descriptor", () => {
    expect(() => viewDescriptorSchema.parse({ id: "x", title: "X" })).not.toThrow();
  });
});

// ============================================================================
// Quality + lenses
// ============================================================================

describe("qualityIssueItemSchema", () => {
  it("accepts a valid issue", () => {
    expect(() => qualityIssueItemSchema.parse(baseIssueItem)).not.toThrow();
  });

  it("rejects a zero id", () => {
    const broken = { ...baseIssueItem, id: 0 };
    expect(() => qualityIssueItemSchema.parse(broken)).toThrow();
  });
});

describe("designFindingSchema", () => {
  it("accepts a valid finding", () => {
    const finding = lensResultFixture.findings[0]!;
    expect(() => designFindingSchema.parse(finding)).not.toThrow();
  });

  it("rejects an out-of-range confidence", () => {
    const finding = { ...lensResultFixture.findings[0]!, confidence: 1.5 };
    expect(() => designFindingSchema.parse(finding)).toThrow();
  });
});

describe("lensDescriptorSchema", () => {
  it("accepts a descriptor", () => {
    expect(() => lensDescriptorSchema.parse(lensDescriptorsFixture[0])).not.toThrow();
  });
});

describe("lensResultSchema", () => {
  it("accepts a result", () => {
    expect(() => lensResultSchema.parse(lensResultFixture)).not.toThrow();
  });
});

// ============================================================================
// Contextual view + blocks
// ============================================================================

describe("contextualViewSchema", () => {
  it("accepts a fully populated view", () => {
    expect(() =>
      contextualViewSchema.parse(contextualViewFixture),
    ).not.toThrow();
  });

  it("defaults `findings` to an empty list when missing", () => {
    const { findings: _findings, ...rest } = contextualViewFixture;
    void _findings;
    const parsed = contextualViewSchema.parse(rest);
    expect(parsed.findings).toEqual([]);
  });
});

describe("viewBlockAnySchema — fallback for unknown block ids", () => {
  it("accepts a known block id and types it as a ViewBlock", () => {
    const block = contextualViewFixture.blocks[0]!;
    const parsed = viewBlockAnySchema.parse(block);
    expect(parsed.id).toBe(block.id);
  });

  it("falls back to UnknownViewBlock for an unknown id", () => {
    const weird = {
      id: "totally_new_block_kind",
      title: "Future",
      body: { something: "anything", count: 42 },
    };
    const parsed = viewBlockAnySchema.parse(weird);
    expect(parsed.id).toBe("totally_new_block_kind");
    // The fallback schema is `body: z.unknown()`, so any shape survives.
    expect(parsed).toEqual(weird);
  });

  it("the typed union rejects the unknown id", () => {
    const weird = {
      id: "totally_new_block_kind",
      title: "Future",
      body: { something: "anything" },
    };
    expect(() => viewBlockSchema.parse(weird)).toThrow();
  });

  it("unknownViewBlockSchema accepts any block shape", () => {
    const weird = {
      id: "any-future-id",
      title: "Anything",
      body: null,
    };
    expect(() => unknownViewBlockSchema.parse(weird)).not.toThrow();
  });
});

describe("viewBlockSchema — every known block id", () => {
  it("accepts a block for each shape in the union", () => {
    // Build a small, valid body for every block id in the union.
    const sampleBodies: Record<string, unknown> = {
      identity: { name: "f", kind: "function", file: "a.rs", line: 1 },
      call_metrics: { fan_in: 0, fan_out: 0 },
      signature: { signature: "fn f()" },
      callers: { count: 0, items: [] },
      callees: { count: 0, items: [] },
      source_slice: {
        file: "a.rs",
        line: 1,
        lines: [{ line: 1, text: "x" }],
      },
      symbol_quality_identity: { file: "a.rs", line: 1, issue_count: 0 },
      symbol_quality_issues: { count: 0, items: [] },
      file_quality_identity: { path: "a.rs", issue_count: 0 },
      file_quality_issues: { count: 0, items: [] },
      file_quality_gate: {
        rating: null,
        total_issues: 0,
        blockers: 0,
        criticals: 0,
        debt_minutes: 0,
        last_run: null,
      },
      scope_quality_identity: {
        scope: "src",
        issue_count: 0,
        by_severity: {},
      },
      scope_quality_gate: {
        rating: null,
        total_issues: 0,
        blockers: 0,
        criticals: 0,
        debt_minutes: 0,
        last_run: null,
      },
      scope_quality_issues: { count: 0, items: [] },
      issue_identity: {
        id: 1,
        rule_id: "rust:S100",
        severity: "warning",
        category: "naming",
        status: "open",
      },
      issue_location: { file: "a.rs", line: 1 },
      issue_message: { message: "msg" },
      rule_identity: { rule_id: "rust:S100", description: "x", open_count: 0 },
      rule_related: { count: 0, items: [] },
      file_identity: { path: "a.rs", line_count: 1, symbol_count: 0 },
      kinds: { breakdown: {} },
      symbols: { count: 0, items: [] },
      scope_identity: {
        path: "src",
        file_count: 0,
        symbol_count: 0,
        promotion_ready: false,
      },
      scope_kinds: { breakdown: {} },
      scope_files: { files: [] },
      cross_scope: { scope: "src", file_count: 0, symbol_count: 0, entries: [] },
      hotspots: { scope: "src", count: 0, items: [] },
    };

    for (const [id, body] of Object.entries(sampleBodies)) {
      const block = { id, title: id, body };
      expect(() => viewBlockSchema.parse(block), `id=${id}`).not.toThrow();
    }
  });

  it("rejects a body that does not match the discriminator", () => {
    // Wrong body shape for `call_metrics` — needs numbers, got strings.
    const block = {
      id: "call_metrics",
      title: "metrics",
      body: { fan_in: "0", fan_out: "0" },
    };
    expect(() => viewBlockSchema.parse(block)).toThrow();
  });
});

// ============================================================================
// Request schemas
// ============================================================================
describe("request schemas", () => {
  it("openWorkspaceRequestSchema accepts a root_path", () => {
    expect(() =>
      openWorkspaceRequestSchema.parse({ root_path: "/tmp" }),
    ).not.toThrow();
  });

  it("generateArtifactRequestSchema accepts known formats", () => {
    expect(() =>
      generateArtifactRequestSchema.parse({ format: "markdown" }),
    ).not.toThrow();
    expect(() =>
      generateArtifactRequestSchema.parse({ format: "json_replay" }),
    ).not.toThrow();
    expect(() =>
      generateArtifactRequestSchema.parse({ format: "html" }),
    ).not.toThrow();
  });

  it("generateArtifactRequestSchema rejects unknown formats", () => {
    expect(() =>
      generateArtifactRequestSchema.parse({ format: "xml" }),
    ).toThrow();
  });
});

describe("decisionArtifactSummarySchema", () => {
  it("accepts a summary", () => {
    expect(() =>
      decisionArtifactSummarySchema.parse(decisionArtifactFixture),
    ).not.toThrow();
  });
});

describe("healthResponseSchema", () => {
  it("accepts a healthy response", () => {
    expect(() =>
      healthResponseSchema.parse({ status: "ok", service: "x" }),
    ).not.toThrow();
  });
});

describe("inspectableObjectTypeSchema", () => {
  it("accepts all 10 known types", () => {
    const types = [
      "workspace",
      "scope",
      "symbol",
      "file",
      "module",
      "evidence",
      "decision_artifact",
      "quality_issue",
      "rule",
      "route",
    ] as const;
    for (const t of types) {
      expect(inspectableObjectTypeSchema.parse(t)).toBe(t);
    }
  });

  it("rejects an unknown type", () => {
    expect(() => inspectableObjectTypeSchema.parse("alien")).toThrow();
  });
});

// ============================================================================
// T17 — multimodal node / edge style class enums (Zod).
// ============================================================================
//
// RED gate: the new 4+4 multimodal buckets are appended to the
// existing 3+3 buckets in `schemas.ts` and the Zod enums here
// mirror them. The string values are the wire contract from
// `cognicode-explorer::api::style_class_for` /
// `cognicode-explorer::api::edge_style_class_for`.

import {
  graphNodeStyleClassSchema,
  graphEdgeStyleClassSchema,
  nodeKindSchema,
  edgeKindSchema,
} from "./schemas";

describe("graphNodeStyleClassSchema — T17 multimodal", () => {
  it("accepts all 7 known node classes (3 legacy + 4 multimodal)", () => {
    const classes = [
      "function",
      "module",
      "external",
      "node-decision",
      "node-doc",
      "node-issue",
      "node-evidence",
    ] as const;
    for (const c of classes) {
      expect(graphNodeStyleClassSchema.parse(c)).toBe(c);
    }
  });

  it("rejects an unknown node class", () => {
    expect(() => graphNodeStyleClassSchema.parse("node-unknown")).toThrow();
  });

  it("the legacy 3 buckets still parse (regression)", () => {
    for (const c of ["function", "module", "external"] as const) {
      expect(graphNodeStyleClassSchema.parse(c)).toBe(c);
    }
  });

  it("accepts node-code (C4 code node — ADR-039)", () => {
    expect(graphNodeStyleClassSchema.parse("node-code")).toBe("node-code");
  });

  it("accepts all landing-page kinds (entry-point, hot, god)", () => {
    for (const c of ["entry-point", "hot", "god"] as const) {
      expect(graphNodeStyleClassSchema.parse(c)).toBe(c);
    }
  });
});

describe("graphEdgeStyleClassSchema — T17 multimodal", () => {
  it("accepts all 7 known edge classes (3 legacy + 4 multimodal)", () => {
    const classes = [
      "edge.calls",
      "edge.implements",
      "edge.uses",
      "edge-cites",
      "edge-justifies",
      "edge-resolves",
      "edge-corroborated",
    ] as const;
    for (const c of classes) {
      expect(graphEdgeStyleClassSchema.parse(c)).toBe(c);
    }
  });

  it("rejects an unknown edge class", () => {
    expect(() => graphEdgeStyleClassSchema.parse("edge-unknown")).toThrow();
  });

  it("the legacy 3 buckets still parse (regression)", () => {
    for (const c of ["edge.calls", "edge.implements", "edge.uses"] as const) {
      expect(graphEdgeStyleClassSchema.parse(c)).toBe(c);
    }
  });

  it("accepts C4 edge variants (edge-part-of, edge-deployed-as, edge-in-system)", () => {
    for (const e of ["edge-part-of", "edge-deployed-as", "edge-in-system"] as const) {
      expect(graphEdgeStyleClassSchema.parse(e)).toBe(e);
    }
  });
});

describe("nodeKindSchema — T17 multimodal node kinds", () => {
  it("accepts all 5 known node kinds", () => {
    const kinds = ["symbol", "decision", "doc", "issue", "evidence"] as const;
    for (const k of kinds) {
      expect(nodeKindSchema.parse(k)).toBe(k);
    }
  });

  it("rejects an unknown kind", () => {
    expect(() => nodeKindSchema.parse("widget")).toThrow();
  });
});

describe("edgeKindSchema — T17 multimodal edge kinds", () => {
  it("accepts all 5 known edge kinds", () => {
    const kinds = [
      "dependency",
      "cites",
      "justifies",
      "resolves",
      "corroborated_by",
    ] as const;
    for (const k of kinds) {
      expect(edgeKindSchema.parse(k)).toBe(k);
    }
  });

  it("rejects an unknown kind", () => {
    expect(() => edgeKindSchema.parse("links_to")).toThrow();
  });
});

// ============================================================================
// Phase 0: Moldable View Runtime — ViewKind / RendererKind / HierarchyKind
// ============================================================================

import {
  viewKindSchema,
  rendererKindSchema,
  hierarchyKindSchema,
  dataSourceSchema,
  transformSchema,
  viewSpecSchema,
} from "./schemas";

describe("viewKindSchema — ADR-008 catalog", () => {
  it("accepts every known ViewKind string", () => {
    const known = [
      "vertical_slice",
      "call_graph",
      "seam_map",
      "dependency_graph",
      "source_view",
      "data_flow",
      "impact_radius",
      "diff_view",
      "c4_context",
      "c4_container",
      "c4_component",
      "c4_code",
      "quality_hotspots",
      "evidence_view",
      "decision_graph",
      "architecture_rationale",
      "architecture_drift",
      "boundary_map",
      "dependency_pressure",
      "change_impact_story",
      "ownership_map",
      "risk_map",
      "decision_trace",
      "test_slice",
      "debug_slice",
      "refactor_plan",
      "callers_and_implementors",
      "usage_examples",
      "api_surface",
      "dead_code_candidates",
      "semantic_search_results",
      "doc_code_alignment",
      "example_object",
      "composed_narrative",
      "project_diary",
      "concept_map",
      "evidence_pack",
    ] as const;
    for (const vk of known) {
      expect(viewKindSchema.parse(vk)).toBe(vk);
    }
  });

  it("accepts an unknown ViewKind string (forward compatibility)", () => {
    // The Rust side maps unknown values to `ViewKind::Custom`.
    expect(viewKindSchema.parse("future_ai_view")).toBe("future_ai_view");
  });
});

describe("rendererKindSchema", () => {
  it("accepts every known RendererKind string", () => {
    const known = [
      "graph",
      "table",
      "tree",
      "code",
      "markdown",
      "vega_lite",
      "json",
      "composite",
    ] as const;
    for (const rk of known) {
      expect(rendererKindSchema.parse(rk)).toBe(rk);
    }
  });

  it("accepts an unknown RendererKind string (forward compatibility)", () => {
    expect(rendererKindSchema.parse("future_renderer")).toBe("future_renderer");
  });
});

describe("hierarchyKindSchema", () => {
  it("accepts every known HierarchyKind string", () => {
    const known = [
      "file_tree",
      "module_tree",
      "type_hierarchy",
      "call_hierarchy",
      "package_graph",
      "c4_hierarchy",
    ] as const;
    for (const hk of known) {
      expect(hierarchyKindSchema.parse(hk)).toBe(hk);
    }
  });

  it("accepts an unknown HierarchyKind string (forward compatibility)", () => {
    expect(hierarchyKindSchema.parse("experimental_x")).toBe("experimental_x");
  });
});

describe("dataSourceSchema — Moldql + permissive Other", () => {
  it("accepts a Moldql data source", () => {
    const ds = { kind: "moldql", query: "symbols where fan_out > 5" };
    expect(() => dataSourceSchema.parse(ds)).not.toThrow();
    const parsed = dataSourceSchema.parse(ds) as { kind: "moldql"; query: string };
    expect(parsed.kind).toBe("moldql");
    expect(parsed.query).toBe("symbols where fan_out > 5");
  });

  it("accepts an unknown data source kind (forward compatibility)", () => {
    // Unknown `kind` falls through to the permissive Other arm.
    const ds = { kind: "graphql", endpoint: "http://example.com/graphql" };
    expect(() => dataSourceSchema.parse(ds)).not.toThrow();
    const parsed = dataSourceSchema.parse(ds) as { kind: string };
    expect(parsed.kind).toBe("graphql");
  });
});

describe("transformSchema — Jsonata + permissive Other", () => {
  it("accepts a Jsonata transform", () => {
    const t = { kind: "jsonata", expression: "$.data" };
    expect(() => transformSchema.parse(t)).not.toThrow();
    const parsed = transformSchema.parse(t) as { kind: "jsonata"; expression: string };
    expect(parsed.kind).toBe("jsonata");
    expect(parsed.expression).toBe("$.data");
  });

  it("accepts an unknown transform kind (forward compatibility)", () => {
    const t = { kind: "custom_transform", config: {} };
    expect(() => transformSchema.parse(t)).not.toThrow();
  });
});

describe("viewSpecSchema — full ViewSpec DTO", () => {
  // Use a valid UUID v4 format (version digit = 4)
  const minimalViewSpec = {
    id: "a1b2c3d4-e5f6-4789-a123-456789abcdef",
    title: "Hot Symbols",
    applies_to: "symbol",
    view_kind: "quality_hotspots",
    data_source: { kind: "moldql", query: "symbols where fan_out > 5" },
    renderer_kind: "table",
    props: {},
    created_at: "2026-06-12T00:00:00Z",
    updated_at: "2026-06-12T00:00:00Z",
  };

  it("accepts a valid ViewSpec", () => {
    expect(() => viewSpecSchema.parse(minimalViewSpec)).not.toThrow();
  });

  it("accepts a ViewSpec with optional transform", () => {
    const withTransform = {
      ...minimalViewSpec,
      transform: { kind: "jsonata", expression: "$.data" },
    };
    expect(() => viewSpecSchema.parse(withTransform)).not.toThrow();
  });

  it("accepts a ViewSpec with null transform (explicit)", () => {
    const withNull = { ...minimalViewSpec, transform: null };
    expect(() => viewSpecSchema.parse(withNull)).not.toThrow();
  });

  it("accepts a ViewSpec with future view_kind (forward compatibility)", () => {
    const withFuture = { ...minimalViewSpec, view_kind: "future_ai_view" };
    expect(() => viewSpecSchema.parse(withFuture)).not.toThrow();
  });

  it("accepts a ViewSpec with future renderer_kind (forward compatibility)", () => {
    const withFuture = { ...minimalViewSpec, renderer_kind: "future_renderer" };
    expect(() => viewSpecSchema.parse(withFuture)).not.toThrow();
  });

  it("rejects an empty title", () => {
    const broken = { ...minimalViewSpec, title: "" };
    expect(() => viewSpecSchema.parse(broken)).toThrow();
  });

  it("rejects a missing id", () => {
    const { id: _id, ...rest } = minimalViewSpec;
    void _id;
    expect(() => viewSpecSchema.parse(rest)).toThrow();
  });

  it("rejects an invalid UUID", () => {
    const broken = { ...minimalViewSpec, id: "not-a-uuid" };
    expect(() => viewSpecSchema.parse(broken)).toThrow();
  });
});
