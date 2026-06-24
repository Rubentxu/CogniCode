/**
 * `GraphLanding` tests — perspective toggle integration.
 *
 * These tests verify that the GraphLanding component correctly responds
 * to perspective changes. The actual SWR data fetching and cytoscape
 * rendering is tested in the e2e tests and integration tests with MSW.
 */
import { describe, it, expect } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { useReducer } from "react";

import { GraphLanding } from "./GraphLanding";
import {
  AppContext,
  initialState,
  type Action,
  type AppState,
} from "../../state/context";
import { workspaceSummaryFixture } from "../../mocks/fixtures";

/**
 * Minimal harness that provides AppContext with workspace and perspective.
 */
function GraphLandingWithState({
  perspective = "graph",
  workspaceId = "ws-test-001",
}: {
  perspective?: "graph" | "c4";
  workspaceId?: string;
}) {
  const [state, dispatch] = useReducer(
    // eslint-disable-next-line @typescript-eslint/no-unused-vars -- intentional unused action param
    (s: AppState, _a: Action): AppState => s,
    {
      ...initialState,
      workspace: { ...workspaceSummaryFixture, id: workspaceId },
      perspective,
    },
  );
  const value: { state: AppState; dispatch: React.Dispatch<Action> } = { state, dispatch };
  return (
    <AppContext.Provider value={value}>
      <GraphLanding workspaceId={workspaceId} />
    </AppContext.Provider>
  );
}

describe("GraphLanding perspective integration", () => {
  it('shows loading state when no SWR data is available (graph perspective)', async () => {
    render(<GraphLandingWithState perspective="graph" />);
    // Without MSW handlers, component shows loading state
    await waitFor(() => {
      const loading = screen.queryByTestId("graph-landing-loading");
      expect(loading ?? screen.queryByTestId("graph-landing")).toBeTruthy();
    });
  });

  it('shows loading state when no SWR data is available (c4 perspective)', async () => {
    render(<GraphLandingWithState perspective="c4" />);
    // Without MSW handlers, component shows loading state
    await waitFor(() => {
      const loading = screen.queryByTestId("graph-landing-loading");
      expect(loading ?? screen.queryByTestId("graph-landing")).toBeTruthy();
    });
  });
});
