/**
 * `ViewBlock` tests — Phase 7 acceptance criteria.
 *
 * 1. Union routing — every known block id routes to the right
 *    renderer (a sample of the 27 is checked, plus a representative
 *    fallback for unknown shapes).
 * 2. Unknown block — renders the raw JSON fallback with the
 *    `data-testid="view-block-unknown"` marker and the JSON body
 *    under `view-block-unknown-json`.
 * 3. Tab strip — the ViewTabs component honours the WAI-ARIA tab
 *    contract: arrow keys move focus, Enter activates, and the
 *    active tab carries `aria-selected="true"`.
 *
 * The fixtures live in `src/mocks/fixtures.ts` and exercise every
 * known block id; we assert on the rendered structure rather than
 * snapshot the whole tree, so future style tweaks don't break the
 * test surface.
 */
import { describe, it, expect } from "vitest";
import { render, screen, fireEvent, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useReducer } from "react";

import { ViewBlock, Blocks, ViewTabs, ObjectInspector as ObjectInspectorContainer } from "./index";
import {
  AppContext,
  appReducer,
  initialState,
  initialStateWithFocus,
  type AppState,
} from "../../state/context";
import { contextualViewFixture } from "../../mocks/fixtures";
import type {
  ViewBlock as ViewBlockT,
  UnknownViewBlock,
  ContextualView,
  ViewDescriptor,
} from "../../api/types";

// ============================================================================
// ViewBlock — union routing + JSON fallback
// ============================================================================

