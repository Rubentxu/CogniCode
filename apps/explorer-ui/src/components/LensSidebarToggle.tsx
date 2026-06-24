/**
 * `LensSidebarToggle` — header button to open/close the LensPanel sidebar.
 * Shows a small badge indicating the number of available lenses.
 */

import { useAppDispatch, useAppSelector } from "../state/context";
import { useLenses } from "../hooks/useLenses";

export function LensSidebarToggle(): JSX.Element {
  const dispatch = useAppDispatch();
  const open = useAppSelector((s) => s.lensSidebar.open);
  const activeObjectId = useAppSelector((s) => s.activeObjectId);
  const { data: lenses } = useLenses(activeObjectId);

  const count = lenses?.length ?? 0;

  return (
    <button
      type="button"
      data-testid="lens-sidebar-toggle"
      aria-label={open ? "Hide analysis lenses" : "Show analysis lenses"}
      aria-pressed={open}
      onClick={() => dispatch({ type: "TOGGLE_LENS_SIDEBAR" })}
      className="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs"
      style={{
        backgroundColor: open
          ? "var(--color-accent)"
          : "var(--color-surface-overlay)",
        color: open
          ? "var(--color-accent-foreground)"
          : "var(--color-text-secondary)",
      }}
    >
      <span aria-hidden="true">🔍</span>
      <span>Lenses</span>
      {count > 0 && (
        <span
          className="rounded-full px-1.5 text-[10px] font-semibold"
          style={{
            backgroundColor: open
              ? "var(--color-accent-foreground)"
              : "var(--color-accent)",
            color: open
              ? "var(--color-accent)"
              : "var(--color-accent-foreground)",
          }}
        >
          {count}
        </span>
      )}
    </button>
  );
}
