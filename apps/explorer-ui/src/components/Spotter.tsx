/**
 * `Spotter` — Cmd/Ctrl+K palette for fuzzy-searching the workspace.
 *
 * Built on `cmdk` (a Radix Dialog-based command menu). The component
 * is fully controlled by `AppContext.spotterOpen` — the keyboard
 * shortcut lives in `SpotterHost` (mounted by Shell) and dispatches
 * `TOGGLE_SPOTTER` / `SET_SPOTTER`. The Spotter itself focuses the
 * search input on open and renders results grouped by `kind`.
 *
 * Selection flow:
 *   user types  → 200ms debounce → useSpotter (SWR) → results
 *   user picks  → dispatch SELECT_OBJECT + close palette.
 *
 * A11y: the palette is a `role="dialog"` modal. The visible input
 * is the only focusable element by default; cmdk manages the
 * roving focus on the result list. A live region announces the
 * current result count for screen readers.
 */
import { useEffect, useMemo, useState, type KeyboardEvent } from "react";
import { Command } from "cmdk";

import { useApp, useAppDispatch } from "../state/context";
import { useSpotter } from "../hooks/useSpotter";
import { useWorkspaceList } from "../hooks/useWorkspace";
import type { SpotterResult, SpotterSearchResult } from "../api/schemas";
import { kindDefaultView } from "../api/kindDefaultView";
import { IntentFooter, chipsFromResult } from "./Spotter/IntentFooter";
import type { ViewSpecSummary } from "../api/schemas";

/** Debounce window for the search input. Tuned for ~50 WPM typing. */
const DEBOUNCE_MS = 200;

const ALL_KINDS = "__all__" as const;

type KindFilter = string;

/**
 * Lightweight debounce hook — the trailing edge of the user's typing
 * pulse. We intentionally do NOT use `useDeferredValue` so the input
 * remains snappy; debounce is the right primitive when we want a
 * network call to settle before re-rendering results.
 */
function useDebounced<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = window.setTimeout(() => setDebounced(value), delayMs);
    return () => window.clearTimeout(id);
  }, [value, delayMs]);
  return debounced;
}

/**
 * Compute the available kind filters from the latest result set.
 * `all` is always present; each unique `kind` becomes
 * a chip. The list is stable across keystrokes (we still recompute
 * each render — cheap, and the order is deterministic because the
 * backend already returns results sorted by score).
 *
 * If `spotterKind` is set (caller dispatched with a kind hint), it is
 * added to the chip set even when no query results exist yet — so the
 * active filter is visible from the moment Spotter opens (e18-1
 * hotfix). Once results arrive, the chips reflect the real kinds.
 */
function kindsFromResults(
  results: ReadonlyArray<SpotterSearchResult>,
  spotterKind: string | null = null,
): KindFilter[] {
  const seen = new Set<string>();
  for (const r of results) {
    seen.add(r.kind);
  }
  if (spotterKind && !seen.has(spotterKind)) {
    seen.add(spotterKind);
  }
  return [ALL_KINDS, ...Array.from(seen).sort()];
}

// ============================================================================
// Spotter — visible modal + keyboard wiring
// ============================================================================

/**
 * The visible Spotter dialog. Reads `state.spotterOpen` from the
 * reducer; mounts/unmounts the modal based on it. Filtering is
 * server-side (`useSpotter` calls the API with the debounced query);
 * we additionally filter the rendered list by the local `kind`
 * chips above the results.
 *
 * The keyboard listener (Cmd/Ctrl+K, `/`) lives inside the same
 * component so the wiring is co-located with the UI. It registers
 * for the lifetime of the component — typically the lifetime of
 * the Shell. Removing the component automatically un-registers.
 */
