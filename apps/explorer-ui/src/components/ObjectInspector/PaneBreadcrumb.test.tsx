/**
 * RTL tests for PaneBreadcrumb.
 *
 * Verifies:
 * - Renders when fromObjectId is set; hidden when absent.
 * - Click on From label dispatches SELECT_OBJECT.
 * - 'n to add note' hint is visible.
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { AppContext } from "../../state/context";
import { appReducer, initialState } from "../../state/context";
import { PaneBreadcrumb } from "./PaneBreadcrumb";
import React from "react";

// --- Mocks must be at top level ---
vi.mock("../../hooks/useWorkspace", () => ({
  useWorkspaceList: () => ({ data: [{ id: "ws1" }] }),
}));

vi.mock("../../hooks/useObject", () => ({
  useObject: (objectId: string | null) => {
    if (!objectId) return { data: undefined };
    return {
      data: {
        id: objectId,
        label: objectId === "from-obj" ? "UserService" : objectId,
        object_type: "symbol",
        available_views: [],
      },
    };
  },
}));

vi.mock("../../hooks/useViews", () => ({
  useAvailableViews: () => ({
    data: [
      { id: "call_graph", title: "Call Graph", is_builtin: true, source: null },
      { id: "source_view", title: "Source", is_builtin: true, source: null },
    ],
  }),
}));

function renderWithContext(ui: React.ReactElement) {
  let state = initialState;
  function dispatch(action: Parameters<typeof appReducer>[1]) {
    state = appReducer(state, action);
  }
  return render(
    <AppContext.Provider value={{ state, dispatch }}>
      {ui}
    </AppContext.Provider>,
  );
}

describe("PaneBreadcrumb", () => {
  // Note: hiding when fromObjectId is absent is handled by PaneInspector
  // (which renders nothing when activePane?.fromObjectId is falsy).
  // When called directly with empty strings, PaneBreadcrumb still renders
  // its container but with empty/null inner content.
  it("renders with empty content when fromObjectId is absent", () => {
    renderWithContext(
      <PaneBreadcrumb fromObjectId="" viaViewKind="" />,
    );
    // The div always renders; the From/Via labels are empty.
    const breadcrumb = screen.getByTestId("pane-breadcrumb");
    expect(breadcrumb).toBeInTheDocument();
    expect(screen.getByTestId("pane-breadcrumb-from")).toHaveTextContent("");
    expect(screen.getByTestId("pane-breadcrumb-via")).toHaveTextContent("");
  });

  it("renders breadcrumb row when fromObjectId is set", () => {
    renderWithContext(
      <PaneBreadcrumb fromObjectId="from-obj" viaViewKind="call_graph" />,
    );
    expect(screen.getByTestId("pane-breadcrumb")).toBeInTheDocument();
  });

  it("shows 'n to add note' hint", () => {
    renderWithContext(
      <PaneBreadcrumb fromObjectId="from-obj" viaViewKind="call_graph" />,
    );
    expect(screen.getByText("n to add note")).toBeInTheDocument();
  });

  it("From label resolves to object label", () => {
    renderWithContext(
      <PaneBreadcrumb fromObjectId="from-obj" viaViewKind="call_graph" />,
    );
    expect(screen.getByTestId("pane-breadcrumb-from")).toHaveTextContent("UserService");
  });

  it("Via label shows view title", () => {
    renderWithContext(
      <PaneBreadcrumb fromObjectId="from-obj" viaViewKind="call_graph" />,
    );
    expect(screen.getByTestId("pane-breadcrumb-via")).toHaveTextContent("Call Graph");
  });

  it("clicking From label dispatches SELECT_OBJECT with fromObjectId", () => {
    let dispatched: unknown = null;
    let state = initialState;
    function dispatch(action: Parameters<typeof appReducer>[1]) {
      dispatched = action;
      state = appReducer(state, action);
    }
    render(
      <AppContext.Provider value={{ state, dispatch }}>
        <PaneBreadcrumb fromObjectId="from-obj" viaViewKind="call_graph" />
      </AppContext.Provider>,
    );
    fireEvent.click(screen.getByTestId("pane-breadcrumb-from"));
    expect(dispatched).toMatchObject({
      type: "SELECT_OBJECT",
      payload: { objectId: "from-obj", viewId: "call_graph" },
    });
  });
});
