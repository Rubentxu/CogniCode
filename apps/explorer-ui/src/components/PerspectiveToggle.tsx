/**
 * `PerspectiveToggle` — Graph ↔ C4 level segmented control.
 *
 * Renders in the Shell header. Dispatches `SET_PERSPECTIVE` to switch
 * the landing graph canvas between graph (symbol neighbourhood) and C4
 * levels (context / container / component / code).
 */
import { useAppDispatch, useAppState } from "../state/context";
import {
  PERSPECTIVE_OPTIONS,
  type Perspective,
} from "../state/c4Levels";

const C4_PERSPECTIVES: Perspective[] = ["c4-context", "c4-container", "c4-component", "c4-code"];

export function PerspectiveToggle() {
  const dispatch = useAppDispatch();
  const { perspective, c4Overlay } = useAppState();

  const isC4Perspective = C4_PERSPECTIVES.includes(perspective as Perspective);

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
      {PERSPECTIVE_OPTIONS.map((option) => {
        const isActive = perspective === option.id;
        return (
          <button
            key={option.id}
            type="button"
            aria-pressed={isActive}
            data-testid={`perspective-${option.id}`}
            onClick={() => dispatch({ type: "SET_PERSPECTIVE", payload: option.id as Perspective })}
            className="rounded px-3 py-1 text-xs transition-colors"
            style={
              isActive
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
            {option.label}
            {option.badge && (
              <span
                aria-hidden="true"
                className="ml-1 text-[10px]"
                style={{ color: "var(--color-text-muted)" }}
                data-testid="perspective-basic-badge"
              >
                {option.badge}
              </span>
            )}
          </button>
        );
      })}
      {/* C4 Overlay toggles */}
      {isC4Perspective && (
        <>
        <div
          aria-hidden="true"
          className="mx-1.5 h-5"
          style={{ width: 1, backgroundColor: "var(--color-border)" }}
        />
        <div className="flex items-center gap-1" data-testid="c4-overlay-toggles">
          <button
            data-testid="c4-overlay-drift"
            aria-pressed={c4Overlay.driftEnabled}
            aria-label="Toggle drift overlay"
            title="Drift: highlights cases where the C4 model diverges from the actual graph (missing containers, wrong sub-kinds)"
            onClick={() => dispatch({ type: "c4-overlay/toggleDrift" })}
            className="text-xs px-2 py-1 rounded border transition-colors"
            style={{
              backgroundColor: c4Overlay.driftEnabled
                ? "color-mix(in srgb, var(--color-error) 18%, transparent)"
                : "var(--color-surface-overlay)",
              borderColor: c4Overlay.driftEnabled
                ? "color-mix(in srgb, var(--color-error) 50%, transparent)"
                : "var(--color-border)",
              color: c4Overlay.driftEnabled
                ? "var(--color-error)"
                : "var(--color-text-secondary)",
            }}
          >
            Drift
          </button>
          <button
            data-testid="c4-overlay-hotspots"
            aria-pressed={c4Overlay.hotspotsEnabled}
            aria-label="Toggle hotspots overlay"
            title="Hotspots: highlights the most-imported / most-depended-on symbols in the workspace"
            onClick={() => dispatch({ type: "c4-overlay/toggleHotspots" })}
            className="text-xs px-2 py-1 rounded border transition-colors"
            style={{
              backgroundColor: c4Overlay.hotspotsEnabled
                ? "color-mix(in srgb, var(--color-warning) 18%, transparent)"
                : "var(--color-surface-overlay)",
              borderColor: c4Overlay.hotspotsEnabled
                ? "color-mix(in srgb, var(--color-warning) 50%, transparent)"
                : "var(--color-border)",
              color: c4Overlay.hotspotsEnabled
                ? "var(--color-warning)"
                : "var(--color-text-secondary)",
            }}
          >
            Hotspots
          </button>
          <button
            data-testid="c4-overlay-boundary-violations"
            aria-pressed={c4Overlay.boundaryViolationsEnabled}
            aria-label="Toggle boundary violations overlay"
            title="Boundary violations: highlights edges that cross architecture boundaries they shouldn't"
            onClick={() => dispatch({ type: "c4-overlay/toggleBoundaryViolations" })}
            className="text-xs px-2 py-1 rounded border transition-colors"
            style={{
              backgroundColor: c4Overlay.boundaryViolationsEnabled
                ? "color-mix(in srgb, var(--color-info) 18%, transparent)"
                : "var(--color-surface-overlay)",
              borderColor: c4Overlay.boundaryViolationsEnabled
                ? "color-mix(in srgb, var(--color-info) 50%, transparent)"
                : "var(--color-border)",
              color: c4Overlay.boundaryViolationsEnabled
                ? "var(--color-info)"
                : "var(--color-text-secondary)",
            }}
          >
            Boundary Violations
          </button>
        </div>
        </>
      )}
    </div>
  );
}