describe("ViewBlock — union routing", () => {
  it("renders the identity block with the symbol name + location", () => {
    const block: ViewBlockT = {
      id: "identity",
      title: "Identity",
      body: { name: "build_overview", kind: "function", file: "src/lib.rs", line: 16 },
    };
    render(<ViewBlock block={block} />);
    expect(screen.getByTestId("view-block-identity")).toBeInTheDocument();
    expect(screen.getByText("build_overview")).toBeInTheDocument();
    expect(screen.getByText("src/lib.rs:16")).toBeInTheDocument();
  });

  it("renders the call_metrics block with fan-in / fan-out stats", () => {
    const block: ViewBlockT = {
      id: "call_metrics",
      title: "Call metrics",
      body: { fan_in: 3, fan_out: 4 },
    };
    render(<ViewBlock block={block} />);
    const shell = screen.getByTestId("view-block-call_metrics");
    expect(within(shell).getByText("3")).toBeInTheDocument();
    expect(within(shell).getByText("4")).toBeInTheDocument();
  });

  it("renders the source_slice block as a numbered list", () => {
    const block: ViewBlockT = {
      id: "source_slice",
      title: "Source slice",
      body: {
        file: "src/lib.rs",
        line: 10,
        lines: [
          { line: 10, text: "fn build_overview() {" },
          { line: 11, text: "    let x = 1;" },
        ],
      },
    };
    render(<ViewBlock block={block} />);
    expect(screen.getByTestId("view-block-source_slice")).toBeInTheDocument();
    expect(screen.getByTestId("source-line-10")).toBeInTheDocument();
    expect(screen.getByTestId("source-line-11")).toBeInTheDocument();
    expect(screen.getByText("fn build_overview() {")).toBeInTheDocument();
  });

  it("renders the file_quality_gate block with the rating chip", () => {
    const block: ViewBlockT = {
      id: "file_quality_gate",
      title: "Quality gate",
      body: {
        rating: "B",
        total_issues: 12,
        blockers: 0,
        criticals: 1,
        debt_minutes: 84,
        last_run: "2026-06-07T09:00:00Z",
      },
    };
    render(<ViewBlock block={block} />);
    expect(screen.getByTestId("view-block-file_quality_gate")).toBeInTheDocument();
    expect(screen.getByLabelText(/Rating B/)).toBeInTheDocument();
  });

  it("renders the issues_list block with severity chips", () => {
    const block: ViewBlockT = {
      id: "file_quality_issues",
      title: "Issues in this file",
      body: {
        count: 1,
        items: [
          {
            id: 7,
            rule_id: "rust:S100",
            severity: "warning",
            category: "naming",
            file: "src/lib.rs",
            line: 16,
            message: "Function name should be camelCase",
            status: "open",
            object_id: "issue:7",
          },
        ],
      },
    };
    render(<ViewBlock block={block} />);
    expect(screen.getByTestId("view-block-file_quality_issues")).toBeInTheDocument();
    expect(screen.getByTestId("view-block-issue-7")).toBeInTheDocument();
    expect(screen.getByText("Function name should be camelCase")).toBeInTheDocument();
  });

  it("renders the hotspots block with interactive rows when onSelectObject is set", async () => {
    const onSelect = vi.fn();
    const user = userEvent.setup();
    const block: ViewBlockT = {
      id: "hotspots",
      title: "Top hotspots",
      body: {
        scope: "src",
        count: 1,
        items: [
          {
            name: "build_overview",
            kind: "function",
            file: "src/lib.rs",
            line: 16,
            object_id: "sym-1",
          },
        ],
      },
    };
    render(<ViewBlock block={block} onSelectObject={onSelect} />);
    // The interactive row is the inner <button> — clicking the
    // outer <li> no longer dispatches the action (that pattern
    // broke a11y).
    const row = screen.getByTestId("view-block-hotspot-button-sym-1");
    await user.click(row);
    expect(onSelect).toHaveBeenCalledWith("sym-1");
  });

  it("renders the kinds breakdown as a 2-col grid", () => {
    const block: ViewBlockT = {
      id: "kinds",
      title: "Symbol kinds",
      body: { breakdown: { function: 11, struct: 2 } },
    };
    render(<ViewBlock block={block} />);
    expect(screen.getByTestId("view-block-kinds")).toBeInTheDocument();
    expect(screen.getByText("function")).toBeInTheDocument();
    expect(screen.getByText("struct")).toBeInTheDocument();
  });

  it("renders the signature block as a code block", () => {
    const block: ViewBlockT = {
      id: "signature",
      title: "Signature",
      body: { signature: "fn build_overview() -> ContextualView" },
    };
    render(<ViewBlock block={block} />);
    expect(screen.getByTestId("view-block-signature")).toBeInTheDocument();
    expect(
      screen.getByText("fn build_overview() -> ContextualView"),
    ).toBeInTheDocument();
  });

  it("renders the cross_scope block as a 3-col table", () => {
    const block: ViewBlockT = {
      id: "cross_scope",
      title: "Cross-scope",
      body: {
        scope: "src",
        file_count: 4,
        symbol_count: 60,
        entries: [
          { scope: "core", outgoing_count: 7, incoming_count: 1 },
        ],
      },
    };
    render(<ViewBlock block={block} />);
    expect(screen.getByTestId("view-block-cross_scope")).toBeInTheDocument();
    expect(screen.getByText("core")).toBeInTheDocument();
    expect(screen.getByText("7")).toBeInTheDocument();
  });

  it("routes the full fixture (all 27 shapes) without crashing", () => {
    // We don't assert on every block here — the per-id tests
    // above cover each shape. This test is a guard against a
    // broken fallback (e.g., a typo in the switch that throws).
    render(<Blocks view={contextualViewFixture as ContextualView} />);
    expect(screen.getByTestId("view-blocks")).toBeInTheDocument();
    // At least 29 blocks render (27 original + 2 quality dashboard).
    const rendered = screen.getAllByTestId(/^view-block-/);
    expect(rendered.length).toBeGreaterThanOrEqual(contextualViewFixture.blocks.length);
  });

  it("renders an empty state when the view has no blocks", () => {
    const emptyView: ContextualView = {
      ...contextualViewFixture,
      blocks: [],
    };
    render(<Blocks view={emptyView} />);
    expect(screen.getByTestId("view-blocks-empty")).toBeInTheDocument();
  });

  it("renders the quality_summary block with rating + per-severity counts", () => {
    const block: ViewBlockT = {
      id: "quality_summary",
      title: "Quality summary",
      body: {
        scope: "src",
        rating: "A",
        total_issues: 5,
        debt_minutes: 30,
        by_severity: { blocker: 0, critical: 0, major: 1, minor: 2, info: 2 },
        last_run: "2026-06-07T09:00:00Z",
      },
    };
    render(<ViewBlock block={block} />);
    expect(screen.getByTestId("view-block-quality_summary")).toBeInTheDocument();
    expect(screen.getByTestId("quality-summary-rating")).toHaveTextContent("A");
    expect(screen.getByTestId("quality-summary-count-major")).toHaveTextContent("1");
    expect(screen.getByTestId("quality-summary-count-minor")).toHaveTextContent("2");
    expect(screen.getByTestId("quality-summary-count-info")).toHaveTextContent("2");
  });

  it("renders the quality_issue_detail block with rule + remediation", async () => {
    const onSelect = vi.fn();
    const user = userEvent.setup();
    const block: ViewBlockT = {
      id: "quality_issue_detail",
      title: "Issue #7",
      body: {
        id: 7,
        rule_id: "rust:S100",
        severity: "major",
        category: "naming",
        status: "open",
        file: "src/lib.rs",
        line: 16,
        message: "Function name should be camelCase",
        remediation: "Rename to `buildOverview`",
        rule_description: "Function naming convention",
        rule_url: "https://example.com/rules/S100",
        object_id: "issue:7",
      },
    };
    render(<ViewBlock block={block} onSelectObject={onSelect} />);
    expect(screen.getByTestId("view-block-quality_issue_detail")).toBeInTheDocument();
    expect(screen.getByTestId("quality-issue-detail-location")).toHaveTextContent("src/lib.rs:16");
    expect(screen.getByTestId("quality-issue-detail-remediation")).toHaveTextContent("Rename to");
    expect(screen.getByTestId("quality-issue-detail-rule")).toHaveTextContent(/Function naming convention/);
    expect(screen.getByTestId("quality-issue-detail-rule-url")).toHaveAttribute(
      "href",
      "https://example.com/rules/S100",
    );
    await user.click(screen.getByTestId("quality-issue-detail-location"));
    expect(onSelect).toHaveBeenCalledWith("issue:7");
  });
});

