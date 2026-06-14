/**
 * `ViewSpecWizard` — Explorer-first authoring UI for runtime ViewSpecs.
 *
 * ADR-008 §Authoring Flow: This is the Phase 4 authoring wizard.
 * It implements a 5-step modal that lets users:
 *   1. Pick a ViewKind (semantic intent)
 *   2. Pick a RendererKind (visual strategy, with smart defaults)
 *   3. Configure the MoldQL data source
 *   4. Optionally set a JSONata transform
 *   5. Save or preview
 *
 * The wizard opens as a modal drawer triggered by the "Create custom view"
 * action in the ViewTabs overflow menu.
 *
 * Edit mode: pass `editSpec` to pre-fill the wizard. Save then calls
 * `PUT /api/viewspecs/:id` instead of `POST`. Ownership is checked:
 * the Edit button is hidden when `owner !== currentUser`.
 *
 * Draft persistence: wizard state is auto-saved to localStorage per object
 * (debounced 1s) and restored on reopen. Cleared on explicit save/cancel.
 */
import { useCallback, useEffect, useMemo, useReducer, useState } from "react";

import {
  executeViewSpec,
  saveViewSpec,
  updateViewSpec,
  type SaveViewSpecRequest,
} from "../../api/client";
import type {
  DataSource,
  InspectableObjectType,
  RendererKind,
  Transform,
  ViewKind,
  ViewSpec,
} from "../../api/schemas";
import { useWizardDraft } from "../../hooks/useWizardDraft";
import { Blocks } from "./ViewBlock";
import { TransformStep } from "./TransformStep";

// ============================================================================
// Wizard step definitions
// ============================================================================

const STEPS = [
  { id: "view-kind", label: "View Kind" },
  { id: "renderer", label: "Renderer" },
  { id: "data-source", label: "Data Source" },
  { id: "transform", label: "Transform" },
  { id: "save", label: "Save" },
] as const;

type StepId = (typeof STEPS)[number]["id"];

// ============================================================================
// Wizard state
// ============================================================================

interface WizardState {
  step: StepId;
  // Form fields
  viewKind: ViewKind | null;
  rendererKind: RendererKind | null;
  query: string;
  transformKind: "none" | "jsonata";
  jsonataExpression: string;
  title: string;
  /** Raw MoldQL result (blocks array) used as JSONata preview input. */
  transformPreviewInput: unknown;
}

type WizardAction =
  | { type: "SET_STEP"; step: StepId }
  | { type: "SET_VIEW_KIND"; viewKind: ViewKind }
  | { type: "SET_RENDERER_KIND"; rendererKind: RendererKind }
  | { type: "SET_QUERY"; query: string }
  | { type: "SET_TRANSFORM_KIND"; transformKind: WizardState["transformKind"] }
  | { type: "SET_JSONATA_EXPRESSION"; expression: string }
  | { type: "SET_TITLE"; title: string }
  | { type: "SET_TRANSFORM_PREVIEW_INPUT"; input: unknown }
  | { type: "RESTORE"; state: WizardState };

function wizardReducer(state: WizardState, action: WizardAction): WizardState {
  switch (action.type) {
    case "SET_STEP":
      return { ...state, step: action.step };
    case "SET_VIEW_KIND":
      return { ...state, viewKind: action.viewKind };
    case "SET_RENDERER_KIND":
      return { ...state, rendererKind: action.rendererKind };
    case "SET_QUERY":
      return { ...state, query: action.query };
    case "SET_TRANSFORM_KIND":
      return { ...state, transformKind: action.transformKind };
    case "SET_JSONATA_EXPRESSION":
      return { ...state, jsonataExpression: action.expression };
    case "SET_TITLE":
      return { ...state, title: action.title };
    case "SET_TRANSFORM_PREVIEW_INPUT":
      return { ...state, transformPreviewInput: action.input };
    case "RESTORE":
      return { ...action.state, step: "view-kind" };
    default:
      return state;
  }
}

