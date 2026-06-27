/**
 * Realistic fixture data for the MSW handlers + tests.
 *
 * Mirrors the shapes produced by `crates/cognicode-explorer/src/dto.rs`
 * and `domain/views.rs`. Kept hand-rolled (no auto-generation) so
 * the fixtures double as documentation of what the wire looks like.
 *
 * The data is small but representative — enough to drive a view
 * render and trigger every block id in the discriminated union.
 */
import type {
  ContextualView,
  DecisionArtifactSummary,
  DesignFinding,
  ExplorationSessionDto,
  InspectableObjectSummary,
  LensDescriptor,
  LensResult,
  SpotterResult,
  ViewBlock,
  WorkspaceSummary,
} from "../api/types";

const WORKSPACE_ID = "ws-cognicode-001";
const SYMBOL_ID = "symbol:crates/cognicode-explorer/src/lib.rs:build_overview:16";
const FILE_ID = "file:crates/cognicode-explorer/src/lib.rs";
const SCOPE_ID = "scope:crates/cognicode-explorer/src";

// ============================================================================
// Workspace + spotter
// ============================================================================

export const workspaceSummaryFixture: WorkspaceSummary = {
  id: WORKSPACE_ID,
  root_path: "/var/home/rubentxu/Proyectos/rust/CogniCode",
  graph_status: "ready",
  indexed_at: "2026-06-07T10:11:12Z",
  symbol_count: 1240,
  relation_count: 4312,
  last_scan_at: "2026-06-07T10:11:12Z",
};

export const inspectableObjectFixture: InspectableObjectSummary = {
  id: SYMBOL_ID,
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
    { id: "source", title: "Source", is_builtin: true, source: null },
    { id: "quality", title: "Quality", is_builtin: true, source: null },
    // e12a–e12e Phase 1 executors
    { id: "usage-examples", title: "Usage Examples", is_builtin: true, source: null },
    { id: "test-slice", title: "Test Slice", is_builtin: true, source: null },
    { id: "debug-slice", title: "Debug Slice", is_builtin: true, source: null },
    { id: "change-impact-story", title: "Change Impact Story", is_builtin: true, source: null },
  ],
};

/** Scope fixture for api-surface view (e12b) — a crate/module scope */
export const inspectableScopeFixture: InspectableObjectSummary = {
  id: SCOPE_ID,
  object_type: "scope",
  label: "cognicode-explorer",
  subtitle: "crates/cognicode-explorer/src",
  properties: [
    { key: "kind", value: "crate", value_type: "string", source: "static" },
    { key: "symbol_count", value: 60, value_type: "number", source: "static" },
    { key: "file_count", value: 12, value_type: "number", source: "static" },
  ],
  available_views: [
    { id: "overview", title: "Overview", is_builtin: true, source: null },
    { id: "api-surface", title: "API Surface", is_builtin: true, source: null },
  ],
};

export const spotterResultsFixture: SpotterResult[] = [
  {
    object: inspectableObjectFixture,
    score: 0.97,
    match_type: "name_exact",
  },
  {
    object: {
      ...inspectableObjectFixture,
      id: `${SYMBOL_ID}-second`,
      label: "build_callgraph",
      subtitle: "crates/cognicode-explorer/src/lib.rs:62",
    },
    score: 0.81,
    match_type: "name_prefix",
  },
  // e12b: scope result for api-surface test
  {
    object: inspectableScopeFixture,
    score: 0.75,
    match_type: "name_prefix",
  },
];

// ============================================================================
// Views
// ============================================================================

/**
 * A representative ContextualView exercising every block id we ship
 * typed renderers for. The UI iterates `view.blocks` and switches on
 * `block.id`; this fixture covers all the branches.
 */
