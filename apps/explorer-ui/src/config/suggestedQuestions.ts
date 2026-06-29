/**
 * `suggestedQuestions` — static, typed map of "What can I do here?" prompts.
 *
 * The map is the source of truth for contextual help. It pairs each
 * `InspectableObjectType` with 3-5 prompts that the user can fire
 * against the focused object. Prompts map to one of four MCP tools:
 *
 *   - `cognicode_ask`             → natural-language question
 *   - `explorer_inspect_object`   → drill into a related object
 *   - `explorer_get_view`         → switch to a different view of the
 *                                   focused object
 *   - `explorer_open_workspace`   → open a workspace
 *
 * The map is exhaustive over the 9 `InspectableObjectType` variants —
 * TypeScript enforces this at compile time via `Record<…, …>`. Adding
 * a 10th kind to the schema triggers a compile error here.
 *
 * Prompts may contain `{label}` and `{id}` placeholders. These are
 * substituted at click time by `useAsk` and the strip's
 * `onDispatch` path, NOT here. Keeping substitution out of the static
 * config means the config stays a pure data structure that is easy
 * to test as a fixture.
 */
import type { GraphStatus, InspectableObjectType } from "../api/types";

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export type InvestigationVerb = "understand" | "trace" | "compare" | "explain" | "save";

export type SuggestedTool =
  | "cognicode_ask"
  | "explorer_inspect_object"
  | "explorer_get_view"
  | "explorer_open_workspace";

export interface SuggestedQuestion {
  readonly id: string;
  readonly label: string;
  readonly tool: SuggestedTool;
  readonly params: Readonly<Record<string, string>>;
  readonly requiresGraph: boolean;
  readonly verb: InvestigationVerb;
  readonly isComingSoon?: boolean;
}

/** Uppercase label for each InvestigationVerb, e.g. "trace" → "TRACE" */
export function verbLabel(v: InvestigationVerb): string {
  return v.toUpperCase();
}

/**
 * Per-verb stub configuration — drives disabled-reason text in the UI.
 * `disabledReason` is shown when `isComingSoon=true`.
 * @ponytail: ceiling=E21-2/INV-2, upgrade=first save prompt added
 */
export const VERB_STUB_CONFIG: Readonly<Record<InvestigationVerb, { disabledReason?: string }>> = {
  understand: {},
  trace: {},
  compare: { disabledReason: "Available when C4 overlays exist" },
  explain: {},
  save: { disabledReason: "Available when investigations exist" },
} as const;

// ---------------------------------------------------------------------------
// Static map
// ---------------------------------------------------------------------------

