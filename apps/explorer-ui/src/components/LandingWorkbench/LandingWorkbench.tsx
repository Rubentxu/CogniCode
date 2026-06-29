/**
 * LandingWorkbench — tabbed landing page for CogniCode Explorer.
 *
 * Replaces the graph-only landing with a 4-tab workbench:
 *   1. Start From — entry point types (Route / Use case / Symbol / Event / Saved exploration)
 *   2. Investigations — common investigations (Trace a request, Find impact radius, etc.)
 *   3. Resume — recent explorations (wraps RecentExplorationsStrip)
 *   4. Graph — existing GraphLanding (rendered as-is, zero regression)
 *
 * Tab state is held in the landingWorkbench slice. C4 perspective
 * automatically switches to the Graph tab (since C4 is a canvas mode).
 *
 * ARIA: implements the WAI-ARIA Tabs pattern with roving focus
 * (arrow keys move between tabs, Home/End jump to first/last).
 */
import { useCallback, useEffect, useRef, type KeyboardEvent } from "react";
import { useAppDispatch, useAppState } from "../../state/context";
import { isGraphPerspective } from "../../state/c4Levels";
import type { LandingTabId } from "../../state/slices/landingWorkbench";
import { GraphLanding } from "../GraphLanding/GraphLanding";
import { StartFromSection } from "./StartFromSection";
import { InvestigationsSection } from "./InvestigationsSection";
import { ResumeSection } from "./ResumeSection";

export interface LandingWorkbenchProps {
  workspaceId: string;
}

const TABS: ReadonlyArray<{ id: LandingTabId; label: string }> = [
  { id: "start", label: "Start From" },
  { id: "investigations", label: "Investigations" },
  { id: "resume", label: "Resume" },
  { id: "graph", label: "Graph" },
];

export function LandingWorkbench({ workspaceId }: LandingWorkbenchProps) {
  const dispatch = useAppDispatch();
  const { landingWorkbench, perspective } = useAppState();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const previousTabRef = useRef<LandingTabId>(landingWorkbench.activeTab);

  // C4 perspective: stash previous tab, force Graph. Restore on exit.
  useEffect(() => {
    if (!isGraphPerspective(perspective)) {
      // Save current tab before forcing Graph
      if (landingWorkbench.activeTab !== "graph") {
        previousTabRef.current = landingWorkbench.activeTab;
        dispatch({ type: "SET_LANDING_TAB", payload: { tab: "graph" } });
      }
    } else {
      // Restore previous tab when leaving C4
      if (previousTabRef.current !== "graph" && landingWorkbench.activeTab === "graph") {
        dispatch({ type: "SET_LANDING_TAB", payload: { tab: previousTabRef.current } });
      }
    }
  }, [perspective, landingWorkbench.activeTab, dispatch]);

  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      const ids = TABS.map((t) => t.id);
      const currentIndex = ids.indexOf(landingWorkbench.activeTab);
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
        default:
          return;
      }
      event.preventDefault();
      const nextId = ids[computed];
      if (nextId) {
        dispatch({ type: "SET_LANDING_TAB", payload: { tab: nextId } });
        const btn = containerRef.current?.querySelector<HTMLButtonElement>(
          `[data-tab-id="${nextId}"]`,
        );
        btn?.focus();
      }
    },
    [landingWorkbench.activeTab, dispatch],
  );

  return (
    <div
      data-testid="landing-workbench"
      data-active-tab={landingWorkbench.activeTab}
      className="flex h-full flex-col"
      style={{ backgroundColor: "var(--color-surface)" }}
    >
      <div
        ref={containerRef}
        role="tablist"
        aria-label="Landing workbench tabs"
        onKeyDown={onKeyDown}
        className="flex items-center gap-1 border-b px-4 py-2"
        style={{ borderColor: "var(--color-border)" }}
      >
        {TABS.map((tab) => {
          const isActive = tab.id === landingWorkbench.activeTab;
          return (
            <button
              key={tab.id}
              type="button"
              role="tab"
              aria-selected={isActive}
              tabIndex={isActive ? 0 : -1}
              data-testid={`landing-tab-${tab.id}`}
              data-tab-id={tab.id}
              onClick={() =>
                dispatch({ type: "SET_LANDING_TAB", payload: { tab: tab.id } })
              }
              className="rounded-md px-3 py-1.5 text-xs font-medium transition-colors"
              style={{
                backgroundColor: isActive
                  ? "var(--color-primary)"
                  : "var(--color-surface-overlay)",
                color: isActive
                  ? "var(--color-primary-foreground)"
                  : "var(--color-text-secondary)",
              }}
            >
              {tab.label}
            </button>
          );
        })}
      </div>
      <div
        role="tabpanel"
        aria-labelledby={`landing-tab-${landingWorkbench.activeTab}`}
        className="flex-1 overflow-auto"
      >
        {landingWorkbench.activeTab === "start" && <StartFromSection />}
        {landingWorkbench.activeTab === "investigations" && (
          <InvestigationsSection />
        )}
        {landingWorkbench.activeTab === "resume" && (
          <ResumeSection workspaceId={workspaceId} />
        )}
        {landingWorkbench.activeTab === "graph" && (
          <GraphLanding workspaceId={workspaceId} />
        )}
      </div>
    </div>
  );
}