export const contextualViewFixture: ContextualView = {
  object_id: SYMBOL_ID,
  view_id: "overview",
  title: "Overview",
  view_kind: "vertical_slice",
  renderer_kind: "composite",
  blocks: [
    {
      id: "identity",
      title: "Identity",
      body: { name: "build_overview", kind: "function", file: "crates/cognicode-explorer/src/lib.rs", line: 16 },
    },
    {
      id: "call_metrics",
      title: "Call metrics",
      body: { fan_in: 3, fan_out: 4 },
    },
    {
      id: "signature",
      title: "Signature",
      body: { signature: "fn build_overview(symbol: &ResolvedSymbol, repo: &dyn SymbolRepository) -> ContextualView" },
    },
    {
      id: "callers",
      title: "Callers (1)",
      body: {
        count: 1,
        items: [
          {
            object_id: "symbol:crates/cognicode-explorer/src/lib.rs:explore:42",
            name: "explore",
            kind: "function",
            file: "crates/cognicode-explorer/src/lib.rs",
            line: 42,
          },
        ],
      },
    },
    {
      id: "callees",
      title: "Callees (2)",
      body: {
        count: 2,
        items: [
          {
            object_id: "symbol:crates/cognicode-explorer/src/lib.rs:fan_in:120",
            name: "fan_in",
            kind: "function",
            file: "crates/cognicode-explorer/src/lib.rs",
            line: 120,
          },
          {
            object_id: "symbol:crates/cognicode-explorer/src/lib.rs:fan_out:124",
            name: "fan_out",
            kind: "function",
            file: "crates/cognicode-explorer/src/lib.rs",
            line: 124,
          },
        ],
      },
    },
    {
      id: "source_slice",
      title: "Source slice (lines 9–24)",
      body: {
        file: "crates/cognicode-explorer/src/lib.rs",
        line: 16,
        lines: [
          { line: 9, text: "/// Build the Overview view: identity + call graph metrics + signature for callables." },
          { line: 10, text: "pub fn build_overview(symbol: &ResolvedSymbol, repo: &dyn SymbolRepository) -> ContextualView {" },
          { line: 16, text: "    let mut blocks: Vec<ViewBlock> = Vec::new();" },
        ],
      },
    },
    {
      id: "symbol_quality_identity",
      title: "Quality",
      body: { file: "crates/cognicode-explorer/src/lib.rs", line: 16, issue_count: 0 },
    },
    {
      id: "symbol_quality_issues",
      title: "Issues at this location (0)",
      body: { count: 0, items: [] },
    },
    {
      id: "issue_identity",
      title: "Issue",
      body: { id: 7, rule_id: "rust:S100", severity: "warning", category: "naming", status: "open" },
    },
    {
      id: "issue_location",
      title: "Location",
      body: { file: "crates/cognicode-explorer/src/lib.rs", line: 16 },
    },
    {
      id: "issue_message",
      title: "Message",
      body: { message: "Function name should be camelCase" },
    },
    {
      id: "rule_identity",
      title: "Rule",
      body: { rule_id: "rust:S100", description: "Function naming convention", open_count: 3 },
    },
    {
      id: "rule_related",
      title: "Related issues (1)",
      body: {
        count: 1,
        items: [
          {
            id: 7,
            rule_id: "rust:S100",
            severity: "warning",
            category: "naming",
            file: "crates/cognicode-explorer/src/lib.rs",
            line: 16,
            message: "Function name should be camelCase",
            status: "open",
            object_id: "issue:7",
          },
        ],
      },
    },
    {
      id: "file_identity",
      title: "File",
      body: { path: "crates/cognicode-explorer/src/lib.rs", line_count: 320, symbol_count: 14 },
    },
    {
      id: "kinds",
      title: "Symbol kinds",
      body: { breakdown: { function: 11, struct: 2, enum: 1 } },
    },
    {
      id: "symbols",
      title: "Symbols in crates/cognicode-explorer/src/lib.rs (1)",
      body: {
        count: 1,
        items: [
          { name: "build_overview", kind: "function", line: 16, object_id: SYMBOL_ID },
        ],
      },
    },
    {
      id: "file_quality_identity",
      title: "Quality",
      body: { path: "crates/cognicode-explorer/src/lib.rs", issue_count: 1 },
    },
    {
      id: "file_quality_issues",
      title: "Issues in this file (1)",
      body: {
        count: 1,
        items: [
          {
            id: 7,
            rule_id: "rust:S100",
            severity: "warning",
            category: "naming",
            file: "crates/cognicode-explorer/src/lib.rs",
            line: 16,
            message: "Function name should be camelCase",
            status: "open",
            object_id: "issue:7",
          },
        ],
      },
    },
    {
      id: "file_quality_gate",
      title: "Quality gate",
      body: {
        rating: "B",
        total_issues: 12,
        blockers: 0,
        criticals: 1,
        debt_minutes: 84,
        last_run: "2026-06-07T09:00:00Z",
      },
    },
    {
      id: "scope_identity",
      title: "Scope",
      body: { path: "crates/cognicode-explorer/src", file_count: 4, symbol_count: 60, promotion_ready: false },
    },
    {
      id: "scope_kinds",
      title: "Symbol kinds",
      body: { breakdown: { function: 48, struct: 7, enum: 5 } },
    },
    {
      id: "scope_files",
      title: "Member files",
      body: { files: ["crates/cognicode-explorer/src/lib.rs", "crates/cognicode-explorer/src/api.rs", "crates/cognicode-explorer/src/dto.rs"] },
    },
    {
      id: "scope_quality_identity",
      title: "Quality",
      body: {
        scope: "crates/cognicode-explorer/src",
        issue_count: 5,
        by_severity: { warning: 4, critical: 1 },
      },
    },
    {
      id: "scope_quality_gate",
      title: "Quality gate",
      body: {
        rating: "B",
        total_issues: 12,
        blockers: 0,
        criticals: 1,
        debt_minutes: 84,
        last_run: "2026-06-07T09:00:00Z",
      },
    },
    {
      id: "scope_quality_issues",
      title: "Issues in this scope (1)",
      body: { count: 1, items: [] },
    },
    {
      id: "cross_scope",
      title: "Cross-scope relations (2)",
      body: {
        scope: "crates/cognicode-explorer/src",
        file_count: 4,
        symbol_count: 60,
        entries: [
          { scope: "crates/cognicode-core/src", outgoing_count: 7, incoming_count: 1 },
          { scope: "crates/cognicode-db/src", outgoing_count: 0, incoming_count: 4 },
        ],
      },
    },
    {
      id: "hotspots",
      title: "Top hotspots (3)",
      body: {
        scope: "crates/cognicode-explorer/src",
        count: 3,
        items: [
          { name: "build_overview", kind: "function", file: "crates/cognicode-explorer/src/lib.rs", line: 16, object_id: SYMBOL_ID },
          { name: "spotter", kind: "function", file: "crates/cognicode-explorer/src/api.rs", line: 86, object_id: "symbol:crates/cognicode-explorer/src/api.rs:spotter:86" },
          { name: "save_exploration", kind: "function", file: "crates/cognicode-explorer/src/api.rs", line: 134, object_id: "symbol:crates/cognicode-explorer/src/api.rs:save_exploration:134" },
        ],
      },
    },
  ] satisfies ViewBlock[],
  relations: [],
  evidence: [
    {
      id: "evidence:symbol_metadata",
      kind: "symbol_metadata",
      title: "Symbol metadata: build_overview",
      file: "crates/cognicode-explorer/src/lib.rs",
      line_range: { start: 16, end: 16 },
      source_tool_or_query: "CallGraphRepository::resolve",
      confidence: 1.0,
      freshness: "fresh",
    },
  ],
  findings: [],
};

