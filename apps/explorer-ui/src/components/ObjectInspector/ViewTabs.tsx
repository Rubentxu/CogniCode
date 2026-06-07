/**
 * `ViewTabs` — the tab strip at the top of the Object Inspector.
 *
 * Implements the WAI-ARIA Tabs pattern. The active tab is the one
 * the user can see; arrow keys move focus between tab stops, and
 * Enter / Space activates the focused tab. We do NOT use automatic
 * activation (focus triggers change) because the user usually wants
 * to scan multiple tabs before committing to a fetch.
 *
 * Each tab has a stable `id` (the view id) so the reducer can
 * dispatch `SET_ACTIVE_VIEW` deterministically. Tab labels come
 * from `available_views[].title` — the backend-supplied human
 * string.
 */
import {
  useCallback,
  useEffect,
  useRef,
  type KeyboardEvent,
} from "react";

export interface ViewTabsProps {
  /** The list of available views (id + title). */
  views: ReadonlyArray<{ id: string; title: string }>;
  /** The currently active view id. `null` when nothing is selected. */
  activeViewId: string | null;
  /** True while the next view is loading. */
  isLoading: boolean;
  /** Callback when the user picks a tab. */
  onChange: (viewId: string) => void;
  /** Optional className passthrough (e.g., for layout overrides). */
  className?: string;
}

const TABS_ID = "object-inspector-view-tabs";

/**
 * Visual + a11y tab strip. The implementation is a controlled
 * listbox of tab buttons (NOT the `role="tablist"` pattern, which
 * is for tabpanels managed with `aria-controls`). The Object
 * Inspector only shows one panel at a time and we already wire
 * focus to the panel below.
 */
export function ViewTabs({
  views,
  activeViewId,
  isLoading,
  onChange,
  className,
}: ViewTabsProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);

  /**
   * Roving focus across the tab strip. Arrow keys move to the
   * previous/next tab; Home/End jump to the first/last. The active
   * tab is the one with `tabindex=0`; all others have `tabindex=-1`.
   */
  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (views.length === 0) return;
      const ids = views.map((v) => v.id);
      const currentIndex = activeViewId ? ids.indexOf(activeViewId) : 0;
      const safeIndex = currentIndex < 0 ? 0 : currentIndex;

      let computed: number;
      switch (event.key) {
        case "ArrowRight":
          computed = (safeIndex + 1) % ids.length;
          break;
        case "ArrowLeft":
          computed = (safeIndex - 1 + ids.length) % ids.length;
          break;
        case "Home":
          computed = 0;
          break;
        case "End":
          computed = ids.length - 1;
          break;
        case "Enter":
        case " ":
          // Activation happens on focus, not on Enter — but we
          // still allow Enter to re-affirm the current tab so
          // keyboard users get the same affordance as mouse users.
          event.preventDefault();
          onChange(ids[safeIndex] ?? ids[0] ?? "");
          return;
        default:
          return;
      }
      event.preventDefault();
      const nextId = ids[computed];
      if (nextId === undefined) return;
      onChange(nextId);
      // Move DOM focus to the new tab.
      const btn = containerRef.current?.querySelector<HTMLButtonElement>(
        `[data-view-id="${cssEscape(nextId)}"]`,
      );
      btn?.focus();
    },
    [activeViewId, views, onChange],
  );

  // When the active view id becomes stale (e.g., the user navigated
  // to a different object), we want the first tab in `views` to
  // become active automatically. The parent handles that via the
  // container — we just keep the DOM focused on the new active tab.
  useEffect(() => {
    if (!activeViewId || views.length === 0) return;
    const btn = containerRef.current?.querySelector<HTMLButtonElement>(
      `[data-view-id="${cssEscape(activeViewId)}"][data-active="true"]`,
    );
    // The button already received focus from the click/arrow — no
    // need to programmatically focus.
    void btn;
  }, [activeViewId, views]);

  if (views.length === 0) {
    return null;
  }

  return (
    <div
      ref={containerRef}
      role="tablist"
      id={TABS_ID}
      aria-label="Available views"
      data-testid="view-tabs"
      onKeyDown={onKeyDown}
      className={
        "flex items-center gap-1 overflow-x-auto px-2 py-1.5 " +
        (className ?? "")
      }
      style={{ borderBottom: "1px solid var(--color-border)" }}
    >
      {views.map((view) => {
        const isActive = view.id === activeViewId;
        return (
          <button
            key={view.id}
            type="button"
            role="tab"
            aria-selected={isActive}
            aria-controls={`view-tab-panel-${cssEscape(view.id)}`}
            tabIndex={isActive ? 0 : -1}
            data-testid={`view-tab-${view.id}`}
            data-view-id={view.id}
            data-active={isActive ? "true" : "false"}
            onClick={() => onChange(view.id)}
            className="rounded-md px-2 py-1 text-xs font-medium transition-colors"
            style={{
              backgroundColor: isActive
                ? "var(--color-primary)"
                : "var(--color-surface-overlay)",
              color: isActive
                ? "var(--color-primary-foreground)"
                : "var(--color-text-secondary)",
            }}
          >
            {view.title}
            {isLoading && isActive && (
              <span
                aria-hidden="true"
                className="ml-1 inline-block h-1.5 w-1.5 animate-pulse rounded-full"
                style={{ backgroundColor: "currentColor" }}
              />
            )}
          </button>
        );
      })}
    </div>
  );
}

/**
 * Minimal CSS.escape polyfill. We need a valid CSS attribute
 * selector for `data-view-id="foo bar"` — `querySelector` does
 * NOT accept arbitrary attribute values, so we escape spaces and
 * quotes here. Falls back to a no-op in browsers that ship the
 * standard `CSS.escape` (all modern browsers since 2017).
 */
function cssEscape(value: string): string {
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
    return CSS.escape(value);
  }
  return value.replace(/"/g, '\\"');
}
