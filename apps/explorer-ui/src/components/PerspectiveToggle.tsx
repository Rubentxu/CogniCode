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
    </div>
  );
}
