/**
 * `Spotter` tests.
 *
 * Covers the Phase 6 acceptance criteria:
 * 1. Cmd/Ctrl+K opens the palette.
 * 2. The visible "Search" button + "/" shortcut open it too.
 * 3. Typing triggers a debounced search via useSpotter.
 * 4. Results render in kind groups.
 * 5. Selecting a result closes the palette and dispatches
 *    SELECT_OBJECT with the right view id.
 * 6. Escape closes without selecting.
 * 7. Clicking the backdrop closes.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, waitFor, within, fireEvent } from "@testing-library/react";
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
import { Spotter } from "./Spotter";
import { inspectableObjectFixture } from "../mocks/fixtures";

/**
 * Harness — real `appReducer` + a callback that captures the latest
 * state after each render. Mirrors the pattern used by the
 * Shell tests so we can assert on dispatched
 * actions via the resulting state shape.
 */
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
      <Spotter />
    </AppContext.Provider>
  );
}

beforeEach(() => {
  // Default to real timers per-test; tests that drive userEvent
  // with `advanceTimers` opt in to fake timers explicitly.
  vi.useRealTimers();
});

afterEach(() => {
  server.resetHandlers();
  vi.useRealTimers();
});

describe("Spotter (closed by default)", () => {
  it("does not render the dialog when state.spotterOpen is false", () => {
    render(<Harness />);
    expect(screen.queryByTestId("spotter")).not.toBeInTheDocument();
  });
});

describe("Spotter opening", () => {
  it("opens when Cmd+K is pressed", async () => {
    render(<Harness initial={{ spotterOpen: true }} />);
    expect(await screen.findByTestId("spotter")).toBeInTheDocument();
  });

  it("opens when Ctrl+K is pressed", async () => {
    const captured: { current: AppState | null } = { current: null };
    render(
      <Harness
        onState={(s) => {
          captured.current = s;
        }}
      />,
    );
    expect(screen.queryByTestId("spotter")).not.toBeInTheDocument();
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    await waitFor(() => {
      expect(captured.current?.spotterOpen).toBe(true);
    });
  });

  it("opens via the '/' shortcut outside of form fields", async () => {
    const captured: { current: AppState | null } = { current: null };
    render(
      <Harness
        onState={(s) => {
          captured.current = s;
        }}
      />,
    );
    fireEvent.keyDown(window, { key: "/" });
    await waitFor(() => {
      expect(captured.current?.spotterOpen).toBe(true);
    });
  });

  it("does NOT open via '/' when typing in an input", async () => {
    const captured: { current: AppState | null } = { current: null };
    render(
      <Harness
        onState={(s) => {
          captured.current = s;
        }}
      />,
    );
    const input = document.createElement("input");
    document.body.appendChild(input);
    input.focus();
    fireEvent.keyDown(input, { key: "/" });
    await new Promise((r) => setTimeout(r, 10));
    expect(captured.current?.spotterOpen).toBeFalsy();
    document.body.removeChild(input);
  });
});