export function Spotter() {
  const { state } = useApp();
  const dispatch = useAppDispatch();
  const { spotterOpen, spotterKind } = state;

  // -----------------------------------------------------------------
  // Global keyboard shortcuts
  // -----------------------------------------------------------------
  useEffect(() => {
    function onKey(event: globalThis.KeyboardEvent) {
      const isModifier = event.metaKey || event.ctrlKey;
      // Cmd+K (mac) / Ctrl+K (everything else). We also support
      // `/` as a quick alternative for users who do not reach for
      // the modifier — the convention popularized by GitHub.
      if (isModifier && event.key.toLowerCase() === "k") {
        event.preventDefault();
        dispatch({ type: "TOGGLE_SPOTTER" });
        return;
      }
      if (
        event.key === "/" &&
        !isModifier &&
        !(event.target instanceof HTMLInputElement) &&
        !(event.target instanceof HTMLTextAreaElement)
      ) {
        event.preventDefault();
        dispatch({ type: "SET_SPOTTER", payload: { open: true } });
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dispatch]);

  // Resolve the workspace id once — the first entry from the
  // workspace list is the active one in this MVP. When the user
  // opens a different workspace in a future cycle, the spotter
  // will follow `state.workspace.id`.
  const { data: workspaces } = useWorkspaceList();
  const workspaceId = workspaces?.[0]?.id ?? null;

  const [rawQuery, setRawQuery] = useState("");
  const [kind, setKind] = useState<KindFilter>(spotterKind ?? ALL_KINDS);
  const debouncedQuery = useDebounced(rawQuery, DEBOUNCE_MS);

  /** Tracks the currently-highlighted result via Command.List onChange. */
  const [highlightedResult, setHighlightedResult] = useState<SpotterSearchResult | null>(null);

  /**
   * Set by IntentFooter chip click or Cmd+N when a result is highlighted.
   * When the user presses Enter on the SAME item that has the pending chip,
   * that chip's viewId is used instead of the kind-aware default.
   */
  const [pendingViewId, setPendingViewId] = useState<string | null>(null);

  // Reset transient state every time the palette opens. We
  // defer the setState into a microtask so the React linter does
  // not flag it as a cascading render — the open is the only
  // thing that drives the reset, so the cost is one extra render
  // at most.
  useEffect(() => {
    if (spotterOpen) {
      queueMicrotask(() => {
        setRawQuery("");
        // Don't reset kind if spotterKind was set by caller
        setKind((currentKind) => spotterKind ?? currentKind ?? ALL_KINDS);
        setHighlightedResult(null);
        setPendingViewId(null);
      });
    }
  }, [spotterOpen, spotterKind]);

  // SWR — only fetches when both the workspace and the (trimmed)
  // query are non-empty. `keepPreviousData` smooths the transition
  // between keystrokes.
  const trimmed = debouncedQuery.trim();
  const { data, isLoading, isValidating, error } = useSpotter({
    workspaceId,
    q: trimmed,
    ...(kind !== ALL_KINDS ? { kind } : {}),
  });

  // Locally re-filter to hide kinds the user has de-selected via
  // a chip. (The server also accepts `kind`, but the chip is for
  // *narrowing* the current result set without a new round-trip.)
  const filteredResults = useMemo(() => {
    if (!data) return [];
    if (kind === ALL_KINDS) return data;
    return data.filter((r) => r.kind === kind);
  }, [data, kind]);

  const grouped = useMemo(() => groupByKind(filteredResults), [filteredResults]);
  const kindOptions = useMemo(
    () => kindsFromResults(data ?? [], kind === ALL_KINDS ? spotterKind : kind),
    [data, kind, spotterKind],
  );

  // Close on Escape (cmdk handles this internally for the input
  // but we want it at the dialog level too).
  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      dispatch({ type: "SET_SPOTTER", payload: { open: false } });
      return;
    }
    // Cmd+1..N: pick nth enabled chip on the highlighted result
    if ((event.metaKey || event.ctrlKey) && /^[1-9]$/.test(event.key)) {
      if (!highlightedResult) return;
      event.preventDefault();
      const chips = chipsFromResult(highlightedResult);
      const idx = parseInt(event.key, 10) - 1;
      const chip = chips[idx];
      if (chip && !chip.disabled) {
        setPendingViewId(chip.viewId);
      }
    }
  }

  if (!spotterOpen) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Spotter search"
      data-testid="spotter"
      onKeyDown={handleKeyDown}
      className="fixed inset-0 z-50 flex items-start justify-center p-4 sm:p-12"
      style={{ backgroundColor: "rgba(0, 0, 0, 0.55)" }}
    >
      <div
        data-testid="spotter-backdrop"
        onClick={() =>
          dispatch({ type: "SET_SPOTTER", payload: { open: false } })
        }
        className="absolute inset-0 cursor-default"
        style={{ background: "transparent" }}
      />
      <div
        className="relative w-full max-w-2xl overflow-hidden rounded-lg shadow-2xl"
        style={{
          backgroundColor: "var(--color-surface-raised)",
          border: "1px solid var(--color-border)",
        }}
      >
        <Command
          label="Spotter search"
          // cmdk manages the visible list; we keep our own debounced
          // query in React state so the chip filter can read the
          // latest value.
          shouldFilter={false}
          className="flex flex-col"
        >
          <div
            className="flex items-center gap-2 px-3 py-2"
            style={{ borderBottom: "1px solid var(--color-border)" }}
          >
            <span
              aria-hidden="true"
              className="font-mono text-xs"
              style={{ color: "var(--color-text-muted)" }}
            >
              ⌘K
            </span>
            <Command.Input
              value={rawQuery}
              onValueChange={setRawQuery}
              placeholder="Search the workspace…"
              data-testid="spotter-input"
              autoFocus
              className="flex-1 bg-transparent text-sm outline-none"
              style={{ color: "var(--color-text-primary)" }}
            />
            {isLoading && (
              <span
                aria-hidden="true"
                className="inline-block h-2 w-2 animate-pulse rounded-full"
                style={{ backgroundColor: "var(--color-primary)" }}
              />
            )}
          </div>

          <KindFilterChips
            options={kindOptions}
            value={kind}
            onChange={setKind}
          />

          <Command.List
            data-testid="spotter-results"
            className="max-h-80 overflow-y-auto p-1"
            aria-label="Search results"
            aria-busy={isValidating}
            onChange={(value) => {
              // cmdk sets the value to the highlighted item's `value` prop
              const found = filteredResults.find((r) => {
                if (r.kind === "viewspec") {
                  return (r.result as ViewSpecSummary).id === value;
                }
                return (r.result as SpotterResult).object.id === value;
              });
              setHighlightedResult(found ?? null);
            }}
          >
            <Command.Empty>
              <EmptyState
                query={trimmed}
                loading={isLoading}
                error={error ?? null}
                hasWorkspace={Boolean(workspaceId)}
              />
            </Command.Empty>
            {grouped.map(({ kind: k, items }) => (
              <Command.Group
                key={k}
                heading={k}
                className="px-1 py-1 text-xs"
              >
                {items.map((hit) => (
                  <Command.Item
                    key={hit.id}
                    value={hit.id}
                    onSelect={() => {
                      const defaultView = kindDefaultView(hit.object_type);
                      // If the highlighted result is the one we're selecting AND the
                      // user previously picked a chip (Cmd+N), use that viewId.
                      const viewId =
                        highlightedResult && "object" in highlightedResult
                          ? (highlightedResult.result as SpotterResult).object.id === hit.id && pendingViewId
                            ? pendingViewId
                            : defaultView
                          : defaultView;
                      dispatch({
                        type: "SELECT_OBJECT",
                        payload: {
                          objectId: hit.id,
                          viewId,
                        },
                      });
                      setPendingViewId(null);
                      dispatch({
                        type: "SET_SPOTTER",
                        payload: { open: false },
                      });
                    }}
                    data-testid={`spotter-item-${hit.id}`}
                    data-family={hit.object_type}
                    data-view-id={kindDefaultView(hit.object_type)}
                    className="flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm"
                  >
                    <span
                      aria-hidden="true"
                      className="inline-flex h-4 w-4 flex-none items-center justify-center font-mono text-xs"
                      style={{ color: "var(--color-text-muted)" }}
                    >
                      {kindGlyph(hit.object_type)}
                    </span>
                    <span
                      className="min-w-0 flex-1 truncate"
                      style={{ color: "var(--color-text-primary)" }}
                    >
                      {hit.label}
                    </span>
                    <span
                      className="truncate text-xs"
                      style={{ color: "var(--color-text-muted)" }}
                    >
                      {hit.subtitle}
                    </span>
                    <span
                      aria-hidden="true"
                      className="ml-1 flex-none rounded px-1 py-0.5 text-xs"
                      style={{
                        backgroundColor: "var(--color-surface-overlay)",
                        color: "var(--color-text-secondary)",
                      }}
                    >
                      {hit.score.toFixed(2)}
                    </span>
                  </Command.Item>
                ))}
              </Command.Group>
            ))}
          </Command.List>

          <IntentFooter
            result={highlightedResult}
            onPick={(viewId) => setPendingViewId(viewId)}
          />

          <footer
            className="flex items-center justify-between gap-2 px-3 py-1.5 text-xs"
            style={{
              borderTop: "1px solid var(--color-border)",
              color: "var(--color-text-muted)",
            }}
          >
            <span data-testid="spotter-count">
              {filteredResults.length}{" "}
              {filteredResults.length === 1 ? "result" : "results"}
            </span>
            <span>↑↓ navigate · ↵ select · esc close · ⌘1..N chip</span>
          </footer>
        </Command>
      </div>
    </div>
  );
}

