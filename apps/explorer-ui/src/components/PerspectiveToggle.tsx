/**
 * `PerspectiveToggle` — Graph ↔ C4 Components toggle button group.
 *
 * Renders in the Shell header. Dispatches `SET_PERSPECTIVE` to switch
 * the landing graph canvas between graph (entry points) and C4 (component
 * directories) perspectives.
 */
import { useAppDispatch, useAppState } from "../state/context";

export function PerspectiveToggle() {
  const dispatch = useAppDispatch();
  const { perspective } = useAppState();

  return (
    <div
      role="group"
      aria-label="Graph perspective"
      data-testid="perspective-toggle"
      className="flex items-center rounded-md p-0.5"
      style={{
        backgroundColor: "var(--color-surface-overlay)",
        border: "1px solid var(--color-border)",
      }}
    >
      <button
        type="button"
        aria-pressed={perspective === "graph"}
        data-testid="perspective-graph"
        onClick={() => dispatch({ type: "SET_PERSPECTIVE", payload: "graph" })}
        className="rounded px-3 py-1 text-xs transition-colors"
        style={
          perspective === "graph"
            ? {
                backgroundColor: "var(--color-surface-raised)",
                color: "var(--color-text-primary)",
                boxShadow: "0 1px 2px rgba(0,0,0,0.2)",
              }
            : {
                backgroundColor: "transparent",
                color: "var(--color-text-muted)",
              }
        }
      >
        Graph
      </button>
      <button
        type="button"
        aria-pressed={perspective === "c4"}
        data-testid="perspective-c4"
        onClick={() => dispatch({ type: "SET_PERSPECTIVE", payload: "c4" })}
        className="rounded px-3 py-1 text-xs transition-colors"
        style={
          perspective === "c4"
            ? {
                backgroundColor: "var(--color-surface-raised)",
                color: "var(--color-text-primary)",
                boxShadow: "0 1px 2px rgba(0,0,0,0.2)",
              }
            : {
                backgroundColor: "transparent",
                color: "var(--color-text-muted)",
              }
        }
      >
        C4 Components
      </button>
    </div>
  );
}