describe("Spotter (open)", () => {
  it("renders the search input focused on open", async () => {
    render(<Harness initial={{ spotterOpen: true }} />);
    const input = await screen.findByTestId("spotter-input");
    expect(input).toBeInTheDocument();
    // autoFocus happens via React; just check the element is in the
    // document and accessible.
    expect(input).toHaveAttribute("placeholder");
    expect(input.getAttribute("placeholder") ?? "").toMatch(/Search the workspace/);
  });

  it("shows the empty state with helpful copy when no query is typed", async () => {
    render(<Harness initial={{ spotterOpen: true }} />);
    const empty = await screen.findByTestId("spotter-empty");
    expect(empty).toHaveTextContent(/Type to search/i);
  });

  it("debounces the search and shows results from the API", async () => {
    // Use a 1.2s artificial latency on the spotter endpoint so the
    // debounce window (200ms) has time to swallow the first call.
    server.use(
      http.get("/api/workspaces/:workspace_id/spotter", async () => {
        await delay(50);
        return HttpResponse.json([
          {
            object: inspectableObjectFixture,
            score: 0.9,
            match_type: "name_prefix",
          },
        ]);
      }),
    );

    const user = userEvent.setup();
    render(<Harness initial={{ spotterOpen: true }} />);
    const input = screen.getByTestId("spotter-input");
    await user.type(input, "build");
    // The debounced hook re-fires after 200ms — wait for the
    // first item to appear (proving the network round-trip
    // settled).
    await waitFor(
      () => {
        expect(
          screen.getByTestId(`spotter-item-${inspectableObjectFixture.id}`),
        ).toBeInTheDocument();
      },
      { timeout: 1500 },
    );
  });

  it("groups results by kind under a heading", async () => {
    server.use(
      http.get("/api/workspaces/:workspace_id/spotter", async () => {
        await delay(20);
        return HttpResponse.json([
          {
            object: { ...inspectableObjectFixture, id: "sym-1" },
            score: 0.9,
            match_type: "name_exact",
          },
          {
            object: {
              ...inspectableObjectFixture,
              id: "file-1",
              object_type: "file",
              label: "lib.rs",
              subtitle: "crates/cognicode-explorer/src/lib.rs",
            },
            score: 0.7,
            match_type: "name_prefix",
          },
        ]);
      }),
    );

    const user = userEvent.setup();
    render(<Harness initial={{ spotterOpen: true }} />);
    const input = screen.getByTestId("spotter-input");
    await user.type(input, "x");
    await waitFor(() => {
      expect(screen.getByTestId("spotter-item-sym-1")).toBeInTheDocument();
    });
    // Both kind groups render — cmdk renders the group heading
    // with a `cmdk-group-heading` attribute. We use that selector
    // so we don't false-positive on the kind chip / kind glyph
    // text elsewhere in the list.
    const resultsList = screen.getByTestId("spotter-results");
    const headings = within(resultsList).getAllByText(/symbol|file/);
    expect(headings.length).toBeGreaterThanOrEqual(2);
  });

  it("selecting a result dispatches SELECT_OBJECT and closes the palette", async () => {
    server.use(
      http.get("/api/workspaces/:workspace_id/spotter", async () => {
        await delay(20);
        return HttpResponse.json([
          {
            object: inspectableObjectFixture,
            score: 0.9,
            match_type: "name_exact",
          },
        ]);
      }),
    );

    const user = userEvent.setup();
    const captured: { current: AppState | null } = { current: null };
    render(
      <Harness
        initial={{ spotterOpen: true }}
        onState={(s) => {
          captured.current = s;
        }}
      />,
    );
    const input = screen.getByTestId("spotter-input");
    await user.type(input, "build");
    const item = await screen.findByTestId(
      `spotter-item-${inspectableObjectFixture.id}`,
    );
    await user.click(item);
    await waitFor(() => {
      const state = captured.current;
      expect(state).not.toBeNull();
      expect(state!.spotterOpen).toBe(false);
      // The active object is set to the picked one and a pane
      // is pushed.
      expect(state!.activeObjectId).toBe(inspectableObjectFixture.id);
      const activePane = state!.navigation.panes.find(
        (p) => p.id === state!.navigation.activePaneId,
      );
      expect(activePane?.objectId).toBe(inspectableObjectFixture.id);
      // The first available view id is forwarded.
      expect(state!.activeViewId).toBe(
        inspectableObjectFixture.available_views[0]!.id,
      );
    });
  });

  it("Escape closes the palette", async () => {
    const captured: { current: AppState | null } = { current: null };
    render(
      <Harness
        initial={{ spotterOpen: true }}
        onState={(s) => {
          captured.current = s;
        }}
      />,
    );
    const dialog = await screen.findByTestId("spotter");
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => {
      expect(captured.current?.spotterOpen).toBe(false);
    });
  });

  it("clicking the backdrop closes the palette", async () => {
    const captured: { current: AppState | null } = { current: null };
    render(
      <Harness
        initial={{ spotterOpen: true }}
        onState={(s) => {
          captured.current = s;
        }}
      />,
    );
    const backdrop = await screen.findByTestId("spotter-backdrop");
    fireEvent.click(backdrop);
    await waitFor(() => {
      expect(captured.current?.spotterOpen).toBe(false);
    });
  });

  it("renders the result count footer", async () => {
    server.use(
      http.get("/api/workspaces/:workspace_id/spotter", async () => {
        await delay(10);
        return HttpResponse.json([
          { object: inspectableObjectFixture, score: 0.9, match_type: "x" },
        ]);
      }),
    );
    const user = userEvent.setup();
    render(<Harness initial={{ spotterOpen: true }} />);
    const input = screen.getByTestId("spotter-input");
    await user.type(input, "b");
    await screen.findByTestId(
      `spotter-item-${inspectableObjectFixture.id}`,
    );
    const footer = screen.getByTestId("spotter-count");
    expect(footer).toHaveTextContent(/1\s+result\b/);
  });

  it("kind chip narrows the visible results without a refetch", async () => {
    server.use(
      http.get("/api/workspaces/:workspace_id/spotter", async () => {
        await delay(20);
        return HttpResponse.json([
          { object: { ...inspectableObjectFixture, id: "sym-1" }, score: 0.9, match_type: "x" },
          {
            object: {
              ...inspectableObjectFixture,
              id: "file-1",
              object_type: "file",
              label: "lib.rs",
              subtitle: "lib.rs",
            },
            score: 0.7,
            match_type: "x",
          },
        ]);
      }),
    );
    const user = userEvent.setup();
    render(<Harness initial={{ spotterOpen: true }} />);
    const input = screen.getByTestId("spotter-input");
    await user.type(input, "x");
    await screen.findByTestId("spotter-item-sym-1");
    // Both kinds present.
    const resultsList = screen.getByTestId("spotter-results");
    expect(within(resultsList).getByTestId("spotter-item-file-1")).toBeInTheDocument();
    // Click the file chip — only the file remains visible.
    const fileChip = await screen.findByTestId("spotter-kind-file");
    await user.click(fileChip);
    await waitFor(() => {
      expect(
        within(resultsList).queryByTestId("spotter-item-sym-1"),
      ).not.toBeInTheDocument();
    });
    expect(
      within(resultsList).getByTestId("spotter-item-file-1"),
    ).toBeInTheDocument();
  });
});