const initialState: WizardState = {
  step: "view-kind",
  viewKind: null,
  rendererKind: null,
  query: "",
  transformKind: "none",
  jsonataExpression: "",
  title: "",
  transformPreviewInput: null,
};

// ============================================================================
// ViewKind catalog — mirrors the Rust ViewKind enum (ADR-008)
// ============================================================================

const VIEW_KIND_GROUPS = {
  Core: [
    { value: "vertical_slice", label: "Vertical Slice" },
    { value: "call_graph", label: "Call Graph" },
    { value: "source_view", label: "Source View" },
    { value: "data_flow", label: "Data Flow" },
  ] as const,
  C4: [
    { value: "c4_context", label: "C4 Context" },
    { value: "c4_container", label: "C4 Container" },
    { value: "c4_component", label: "C4 Component" },
    { value: "c4_code", label: "C4 Code" },
  ] as const,
  Architecture: [
    { value: "architecture_rationale", label: "Architecture Rationale" },
    { value: "architecture_drift", label: "Architecture Drift" },
    { value: "boundary_map", label: "Boundary Map" },
    { value: "dependency_pressure", label: "Dependency Pressure" },
    { value: "dependency_graph", label: "Dependency Graph" },
  ] as const,
  Development: [
    { value: "callers_and_implementors", label: "Callers & Implementors" },
    { value: "usage_examples", label: "Usage Examples" },
    { value: "api_surface", label: "API Surface" },
    { value: "test_slice", label: "Test Slice" },
    { value: "debug_slice", label: "Debug Slice" },
    { value: "refactor_plan", label: "Refactor Plan" },
    { value: "dead_code_candidates", label: "Dead Code Candidates" },
  ] as const,
  Quality: [
    { value: "quality_hotspots", label: "Quality Hotspots" },
    { value: "decision_trace", label: "Decision Trace" },
  ] as const,
  "Living Doc": [
    { value: "doc_code_alignment", label: "Doc Code Alignment" },
    { value: "composed_narrative", label: "Composed Narrative" },
    { value: "project_diary", label: "Project Diary" },
    { value: "concept_map", label: "Concept Map" },
    { value: "evidence_pack", label: "Evidence Pack" },
    { value: "evidence_view", label: "Evidence View" },
    { value: "decision_graph", label: "Decision Graph" },
  ] as const,
  Search: [
    { value: "semantic_search_results", label: "Semantic Search Results" },
    { value: "change_impact_story", label: "Change Impact Story" },
    { value: "ownership_map", label: "Ownership Map" },
    { value: "risk_map", label: "Risk Map" },
  ] as const,
} as const;

type ViewKindGroupKey = keyof typeof VIEW_KIND_GROUPS;

type ViewKindGroupedEntries = Array<{
  group: ViewKindGroupKey;
  items: ReadonlyArray<{ value: string; label: string }>;
}>;

// Default renderer for each view kind (from CONTEXT.md §ViewSpec)
const VIEW_KIND_DEFAULT_RENDERER: Record<string, RendererKind> = {
  vertical_slice: "graph",
  call_graph: "graph",
  seam_map: "graph",
  dependency_graph: "graph",
  source_view: "code",
  data_flow: "graph",
  impact_radius: "graph",
  diff_view: "code",
  c4_context: "graph",
  c4_container: "graph",
  c4_component: "graph",
  c4_code: "tree",
  quality_hotspots: "table",
  evidence_view: "markdown",
  decision_graph: "graph",
  architecture_rationale: "markdown",
  architecture_drift: "table",
  boundary_map: "graph",
  dependency_pressure: "table",
  change_impact_story: "markdown",
  ownership_map: "graph",
  risk_map: "table",
  decision_trace: "graph",
  test_slice: "table",
  debug_slice: "graph",
  refactor_plan: "markdown",
  callers_and_implementors: "graph",
  usage_examples: "code",
  api_surface: "table",
  dead_code_candidates: "table",
  semantic_search_results: "table",
  doc_code_alignment: "markdown",
  example_object: "code",
  composed_narrative: "markdown",
  project_diary: "markdown",
  concept_map: "graph",
  evidence_pack: "markdown",
};

