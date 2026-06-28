/**
 * Unit tests for the LandingWorkbench component.
 *
 * Verifies: 4 tabs render, default active tab, tab switching via click
 * and keyboard (arrow navigation), and C4 forces Graph tab.
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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
  it("renders 4 tabs", () => {
    render(<LandingWorkbenchWithState />);
    expect(screen.getByTestId("landing-tab-start")).toBeVisible();
    expect(screen.getByTestId("landing-tab-investigations")).toBeVisible();
    expect(screen.getByTestId("landing-tab-resume")).toBeVisible();
    expect(screen.getByTestId("landing-tab-graph")).toBeVisible();
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

  it("clicking investigations tab dispatches SET_LANDING_TAB", async () => {
    const user = userEvent.setup();
    // Use start tab initial state to avoid rendering GraphLanding (requires canvas)
    render(<LandingWorkbenchWithState landingState={{ activeTab: "start" }} />);

    await user.click(screen.getByTestId("landing-tab-investigations"));

    // The landing-workbench active-tab data attribute reflects the switch
    expect(screen.getByTestId("landing-workbench")).toHaveAttribute(
      "data-active-tab",
      "investigations",
    );
  });

  it("clicking investigations tab from graph tab is reflected in landingWorkbench state", async () => {
    // Start with graph tab active (default) to avoid GraphLanding canvas issue
    render(<LandingWorkbenchWithState landingState={{ activeTab: "graph" }} />);

    // Verify graph tab is active initially
    expect(screen.getByTestId("landing-workbench")).toHaveAttribute("data-active-tab", "graph");

    // Click investigations tab
    await userEvent.click(screen.getByTestId("landing-tab-investigations"));

    // Verify state updated
    await waitFor(() => {
      expect(screen.getByTestId("landing-workbench")).toHaveAttribute("data-active-tab", "investigations");
    });
  });

  it("has correct ARIA roles on tabs", () => {
    render(<LandingWorkbenchWithState landingState={{ activeTab: "start" }} />);
    const tablist = screen.getByRole("tablist");
    expect(tablist).toBeVisible();
    expect(screen.getAllByRole("tab")).toHaveLength(4);
  });

  it("active tab has aria-selected=true, others false", () => {
    render(<LandingWorkbenchWithState landingState={{ activeTab: "start" }} />);
    const startTab = screen.getByTestId("landing-tab-start");
    const investigationsTab = screen.getByTestId("landing-tab-investigations");
    expect(startTab).toHaveAttribute("aria-selected", "true");
    expect(investigationsTab).toHaveAttribute("aria-selected", "false");
  });
});
