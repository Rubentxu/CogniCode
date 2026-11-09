/**
 * `updateInvestigation` and `deleteInvestigation` talk to endpoints that
 * answer `{ "ok": true }` (crates/cognicode-explorer/src/api.rs). Both
 * must resolve when the server accepts the request.
 */
import { beforeEach, describe, expect, it } from "vitest";
import { http, HttpResponse } from "msw";

import { server } from "../mocks/node";
import { deleteInvestigation, updateInvestigation } from "./useInvestigations";

beforeEach(() => {
  server.resetHandlers();
});

describe("investigation mutations", () => {
  it("deleteInvestigation resolves when the server answers { ok: true }", async () => {
    let deletedId: string | undefined;
    server.use(
      http.delete("/api/investigations/:id", ({ params }) => {
        deletedId = String(params.id);
        return HttpResponse.json({ ok: true });
      }),
    );

    await expect(deleteInvestigation("inv-1")).resolves.toBeUndefined();
    expect(deletedId).toBe("inv-1");
  });

  it("updateInvestigation resolves when the server answers { ok: true }", async () => {
    let body: unknown;
    server.use(
      http.put("/api/investigations/:id", async ({ request }) => {
        body = await request.json();
        return HttpResponse.json({ ok: true });
      }),
    );

    await expect(
      updateInvestigation({
        id: "inv-1",
        workspace_id: "ws",
        title: "t",
        goal: "g",
        status: "active",
        entry_point: null,
        panes: [],
        evidence: [],
        artifacts: [],
        narrative: "",
        related_adrs: [],
      }),
    ).resolves.toBeUndefined();
    expect((body as { id: string }).id).toBe("inv-1");
  });
});
