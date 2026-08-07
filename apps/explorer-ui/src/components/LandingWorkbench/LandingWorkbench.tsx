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
import { useEffect, useRef } from "react";
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

const TAB_COPY = {
  start: {
    eyebrow: "Entry",
    title: "Begin from a meaningful object",
    description:
      "Choose a precise entry point. Start calm, then deepen through panes and representations.",
  },
  investigations: {
    eyebrow: "Continuity",
    title: "Work through live investigations",
    description:
      "Use investigations as the durable thread that connects evidence, artifacts, and architectural reasoning.",
  },
  resume: {
    eyebrow: "Recent work",
    title: "Resume an existing exploration",
    description:
      "Pick up a previous path without reconstructing the full context from memory.",
  },
  graph: {
    eyebrow: "Map",
    title: "Step back to the broadest system view",
    description:
      "Use the graph when you need orientation, topology, and large-scale relationships before drilling back in.",
  },
} as const;

export function LandingWorkbench({ workspaceId }: LandingWorkbenchProps) {
  const dispatch = useAppDispatch();
  const { landingWorkbench, perspective } = useAppState();
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

  const copy = TAB_COPY[landingWorkbench.activeTab];

  return (
    <div
      data-testid="landing-workbench"
      data-active-tab={landingWorkbench.activeTab}
      className="flex h-full flex-col"
      style={{ backgroundColor: "var(--color-surface)" }}
    >
      <div className="flex flex-1 overflow-hidden md:min-h-0">
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <div
            className="border-b px-6 py-5"
            style={{ borderColor: "var(--color-border)", backgroundColor: "var(--color-surface)" }}
          >
            <p
              className="text-xs font-medium"
              style={{ color: "var(--color-primary)" }}
            >
              {copy.eyebrow}
            </p>
            <h2
              className="mt-1.5 text-lg font-semibold"
              style={{ color: "var(--color-text-primary)" }}
            >
              {copy.title}
            </h2>
            <p
              className="mt-2 max-w-[72ch] text-sm leading-6"
              style={{ color: "var(--color-text-secondary)" }}
            >
              {copy.description}
            </p>
          </div>

          <div
            role="tabpanel"
            aria-labelledby={`landing-tab-${landingWorkbench.activeTab}`}
            className="flex-1 overflow-auto"
          >
            {landingWorkbench.activeTab === "start" && <StartFromSection />}
            {landingWorkbench.activeTab === "investigations" && (
              <InvestigationsSection workspaceId={workspaceId} />
            )}
            {landingWorkbench.activeTab === "resume" && (
              <ResumeSection workspaceId={workspaceId} />
            )}
            {landingWorkbench.activeTab === "graph" && (
              <GraphLanding workspaceId={workspaceId} />
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
