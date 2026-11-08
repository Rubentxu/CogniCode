/**
 * Pinning evidence without a captured view (`viewId === null`) must send
 * `view_id: null`: the request schema marks the field nullable but
 * required, so `undefined` is rejected before the request leaves.
 */
import { describe, it, expect, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { SWRConfig } from "swr";

import { server } from "../../mocks/node";
import { PinEvidenceModal } from "./PinEvidenceModal";

const investigation = {
  id: "inv-1",
  workspace_id: "ws",
  title: "Auth review",
  goal: "g",
  status: "active",
  entry_point: null,
  panes: [],
  evidence: [],
  artifacts: [],
  narrative: "",
  related_adrs: [],
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-01T00:00:00Z",
};

describe("PinEvidenceModal", () => {
  it("pins evidence with view_id null when no view was captured", async () => {
    let body: Record<string, unknown> | undefined;
    server.use(
      http.get("/api/investigations", () => HttpResponse.json([investigation])),
      http.post("/api/investigations/:id/evidence", async ({ request }) => {
        body = (await request.json()) as Record<string, unknown>;
        return HttpResponse.json({ ok: true });
      }),
    );
    const onClose = vi.fn();

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <PinEvidenceModal
          objectId="sym-1"
          viewId={null}
          workspaceId="ws"
          onClose={onClose}
        />
      </SWRConfig>,
    );

    await screen.findByText(/Auth review/);
    fireEvent.change(screen.getByTestId("pin-evidence-note"), {
      target: { value: "entry point of the flow" },
    });
    await waitFor(() =>
      expect(screen.getByTestId("pin-evidence-submit")).toBeEnabled(),
    );
    fireEvent.click(screen.getByTestId("pin-evidence-submit"));

    await screen.findByTestId("pin-evidence-success");
    expect(screen.queryByTestId("pin-evidence-error")).toBeNull();
    expect(body).toMatchObject({ object_id: "sym-1", view_id: null });
  });
});
