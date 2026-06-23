/**
 * Landing page fixtures for MSW handlers + tests.
 *
 * Mirrors the `LandingPayload` shape from
 * `crates/cognicode-explorer/src/dto.rs`.
 */
import type { LandingPayload } from "../api/types";

export const landingFixture: LandingPayload = {
  workspace: {
    id: "ws-cognicode-001",
    root_path: "/var/home/rubentxu/Proyectos/rust/CogniCode",
    graph_status: "ready",
    indexed_at: "2026-06-07T10:11:12Z",
    symbol_count: 1240,
    relation_count: 4312,
    last_scan_at: "2026-06-07T10:11:12Z",
  },
  nodes: [
    {
      id: "symbol:crates/cognicode-explorer/src/lib.rs:build_overview:16",
      label: "build_overview",
      kind: "function",
      file: "crates/cognicode-explorer/src/lib.rs",
      line: 16,
      style_class: "function",
    },
    {
      id: "symbol:crates/cognicode-explorer/src/api.rs:spotter:86",
      label: "spotter",
      kind: "function",
      file: "crates/cognicode-explorer/src/api.rs",
      line: 86,
      style_class: "function",
    },
    {
      id: "file:crates/cognicode-explorer/src/lib.rs",
      label: "crates/cognicode-explorer/src/lib.rs",
      kind: "file",
      file: "crates/cognicode-explorer/src/lib.rs",
      style_class: "module",
    },
  ],
  edges: [
    {
      source: "symbol:crates/cognicode-explorer/src/lib.rs:build_overview:16",
      target: "symbol:crates/cognicode-explorer/src/api.rs:spotter:86",
      relation: "calls",
      style_class: "edge.calls",
    },
    {
      source: "symbol:crates/cognicode-explorer/src/api.rs:spotter:86",
      target: "file:crates/cognicode-explorer/src/lib.rs",
      relation: "lives_in",
      style_class: "edge.calls",
    },
  ],
  entry_points: [
    {
      id: "symbol:crates/cognicode-explorer/src/lib.rs:build_overview:16",
      object_type: "symbol",
      label: "build_overview",
      subtitle: "crates/cognicode-explorer/src/lib.rs:16",
      properties: [
        { key: "kind", value: "function", value_type: "string", source: "static" },
        { key: "visibility", value: "pub", value_type: "string", source: "static" },
      ],
      available_views: [
        { id: "overview", title: "Overview", is_builtin: true, source: null },
        { id: "call-graph", title: "Call graph", is_builtin: true, source: null },
      ],
    },
    {
      id: "symbol:crates/cognicode-explorer/src/api.rs:spotter:86",
      object_type: "symbol",
      label: "spotter",
      subtitle: "crates/cognicode-explorer/src/api.rs:86",
      properties: [
        { key: "kind", value: "function", value_type: "string", source: "static" },
        { key: "visibility", value: "pub", value_type: "string", source: "static" },
      ],
      available_views: [
        { id: "overview", title: "Overview", is_builtin: true, source: null },
      ],
    },
  ],
  hot_paths: [
    {
      id: "symbol:crates/cognicode-explorer/src/lib.rs:build_overview:16",
      object_type: "symbol",
      label: "build_overview",
      subtitle: "crates/cognicode-explorer/src/lib.rs:16 (hot)",
      properties: [
        { key: "kind", value: "function", value_type: "string", source: "static" },
        { key: "fan_in", value: 12, value_type: "number", source: "static" },
      ],
      available_views: [
        { id: "overview", title: "Overview", is_builtin: true, source: null },
      ],
    },
  ],
  god_nodes: [
    {
      id: "symbol:crates/cognicode-explorer/src/lib.rs:build_overview:16",
      label: "build_overview",
      score: 0.95,
    },
    {
      id: "symbol:crates/cognicode-explorer/src/api.rs:spotter:86",
      label: "spotter",
      score: 0.87,
    },
  ],
  suggested_questions: [
    "What are the main entry points in this workspace?",
    "Show me the hot paths and god nodes",
    "What modules have the most dependencies?",
  ],
  graph_status: "ready",
};