export const SUGGESTED_QUESTIONS: {
  readonly [K in InspectableObjectType]: readonly SuggestedQuestion[];
} = {
  symbol: [
    {
      id: "who-calls",
      label: "Who calls this?",
      tool: "cognicode_ask",
      params: { question: "who calls `{label}`?" },
      requiresGraph: true,
      verb: "trace",
    },
    {
      id: "what-does-call",
      label: "What does this call?",
      tool: "cognicode_ask",
      params: { question: "what does `{label}` call?" },
      requiresGraph: true,
      verb: "trace",
    },
    {
      id: "risky-here",
      label: "What is risky to change here?",
      tool: "cognicode_ask",
      params: { question: "is `{label}` risky to change?" },
      requiresGraph: true,
      verb: "explain",
    },
    {
      id: "where-belongs",
      label: "Where does this belong?",
      tool: "cognicode_ask",
      params: { question: "where does `{label}` belong?" },
      requiresGraph: true,
      verb: "explain",
    },
    {
      id: "what-justifies",
      label: "What justifies this?",
      tool: "explorer_inspect_object",
      params: { object_id: "{id}" },
      requiresGraph: false,
      verb: "explain",
    },
  ],

  file: [
    {
      id: "in-file",
      label: "What is in this file?",
      tool: "explorer_inspect_object",
      params: { object_id: "{id}" },
      requiresGraph: false,
      verb: "understand",
    },
    {
      id: "risky-file",
      label: "What is risky in this file?",
      tool: "cognicode_ask",
      params: { question: "is `{label}` risky?" },
      requiresGraph: true,
      verb: "explain",
    },
    {
      id: "changed-file",
      label: "What changed in this file?",
      tool: "explorer_get_view",
      params: { view_id: "changelog" },
      requiresGraph: false,
      verb: "compare",
      isComingSoon: true,
    },
    {
      id: "file-where-belongs",
      label: "Where does this file belong?",
      tool: "explorer_inspect_object",
      params: { object_id: "{id}" },
      requiresGraph: false,
      verb: "explain",
    },
  ],

  scope: [
    {
      id: "lives-here",
      label: "What lives in this scope?",
      tool: "explorer_inspect_object",
      params: { object_id: "{id}" },
      requiresGraph: false,
      verb: "understand",
    },
    {
      id: "depends-on",
      label: "What depends on this scope?",
      tool: "cognicode_ask",
      params: { question: "who depends on `{label}`?" },
      requiresGraph: true,
      verb: "trace",
    },
    {
      id: "changed-scope",
      label: "What changed in this scope?",
      tool: "explorer_get_view",
      params: { view_id: "changelog" },
      requiresGraph: false,
      verb: "compare",
      isComingSoon: true,
    },
  ],

  module: [
    {
      id: "lives-here",
      label: "What lives in this module?",
      tool: "explorer_inspect_object",
      params: { object_id: "{id}" },
      requiresGraph: false,
      verb: "understand",
    },
    {
      id: "depends-on",
      label: "What depends on this module?",
      tool: "cognicode_ask",
      params: { question: "who depends on `{label}`?" },
      requiresGraph: true,
      verb: "trace",
    },
    {
      id: "changed-scope",
      label: "What changed in this module?",
      tool: "explorer_get_view",
      params: { view_id: "changelog" },
      requiresGraph: false,
      verb: "compare",
      isComingSoon: true,
    },
  ],

  workspace: [
    {
      id: "moving-parts",
      label: "What are the moving parts?",
      tool: "cognicode_ask",
      params: { question: "what are the moving parts of `{label}`?" },
      requiresGraph: false,
      verb: "understand",
    },
    {
      id: "shape",
      label: "What is the shape?",
      tool: "cognicode_ask",
      params: { question: "architecture shape of `{label}`?" },
      requiresGraph: true,
      verb: "understand",
    },
    {
      id: "where-start",
      label: "Where do I start?",
      tool: "cognicode_ask",
      params: { question: "where to start in `{label}`?" },
      requiresGraph: true,
      verb: "understand",
    },
  ],

  evidence: [
    {
      id: "inspect-context",
      label: "Inspect this in context",
      tool: "explorer_inspect_object",
      params: { object_id: "{id}" },
      requiresGraph: false,
      verb: "understand",
    },
    {
      id: "cites",
      label: "What cites this evidence?",
      tool: "explorer_get_view",
      params: { view_id: "cited-by" },
      requiresGraph: false,
      verb: "trace",
    },
    {
      id: "justifies",
      label: "What does this justify?",
      tool: "explorer_get_view",
      params: { view_id: "justifies" },
      requiresGraph: false,
      verb: "explain",
    },
  ],

  decision_artifact: [
    {
      id: "what-justifies",
      label: "What does this justify?",
      tool: "explorer_inspect_object",
      params: { object_id: "{id}" },
      requiresGraph: false,
      verb: "explain",
    },
    {
      id: "contradicts",
      label: "What contradicts this?",
      tool: "explorer_get_view",
      params: { view_id: "evidence" },
      requiresGraph: false,
      verb: "compare",
    },
    {
      id: "inspect-context",
      label: "Inspect in context",
      tool: "explorer_inspect_object",
      params: { object_id: "{id}" },
      requiresGraph: false,
      verb: "understand",
    },
  ],

  quality_issue: [
    {
      id: "resolves",
      label: "What does this resolve?",
      tool: "explorer_get_view",
      params: { view_id: "resolves" },
      requiresGraph: false,
      verb: "understand",
    },
    {
      id: "cites",
      label: "What cites this issue?",
      tool: "explorer_get_view",
      params: { view_id: "cited-by" },
      requiresGraph: false,
      verb: "trace",
    },
    {
      id: "inspect-context",
      label: "Inspect this issue",
      tool: "explorer_inspect_object",
      params: { object_id: "{id}" },
      requiresGraph: false,
      verb: "understand",
    },
  ],

  rule: [
    {
      id: "inspect-rule",
      label: "Inspect this rule",
      tool: "explorer_inspect_object",
      params: { object_id: "{id}" },
      requiresGraph: false,
      verb: "understand",
    },
    {
      id: "violations",
      label: "What violations cite this rule?",
      tool: "explorer_get_view",
      params: { view_id: "violations" },
      requiresGraph: false,
      verb: "trace",
    },
    {
      id: "examples",
      label: "What are examples?",
      tool: "explorer_get_view",
      params: { view_id: "examples" },
      requiresGraph: false,
      verb: "understand",
    },
  ],

  route: [
    {
      id: "trace-handler",
      label: "Trace the handler",
      tool: "cognicode_ask",
      params: { question: "trace the handler for `{label}`" },
      requiresGraph: true,
      verb: "trace",
    },
    {
      id: "related-tests",
      label: "Show related tests",
      tool: "explorer_get_view",
      params: { view_id: "test_slice" },
      requiresGraph: true,
      verb: "trace",
    },
    {
      id: "downstream-effects",
      label: "What does this route trigger downstream?",
      tool: "cognicode_ask",
      params: { question: "what does `{label}` call?" },
      requiresGraph: true,
      verb: "trace",
    },
  ],
} as const;

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/**
 * Filter the visible prompt list according to the current `graphStatus`.
 *
 *   - `"ready"`         → return all prompts
 *   - `"stale"`         → return all prompts (caller is responsible for
 *                         disabling the graph-dependent ones in the UI
 *                         and rejecting dispatch in the hook)
 *   - `"missing"` |     → drop every `requiresGraph` prompt
 *     `"indexing"` |
 *     `null`
 *
 * The function is pure so it lives here next to the data and is easy
 * to unit-test as part of the suggested-questions surface.
 */
export function filterByGraph(
  prompts: readonly SuggestedQuestion[],
  status: GraphStatus | null,
): readonly SuggestedQuestion[] {
  if (status === "ready" || status === "stale") {
    return prompts;
  }
  return prompts.filter((p) => !p.requiresGraph);
}

/**
 * Substitute `{label}` and `{id}` placeholders in a prompt's `params`.
 *
 * Pure utility — used by `useAsk` and (potentially) the strip. Lives
 * here so any future refactor can target a single source of truth.
 */
export function substituteParams(
  params: Readonly<Record<string, string>>,
  ctx: { label: string; id: string },
): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(params)) {
    out[key] = value
      .replace(/\{label\}/g, ctx.label)
      .replace(/\{id\}/g, ctx.id);
  }
  return out;
}