// ============================================================================
// Lenses
// ============================================================================

export const lensDescriptorsFixture: LensDescriptor[] = [
  {
    id: "lens.callgraph",
    name: "Call graph",
    description: "Reveal incoming and outgoing relations with one click.",
    applicable_types: ["symbol", "file", "scope"],
  },
  {
    id: "lens.hotspots",
    name: "Hotspots",
    description: "Surface the highest fan-in symbols in a scope.",
    applicable_types: ["scope", "file"],
  },
  {
    id: "lens.dead-code",
    name: "Dead code",
    description: "Highlight unreferenced modules and unreachable symbols.",
    applicable_types: ["scope", "file"],
  },
  {
    id: "lens.quality",
    name: "Quality lens",
    description: "Bucket open quality issues by severity for a scope.",
    applicable_types: ["scope", "file", "symbol"],
  },
];

const designFindingFixture: DesignFinding = {
  id: "finding:1",
  lens_id: "lens.hotspots",
  title: "build_overview is a hotspot",
  hypothesis: "build_overview may be a hotspot — 12 callers across 5 modules",
  severity: "warning",
  confidence: 0.78,
  object_ids: [SYMBOL_ID],
  evidence_ids: ["evidence:scope_hotspots"],
};

export const lensResultFixture: LensResult = {
  lens_id: "lens.hotspots",
  findings: [designFindingFixture],
  summary: "Detected 1 hotspot with 3+ callers across the workspace",
};

// ============================================================================
// Explorations + artifacts
// ============================================================================