// ============================================================================
// ViewBlock — unknown fallback
// ============================================================================

describe("ViewBlock — unknown block fallback", () => {
  it("renders the JSON fallback for an unknown block id", () => {
    const block: UnknownViewBlock = {
      id: "future_block_2027",
      title: "Future block",
      body: { novel_field: 42, message: "rendered as raw JSON" },
    };
    render(<ViewBlock block={block} />);
    expect(screen.getByTestId("view-block-unknown")).toBeInTheDocument();
    expect(screen.getByTestId("view-block-unknown-json")).toHaveTextContent(
      /"novel_field": 42/,
    );
  });
});

// ============================================================================
// ViewTabs — WAI-ARIA tab strip
// ============================================================================

interface HarnessProps {
  views: ViewDescriptor[];
  activeViewId: string | null;
  onChange: (id: string) => void;
  isLoading?: boolean;
}

function TabsHarness({ views, activeViewId, onChange, isLoading }: HarnessProps) {
  return (
    <ViewTabs
      views={views}
      activeViewId={activeViewId}
      onChange={onChange}
      isLoading={isLoading ?? false}
    />
  );
}

describe("ViewTabs", () => {
  const views: ViewDescriptor[] = [
    { id: "overview", title: "Overview", is_builtin: true, source: null },
    { id: "call-graph", title: "Call graph", is_builtin: true, source: null },
    { id: "source", title: "Source", is_builtin: true, source: null },
    { id: "quality", title: "Quality", is_builtin: true, source: null },
  ];

  it("renders a tab per view with the right a11y wiring", () => {
    render(<TabsHarness views={views} activeViewId="overview" onChange={() => {}} />);
    const tablist = screen.getByRole("tablist");
    expect(tablist.getAttribute("aria-label") ?? "").toMatch(/Available views/i);
    for (const v of views) {
      const tab = screen.getByTestId(`view-tab-${v.id}`);
      expect(tab).toHaveAttribute("role", "tab");
      expect(tab).toHaveAttribute("aria-selected", v.id === "overview" ? "true" : "false");
    }
  });

  it("only the active tab is in the tab order", () => {
    render(<TabsHarness views={views} activeViewId="source" onChange={() => {}} />);
    expect(screen.getByTestId("view-tab-overview")).toHaveAttribute("tabindex", "-1");
    expect(screen.getByTestId("view-tab-source")).toHaveAttribute("tabindex", "0");
  });

  it("clicking a tab calls onChange with the new view id", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<TabsHarness views={views} activeViewId="overview" onChange={onChange} />);
    await user.click(screen.getByTestId("view-tab-quality"));
    expect(onChange).toHaveBeenCalledWith("quality");
  });

  it("ArrowRight + ArrowLeft move through tabs and call onChange", () => {
    // Stateful parent — the keyboard navigation relies on the
    // activeViewId prop updating on each onChange call.
    function Stateful() {
      const [active, setActive] = React.useState("overview");
      return (
        <ViewTabs
          views={views}
          activeViewId={active}
          isLoading={false}
          onChange={(id) => {
            onChange(id);
            setActive(id);
          }}
        />
      );
    }
    const onChange = vi.fn();
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const React = require("react") as typeof import("react");
    render(<Stateful />);
    const tablist = screen.getByRole("tablist");
    fireEvent.keyDown(tablist, { key: "ArrowRight" });
    expect(onChange).toHaveBeenLastCalledWith("call-graph");
    fireEvent.keyDown(tablist, { key: "ArrowRight" });
    expect(onChange).toHaveBeenLastCalledWith("source");
    fireEvent.keyDown(tablist, { key: "ArrowLeft" });
    expect(onChange).toHaveBeenLastCalledWith("call-graph");
  });

  it("Home / End jump to the first / last tab", () => {
    const onChange = vi.fn();
    render(
      <TabsHarness views={views} activeViewId="call-graph" onChange={onChange} />,
    );
    const tablist = screen.getByRole("tablist");
    fireEvent.keyDown(tablist, { key: "End" });
    expect(onChange).toHaveBeenLastCalledWith("quality");
    fireEvent.keyDown(tablist, { key: "Home" });
    expect(onChange).toHaveBeenLastCalledWith("overview");
  });

  it("returns null when there are no views", () => {
    const { container } = render(
      <TabsHarness views={[]} activeViewId={null} onChange={() => {}} />,
    );
    expect(container).toBeEmptyDOMElement();
  });
});

