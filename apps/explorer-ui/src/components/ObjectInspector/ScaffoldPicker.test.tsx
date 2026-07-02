/**
 * `ScaffoldPicker` — tests for the scaffold-first picker step.
 *
 * Behavioural contract (from spec):
 *   1. Renders scaffolds grouped by object_type (filtered to current objectType).
 *   2. Selecting a scaffold highlights it.
 *   3. "Use Scaffold" button calls onSelect with viewKind, rendererKind, and
 *      the query with `{{object_id}}` substituted.
 *   4. "Custom Query" calls onCustomQuery (skips the scaffold path).
 *   5. Search filters scaffolds by label or intent.
 *   6. Shows query preview for the selected scaffold.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ScaffoldPicker, type ScaffoldSelection } from "./ScaffoldPicker";
import * as useScaffoldRegistry from "../../hooks/useScaffoldRegistry";
import type { Scaffold } from "../../api/scaffoldSchema";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const SYMBOL_SCAFFOLDS: Scaffold[] = [
  {
    id: "focus_on_symbol",
    object_type: "symbol",
    intent: "Focus on a symbol",
    label: "Focus",
    description: "Centers the view on the selected symbol.",
    query_template: "symbols where id = '{{object_id}}'",
    view_kind: "vertical_slice",
    renderer_kind: "composite",
    applies_when: null,
    produces_relation_candidates: false,
  },
  {
    id: "callers_and_callees",
    object_type: "symbol",
    intent: "Show callers and callees",
    label: "Callers & Callees",
    description: "Displays direct callers and callees.",
    query_template: "calls from '{{object_id}}' depth 1",
    view_kind: "call_graph",
    renderer_kind: "graph",
    applies_when: null,
    produces_relation_candidates: false,
  },
];

const OBJECT_ID = "sym:42";

const makeProps = (overrides: Partial<{
  objectType: "symbol" | "workspace" | "scope" | "file" | "module" | "evidence" | "decision_artifact" | "quality_issue" | "rule" | "route" | "investigation";
  objectId: string;
  onSelect: (result: ScaffoldSelection) => void;
  onCustomQuery: () => void;
} & Record<string, unknown>> = {}) => ({
  objectType: "symbol" as const,
  objectId: OBJECT_ID,
  onSelect: vi.fn<(result: ScaffoldSelection) => void>(),
  onCustomQuery: vi.fn<() => void>(),
  ...overrides,
});

// ---------------------------------------------------------------------------
// Test setup — mock useScaffoldRegistry
// ---------------------------------------------------------------------------

beforeEach(() => {
  vi.clearAllMocks();
  vi.spyOn(useScaffoldRegistry, "useScaffoldRegistry").mockReturnValue(SYMBOL_SCAFFOLDS);
});

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

describe("ScaffoldPicker — rendering", () => {
  it("renders the scaffold picker with a search input", () => {
    render(<ScaffoldPicker {...makeProps()} />);
    expect(screen.getByPlaceholderText("Search scaffolds…")).toBeInTheDocument();
  });

  it("renders scaffolds grouped by object_type", () => {
    render(<ScaffoldPicker {...makeProps()} />);
    // Scaffold list should show symbol scaffolds
    expect(screen.getByText("Focus")).toBeInTheDocument();
    expect(screen.getByText("Callers & Callees")).toBeInTheDocument();
  });

  it("renders the Use Scaffold and Custom Query buttons", () => {
    render(<ScaffoldPicker {...makeProps()} />);
    expect(screen.getByRole("button", { name: "Use Scaffold" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Custom Query" })).toBeInTheDocument();
  });

  it("Use Scaffold is disabled when no scaffold is selected", () => {
    render(<ScaffoldPicker {...makeProps()} />);
    expect(screen.getByRole("button", { name: "Use Scaffold" })).toBeDisabled();
  });
});

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

describe("ScaffoldPicker — selection", () => {
  it("highlights the selected scaffold", async () => {
    const user = userEvent.setup();
    render(<ScaffoldPicker {...makeProps()} />);

    await user.click(screen.getByText("Callers & Callees"));

    // The selected button should have primary background color (checked via style)
    const btn = screen.getByText("Callers & Callees").closest("button");
    expect(btn).toHaveStyle({
      backgroundColor: "var(--color-primary)",
    });
  });

  it("enables Use Scaffold button after selection", async () => {
    const user = userEvent.setup();
    render(<ScaffoldPicker {...makeProps()} />);

    await user.click(screen.getByText("Focus"));
    expect(screen.getByRole("button", { name: "Use Scaffold" })).toBeEnabled();
  });

  it("calls onSelect with correct values when Use Scaffold is clicked", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(<ScaffoldPicker {...makeProps({ onSelect })} />);

    await user.click(screen.getByText("Callers & Callees"));
    await user.click(screen.getByRole("button", { name: "Use Scaffold" }));

    expect(onSelect).toHaveBeenCalledTimes(1);
    const result = onSelect.mock.calls[0]![0];
    expect(result.scaffoldId).toBe("callers_and_callees");
    expect(result.viewKind).toBe("call_graph");
    expect(result.rendererKind).toBe("graph");
    expect(result.query).toBe("calls from 'sym:42' depth 1");
  });

  it("substitutes {{object_id}} placeholder with the actual objectId", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(<ScaffoldPicker {...makeProps({ objectId: "sym:99", onSelect })} />);

    await user.click(screen.getByText("Focus"));
    await user.click(screen.getByRole("button", { name: "Use Scaffold" }));

    const result = onSelect.mock.calls[0]![0];
    expect(result.query).toBe("symbols where id = 'sym:99'");
  });
});

// ---------------------------------------------------------------------------
// Custom query
// ---------------------------------------------------------------------------

describe("ScaffoldPicker — custom query", () => {
  it("calls onCustomQuery when Custom Query is clicked", async () => {
    const user = userEvent.setup();
    const onCustomQuery = vi.fn();
    render(<ScaffoldPicker {...makeProps({ onCustomQuery })} />);

    await user.click(screen.getByRole("button", { name: "Custom Query" }));

    expect(onCustomQuery).toHaveBeenCalledTimes(1);
  });

  it("does not call onSelect when Custom Query is clicked", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    const onCustomQuery = vi.fn();
    render(<ScaffoldPicker {...makeProps({ onSelect, onCustomQuery })} />);

    await user.click(screen.getByRole("button", { name: "Custom Query" }));

    expect(onSelect).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

describe("ScaffoldPicker — search", () => {
  it("filters scaffolds by label when searching", async () => {
    const user = userEvent.setup();
    render(<ScaffoldPicker {...makeProps()} />);

    await user.type(screen.getByPlaceholderText("Search scaffolds…"), "Callers");

    expect(screen.getByText("Callers & Callees")).toBeInTheDocument();
    expect(screen.queryByText("Focus")).not.toBeInTheDocument();
  });

  it("filters scaffolds by intent when searching", async () => {
    const user = userEvent.setup();
    render(<ScaffoldPicker {...makeProps()} />);

    await user.type(screen.getByPlaceholderText("Search scaffolds…"), "symbol");

    // Both have "symbol" in their intent or label
    expect(screen.getByText("Focus")).toBeInTheDocument();
  });

  it("shows no scaffolds when search matches nothing", async () => {
    const user = userEvent.setup();
    render(<ScaffoldPicker {...makeProps()} />);

    await user.type(screen.getByPlaceholderText("Search scaffolds…"), "nonexistent");

    expect(screen.queryByText("Focus")).not.toBeInTheDocument();
    expect(screen.queryByText("Callers & Callees")).not.toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Query preview
// ---------------------------------------------------------------------------

describe("ScaffoldPicker — query preview", () => {
  it("shows query preview after selecting a scaffold", async () => {
    const user = userEvent.setup();
    render(<ScaffoldPicker {...makeProps()} />);

    await user.click(screen.getByText("Callers & Callees"));

    // Query preview is in a <pre> element
    const preElements = screen.getAllByText(/calls from 'sym:42' depth 1/);
    expect(preElements.length).toBeGreaterThan(0);
  });

  it("query preview shows substituted object_id", async () => {
    const user = userEvent.setup();
    render(<ScaffoldPicker {...makeProps({ objectId: "sym:77" })} />);

    await user.click(screen.getByText("Focus"));

    const preElements = screen.getAllByText(/symbols where id = 'sym:77'/);
    expect(preElements.length).toBeGreaterThan(0);
  });
});