export const explorationSessionFixture: ExplorationSessionDto = {
  id: "exploration-001",
  workspace_id: WORKSPACE_ID,
  events: [
    { object_id: SCOPE_ID, view_id: "overview", query: null, ts: "2026-06-07T12:00:00Z" },
    { object_id: FILE_ID, view_id: "overview", query: null, ts: "2026-06-07T12:01:00Z" },
    { object_id: SYMBOL_ID, view_id: "overview", query: null, ts: "2026-06-07T12:02:00Z" },
  ],
  navigation_mode: "pane-stack",
  panes: [
    { pane_id: "pane-1", object_id: SCOPE_ID, view_id: "overview", scroll_y: 0, viewport: null },
    { pane_id: "pane-2", object_id: FILE_ID, view_id: "overview", scroll_y: 0, viewport: null },
    { pane_id: "pane-3", object_id: SYMBOL_ID, view_id: "overview", scroll_y: 0, viewport: null },
  ],
  created_at: "2026-06-07T12:00:00Z",
};

export const decisionArtifactFixture: DecisionArtifactSummary = {
  id: "artifact-001",
  format: "markdown",
  title: "build_overview exploration",
  content: "# Exploration\n\n**Lens:** hotspots\n\n- [build_overview] hotspots, warning (0.78)",
};

// ============================================================================
// Phase 1 executor fixtures (e12a–e12e)
// ============================================================================

/** e12a — UsageExamplesExecutor: callers + callees as Table blocks */
export const usageExamplesViewFixture = {
  object_id: SYMBOL_ID,
  view_id: "usage-examples",
  title: "Usage Examples",
  view_kind: "usage_examples",
  renderer_kind: "table",
  blocks: [
    {
      id: "callers",
      title: "Called by (2)",
      body: {
        columns: ["name", "file", "line", "kind"],
        rows: [
          { object_id: "symbol:crates/cognicode-explorer/src/lib.rs:explore:42", name: "explore", file: "crates/cognicode-explorer/src/lib.rs", line: 42, kind: "function" },
          { object_id: "symbol:crates/cognicode-explorer/src/lib.rs:fan_in:120", name: "fan_in", file: "crates/cognicode-explorer/src/lib.rs", line: 120, kind: "function" },
        ],
      },
    },
    {
      id: "callees",
      title: "Calls (3)",
      body: {
        columns: ["name", "file", "line", "kind"],
        rows: [
          { object_id: "symbol:crates/cognicode-explorer/src/lib.rs:build_symbols:55", name: "build_symbols", file: "crates/cognicode-explorer/src/lib.rs", line: 55, kind: "function" },
          { object_id: "symbol:crates/cognicode-explorer/src/lib.rs:fan_out:124", name: "fan_out", file: "crates/cognicode-explorer/src/lib.rs", line: 124, kind: "function" },
          { object_id: "symbol:crates/cognicode-explorer/src/lib.rs:page_rank:88", name: "page_rank", file: "crates/cognicode-explorer/src/lib.rs", line: 88, kind: "function" },
        ],
      },
    },
  ],
  relations: [],
  evidence: [],
  findings: [],
};

/** e12b — ApiSurfaceExecutor: all scope symbols as Table */
export const apiSurfaceViewFixture = {
  object_id: SCOPE_ID,
  view_id: "api-surface",
  title: "API Surface",
  view_kind: "api_surface",
  renderer_kind: "table",
  blocks: [
    {
      id: "api_surface",
      title: "crates/cognicode-explorer/src",
      body: {
        columns: ["name", "kind", "file", "line"],
        rows: [
          { name: "build_overview", kind: "function", file: "crates/cognicode-explorer/src/lib.rs", line: 16 },
          { name: "build_symbols", kind: "function", file: "crates/cognicode-explorer/src/lib.rs", line: 55 },
          { name: "fan_in", kind: "function", file: "crates/cognicode-explorer/src/lib.rs", line: 120 },
          { name: "fan_out", kind: "function", file: "crates/cognicode-explorer/src/lib.rs", line: 124 },
          { name: "page_rank", kind: "function", file: "crates/cognicode-explorer/src/lib.rs", line: 88 },
          { name: "CommunityDetection", kind: "struct", file: "crates/cognicode-explorer/src/lib.rs", line: 200 },
        ],
      },
    },
  ],
  relations: [],
  evidence: [],
  findings: [],
};

