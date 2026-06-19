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
  ],
  edges: [
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
  ],
  truncated: false,
  truncated_reason: null,
  corroboration_scores: {},
};
