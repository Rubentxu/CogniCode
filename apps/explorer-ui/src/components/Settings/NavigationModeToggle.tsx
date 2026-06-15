/**
 * NavigationModeToggle — UI for switching between column-based
 * and pane-stack navigation.
 *
 * Sits in the settings menu (or wherever the host wants it). On
 * change, dispatches `SET_NAVIGATION_MODE` so the reducer swaps
 * the active adapter atomically, and persists the preference to
 * localStorage via `useNavigationMode`.
 *
 * See ADR-016 for rationale.
 */
import { useAppDispatch } from "../../state/context";
import { useNavigationMode, type NavigationMode } from "../../state/navigation";

interface NavigationModeToggleProps {
  /**
   * Optional override label for the option, used by tests to
   * disambiguate. Production: "Vertical drill-down" / "Side-by-side panes".
   */
  labels?: Partial<Record<NavigationMode, string>>;
  /**
   * Optional test hook. Defaults to a `<select>`.
   */
  as?: "select" | "buttons";
}

const DEFAULT_LABELS: Record<NavigationMode, string> = {
  column: "Vertical drill-down (default)",
  "pane-stack": "Side-by-side panes (gtoolkit-style)",
};

export function NavigationModeToggle({
  labels = {},
  as = "select",
}: NavigationModeToggleProps) {
  const dispatch = useAppDispatch();
  const [mode, setMode] = useNavigationMode(dispatch);
  const labelFor = (m: NavigationMode) => labels[m] ?? DEFAULT_LABELS[m];

  if (as === "buttons") {
    return (
      <div role="radiogroup" aria-label="Navigation mode" className="flex gap-2">
        {(["column", "pane-stack"] as const).map((m) => (
          <button
            key={m}
            type="button"
            role="radio"
            aria-checked={mode === m}
            data-testid={`nav-mode-${m}`}
            onClick={() => setMode(m)}
            className={mode === m ? "nav-mode-active" : "nav-mode-inactive"}
          >
            {labelFor(m)}
          </button>
        ))}
      </div>
    );
  }

  return (
    <label className="flex items-center gap-2 text-sm">
      <span>Navigation</span>
      <select
        value={mode}
        onChange={(e) => setMode(e.target.value as NavigationMode)}
        data-testid="nav-mode-select"
        aria-label="Navigation mode"
      >
        {(["column", "pane-stack"] as const).map((m) => (
          <option key={m} value={m}>
            {labelFor(m)}
          </option>
        ))}
      </select>
    </label>
  );
}
