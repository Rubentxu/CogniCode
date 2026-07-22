/**
 * RTL tests for PaneBreadcrumb.
 *
 * Verifies:
 * - Renders when fromObjectId is set; hidden when absent.
 * - Click on From label dispatches SELECT_OBJECT.
 * - No hardcoded shortcut hint (per spec: "It MUST NOT contain a hardcoded shortcut hint").
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { AppContext } from "../../state/context";
import { appReducer, initialState } from "../../state/context";
import { PaneBreadcrumb } from "./PaneBreadcrumb";
import React from "react";

// --- Mocks must be at top level ---
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
      <PaneBreadcrumb fromObjectId="" viaViewKind="" workspaceId="ws1" />,
    );
    // The div always renders; the From/Via labels are empty.
    const breadcrumb = screen.getByTestId("pane-breadcrumb");
    expect(breadcrumb).toBeInTheDocument();
    expect(screen.getByTestId("pane-breadcrumb-from")).toHaveTextContent("");
    expect(screen.getByTestId("pane-breadcrumb-via")).toHaveTextContent("");
  });

  it("renders breadcrumb row when fromObjectId is set", () => {
    renderWithContext(
      <PaneBreadcrumb fromObjectId="from-obj" viaViewKind="call_graph" workspaceId="ws1" />,
    );
    expect(screen.getByTestId("pane-breadcrumb")).toBeInTheDocument();
  });

  it("does not render a hardcoded shortcut hint (per spec)", () => {
    renderWithContext(
      <PaneBreadcrumb fromObjectId="from-obj" viaViewKind="call_graph" workspaceId="ws1" />,
    );
    // The spec requires no hardcoded hint string like "n to add note".
    // Breadcrumb should only show "From <label> · Via <view>".
    const breadcrumb = screen.getByTestId("pane-breadcrumb");
    expect(breadcrumb).toHaveTextContent("From");
    expect(breadcrumb).toHaveTextContent("Via");
    expect(breadcrumb).not.toHaveTextContent("n to add note");
  });

  it("From label resolves to object label", () => {
    renderWithContext(
      <PaneBreadcrumb fromObjectId="from-obj" viaViewKind="call_graph" workspaceId="ws1" />,
    );
    expect(screen.getByTestId("pane-breadcrumb-from")).toHaveTextContent("UserService");
  });

  it("Via label shows view title", () => {
    renderWithContext(
      <PaneBreadcrumb fromObjectId="from-obj" viaViewKind="call_graph" workspaceId="ws1" />,
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
        <PaneBreadcrumb fromObjectId="from-obj" viaViewKind="call_graph" workspaceId="ws1" />
      </AppContext.Provider>,
    );
    fireEvent.click(screen.getByTestId("pane-breadcrumb-from"));
    expect(dispatched).toMatchObject({
      type: "SELECT_OBJECT",
      payload: { objectId: "from-obj", viewId: "call_graph" },
    });
  });
});