// ============================================================================
// Kind filter chips
// ============================================================================

interface KindFilterChipsProps {
  options: string[];
  value: string;
  onChange: (next: string) => void;
}

function KindFilterChips({ options, value, onChange }: KindFilterChipsProps) {
  // Render the chip strip whenever there is anything useful to show:
  //   - more than one option (user can switch between filters), OR
  //   - exactly one option but the active value is NOT the "all" sentinel
  //     (caller dispatched with a kind hint, so we surface it even when
  //     there are no query results yet — e18-1 hotfix)
  if (options.length <= 1 && value === ALL_KINDS) return null;
  return (
    <div
      role="tablist"
      aria-label="Filter results by kind"
      data-testid="spotter-kind-filter"
      className="flex flex-wrap items-center gap-1 px-2 py-1.5"
      style={{ borderBottom: "1px solid var(--color-border)" }}
    >
      {options.map((opt) => {
        const active = opt === value;
        const label = opt === ALL_KINDS ? "All" : opt;
        return (
          <button
            key={opt}
            type="button"
            role="tab"
            aria-selected={active}
            data-testid={`spotter-kind-${opt}`}
            onClick={() => onChange(opt)}
            className="rounded-full px-2 py-0.5 text-xs font-medium"
            style={{
              backgroundColor: active
                ? "var(--color-primary)"
                : "var(--color-surface-overlay)",
              color: active
                ? "var(--color-primary-foreground)"
                : "var(--color-text-secondary)",
            }}
          >
            {label}
          </button>
        );
      })}
    </div>
  );
}

