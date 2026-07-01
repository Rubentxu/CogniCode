/**
 * Unit tests for ExportMenu component.
 *
 * Verifies: render, dropdown toggle, "Open in draw.io" action,
 * Mermaid extraction from view blocks, and download PNG/SVG options.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ExportMenu } from "./ExportMenu";
import { handleOpenInDrawIo } from "../utils/drawio";
import { fetchSnapshot } from "../api/client";
import type { ContextualView } from "../api/types";

vi.mock("../utils/drawio", () => ({
  handleOpenInDrawIo: vi.fn().mockImplementation((_mermaidText, options) => {
    // Simulate the real behavior: call the notify callback if provided
    options?.notify?.("Mermaid copied! In draw.io: Arrange > Insert > Mermaid");
    return Promise.resolve();
  }),
}));

vi.mock("../api/client", () => ({
  fetchSnapshot: vi.fn().mockResolvedValue(new Blob(["fake-image-data"], { type: "image/png" })),
  makeSwrFetcher: vi.fn().mockReturnValue(vi.fn()),
}));

const mockViewWithMermaid: ContextualView = {
  object_id: "obj-1",
  view_id: "call-graph",
  title: "Call Graph",
  view_kind: "call_graph",
  blocks: [
    {
      id: "mermaid-block",
      title: "Mermaid Diagram",
      body: { mermaidText: "graph TD\n  A --> B" },
    },
  ],
  relations: [],
  evidence: [],
  findings: [],
  renderer_kind: "graph",
};

const mockViewWithoutMermaid: ContextualView = {
  object_id: "obj-2",
  view_id: "overview",
  title: "Overview",
  blocks: [
    {
      id: "identity-block",
      title: "Identity",
      body: { label: "SomeSymbol" },
    },
  ],
  relations: [],
  evidence: [],
  findings: [],
  renderer_kind: "json",
};

function ExportMenuWithContext({
  view = null,
  workspaceId = "test-workspace-id",
  onShowNotification = vi.fn(),
}: {
  view?: ContextualView | null;
  workspaceId?: string;
  onShowNotification?: typeof vi.fn;
}) {
  return (
    <ExportMenu view={view} workspaceId={workspaceId} onShowNotification={onShowNotification} />
  );
}

describe("ExportMenu component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the export trigger button", () => {
    render(<ExportMenuWithContext view={null} />);
    expect(screen.getByTestId("export-menu-trigger")).toBeVisible();
    expect(screen.getByTestId("export-menu-trigger")).toHaveTextContent("Export");
  });

  it("opens dropdown menu on click", async () => {
    const user = userEvent.setup();
    render(<ExportMenuWithContext view={null} />);

    await user.click(screen.getByTestId("export-menu-trigger"));

    expect(screen.getByTestId("export-menu-dropdown")).toBeVisible();
    expect(screen.getByTestId("export-menu-open-drawio")).toBeVisible();
  });

  it("closes dropdown when clicking outside", async () => {
    const user = userEvent.setup();
    render(<ExportMenuWithContext view={null} />);

    await user.click(screen.getByTestId("export-menu-trigger"));
    expect(screen.getByTestId("export-menu-dropdown")).toBeVisible();

    // Click outside the menu
    await user.click(document.body);

    await waitFor(() => {
      expect(screen.queryByTestId("export-menu-dropdown")).toBeNull();
    });
  });

  it('shows "Open in draw.io" option in dropdown', async () => {
    const user = userEvent.setup();
    render(<ExportMenuWithContext view={mockViewWithMermaid} />);

    await user.click(screen.getByTestId("export-menu-trigger"));

    expect(screen.getByTestId("export-menu-open-drawio")).toHaveTextContent("Open in draw.io");
  });

  it("extracts mermaidText from view blocks and calls handleOpenInDrawIo", async () => {
    const user = userEvent.setup();
    render(<ExportMenuWithContext view={mockViewWithMermaid} />);

    await user.click(screen.getByTestId("export-menu-trigger"));
    await user.click(screen.getByTestId("export-menu-open-drawio"));

    await waitFor(() => {
      expect(handleOpenInDrawIo).toHaveBeenCalledWith(
        "graph TD\n  A --> B",
        expect.objectContaining({ notify: expect.any(Function) }),
      );
    });
  });

  it("shows notification when no Mermaid is available", async () => {
    const user = userEvent.setup();
    const onShowNotification = vi.fn();

    render(
      <ExportMenuWithContext
        view={mockViewWithoutMermaid}
        onShowNotification={onShowNotification}
      />,
    );

    await user.click(screen.getByTestId("export-menu-trigger"));
    await user.click(screen.getByTestId("export-menu-open-drawio"));

    await waitFor(() => {
      expect(onShowNotification).toHaveBeenCalledWith("No Mermaid diagram available in this view");
    });
    expect(handleOpenInDrawIo).not.toHaveBeenCalled();
  });

  it("shows notification on successful draw.io open", async () => {
    const user = userEvent.setup();
    const onShowNotification = vi.fn();

    render(
      <ExportMenuWithContext
        view={mockViewWithMermaid}
        onShowNotification={onShowNotification}
      />,
    );

    await user.click(screen.getByTestId("export-menu-trigger"));
    await user.click(screen.getByTestId("export-menu-open-drawio"));

    await waitFor(() => {
      expect(onShowNotification).toHaveBeenCalledWith(
        "Mermaid copied! In draw.io: Arrange > Insert > Mermaid",
      );
    });
  });

  it("closes dropdown after selecting Open in draw.io", async () => {
    const user = userEvent.setup();
    render(<ExportMenuWithContext view={mockViewWithMermaid} />);

    await user.click(screen.getByTestId("export-menu-trigger"));
    expect(screen.getByTestId("export-menu-dropdown")).toBeVisible();

    await user.click(screen.getByTestId("export-menu-open-drawio"));

    await waitFor(() => {
      expect(screen.queryByTestId("export-menu-dropdown")).toBeNull();
    });
  });

  it('shows "Download as PNG" option in dropdown', async () => {
    const user = userEvent.setup();
    render(<ExportMenuWithContext view={mockViewWithMermaid} />);

    await user.click(screen.getByTestId("export-menu-trigger"));

    expect(screen.getByTestId("export-menu-download-png")).toHaveTextContent("Download as PNG");
  });

  it('shows "Download as SVG" option in dropdown', async () => {
    const user = userEvent.setup();
    render(<ExportMenuWithContext view={mockViewWithMermaid} />);

    await user.click(screen.getByTestId("export-menu-trigger"));

    expect(screen.getByTestId("export-menu-download-svg")).toHaveTextContent("Download as SVG");
  });

  it("calls fetchSnapshot with correct params when Download PNG is clicked", async () => {
    const user = userEvent.setup();
    render(<ExportMenuWithContext view={mockViewWithMermaid} workspaceId="ws-123" />);

    await user.click(screen.getByTestId("export-menu-trigger"));
    await user.click(screen.getByTestId("export-menu-download-png"));

    await waitFor(() => {
      expect(fetchSnapshot).toHaveBeenCalledWith(
        "ws-123",
        "call_graph",
        "png",
        "obj-1",
      );
    });
  });

  it("calls fetchSnapshot with correct params when Download SVG is clicked", async () => {
    const user = userEvent.setup();
    render(<ExportMenuWithContext view={mockViewWithMermaid} workspaceId="ws-456" />);

    await user.click(screen.getByTestId("export-menu-trigger"));
    await user.click(screen.getByTestId("export-menu-download-svg"));

    await waitFor(() => {
      expect(fetchSnapshot).toHaveBeenCalledWith(
        "ws-456",
        "call_graph",
        "svg",
        "obj-1",
      );
    });
  });

  it("shows notification when view_kind is missing on download", async () => {
    const user = userEvent.setup();
    const onShowNotification = vi.fn();
    const viewWithoutKind: ContextualView = {
      ...mockViewWithMermaid,
      view_kind: undefined,
    };

    render(
      <ExportMenuWithContext
        view={viewWithoutKind}
        workspaceId="ws-123"
        onShowNotification={onShowNotification}
      />,
    );

    await user.click(screen.getByTestId("export-menu-trigger"));
    await user.click(screen.getByTestId("export-menu-download-png"));

    await waitFor(() => {
      expect(onShowNotification).toHaveBeenCalledWith("Cannot download: view kind not available");
    });
  });

  it("closes dropdown after selecting Download PNG", async () => {
    const user = userEvent.setup();
    render(<ExportMenuWithContext view={mockViewWithMermaid} />);

    await user.click(screen.getByTestId("export-menu-trigger"));
    expect(screen.getByTestId("export-menu-dropdown")).toBeVisible();

    await user.click(screen.getByTestId("export-menu-download-png"));

    await waitFor(() => {
      expect(screen.queryByTestId("export-menu-dropdown")).toBeNull();
    });
  });

  it("closes dropdown after selecting Download SVG", async () => {
    const user = userEvent.setup();
    render(<ExportMenuWithContext view={mockViewWithMermaid} />);

    await user.click(screen.getByTestId("export-menu-trigger"));
    expect(screen.getByTestId("export-menu-dropdown")).toBeVisible();

    await user.click(screen.getByTestId("export-menu-download-svg"));

    await waitFor(() => {
      expect(screen.queryByTestId("export-menu-dropdown")).toBeNull();
    });
  });
});
