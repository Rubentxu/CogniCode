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
  ExplorationPath,
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
    { id: "overview", title: "Overview" },
    { id: "call-graph", title: "Call graph" },
    { id: "source", title: "Source" },
    { id: "quality", title: "Quality" },
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

export const explorationPathFixture: ExplorationPath = {
  id: "exploration-001",
  workspace_id: WORKSPACE_ID,
  columns: [
    { object_id: SCOPE_ID, active_view: "overview" },
    { object_id: FILE_ID, active_view: "overview" },
    { object_id: SYMBOL_ID, active_view: "overview" },
  ],
  objects: [
    { id: SYMBOL_ID, object_type: "symbol", natural_key: "crates/cognicode-explorer/src/lib.rs:build_overview:16", first_seen: "2026-06-07T12:00:00Z" },
  ],
  lens: "lens.hotspots",
  created_at: "2026-06-07T12:00:00Z",
};

export const decisionArtifactFixture: DecisionArtifactSummary = {
  id: "artifact-001",
  format: "markdown",
  title: "build_overview exploration",
  content: "# Exploration\n\n**Lens:** hotspots\n\n- [build_overview] hotspots, warning (0.78)",
};
