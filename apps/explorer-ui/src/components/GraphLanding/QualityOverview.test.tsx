/**
 * `QualityOverview` tests — PR #35 moldable wiring phase1.
 *
 * Verifies:
 * 1. Renders the quality overview section.
 * 2. Collapse toggle hides/shows the QualityDashboard.
 * 3. Shows error state when fetch fails.
 * 4. Renders QualityDashboard with data when fetch succeeds.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http, HttpResponse } from "msw";
import { createElement, type ReactNode } from "react";
import { SWRConfig } from "swr";

import { server } from "../../mocks/node";
import { QualityOverview } from "./QualityOverview";
import type { WorkspaceQualityPayload } from "../../api/client";

const WORKSPACE_ID = "ws-test-quality-001";

const MOCK_PAYLOAD: WorkspaceQualityPayload = {
  summary: {
    scope: "workspace",
    rating: "B",
    total_issues: 3,
    debt_minutes: 60,
    by_severity: {
      blocker: 0,
      critical: 1,
      major: 1,
      minor: 1,
      info: 0,
    },
    last_run: "2026-06-07T09:00:00Z",
  },
  issues: [
    {
      id: 1,
      rule_id: "rust:S100",
      severity: "critical",
      category: "safety",
      file: "src/lib.rs",
      line: 42,
      message: "Critical safety issue",
      status: "open",
      object_id: "issue:1",
    },
  ],
};

// ─── SWR wrapper ─────────────────────────────────────────────────────────────

function withSWR() {
  return function Wrapper({ children }: { children: ReactNode }) {
    return createElement(SWRConfig, {
      value: { provider: () => new Map(), dedupingInterval: 0 },
    }, children);
  };
}

// ─── Tests ───────────────────────────────────────────────────────────────────

describe("QualityOverview", () => {
  beforeEach(() => {
    server.resetHandlers();
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("renders the quality overview section", async () => {
    // Override with handler that returns data
    server.use(
      http.get(`/api/workspaces/${WORKSPACE_ID}/quality-summary`, () =>
        HttpResponse.json(MOCK_PAYLOAD),
      ),
    );

    render(createElement(QualityOverview, { workspaceId: WORKSPACE_ID }), {
      wrapper: withSWR(),
    });

    await waitFor(() => {
      expect(screen.getByTestId("quality-overview")).toBeInTheDocument();
    });
    expect(screen.getByText("Workspace Quality")).toBeInTheDocument();
  });

  it("collapsing hides the QualityDashboard", async () => {
    const user = userEvent.setup();
    server.use(
      http.get(`/api/workspaces/${WORKSPACE_ID}/quality-summary`, () =>
        HttpResponse.json(MOCK_PAYLOAD),
      ),
    );

    render(createElement(QualityOverview, { workspaceId: WORKSPACE_ID }), {
      wrapper: withSWR(),
    });

    await waitFor(() => {
      expect(screen.getByTestId("quality-overview")).toBeInTheDocument();
    });

    const toggle = screen.getByTestId("quality-overview-toggle");
    await user.click(toggle);

    expect(screen.queryByTestId("quality-dashboard")).not.toBeInTheDocument();
  });

  it("expanding shows the QualityDashboard after collapse", async () => {
    const user = userEvent.setup();
    server.use(
      http.get(`/api/workspaces/${WORKSPACE_ID}/quality-summary`, () =>
        HttpResponse.json(MOCK_PAYLOAD),
      ),
    );

    render(createElement(QualityOverview, { workspaceId: WORKSPACE_ID }), {
      wrapper: withSWR(),
    });

    await waitFor(() => {
      expect(screen.getByTestId("quality-overview")).toBeInTheDocument();
    });

    // Collapse first
    const toggle = screen.getByTestId("quality-overview-toggle");
    await user.click(toggle);
    expect(screen.queryByTestId("quality-dashboard")).not.toBeInTheDocument();

    // Expand again
    await user.click(toggle);

    await waitFor(() => {
      expect(screen.getByTestId("quality-dashboard")).toBeInTheDocument();
    });
  });

  it("shows error state when fetch fails", async () => {
    // Use a network error to trigger SWR's error state
    server.use(
      http.get(`/api/workspaces/${WORKSPACE_ID}/quality-summary`, () =>
        HttpResponse.error(),
      ),
    );

    render(createElement(QualityOverview, { workspaceId: WORKSPACE_ID }), {
      wrapper: withSWR(),
    });

    await waitFor(() => {
      expect(screen.getByTestId("quality-overview")).toBeInTheDocument();
    });

    await waitFor(() => {
      expect(screen.getByTestId("quality-overview-error")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("quality-dashboard")).not.toBeInTheDocument();
  });

  it("renders QualityDashboard with data when fetch succeeds", async () => {
    server.use(
      http.get(`/api/workspaces/${WORKSPACE_ID}/quality-summary`, () =>
        HttpResponse.json(MOCK_PAYLOAD),
      ),
    );

    render(createElement(QualityOverview, { workspaceId: WORKSPACE_ID }), {
      wrapper: withSWR(),
    });

    await waitFor(() => {
      expect(screen.getByTestId("quality-dashboard")).toBeInTheDocument();
    });

    // Rating badge should show the B rating from mock payload
    expect(screen.getByTestId("quality-dashboard-rating")).toHaveTextContent("B");
  });
});