/** e12c — TestSliceExecutor: test callers of a symbol as Table */
export const testSliceViewFixture = {
  object_id: SYMBOL_ID,
  view_id: "test-slice",
  title: "Test Slice",
  view_kind: "test_slice",
  renderer_kind: "table",
  blocks: [
    {
      id: "test_slice",
      title: "Tests (2)",
      body: {
        columns: ["name", "file", "line", "kind"],
        rows: [
          { name: "test_build_overview", file: "crates/cognicode-explorer/src/lib_test.rs", line: 10, kind: "function" },
          { name: "test_overview_fan_in", file: "crates/cognicode-explorer/src/lib_test.rs", line: 45, kind: "function" },
        ],
      },
    },
  ],
  relations: [],
  evidence: [],
  findings: [],
};

/** e12d — DebugSliceExecutor: debug callers + callees as Graph + Table blocks */
export const debugSliceViewFixture = {
  object_id: SYMBOL_ID,
  view_id: "debug-slice",
  title: "Debug Slice",
  view_kind: "debug_slice",
  renderer_kind: "graph",
  blocks: [
    {
      id: "debug_callers",
      title: "Debug Callers (1)",
      body: {
        columns: ["name", "file", "line", "kind"],
        rows: [
          { name: "log_debug", file: "crates/cognicode-explorer/src/lib.rs", line: 77, kind: "function" },
        ],
      },
    },
    {
      id: "debug_callees",
      title: "Debug Callees (2)",
      body: {
        columns: ["name", "file", "line", "kind"],
        rows: [
          { name: "assert_eq", file: "crates/cognicode-explorer/src/lib.rs", line: 33, kind: "function" },
          { name: "dbg_trace", file: "crates/cognicode-explorer/src/lib.rs", line: 99, kind: "function" },
        ],
      },
    },
  ],
  relations: [
    {
      relation_type: "calls",
      direction: "outgoing",
      target_object_id: "symbol:crates/cognicode-explorer/src/lib.rs:assert_eq:33",
      target_label: "assert_eq",
      evidence_ids: [],
    },
    {
      relation_type: "calls",
      direction: "incoming",
      target_object_id: "symbol:crates/cognicode-explorer/src/lib.rs:log_debug:77",
      target_label: "log_debug",
      evidence_ids: [],
    },
  ],
  evidence: [
    {
      id: "evidence:debug_slice",
      kind: "debug_slice",
      title: "Debug slice: build_overview",
      file: "crates/cognicode-explorer/src/lib.rs",
      line_range: { start: 16, end: 16 },
      source_tool_or_query: "GraphQueryPort::callers/callees + is_debug_relevant",
      confidence: null,
      freshness: "unknown",
    },
  ],
  findings: [],
};

/** e12e — ChangeImpactStoryExecutor: BFS upstream + downstream as Table */
export const changeImpactStoryViewFixture = {
  object_id: SYMBOL_ID,
  view_id: "change-impact-story",
  title: "Change Impact Story",
  view_kind: "change_impact_story",
  renderer_kind: "table",
  blocks: [
    {
      id: "upstream",
      title: "Upstream — Who is affected by changes to `build_overview` (3)",
      body: {
        columns: ["name", "file", "line", "depth", "relationship"],
        rows: [
          { name: "explore", file: "crates/cognicode-explorer/src/lib.rs", line: 42, depth: 1, relationship: "direct caller" },
          { name: "fan_in", file: "crates/cognicode-explorer/src/lib.rs", line: 120, depth: 1, relationship: "direct caller" },
          { name: "analyze_workspace", file: "crates/cognicode-explorer/src/lib.rs", line: 200, depth: 2, relationship: "transitive caller" },
        ],
      },
    },
    {
      id: "downstream",
      title: "Downstream — What `build_overview` affects (3)",
      body: {
        columns: ["name", "file", "line", "depth", "relationship"],
        rows: [
          { name: "build_symbols", file: "crates/cognicode-explorer/src/lib.rs", line: 55, depth: 1, relationship: "direct callee" },
          { name: "page_rank", file: "crates/cognicode-explorer/src/lib.rs", line: 88, depth: 1, relationship: "direct callee" },
          { name: "graph_stats", file: "crates/cognicode-explorer/src/lib.rs", line: 150, depth: 2, relationship: "transitive callee" },
        ],
      },
    },
  ],
  relations: [],
  evidence: [
    {
      id: "evidence:change_impact_story",
      kind: "change_impact_story",
      title: "Change impact story: build_overview",
      file: "crates/cognicode-explorer/src/lib.rs",
      line_range: { start: 16, end: 16 },
      source_tool_or_query: "GraphQueryPort::traverse_callers/traverse_callees (max_depth=3)",
      confidence: null,
      freshness: "unknown",
    },
  ],
  findings: [],
};
