/**
 * Architecture view fixtures for MSW handlers + tests.
 *
 * Mirrors the `ArchitecturePayload` shape (SubgraphResponse with
 * node-component nodes and part_of edges).
 */
import type { ArchitecturePayload } from "../api/types";

export const architectureFixture: ArchitecturePayload = {
  root: "architecture",
  nodes: [
    {
      id: "system:cognicode",
      label: "CogniCode",
      kind: "system",
      file: undefined,
      line: undefined,
      style_class: "node-system",
    },
    {
      id: "container:api",
      label: "cognicode-explorer (API)",
      kind: "container",
      file: undefined,
      line: undefined,
      style_class: "node-container",
    },
    {
      id: "container:ui",
      label: "explorer-ui",
      kind: "container",
      file: undefined,
      line: undefined,
      style_class: "node-container",
    },
    {
      id: "component:crates",
      label: "crates",
      kind: "component",
      file: undefined,
      line: undefined,
      style_class: "node-component",
    },
    {
      id: "component:crates/cognicode-explorer",
      label: "cognicode-explorer",
      kind: "component",
      file: undefined,
      line: undefined,
      style_class: "node-component",
    },
    {
      id: "component:crates/cognicode-core",
      label: "cognicode-core",
      kind: "component",
      file: undefined,
      line: undefined,
      style_class: "node-component",
    },
    {
      id: "component:apps",
      label: "apps",
      kind: "component",
      file: undefined,
      line: undefined,
      style_class: "node-component",
    },
    {
      id: "component:apps/explorer-ui",
      label: "explorer-ui",
      kind: "component",
      file: undefined,
      line: undefined,
      style_class: "node-component",
    },
    {
      id: "code:fn:analyze",
      label: "analyze()",
      kind: "code",
      file: undefined,
      line: undefined,
      style_class: "node-code",
    },
  ],
  edges: [
    {
      source: "container:api",
      target: "system:cognicode",
      relation: "part_of",
      style_class: "edge-part-of",
    },
    {
      source: "container:ui",
      target: "system:cognicode",
      relation: "part_of",
      style_class: "edge-part-of",
    },
    {
      source: "component:crates",
      target: "container:api",
      relation: "part_of",
      style_class: "edge-part-of",
    },
    {
      source: "component:crates/cognicode-explorer",
      target: "component:crates",
      relation: "part_of",
      style_class: "edge-part-of",
    },
    {
      source: "component:crates/cognicode-core",
      target: "component:crates",
      relation: "part_of",
      style_class: "edge-part-of",
    },
    {
      source: "component:apps/explorer-ui",
      target: "component:apps",
      relation: "part_of",
      style_class: "edge-part-of",
    },
    {
      source: "code:fn:analyze",
      target: "component:crates/cognicode-core",
      relation: "part_of",
      style_class: "edge-part-of",
    },
  ],
  truncated: false,
  truncated_reason: null,
  corroboration_scores: {},
};
