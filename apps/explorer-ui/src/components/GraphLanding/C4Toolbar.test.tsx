/**
 * Unit tests for C4Toolbar component.
 *
 * Verifies: visibility gating for C4 perspectives, button renders correctly,
 * and draw.io integration works.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useReducer, type ReactNode } from "react";

// Stub fetchC4Mermaid and handleOpenInDrawIo
vi.mock("../../api/client", () => ({
  fetchC4Mermaid: vi.fn().mockResolvedValue("graph TD\n  A --> B"),
}));

vi.mock("../../utils/drawio", () => ({
  handleOpenInDrawIo: vi.fn().mockResolvedValue(undefined),
}));

import { C4Toolbar } from "./C4Toolbar";
import {
  AppContext,
  initialState,
  type Action,
  type AppState,
} from "../../state/context";
import { NotificationContext } from "../Notifications/NotificationProvider";
import type { C4Level } from "../../state/c4Levels";

function C4ToolbarWithState({
  perspective = "c4-context" as AppState["perspective"],
  workspaceId = "ws-test-001",
  notificationContextValue = { showNotification: vi.fn() },
}: {
  perspective?: AppState["perspective"];
  workspaceId?: string;
  notificationContextValue?: { showNotification: typeof vi.fn };
}) {
  const notifValue = notificationContextValue ?? { showNotification: vi.fn() };

  const [state, dispatch] = useReducer(
    (s: AppState, a: Action): AppState => s,
    {
      ...initialState,
      perspective,
    },
  );
  const value = { state, dispatch };

  return (
    <AppContext.Provider value={value}>
      <NotificationContext.Provider value={notifValue}>
        <C4Toolbar workspaceId={workspaceId} />
      </NotificationContext.Provider>
    </AppContext.Provider>
  );
}

describe("C4Toolbar component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("visibility gating", () => {
    it("renders when perspective is c4-context", () => {
      render(<C4ToolbarWithState perspective="c4-context" />);
      expect(screen.getByTestId("c4-toolbar")).toBeVisible();
    });

    it("renders when perspective is c4-container", () => {
      render(<C4ToolbarWithState perspective="c4-container" />);
      expect(screen.getByTestId("c4-toolbar")).toBeVisible();
    });

    it("renders when perspective is c4-component", () => {
      render(<C4ToolbarWithState perspective="c4-component" />);
      expect(screen.getByTestId("c4-toolbar")).toBeVisible();
    });

    it("does NOT render when perspective is graph", () => {
      render(<C4ToolbarWithState perspective="graph" />);
      expect(screen.queryByTestId("c4-toolbar")).toBeNull();
    });

    it("does NOT render when perspective is c4-code", () => {
      render(<C4ToolbarWithState perspective="c4-code" />);
      expect(screen.queryByTestId("c4-toolbar")).toBeNull();
    });
  });

  describe("button behavior", () => {
    it("renders the Open in draw.io button", () => {
      render(<C4ToolbarWithState perspective="c4-context" />);
      expect(screen.getByTestId("c4-toolbar-open-drawio")).toBeVisible();
      expect(screen.getByTestId("c4-toolbar-open-drawio")).toHaveTextContent("Open in draw.io");
    });

    it("fetches C4 Mermaid and opens draw.io on click", async () => {
      const user = userEvent.setup();
      const { fetchC4Mermaid } = await import("../../api/client");
      const { handleOpenInDrawIo } = await import("../../utils/drawio");

      render(<C4ToolbarWithState perspective="c4-context" />);

      await user.click(screen.getByTestId("c4-toolbar-open-drawio"));

      await waitFor(() => {
        expect(fetchC4Mermaid).toHaveBeenCalledWith("ws-test-001", ".", "c4-context");
      });
      await waitFor(() => {
        expect(handleOpenInDrawIo).toHaveBeenCalled();
      });
    });

    it("shows error notification on fetch failure", async () => {
      const user = userEvent.setup();
      const showNotificationMock = vi.fn();
      const { fetchC4Mermaid } = await import("../../api/client");
      (fetchC4Mermaid as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
        new Error("Network error"),
      );

      render(
        <C4ToolbarWithState
          perspective="c4-context"
          notificationContextValue={{ showNotification: showNotificationMock }}
        />,
      );

      await user.click(screen.getByTestId("c4-toolbar-open-drawio"));

      await waitFor(() => {
        expect(showNotificationMock).toHaveBeenCalledWith("Failed to open diagram in draw.io");
      });
    });
  });
});
