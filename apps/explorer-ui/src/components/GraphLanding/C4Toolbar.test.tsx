/**
 * Unit tests for C4Toolbar component.
 *
 * Verifies: visibility gating for C4 perspectives, button renders correctly,
 * and draw.io integration works.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useReducer } from "react";

// Stub fetchC4Mermaid, fetchSnapshot, and handleOpenInDrawIo
vi.mock("../../api/client", () => ({
  fetchC4Mermaid: vi.fn().mockResolvedValue("graph TD\n  A --> B"),
  fetchSnapshot: vi.fn().mockResolvedValue(new Blob(["fake-png"], { type: "image/png" })),
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
    // eslint-disable-next-line @typescript-eslint/no-unused-vars -- intentional unused action param
    (s: AppState, _action: Action): AppState => s,
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

  describe("download buttons", () => {
    it("renders the Download PNG button", () => {
      render(<C4ToolbarWithState perspective="c4-context" />);
      expect(screen.getByTestId("c4-toolbar-download-png")).toBeVisible();
      expect(screen.getByTestId("c4-toolbar-download-png")).toHaveTextContent("Download PNG");
    });

    it("renders the Download SVG button", () => {
      render(<C4ToolbarWithState perspective="c4-context" />);
      expect(screen.getByTestId("c4-toolbar-download-svg")).toBeVisible();
      expect(screen.getByTestId("c4-toolbar-download-svg")).toHaveTextContent("Download SVG");
    });

    it("calls fetchSnapshot with png format when Download PNG is clicked", async () => {
      const user = userEvent.setup();
      const { fetchSnapshot } = await import("../../api/client");

      render(<C4ToolbarWithState perspective="c4-context" workspaceId="ws-test-001" />);

      await user.click(screen.getByTestId("c4-toolbar-download-png"));

      await waitFor(() => {
        expect(fetchSnapshot).toHaveBeenCalledWith(
          "ws-test-001",
          "c4_context",
          "png",
          ".",
        );
      });
    });

    it("calls fetchSnapshot with svg format when Download SVG is clicked", async () => {
      const user = userEvent.setup();
      const { fetchSnapshot } = await import("../../api/client");

      render(<C4ToolbarWithState perspective="c4-container" workspaceId="ws-test-002" />);

      await user.click(screen.getByTestId("c4-toolbar-download-svg"));

      await waitFor(() => {
        expect(fetchSnapshot).toHaveBeenCalledWith(
          "ws-test-002",
          "c4_container",
          "svg",
          ".",
        );
      });
    });

    it("maps perspective c4-component to c4_component view_kind", async () => {
      const user = userEvent.setup();
      const { fetchSnapshot } = await import("../../api/client");

      render(<C4ToolbarWithState perspective="c4-component" workspaceId="ws-test-003" />);

      await user.click(screen.getByTestId("c4-toolbar-download-png"));

      await waitFor(() => {
        expect(fetchSnapshot).toHaveBeenCalledWith(
          "ws-test-003",
          "c4_component",
          "png",
          ".",
        );
      });
    });

    it("shows error notification on snapshot fetch failure", async () => {
      const user = userEvent.setup();
      const showNotificationMock = vi.fn();
      const { fetchSnapshot } = await import("../../api/client");
      (fetchSnapshot as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
        new Error("Network error"),
      );

      render(
        <C4ToolbarWithState
          perspective="c4-context"
          notificationContextValue={{ showNotification: showNotificationMock }}
        />,
      );

      await user.click(screen.getByTestId("c4-toolbar-download-png"));

      await waitFor(() => {
        expect(showNotificationMock).toHaveBeenCalledWith("Failed to download PNG snapshot");
      });
    });
  });
});