// ============================================================================
// Renderer catalog — mirrors the Rust RendererKind enum (ADR-008)
// ============================================================================

const RENDERER_OPTIONS: Array<{ value: RendererKind; label: string }> = [
  { value: "graph", label: "Graph — interactive node/edge visualization" },
  { value: "table", label: "Table — rows and columns" },
  { value: "tree", label: "Tree — hierarchical navigation" },
  { value: "code", label: "Code — syntax-highlighted source" },
  { value: "markdown", label: "Markdown — rich text rendering" },
  { value: "json", label: "JSON — raw JSON viewer" },
  { value: "vega_lite", label: "Vega-Lite — charts and graphs" },
  { value: "composite", label: "Composite — multi-panel layout" },
];

// ============================================================================
// Props
// ============================================================================

export interface ViewSpecWizardProps {
  /** Whether the wizard is open. */
  isOpen: boolean;
  /** Callback to close the wizard without saving. */
  onClose: () => void;
  /** The object being inspected (determines applies_to). */
  objectId: string;
  objectType: InspectableObjectType;
  objectLabel: string;
  /** The active workspace id for scoping saves. */
  workspaceId: string;
  /** The current user/owner for saves. */
  owner: string;
  /**
   * When provided, the wizard opens in edit mode pre-filled from `editSpec`.
   * Save will call `PUT /api/viewspecs/:id` instead of `POST`.
   */
  editSpec?: ViewSpec;
  /** Called after a successful save (receives the new spec id). */
  onSaved?: (id: string) => void;
}

// ============================================================================
// Component
// ============================================================================

