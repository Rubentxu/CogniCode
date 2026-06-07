/**
 * `LensPanel` tests — Phase 9 acceptance criteria.
 *
 * 1. Empty state when no object is selected.
 * 2. List of available lenses is rendered from `useLenses`.
 * 3. Clicking a lens triggers `useLensResult` and renders findings.
 * 4. Findings are grouped by severity.
 * 5. Severity chips are color-coded.
 * 6. Confidence bar + percentage render.
 * 7. "Blockers only" toggle filters non-blocker findings.
 * 8. Clicking a finding dispatches SELECT_OBJECT.
 * 9. Error state surfaces a Retry button.
 * 10. Empty state when the lens returns zero findings.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useReducer } from "react";
import { http, HttpResponse, delay } from "msw";

import { server } from "../mocks/node";
import {
  AppContext,
  appReducer,
  initialState,
  type AppState,
} from "../state/context";
import { LensPanel } from "./LensPanel";
import {
  lensDescriptorsFixture,
  lensResultFixture,
  inspectableObjectFixture,
} from "../mocks/fixtures";
import type { DesignFinding, LensResult } from "../api/types";

const ACTIVE_OBJECT_ID = inspectableObjectFixture.id;

function Harness({
  onState,
  initial,
}: {
  onState?: (s: AppState) => void;
  initial?: Partial<AppState>;
}) {
  const [state, dispatch] = useReducer(appReducer, {
    ...initialState,
    ...initial,
  });
  if (onState) {
    Promise.resolve().then(() => onState(state));
  }
  return (
    <AppContext.Provider value={{ state, dispatch }}>
      <LensPanel />
    </AppContext.Provider>
  );
}

beforeEach(() => {
  vi.useRealTimers();
});

afterEach(() => {
  server.resetHandlers();
  vi.useRealTimers();
});

describe("LensPanel — empty state", () => {
  it("shows the empty state when no object is selected", () => {
    render(<Harness />);
    expect(screen.getByTestId("lens-panel-empty")).toBeInTheDocument();
    expect(screen.getByText(/Select an object/i)).toBeInTheDocument();
  });
});

describe("LensPanel — list of lenses", () => {
  it("renders the available lenses from useLenses", async () => {
    render(<Harness initial={{ activeObjectId: ACTIVE_OBJECT_ID }} />);
    const list = await screen.findByTestId("lens-list");
    for (const lens of lensDescriptorsFixture) {
      expect(
        within(list).getByTestId(`lens-item-${lens.id}`),
      ).toBeInTheDocument();
    }
  });

  it("highlights the currently active lens", async () => {
    render(
      <Harness
        initial={{ activeObjectId: ACTIVE_OBJECT_ID, activeLensId: "lens.callgraph" }}
      />,
    );
    const item = await screen.findByTestId("lens-item-lens.callgraph");
    expect(item).toHaveAttribute("aria-pressed", "true");
  });
});

describe("LensPanel — applying a lens", () => {
  it("renders the findings once a lens is selected", async () => {
    const user = userEvent.setup();
    render(<Harness initial={{ activeObjectId: ACTIVE_OBJECT_ID }} />);
    const item = await screen.findByTestId("lens-item-lens.hotspots");
    await user.click(item);
    // The findings panel renders after the SWR call resolves.
    expect(await screen.findByTestId("lens-result")).toBeInTheDocument();
    expect(
      screen.getByText(lensResultFixture.findings[0]!.title),
    ).toBeInTheDocument();
  });

  it("groups findings by severity", async () => {
    const findings: DesignFinding[] = [
      {
        id: "f-blocker",
        lens_id: "lens.quality",
        title: "Blocker finding",
        hypothesis: "Critical issue in the module",
        severity: "critical",
        confidence: 0.9,
        object_ids: [ACTIVE_OBJECT_ID],
        evidence_ids: [],
      },
      {
        id: "f-info",
        lens_id: "lens.quality",
        title: "Info finding",
        hypothesis: "Style nit",
        severity: "info",
        confidence: 0.4,
        object_ids: [ACTIVE_OBJECT_ID],
        evidence_ids: [],
      },
    ];
    server.use(
      http.get("/api/objects/:object_id/lenses/:lens_id/apply", async () => {
        await delay(10);
        return HttpResponse.json({
          lens_id: "lens.quality",
          findings,
          summary: "2 findings",
        } satisfies LensResult);
      }),
    );
    const user = userEvent.setup();
    render(<Harness initial={{ activeObjectId: ACTIVE_OBJECT_ID }} />);
    await user.click(await screen.findByTestId("lens-item-lens.quality"));
    expect(await screen.findByTestId("lens-group-critical")).toBeInTheDocument();
    expect(await screen.findByTestId("lens-group-info")).toBeInTheDocument();
  });

  it("renders a confidence bar with a percentage", async () => {
    const user = userEvent.setup();
    render(<Harness initial={{ activeObjectId: ACTIVE_OBJECT_ID }} />);
    await user.click(await screen.findByTestId("lens-item-lens.hotspots"));
    const meter = await screen.findByTestId("lens-confidence");
    expect(meter).toHaveAttribute("aria-valuenow", "78");
    expect(meter).toHaveTextContent("78%");
  });

  it("shows a color-coded severity chip", async () => {
    const user = userEvent.setup();
    render(<Harness initial={{ activeObjectId: ACTIVE_OBJECT_ID }} />);
    await user.click(await screen.findByTestId("lens-item-lens.hotspots"));
    const chip = await screen.findByTestId("lens-severity-warning");
    expect(chip).toBeInTheDocument();
  });

  it("renders the summary text from the lens result", async () => {
    const user = userEvent.setup();
    render(<Harness initial={{ activeObjectId: ACTIVE_OBJECT_ID }} />);
    await user.click(await screen.findByTestId("lens-item-lens.hotspots"));
    const summary = await screen.findByTestId("lens-result-summary");
    expect(summary).toHaveTextContent(lensResultFixture.summary);
  });
});

describe("LensPanel — blockers-only filter", () => {
  const MULTI_FINDINGS: LensResult = {
    lens_id: "lens.quality",
    summary: "Mixed severities",
    findings: [
      {
        id: "f-blocker",
        lens_id: "lens.quality",
        title: "Blocker finding",
        hypothesis: "Critical",
        severity: "critical",
        confidence: 0.9,
        object_ids: [ACTIVE_OBJECT_ID],
        evidence_ids: [],
      },
      {
        id: "f-info",
        lens_id: "lens.quality",
        title: "Info finding",
        hypothesis: "Style nit",
        severity: "info",
        confidence: 0.4,
        object_ids: [ACTIVE_OBJECT_ID],
        evidence_ids: [],
      },
    ],
  };

  it("filters out non-blocker findings when toggled", async () => {
    server.use(
      http.get("/api/objects/:object_id/lenses/:lens_id/apply", async () => {
        await delay(10);
        return HttpResponse.json(MULTI_FINDINGS);
      }),
    );
    const user = userEvent.setup();
    render(<Harness initial={{ activeObjectId: ACTIVE_OBJECT_ID }} />);
    await user.click(await screen.findByTestId("lens-item-lens.quality"));
    expect(await screen.findByTestId("lens-group-info")).toBeInTheDocument();
    // Toggle blockers-only.
    await user.click(screen.getByTestId("lens-blocker-toggle"));
    await waitFor(() => {
      expect(screen.queryByTestId("lens-group-info")).not.toBeInTheDocument();
    });
    expect(screen.getByTestId("lens-group-critical")).toBeInTheDocument();
  });
});

describe("LensPanel — navigation", () => {
  it("clicking a finding dispatches SELECT_OBJECT with the affected object id", async () => {
    const captured: { current: AppState | null } = { current: null };
    const user = userEvent.setup();
    render(
      <Harness
        initial={{ activeObjectId: ACTIVE_OBJECT_ID }}
        onState={(s) => {
          captured.current = s;
        }}
      />,
    );
    await user.click(await screen.findByTestId("lens-item-lens.hotspots"));
    const finding = await screen.findByTestId(
      `lens-finding-${lensResultFixture.findings[0]!.id}`,
    );
    await user.click(finding);
    await waitFor(() => {
      expect(captured.current?.activeObjectId).toBe(ACTIVE_OBJECT_ID);
    });
  });
});

describe("LensPanel — error + empty states", () => {
  it("surfaces an error with a Retry button when the lens call fails", async () => {
    // Use a unique objectId for this test so the SWR cache from
    // previous tests does not short-circuit our override.
    const uniqueObject = `${ACTIVE_OBJECT_ID}-error-test`;
    server.use(
      http.get("/api/objects/:object_id/lenses/lens.callgraph/apply", async () => {
        await delay(10);
        return HttpResponse.json(
          { error: "Server on fire" },
          { status: 500 },
        );
      }),
    );
    const user = userEvent.setup();
    render(<Harness initial={{ activeObjectId: uniqueObject }} />);
    await user.click(await screen.findByTestId("lens-item-lens.callgraph"));
    const alert = await screen.findByTestId("lens-result-error");
    expect(alert).toHaveTextContent(/Failed to apply lens/);
    const retry = screen.getByTestId("lens-result-retry");
    expect(retry).toBeInTheDocument();
  });

  it("shows an empty state when the lens returns zero findings", async () => {
    const uniqueObject = `${ACTIVE_OBJECT_ID}-empty-test`;
    server.use(
      http.get("/api/objects/:object_id/lenses/lens.hotspots/apply", async () => {
        await delay(10);
        return HttpResponse.json({
          lens_id: "lens.hotspots",
          findings: [],
          summary: "Nothing to report",
        } satisfies LensResult);
      }),
    );
    const user = userEvent.setup();
    render(<Harness initial={{ activeObjectId: uniqueObject }} />);
    await user.click(await screen.findByTestId("lens-item-lens.hotspots"));
    expect(await screen.findByTestId("lens-result-empty")).toBeInTheDocument();
  });
});

describe("LensPanel — finding count badge", () => {
  it("displays the finding count in the header", async () => {
    const user = userEvent.setup();
    render(<Harness initial={{ activeObjectId: ACTIVE_OBJECT_ID }} />);
    await user.click(await screen.findByTestId("lens-item-lens.hotspots"));
    const badge = await screen.findByTestId("lens-finding-count");
    expect(badge).toHaveTextContent(String(lensResultFixture.findings.length));
  });
});
