/**
 * Subgraph response schemas — wire contract mirroring the Rust
 * `SubgraphResponse` / `GraphNode` / `GraphEdge` DTOs in
 * `crates/cognicode-explorer/src/dto.rs`.
 *
 * TDD: these tests are RED until the matching schemas land in
 * `schemas.ts`. The `style_class` enums are the strict
 * cytoscape-taxonomy buckets that the Rust `style_class_for` /
 * `edge_style_class_for` helpers produce.
 */
import { describe, expect, it } from "vitest";

import {
  graphNodeSchema,
  graphEdgeSchema,
  subgraphResponseSchema,
} from "./schemas";

const baseNode = {
  id: "sym:foo::bar",
  label: "bar",
  kind: "function",
  file: "foo.rs",
  line: 10,
  style_class: "function",
};

const baseEdge = {
  source: "sym:foo::bar",
  target: "sym:foo::baz",
  relation: "calls",
  style_class: "edge.calls",
};

const baseResponse = {
  root: "sym:foo::bar",
  nodes: [baseNode],
  edges: [baseEdge],
  truncated: false,
};

describe("subgraphResponseSchema", () => {
  it("parses a valid SubgraphResponse", () => {
    expect(() => subgraphResponseSchema.parse(baseResponse)).not.toThrow();
  });

  it("accepts every known node style_class bucket", () => {
    for (const cls of ["function", "module", "external"]) {
      const n = { ...baseNode, id: `sym:n:${cls}`, style_class: cls };
      expect(() =>
        subgraphResponseSchema.parse({ ...baseResponse, nodes: [n], edges: [] }),
      ).not.toThrow();
    }
  });

  it("rejects an unknown node style_class", () => {
    const n = { ...baseNode, style_class: "alien" };
    const result = subgraphResponseSchema.safeParse({
      ...baseResponse,
      nodes: [n],
      edges: [],
    });
    expect(result.success).toBe(false);
    if (!result.success) {
      // The error path must point at nodes.0.style_class.
      const path = result.error.issues[0]?.path.join(".");
      expect(path).toBe("nodes.0.style_class");
    }
  });

  it("accepts every known edge style_class bucket", () => {
    for (const cls of ["edge.calls", "edge.implements", "edge.uses"]) {
      const e = { ...baseEdge, style_class: cls };
      expect(() =>
        subgraphResponseSchema.parse({ ...baseResponse, edges: [e] }),
      ).not.toThrow();
    }
  });

  it("accepts a response without truncated_reason", () => {
    // truncated_reason is optional + nullable; absence is fine.
    const r = { ...baseResponse };
    delete (r as { truncated_reason?: unknown }).truncated_reason;
    expect(() => subgraphResponseSchema.parse(r)).not.toThrow();
  });

  it("accepts truncated_reason: null", () => {
    const r = { ...baseResponse, truncated_reason: null };
    expect(() => subgraphResponseSchema.parse(r)).not.toThrow();
  });
});

describe("graphNodeSchema (standalone)", () => {
  it("accepts a node without optional file/line", () => {
    const n = { ...baseNode, style_class: "function" };
    delete (n as { file?: unknown }).file;
    delete (n as { line?: unknown }).line;
    expect(() => graphNodeSchema.parse(n)).not.toThrow();
  });
});

describe("graphEdgeSchema (standalone)", () => {
  it("requires source and target", () => {
    expect(() => graphEdgeSchema.parse(baseEdge)).not.toThrow();
    const broken = { ...baseEdge, target: undefined };
    expect(() => graphEdgeSchema.parse(broken)).toThrow();
  });
});