export function ViewSpecWizard({
  isOpen,
  onClose,
  objectId,
  objectType,
  objectLabel,
  workspaceId,
  owner,
  editSpec,
  onSaved,
}: ViewSpecWizardProps) {
  const isEditMode = editSpec != null;

  /** Derive the initial state: pre-fill from editSpec or default title. */
  const buildInitialState = useCallback(
    (): WizardState => {
      if (editSpec) {
        // Safe cast: we control the wire shape and only emit well-typed MoldQL sources.
        const ds = editSpec.data_source as { kind: "moldql"; query: string };
        const query = ds.kind === "moldql" ? ds.query : "";
        const transformKind: WizardState["transformKind"] =
          editSpec.transform?.kind === "jsonata" ? "jsonata" : "none";
        const jsonataExpression = editSpec.transform?.kind === "jsonata"
          ? (editSpec.transform as { kind: "jsonata"; expression: string }).expression
          : "";
        return {
          ...initialState,
          viewKind: editSpec.view_kind as ViewKind,
          rendererKind: editSpec.renderer_kind as RendererKind,
          query,
          transformKind,
          jsonataExpression,
          title: editSpec.title,
        };
      }
      return {
        ...initialState,
        title: `Custom view for ${objectLabel}`,
      };
    },
    [editSpec, objectLabel],
  );

  const [state, dispatch] = useReducer(wizardReducer, undefined, buildInitialState);

  // Restore draft when wizard opens.
  const { save, clear } = useWizardDraft({
    objectId,
    editSpec,
    onRestore: (restored) => {
      dispatch({ type: "RESTORE", state: { ...buildInitialState(), ...restored } });
    },
  });

  // Auto-save draft on state changes (debounced in the hook).
  useEffect(() => {
    if (!isOpen) return;
    save({
      viewKind: state.viewKind,
      rendererKind: state.rendererKind,
      query: state.query,
      transformKind: state.transformKind,
      jsonataExpression: state.jsonataExpression,
      title: state.title,
    });
  }, [isOpen, state, save]);

  // Preview state
  const [previewData, setPreviewData] = useState<Awaited<ReturnType<typeof executeViewSpec>> | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);

  // Save state
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  // Derived: can advance from step 1?
  const canAdvanceFromStep1 = state.viewKind !== null;
  const canAdvanceFromStep2 = state.rendererKind !== null;
  const canAdvanceFromStep3 = state.query.trim().length > 0;
  const canSave =
    state.viewKind !== null &&
    state.rendererKind !== null &&
    state.query.trim().length > 0 &&
    state.title.trim().length > 0;

  const stepIndex = STEPS.findIndex((s) => s.id === state.step);

  /** Build the in-progress ViewSpec from wizard state. */
  const buildSpec = useCallback((): Parameters<typeof executeViewSpec>[0] | null => {
    if (!state.viewKind || !state.rendererKind) return null;
    const id = isEditMode ? editSpec!.id : crypto.randomUUID();
    const now = new Date().toISOString();
    const dataSource: DataSource =
      state.query.trim().length > 0
        ? { kind: "moldql", query: state.query.trim() }
        : { kind: "moldql", query: "" };
    const transform: Transform | null =
      state.transformKind === "jsonata" && state.jsonataExpression.trim().length > 0
        ? { kind: "jsonata", expression: state.jsonataExpression.trim() }
        : null;
    return {
      id,
      title: state.title.trim(),
      applies_to: objectType,
      view_kind: state.viewKind,
      data_source: dataSource,
      transform: transform ?? undefined,
      renderer_kind: state.rendererKind,
      props: {},
      created_at: now,
      updated_at: now,
      owner,
    };
  }, [state, objectType, isEditMode, editSpec, owner]);

  /** Preview the current wizard state by executing the spec. */
  const runPreview = useCallback(async () => {
    const spec = buildSpec();
    if (!spec || !state.query.trim()) return;
    setPreviewLoading(true);
    setPreviewError(null);
    try {
      const result = await executeViewSpec(spec, objectId);
      setPreviewData(result);
      // Capture blocks as the JSONata preview input
      dispatch({
        type: "SET_TRANSFORM_PREVIEW_INPUT",
        input: result.blocks ?? result,
      });
    } catch (err) {
      setPreviewError(err instanceof Error ? err.message : String(err));
    } finally {
      setPreviewLoading(false);
    }
  }, [buildSpec, objectId, state.query]);

  /** Save the current wizard state as a ViewSpec. */
  const runSave = useCallback(async () => {
    const spec = buildSpec();
    if (!spec) return;
    setSaving(true);
    setSaveError(null);
    try {
      if (isEditMode) {
        // PUT /api/viewspecs/:id — replace existing spec.
        const { id } = await updateViewSpec(editSpec!.id, { spec });
        clear();
        onSaved?.(id);
        onClose();
      } else {
        // POST /api/viewspecs — create new spec.
        const request: SaveViewSpecRequest = {
          workspace_id: workspaceId,
          owner,
          spec,
        };
        const { id } = await saveViewSpec(request);
        clear();
        onSaved?.(id);
        onClose();
      }
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }, [buildSpec, workspaceId, owner, isEditMode, editSpec, clear, onSaved, onClose]);

  // Wrap onClose so cancel also clears the draft.
  const handleClose = useCallback(() => {
    clear();
    onClose();
  }, [clear, onClose]);

  if (!isOpen) return null;

  return (
    <div
      data-testid="viewspec-wizard"
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: "rgba(0,0,0,0.6)" }}
      role="dialog"
      aria-modal="true"
              aria-label={isEditMode ? "Edit custom view" : "Create custom view"}
    >
      <div
        className="flex h-[90vh] w-[85vw] max-w-5xl flex-col overflow-hidden rounded-lg shadow-2xl"
        style={{
          backgroundColor: "var(--color-surface)",
          border: "1px solid var(--color-border)",
        }}
      >
        {/* Header */}
        <div
          className="flex items-center justify-between border-b px-6 py-4"
          style={{ borderColor: "var(--color-border)" }}
        >
          <div>
            <h2
              className="text-base font-semibold"
              style={{ color: "var(--color-text-primary)" }}
            >
              {isEditMode ? "Edit Custom View" : "Create Custom View"}
            </h2>
            <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
              Step {stepIndex + 1} of {STEPS.length} — {STEPS[stepIndex]?.label ?? ""}
            </p>
          </div>
          <button
            type="button"
            onClick={handleClose}
            aria-label="Close"
            className="rounded-md p-2 transition-colors hover:bg-black/10"
            style={{ color: "var(--color-text-muted)" }}
          >
            ✕
          </button>
        </div>

        {/* Step indicator */}
        <div
          className="flex gap-1 border-b px-6 py-3"
          style={{ borderColor: "var(--color-border)" }}
        >
          {STEPS.map((step, idx) => {
            const isActive = step.id === state.step;
            const isPast = idx < stepIndex;
            return (
              <div key={step.id} className="flex items-center gap-1">
                <button
                  type="button"
                  onClick={() => dispatch({ type: "SET_STEP", step: step.id as StepId })}
                  className="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs transition-colors"
                  style={{
                    backgroundColor: isActive
                      ? "var(--color-primary)"
                      : isPast
                        ? "var(--color-surface-overlay)"
                        : "transparent",
                    color: isActive
                      ? "var(--color-primary-foreground)"
                      : isPast
                        ? "var(--color-text-primary)"
                        : "var(--color-text-muted)",
                    opacity: idx > stepIndex + 1 ? 0.5 : 1,
                  }}
                  disabled={idx > stepIndex + 1}
                >
                  <span
                    className="flex h-5 w-5 items-center justify-center rounded-full text-[10px] font-semibold"
                    style={{
                      backgroundColor: isActive
                        ? "var(--color-primary-foreground)"
                        : isPast
                          ? "var(--color-primary)"
                          : "var(--color-surface-overlay)",
                      color: isActive
                        ? "var(--color-primary)"
                        : "var(--color-primary-foreground)",
                    }}
                  >
                    {isPast ? "✓" : idx + 1}
                  </span>
                  {step.label}
                </button>
                {idx < STEPS.length - 1 && (
                  <span style={{ color: "var(--color-border)" }}>›</span>
                )}
              </div>
            );
          })}
        </div>

        {/* Body: step content + preview */}
        <div className="flex flex-1 overflow-hidden">
          {/* Step content */}
          <div className="flex-1 overflow-y-auto p-6">
            {state.step === "view-kind" && (
              <ViewKindStep
                selected={state.viewKind}
                onSelect={(vk) => {
                  dispatch({ type: "SET_VIEW_KIND", viewKind: vk });
                  // Auto-default the renderer
                  const defRenderer = VIEW_KIND_DEFAULT_RENDERER[vk] ?? "json";
                  dispatch({ type: "SET_RENDERER_KIND", rendererKind: defRenderer as RendererKind });
                }}
              />
            )}
            {state.step === "renderer" && (
              <RendererKindStep
                selected={state.rendererKind}
                onSelect={(rk) => dispatch({ type: "SET_RENDERER_KIND", rendererKind: rk })}
              />
            )}
            {state.step === "data-source" && (
              <DataSourceStep
                query={state.query}
                onChange={(q) => dispatch({ type: "SET_QUERY", query: q })}
              />
            )}
            {state.step === "transform" && (
              <TransformStep
                transformKind={state.transformKind}
                expression={state.jsonataExpression}
                previewInput={state.transformPreviewInput}
                onTransformKindChange={(tk) =>
                  dispatch({ type: "SET_TRANSFORM_KIND", transformKind: tk })
                }
                onExpressionChange={(e) =>
                  dispatch({ type: "SET_JSONATA_EXPRESSION", expression: e })
                }
              />
            )}
            {state.step === "save" && (
              <SaveStep
                title={state.title}
                objectLabel={objectLabel}
                onTitleChange={(t) => dispatch({ type: "SET_TITLE", title: t })}
                saving={saving}
                saveError={saveError}
                onSave={runSave}
              />
            )}
          </div>

          {/* Live preview panel */}
          <div
            className="w-[420px] flex-shrink-0 overflow-y-auto border-l p-4"
            style={{ borderColor: "var(--color-border)", backgroundColor: "var(--color-surface-raised)" }}
          >
            <div className="mb-3 flex items-center justify-between">
              <h3 className="text-xs font-semibold uppercase tracking-wide" style={{ color: "var(--color-text-secondary)" }}>
                Live Preview
              </h3>
              <button
                type="button"
                onClick={runPreview}
                disabled={previewLoading || !state.query.trim()}
                className="rounded-md px-2 py-1 text-xs font-medium transition-colors disabled:opacity-50"
                style={{
                  backgroundColor: "var(--color-primary)",
                  color: "var(--color-primary-foreground)",
                }}
              >
                {previewLoading ? "…" : "Run"}
              </button>
            </div>
            {previewError && (
              <div
                className="mb-3 rounded-md p-2 text-xs"
                style={{
                  backgroundColor: "rgba(239,68,68,0.1)",
                  color: "var(--color-error)",
                  border: "1px solid var(--color-error)",
                }}
              >
                {previewError}
              </div>
            )}
            {previewData ? (
              <div className="flex flex-col gap-2">
                <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
                  {previewData.blocks.length} block{previewData.blocks.length !== 1 ? "s" : ""}
                </p>
                <Blocks view={previewData} />
              </div>
            ) : (
              <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
                Configure steps 1–3, then click Run to preview.
              </p>
            )}
          </div>
        </div>

        {/* Footer navigation */}
        <div
          className="flex items-center justify-between border-t px-6 py-4"
          style={{ borderColor: "var(--color-border)" }}
        >
          <button
            type="button"
            onClick={() => {
              const idx = STEPS.findIndex((s) => s.id === state.step);
              if (idx > 0) {
                const prev = STEPS[idx - 1];
                if (prev) dispatch({ type: "SET_STEP", step: prev.id as StepId });
              }
            }}
            disabled={stepIndex === 0}
            className="rounded-md px-4 py-2 text-sm font-medium transition-colors disabled:opacity-40"
            style={{
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-primary)",
            }}
          >
            ← Back
          </button>

          <div className="flex gap-3">
            {state.step !== "save" ? (
              <button
                type="button"
                onClick={() => {
                  const idx = STEPS.findIndex((s) => s.id === state.step);
                  if (idx < STEPS.length - 1) {
                    const next = STEPS[idx + 1];
                    if (next) dispatch({ type: "SET_STEP", step: next.id as StepId });
                  }
                }}
                disabled={
                  (state.step === "view-kind" && !canAdvanceFromStep1) ||
                  (state.step === "renderer" && !canAdvanceFromStep2) ||
                  (state.step === "data-source" && !canAdvanceFromStep3)
                }
                className="rounded-md px-4 py-2 text-sm font-medium transition-colors disabled:opacity-40"
                style={{
                  backgroundColor: "var(--color-primary)",
                  color: "var(--color-primary-foreground)",
                }}
              >
                Next →
              </button>
            ) : (
              <button
                type="button"
                onClick={runSave}
                disabled={saving || !canSave}
                className="rounded-md px-4 py-2 text-sm font-medium transition-colors disabled:opacity-40"
                style={{
                  backgroundColor: saving ? "var(--color-surface-overlay)" : "var(--color-success)",
                  color: saving ? "var(--color-text-muted)" : "var(--color-surface)",
                }}
              >
                {saving ? "Saving…" : isEditMode ? "Update View" : "Save View"}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Step content components
// ============================================================================

interface ViewKindStepProps {
  selected: ViewKind | null;
  onSelect: (vk: ViewKind) => void;
}

function ViewKindStep({ selected, onSelect }: ViewKindStepProps) {
  const [search, setSearch] = useState("");

  const groupedEntries = useMemo((): ViewKindGroupedEntries => {
    return Object.entries(VIEW_KIND_GROUPS).map(([group, items]) => ({
      group: group as ViewKindGroupKey,
      items,
    }));
  }, []);

  const filtered = useMemo((): ViewKindGroupedEntries | null => {
    if (!search.trim()) return null;
    const q = search.toLowerCase();
    return groupedEntries
      .map((entry) => ({
        ...entry,
        items: entry.items.filter(
          (item) =>
            item.label.toLowerCase().includes(q) || item.value.toLowerCase().includes(q),
        ),
      }))
      .filter((entry) => entry.items.length > 0);
  }, [search, groupedEntries]);

  return (
    <div className="flex flex-col gap-4">
      <div>
        <h3 className="mb-1 text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
          What do you want to understand?
        </h3>
        <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
          Select the semantic intent of your view. Use search to filter the catalog.
        </p>
      </div>

      {/* Search */}
      <div className="relative">
        <input
          type="search"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search view kinds…"
          className="w-full rounded-md px-3 py-2 text-sm"
          style={{
            backgroundColor: "var(--color-surface-overlay)",
            color: "var(--color-text-primary)",
            border: "1px solid var(--color-border)",
          }}
        />
      </div>

      {/* Grouped list */}
      <div className="flex flex-col gap-4">
        {(filtered ?? groupedEntries).map((entry) => (
            <div key={entry.group}>
              <h4
                className="mb-2 text-[10px] font-semibold uppercase tracking-widest"
                style={{ color: "var(--color-text-muted)" }}
              >
                {entry.group}
              </h4>
              <div className="grid grid-cols-2 gap-2">
                {entry.items.map((item) => {
                  const isSelected = selected === item.value;
                  return (
                    <button
                      key={item.value}
                      type="button"
                      onClick={() => onSelect(item.value as ViewKind)}
                      className="rounded-md p-3 text-left text-sm transition-colors"
                      style={{
                        backgroundColor: isSelected
                          ? "var(--color-primary)"
                          : "var(--color-surface-overlay)",
                        color: isSelected
                          ? "var(--color-primary-foreground)"
                          : "var(--color-text-primary)",
                        border: isSelected
                          ? "2px solid var(--color-primary)"
                          : "1px solid var(--color-border)",
                      }}
                    >
                      {item.label}
                    </button>
                  );
                })}
              </div>
            </div>
          ),
        )}
      </div>
    </div>
  );
}

interface RendererKindStepProps {
  selected: RendererKind | null;
  onSelect: (rk: RendererKind) => void;
}

function RendererKindStep({ selected, onSelect }: RendererKindStepProps) {
  return (
    <div className="flex flex-col gap-4">
      <div>
        <h3 className="mb-1 text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
          How should the view be rendered?
        </h3>
        <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
          Pick the visual rendering strategy. The default was chosen based on your ViewKind.
        </p>
      </div>
      <div className="flex flex-col gap-2">
        {RENDERER_OPTIONS.map((opt) => {
          const isSelected = selected === opt.value;
          return (
            <button
              key={opt.value}
              type="button"
              onClick={() => onSelect(opt.value)}
              className="rounded-md p-3 text-left text-sm transition-colors"
              style={{
                backgroundColor: isSelected
                  ? "var(--color-primary)"
                  : "var(--color-surface-overlay)",
                color: isSelected
                  ? "var(--color-primary-foreground)"
                  : "var(--color-text-primary)",
                border: isSelected
                  ? "2px solid var(--color-primary)"
                  : "1px solid var(--color-border)",
              }}
            >
              <span className="font-medium">{opt.label}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

interface DataSourceStepProps {
  query: string;
  onChange: (q: string) => void;
}

function DataSourceStep({ query, onChange }: DataSourceStepProps) {
  return (
    <div className="flex flex-col gap-4">
      <div>
        <h3 className="mb-1 text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
          MoldQL Data Source
        </h3>
        <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
          Enter a MoldQL query to select the data for this view. Available objects:{" "}
          <code className="rounded px-1" style={{ backgroundColor: "var(--color-surface-overlay)" }}>
            symbols
          </code>
          ,{" "}
          <code className="rounded px-1" style={{ backgroundColor: "var(--color-surface-overlay)" }}>
            docs
          </code>
          ,{" "}
          <code className="rounded px-1" style={{ backgroundColor: "var(--color-surface-overlay)" }}>
            evidence
          </code>
          ,{" "}
          <code className="rounded px-1" style={{ backgroundColor: "var(--color-surface-overlay)" }}>
            issues
          </code>
          ,{" "}
          <code className="rounded px-1" style={{ backgroundColor: "var(--color-surface-overlay)" }}>
            rules
          </code>
          , decisions.
        </p>
      </div>
      <textarea
        value={query}
        onChange={(e) => onChange(e.target.value)}
        placeholder="symbols where kind = 'function' and fan_out > 5"
        rows={6}
        className="w-full rounded-md px-3 py-2 font-mono text-xs"
        style={{
          backgroundColor: "var(--color-surface-overlay)",
          color: "var(--color-text-primary)",
          border: "1px solid var(--color-border)",
          resize: "vertical",
        }}
      />
      <div
        className="rounded-md p-3 text-xs"
        style={{ backgroundColor: "var(--color-surface-overlay)", color: "var(--color-text-muted)" }}
      >
        <strong>Example queries:</strong>
        <ul className="mt-1 list-disc pl-4">
          <li>
            <code className="font-mono">symbols where kind = 'function'</code> — all functions
          </li>
          <li>
            <code className="font-mono">symbols where fan_out &gt; 10</code> — high fan-out
          </li>
          <li>
            <code className="font-mono">issues where severity = 'critical'</code> — critical issues
          </li>
        </ul>
      </div>
    </div>
  );
}

interface SaveStepProps {
  title: string;
  objectLabel: string;
  onTitleChange: (t: string) => void;
  saving: boolean;
  saveError: string | null;
  onSave: () => void;
}

function SaveStep({
  title,
  objectLabel,
  onTitleChange,
  saving: _saving,
  saveError,
  onSave: _onSave,
}: SaveStepProps) {
  return (
    <div className="flex flex-col gap-4">
      <div>
        <h3 className="mb-1 text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
          Save Your View
        </h3>
        <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
          Give your view a title. It will be saved for the current workspace and
          available in the ViewTabs for "{objectLabel}".
        </p>
      </div>

      <div className="flex flex-col gap-1">
        <label
          htmlFor="viewspec-title"
          className="text-xs font-medium"
          style={{ color: "var(--color-text-secondary)" }}
        >
          View Title *
        </label>
        <input
          id="viewspec-title"
          type="text"
          value={title}
          onChange={(e) => onTitleChange(e.target.value)}
          placeholder="e.g. High Fan-Out Functions"
          maxLength={200}
          className="w-full rounded-md px-3 py-2 text-sm"
          style={{
            backgroundColor: "var(--color-surface-overlay)",
            color: "var(--color-text-primary)",
            border: "1px solid var(--color-border)",
          }}
        />
        <p className="text-xs" style={{ color: "var(--color-text-muted)" }}>
          {title.length}/200 characters
        </p>
      </div>

      {saveError && (
        <div
          className="rounded-md p-3 text-sm"
          style={{
            backgroundColor: "rgba(239,68,68,0.1)",
            color: "var(--color-error)",
            border: "1px solid var(--color-error)",
          }}
        >
          Save failed: {saveError}
        </div>
      )}

      <div
        className="rounded-md p-3 text-xs"
        style={{ backgroundColor: "var(--color-surface-overlay)", color: "var(--color-text-muted)" }}
      >
        <strong>Note:</strong> This view will be persisted to the workspace database.
        JSONata transform preview is shown in the live preview panel, but JSONata
        execution is deferred to Phase 4 full implementation.
      </div>
    </div>
  );
}