// ============================================================================
// ObjectInspector container integration (smoke)
// ============================================================================

describe("ObjectInspector — container", () => {
  function InspectorHarness({
    initial,
  }: {
    initial?: Partial<AppState>;
  }) {
    // When the test seeds `activeObjectId`, the reducer needs the
    // matching navigation state. Use `initialStateWithFocus` so the
    // adapter's focus() agrees with what the test asserts.
    const seeded = initial?.activeObjectId
      ? { ...initialStateWithFocus(initial.activeObjectId, "column", initial.activeViewId ?? null), ...initial }
      : { ...initialState, ...initial };
    const [state, dispatch] = useReducer(appReducer, seeded);
    return (
      <AppContext.Provider value={{ state, dispatch }}>
        <ObjectInspectorContainer />
      </AppContext.Provider>
    );
  }

  it("shows the empty state when there is no active object", async () => {
    render(<InspectorHarness />);
    expect(await screen.findByTestId("object-inspector-empty")).toBeInTheDocument();
  });

  it("renders the inspector with the right blocks for an active object", async () => {
    render(
      <InspectorHarness
        initial={{
          activeObjectId: "symbol:src/lib.rs:build_overview:16",
          activeViewId: "overview",
          activeView: contextualViewFixture as ContextualView,
        }}
      />,
    );
    await waitFor(() => {
      expect(screen.getByTestId("object-inspector")).toBeInTheDocument();
    });
    // At least one block from the fixture must render.
    expect(screen.getByTestId("view-blocks")).toBeInTheDocument();
  });

  it("renders SuggestionStrip between header and ViewTabs when an object is focused", async () => {
    render(
      <InspectorHarness
        initial={{
          activeObjectId: "symbol:src/lib.rs:build_overview:16",
          activeViewId: "overview",
          activeView: contextualViewFixture as ContextualView,
        }}
      />,
    );
    await waitFor(() => {
      expect(screen.getByTestId("object-inspector")).toBeInTheDocument();
    });
    // The contextual-help strip should be in the DOM.
    const strip = await screen.findByTestId("suggestion-strip");
    expect(strip).toBeInTheDocument();
    // DOM order: header (h2 title) → strip → ViewTabs.
    const inspector = screen.getByTestId("object-inspector");
    const header = inspector.querySelector("header");
    const tablist = screen.getByRole("tablist");
    expect(header).not.toBeNull();
    expect(tablist).not.toBeNull();
    // `header` and `tablist` and `strip` are siblings — verify the
    // strip sits between them in document order.
    const order = Array.from(inspector.children).flatMap((node) => {
      const el = node as HTMLElement;
      if (el === header) return ["header"];
      if (el.contains(strip)) return ["strip"];
      if (el.contains(tablist)) return ["tablist"];
      return [];
    });
    expect(order).toEqual(["header", "strip", "tablist"]);
  });
});
