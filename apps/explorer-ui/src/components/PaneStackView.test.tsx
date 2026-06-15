/**
 * PaneStackView — smoke tests.
 */
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { useReducer } from "react";

import {
  AppContext,
  appReducer,
  initialStateWithFocus,
  type AppState,
} from "../state/context";
import { PaneStackView } from "./PaneStackView";

describe("PaneStackView", () => {
  it("renders empty state when no panes", () => {
    const s = initialStateWithFocus("a", "pane-stack", "overview", "symbol");
    const firstPaneId = s.navigation.panes[0]?.id ?? "";
    const state: AppState = appReducer(s, {
      type: "CLOSE_PANE",
      payload: { paneId: firstPaneId },
    });

    function Harness() {
      const [st] = useReducer(
        (_prev: AppState) => state,
        state,
      );
      return (
        <AppContext.Provider value={{ state: st, dispatch: () => {} }}>
          <PaneStackView />
        </AppContext.Provider>
      );
    }
    render(<Harness />);
    expect(screen.getByTestId("pane-stack-empty")).toBeInTheDocument();
  });
});
