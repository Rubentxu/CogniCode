/**
 * Unit tests for the LandingWorkbench component.
 *
 * Verifies: 4 tabs render, default active tab, tab switching via click
 * and keyboard (arrow navigation), and C4 forces Graph tab.
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { useReducer } from "react";

// Stub GraphLanding to prevent canvas crashes in jsdom
vi.mock("../GraphLanding/GraphLanding", () => ({
  GraphLanding: () => <div data-testid="graph-landing-canvas">GraphLanding stub</div>,
}));

import { LandingWorkbench } from "./LandingWorkbench";
import {
  AppContext,
  initialState,
  type Action,
  type AppState,
} from "../../state/context";
import { workspaceSummaryFixture } from "../../mocks/fixtures";
import type { LandingWorkbenchState } from "../../state/slices/landingWorkbench";

function LandingWorkbenchWithState({
  landingState = { activeTab: "graph" as LandingWorkbenchState["activeTab"] },
  perspective = "graph" as AppState["perspective"],
  workspaceId = "ws-test-001",
}: {
  landingState?: LandingWorkbenchState;
  perspective?: AppState["perspective"];
  workspaceId?: string;
}) {
  const [state, dispatch] = useReducer(
    (s: AppState, a: Action): AppState => {
      // Handle landingWorkbench slice actions
      if (a.type === "SET_LANDING_TAB") {
        return {
          ...s,
          landingWorkbench: { activeTab: a.payload.tab },
        };
      }
      if (a.type === "RESET") {
        return {
          ...s,
          landingWorkbench: { activeTab: "graph" },
        };
      }
      // Pass through for other actions
      if (a.type === "SET_SPOTTER") return s;
      return s;
    },
    {
      ...initialState,
      workspace: { ...workspaceSummaryFixture, id: workspaceId },
      perspective,
      landingWorkbench: landingState,
    },
  );
  const value = { state, dispatch };
  return (
    <AppContext.Provider value={value}>
      <LandingWorkbench workspaceId={workspaceId} />
    </AppContext.Provider>
  );
}

describe("LandingWorkbench component", () => {
  // NOTE: StartRail (rail rendering, tab interaction, ARIA, keyboard navigation)
  // moved to Shell-level tests (E27.1).

  it("shows progressive entry copy for the active tab", () => {
    render(<LandingWorkbenchWithState landingState={{ activeTab: "start" }} />);
    expect(screen.getByText(/Begin from a meaningful object/i)).toBeVisible();
    expect(screen.getByText(/Choose a precise entry point/i)).toBeVisible();
  });

  it("defaults to graph tab as active", () => {
    render(<LandingWorkbenchWithState />);
    expect(screen.getByTestId("landing-workbench")).toHaveAttribute("data-active-tab", "graph");
  });

  it("shows StartFromSection when start tab is active", () => {
    render(<LandingWorkbenchWithState landingState={{ activeTab: "start" }} />);
    expect(screen.getByTestId("start-from-section")).toBeVisible();
  });

  it("shows InvestigationsSection when investigations tab is active", () => {
    render(<LandingWorkbenchWithState landingState={{ activeTab: "investigations" }} />);
    expect(screen.getByTestId("investigations-section")).toBeVisible();
  });

  it("shows ResumeSection when resume tab is active", () => {
    render(<LandingWorkbenchWithState landingState={{ activeTab: "resume" }} />);
    expect(screen.getByTestId("resume-section")).toBeVisible();
  });
});
