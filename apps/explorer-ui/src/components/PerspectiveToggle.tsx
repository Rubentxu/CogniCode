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
        <div className="flex items-center gap-1 ml-2" data-testid="c4-overlay-toggles">
          <button
            data-testid="c4-overlay-drift"
            aria-pressed={c4Overlay.driftEnabled}
            onClick={() => dispatch({ type: "c4-overlay/toggleDrift" })}
            className={
              "text-xs px-2 py-1 rounded border transition-colors " +
              (c4Overlay.driftEnabled
                ? "bg-red-50 border-red-300 text-red-700"
                : "bg-gray-50 border-gray-200 text-gray-500")
            }
          >
            Drift
          </button>
          <button
            data-testid="c4-overlay-hotspots"
            aria-pressed={c4Overlay.hotspotsEnabled}
            onClick={() => dispatch({ type: "c4-overlay/toggleHotspots" })}
            className={
              "text-xs px-2 py-1 rounded border transition-colors " +
              (c4Overlay.hotspotsEnabled
                ? "bg-orange-50 border-orange-300 text-orange-700"
                : "bg-gray-50 border-gray-200 text-gray-500")
            }
          >
            Hotspots
          </button>
          <button
            data-testid="c4-overlay-boundary-violations"
            aria-pressed={c4Overlay.boundaryViolationsEnabled}
            onClick={() => dispatch({ type: "c4-overlay/toggleBoundaryViolations" })}
            className={
              "text-xs px-2 py-1 rounded border transition-colors " +
              (c4Overlay.boundaryViolationsEnabled
                ? "bg-blue-50 border-blue-300 text-blue-700"
                : "bg-gray-50 border-gray-200 text-gray-500")
            }
          >
            Boundary Violations
          </button>
        </div>
      )}
    </div>
  );
}
