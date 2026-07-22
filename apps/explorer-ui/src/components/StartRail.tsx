import { type KeyboardEvent, useCallback, useRef } from "react";
import { useAppDispatch, useAppState } from "../state/context";
import type { LandingTabId } from "../state/slices/landingWorkbench";

const RAIL_ITEMS: ReadonlyArray<{
  id: LandingTabId;
  label: string;
  description: string;
}> = [
  {
    id: "start",
    label: "Start from",
    description: "Enter through a meaningful object or question.",
  },
  {
    id: "investigations",
    label: "Investigations",
    description: "Continue active architecture and evidence work.",
  },
  {
    id: "resume",
    label: "Recent",
    description: "Pick up a saved or recent exploration thread.",
  },
  {
    id: "graph",
    label: "Graph",
    description: "Drop into the canvas when you need the whole map.",
  },
];

export function StartRail() {
  const dispatch = useAppDispatch();
  const {
    landingWorkbench: { activeTab },
  } = useAppState();
  const containerRef = useRef<HTMLDivElement | null>(null);

  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      const ids = RAIL_ITEMS.map((item) => item.id);
      const currentIndex = ids.indexOf(activeTab);
      const safeIndex = currentIndex < 0 ? 0 : currentIndex;

      let computed: number;
      switch (event.key) {
        case "ArrowDown":
        case "ArrowRight":
          computed = (safeIndex + 1) % ids.length;
          break;
        case "ArrowUp":
        case "ArrowLeft":
          computed = (safeIndex - 1 + ids.length) % ids.length;
          break;
        case "Home":
          computed = 0;
          break;
        case "End":
          computed = ids.length - 1;
          break;
        default:
          return;
      }

      event.preventDefault();
      const nextId = ids[computed];
      if (nextId) {
        dispatch({ type: "SET_LANDING_TAB", payload: { tab: nextId } });
        const btn = containerRef.current?.querySelector<HTMLButtonElement>(
          `[data-rail-id="${nextId}"]`,
        );
        btn?.focus();
      }
    },
    [activeTab, dispatch],
  );

  return (
    <aside
      data-testid="start-rail"
      className="flex min-w-0 flex-col gap-4 border-b px-4 py-4 md:w-72 md:flex-shrink-0 md:border-b-0 md:border-r"
      style={{
        borderColor: "var(--color-border)",
        backgroundColor: "var(--color-surface-raised)",
      }}
      aria-label="Explorer entry rail"
    >
      <div className="space-y-1">
        <p
          className="text-[11px] font-semibold uppercase tracking-[0.08em]"
          style={{ color: "var(--color-text-muted)" }}
        >
          Workbench
        </p>
        <h2 className="text-sm font-semibold" style={{ color: "var(--color-text-primary)" }}>
          Start or continue
        </h2>
        <p className="text-xs leading-5" style={{ color: "var(--color-text-secondary)" }}>
          Enter through an object, resume a thread, or switch to the graph when
          you need the broadest view.
        </p>
      </div>

      <div
        ref={containerRef}
        role="tablist"
        aria-orientation="vertical"
        aria-label="Workbench entry rail"
        onKeyDown={onKeyDown}
        className="grid gap-2"
      >
        {RAIL_ITEMS.map((item) => {
          const isActive = item.id === activeTab;
          return (
            <button
              key={item.id}
              type="button"
              role="tab"
              aria-selected={isActive}
              tabIndex={isActive ? 0 : -1}
              data-testid={`landing-tab-${item.id}`}
              data-rail-id={item.id}
              onClick={() => dispatch({ type: "SET_LANDING_TAB", payload: { tab: item.id } })}
              className="rounded-lg border px-3 py-3 text-left transition-colors"
              style={{
                borderColor: isActive ? "var(--color-primary)" : "var(--color-border)",
                backgroundColor: isActive
                  ? "color-mix(in srgb, var(--color-primary) 12%, var(--color-surface-raised))"
                  : "var(--color-surface)",
              }}
            >
              <div className="text-sm font-medium" style={{ color: "var(--color-text-primary)" }}>
                {item.label}
              </div>
              <p className="mt-1 text-xs leading-5" style={{ color: "var(--color-text-secondary)" }}>
                {item.description}
              </p>
            </button>
          );
        })}
      </div>
    </aside>
  );
}