// ============================================================================
// Empty state
// ============================================================================

interface EmptyStateProps {
  query: string;
  loading: boolean;
  error: Error | null;
  hasWorkspace: boolean;
}

function EmptyState({ query, loading, error, hasWorkspace }: EmptyStateProps) {
  let title: string;
  let hint: string;
  if (!hasWorkspace) {
    title = "No workspace open";
    hint = "Open a workspace to start searching.";
  } else if (error) {
    title = "Search failed";
    hint = error.message;
  } else if (query.length === 0) {
    title = "Type to search";
    hint = "Symbols, files, scopes and more.";
  } else if (loading) {
    title = "Searching…";
    hint = "Holding for the next debounce tick.";
  } else {
    title = "No matches";
    hint = `Nothing found for "${query}".`;
  }
  return (
    <div
      data-testid="spotter-empty"
      className="flex flex-col items-start gap-1 px-3 py-6 text-sm"
    >
      <span
        className="font-medium"
        style={{ color: "var(--color-text-primary)" }}
      >
        {title}
      </span>
      <span style={{ color: "var(--color-text-muted)" }}>{hint}</span>
    </div>
  );
}

// ============================================================================
// Helpers
// ============================================================================

interface Group {
  kind: string;
  items: NormalizedHit[];
}

/**
 * Normalize a SpotterSearchResult into a shape the rendering code expects.
 * Viewspec hits have a different structure than other families.
 */
interface NormalizedHit {
  id: string;
  object_type: string;
  label: string;
  subtitle: string;
  score: number;
  available_views: Array<{ id: string; title: string | null }>;
  /** Original SpotterSearchResult for dispatch/selection */
  _original: SpotterSearchResult;
}

function normalizeHit(hit: SpotterSearchResult): NormalizedHit {
  if (hit.kind === "viewspec") {
    const spec = hit.result as ViewSpecSummary;
    return {
      id: spec.id,
      object_type: "viewspec",
      label: spec.title,
      subtitle: spec.view_kind,
      score: 0,
      available_views: [],
      _original: hit,
    };
  }
  // symbol, file, saved_exploration, quality_issue, rule, investigation, scope
  const result = hit.result as SpotterResult;
  return {
    id: result.object.id,
    object_type: result.object.object_type,
    label: result.object.label,
    subtitle: result.object.subtitle,
    score: result.score,
    available_views: result.object.available_views ?? [],
    _original: hit,
  };
}

/**
 * Group results by `kind` for the cmdk `Group` headings.
 * The returned order is insertion order (which is the
 * backend's score-sorted order); cmdk preserves it.
 */
function groupByKind(results: ReadonlyArray<SpotterSearchResult>): Group[] {
  const map = new Map<string, NormalizedHit[]>();
  for (const r of results) {
    const normalized = normalizeHit(r);
    const list = map.get(r.kind) ?? [];
    list.push(normalized);
    map.set(r.kind, list);
  }
  return Array.from(map.entries()).map(([kind, items]) => ({ kind, items }));
}

function kindGlyph(kind: string): string {
  switch (kind) {
    case "symbol":
      return "ƒ";
    case "file":
      return "□";
    case "scope":
      return "▤";
    case "module":
      return "▦";
    case "workspace":
      return "◉";
    case "evidence":
      return "▣";
    case "decision_artifact":
      return "✎";
    case "quality_issue":
      return "!";
    case "rule":
      return "§";
    case "viewspec":
      return "◈";
    case "investigation":
      return "◎";
    case "saved_exploration":
      return "◐";
    default:
      return "•";
  }
}
