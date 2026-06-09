/**
 * `SuggestionStrip` — the "What can I do here?" prompt strip.
 *
 * Rendered between the `ObjectInspector` header and the `ViewTabs`.
 * The strip branches on viewport:
 *
 *   - `small`      → a single button + `<SuggestionPopover>` (dialog).
 *   - `tablet |    → an inline row of pill buttons. One per prompt,
 *     desktop |     after graph-status gating.
 *     ultrawide`
 *
 * Graph-status gating (see `filterByGraph` for the rules):
 *
 *   - `ready`         → all prompts visible, all enabled.
 *   - `stale`         → all prompts visible, graph-dependent pills are
 *                       marked `aria-disabled="true"` and clicking
 *                       them does NOT dispatch.
 *   - `missing` |     → only non-graph prompts visible.
 *     `indexing` |
 *     `null`
 *
 * The strip is purely presentational. The data lives in
 * `SUGGESTED_QUESTIONS`; dispatch routing lives in `useAsk`.
 */
import { useMemo } from "react";
import type React from "react";

import type { GraphStatus, InspectableObjectType } from "../../api/types";
import { SUGGESTED_QUESTIONS, filterByGraph, type SuggestedQuestion } from "../../config/suggestedQuestions";
import { SuggestionPopover } from "./SuggestionPopover";
import type { ShellViewport } from "../viewport";

export interface SuggestionStripProps {
  objectType: InspectableObjectType;
  objectId: string;
  objectLabel: string;
  graphStatus: GraphStatus | null;
  viewport: ShellViewport;
  onDispatch: (q: SuggestedQuestion) => void;
}

/**
 * The strip — branches between pill row (default) and popover
 * (small viewport). The popover is the same component, just
 * wrapped in the viewport-aware switch.
 */
export function SuggestionStrip(props: SuggestionStripProps): React.ReactElement {
  const allPrompts = SUGGESTED_QUESTIONS[props.objectType];
  // Stale status keeps every prompt visible so the user can see what
  // is unavailable (the strip marks the graph-dependent ones with
  // `aria-disabled`). The other statuses apply the gate.
  const visiblePrompts = useMemo(
    () => filterByGraph(allPrompts, props.graphStatus),
    [allPrompts, props.graphStatus],
  );

  if (props.viewport === "small") {
    return (
      <SuggestionPopover
        prompts={visiblePrompts}
        onDispatch={props.onDispatch}
        ariaLabel="What can I do here?"
      />
    );
  }

  return (
    <aside
      data-testid="suggestion-strip"
      aria-label="What can I do here?"
      className="flex flex-wrap items-center gap-2 px-4 py-2"
      style={{ borderBottom: "1px solid var(--color-border)" }}
    >
      {visiblePrompts.map((prompt) => {
        // Stale + graph-dependent → disabled. We render the pill but
        // block the click handler so the hook is never reached.
        const disabled =
          prompt.requiresGraph && props.graphStatus === "stale";
        return (
          <button
            key={prompt.id}
            type="button"
            data-testid={`suggestion-pill-${prompt.id}`}
            data-suggestion-pill=""
            aria-disabled={disabled ? "true" : undefined}
            disabled={disabled}
            title={disabled ? "Graph is stale — re-index to refresh" : undefined}
            onClick={() => {
              if (disabled) return;
              props.onDispatch(prompt);
            }}
            className="rounded-full px-3 py-1 text-xs"
            style={{
              backgroundColor: "var(--color-surface-overlay)",
              color: "var(--color-text-primary)",
              opacity: disabled ? 0.5 : 1,
              cursor: disabled ? "not-allowed" : "pointer",
            }}
          >
            {prompt.label}
          </button>
        );
      })}
    </aside>
  );
}
