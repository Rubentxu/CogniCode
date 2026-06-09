/**
 * `SuggestionStrip` — tests for the per-`InspectableObjectType` strip.
 *
 * Behavioural contract (from spec):
 *   1. Renders pills on `tablet | desktop | ultrawide` viewports.
 *   2. Renders `<SuggestionPopover />` (single "What can I do here?"
 *      trigger) on `small` viewports.
 *   3. Hides `requiresGraph` prompts when `graphStatus` is `missing`,
 *      `indexing`, or `null` (no workspace open).
 *   4. Shows every prompt (without filtering) when `graphStatus` is
 *      `stale` — the strip marks them with `aria-disabled="true"`
 *      instead. The hook is responsible for rejecting dispatch.
 *   5. Shows every prompt when `graphStatus` is `ready`.
 *   6. Clicking a pill calls `onDispatch` with the matching prompt.
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SuggestionStrip } from "./SuggestionStrip";
import { SUGGESTED_QUESTIONS } from "../../config/suggestedQuestions";
import type { InspectableObjectType, GraphStatus } from "../../api/types";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const SYMBOL_TYPE: InspectableObjectType = "symbol";
const FILE_TYPE: InspectableObjectType = "file";

const makeProps = (overrides: Partial<React.ComponentProps<typeof SuggestionStrip>> = {}) => ({
  objectType: SYMBOL_TYPE,
  objectId: "sym:42",
  objectLabel: "build_overview",
  graphStatus: "ready" as GraphStatus | null,
  viewport: "desktop" as const,
  onDispatch: vi.fn(),
  ...overrides,
});

// ---------------------------------------------------------------------------
// Viewport branches
// ---------------------------------------------------------------------------

describe("SuggestionStrip — viewport branches", () => {
  it("renders a row of pills on desktop viewport", () => {
    render(<SuggestionStrip {...makeProps({ viewport: "desktop" })} />);
    const strip = screen.getByTestId("suggestion-strip");
    expect(strip).toBeInTheDocument();
    // No popover trigger on non-small viewports.
    expect(screen.queryByTestId("suggestion-popover-trigger")).not.toBeInTheDocument();
    // One pill per symbol prompt.
    const pills = within(strip).getAllByTestId(/^suggestion-pill-/);
    expect(pills.length).toBe(SUGGESTED_QUESTIONS.symbol.length);
  });

  it("renders pills on tablet viewport (≥ 900px)", () => {
    render(<SuggestionStrip {...makeProps({ viewport: "tablet" })} />);
    expect(
      screen.getAllByTestId(/^suggestion-pill-/).length,
    ).toBe(SUGGESTED_QUESTIONS.symbol.length);
    expect(screen.queryByTestId("suggestion-popover-trigger")).not.toBeInTheDocument();
  });

  it("renders pills on ultrawide viewport (≥ 1440px)", () => {
    render(<SuggestionStrip {...makeProps({ viewport: "ultrawide" })} />);
    expect(
      screen.getAllByTestId(/^suggestion-pill-/).length,
    ).toBe(SUGGESTED_QUESTIONS.symbol.length);
  });

  it("renders a popover trigger (and no pills) on small viewport", () => {
    render(<SuggestionStrip {...makeProps({ viewport: "small" })} />);
    expect(screen.getByTestId("suggestion-popover-trigger")).toBeInTheDocument();
    expect(screen.queryByTestId(/^suggestion-pill-/)).not.toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Graph gating
// ---------------------------------------------------------------------------

describe("SuggestionStrip — graph gating", () => {
  it("hides graph-dependent prompts when graphStatus is 'missing'", () => {
    render(<SuggestionStrip {...makeProps({ graphStatus: "missing" })} />);
    const strip = screen.getByTestId("suggestion-strip");
    const labels = SUGGESTED_QUESTIONS.symbol
      .filter((p) => p.requiresGraph)
      .map((p) => p.label);
    const nonGraphLabels = SUGGESTED_QUESTIONS.symbol
      .filter((p) => !p.requiresGraph)
      .map((p) => p.label);
    // Graph-dependent labels must be absent.
    for (const label of labels) {
      expect(within(strip).queryByText(label)).not.toBeInTheDocument();
    }
    // Non-graph labels must be present.
    for (const label of nonGraphLabels) {
      expect(within(strip).getByText(label)).toBeInTheDocument();
    }
  });

  it("hides graph-dependent prompts when graphStatus is null (no workspace)", () => {
    render(<SuggestionStrip {...makeProps({ graphStatus: null })} />);
    const strip = screen.getByTestId("suggestion-strip");
    const labels = SUGGESTED_QUESTIONS.symbol.filter((p) => p.requiresGraph).map((p) => p.label);
    for (const label of labels) {
      expect(within(strip).queryByText(label)).not.toBeInTheDocument();
    }
  });

  it("shows all prompts when graphStatus is 'ready'", () => {
    render(<SuggestionStrip {...makeProps({ graphStatus: "ready" })} />);
    expect(
      screen.getAllByTestId(/^suggestion-pill-/).length,
    ).toBe(SUGGESTED_QUESTIONS.symbol.length);
  });

  it("shows every prompt (graph-dependent disabled) when graphStatus is 'stale'", () => {
    render(<SuggestionStrip {...makeProps({ graphStatus: "stale" })} />);
    const strip = screen.getByTestId("suggestion-strip");
    const pills = within(strip).getAllByTestId(/^suggestion-pill-/);
    expect(pills.length).toBe(SUGGESTED_QUESTIONS.symbol.length);
    // Graph-dependent pills must carry aria-disabled.
    const graphIds = SUGGESTED_QUESTIONS.symbol.filter((p) => p.requiresGraph).map((p) => p.id);
    for (const id of graphIds) {
      const pill = within(strip).getByTestId(`suggestion-pill-${id}`);
      expect(pill).toHaveAttribute("aria-disabled", "true");
    }
    // Non-graph pills are not disabled.
    const nonGraphIds = SUGGESTED_QUESTIONS.symbol.filter((p) => !p.requiresGraph).map((p) => p.id);
    for (const id of nonGraphIds) {
      const pill = within(strip).getByTestId(`suggestion-pill-${id}`);
      expect(pill).not.toHaveAttribute("aria-disabled");
    }
  });
});

// ---------------------------------------------------------------------------
// Click → dispatch
// ---------------------------------------------------------------------------

describe("SuggestionStrip — click dispatch", () => {
  it("calls onDispatch with the matching prompt when a pill is clicked", async () => {
    const onDispatch = vi.fn();
    const user = userEvent.setup();
    render(<SuggestionStrip {...makeProps({ onDispatch })} />);

    const target = SUGGESTED_QUESTIONS.symbol[0]!;
    await user.click(screen.getByTestId(`suggestion-pill-${target.id}`));

    expect(onDispatch).toHaveBeenCalledTimes(1);
    expect(onDispatch.mock.calls[0]![0]).toEqual(target);
  });

  it("does not call onDispatch when a stale + graph-dependent pill is clicked", async () => {
    const onDispatch = vi.fn();
    const user = userEvent.setup();
    render(<SuggestionStrip {...makeProps({ graphStatus: "stale", onDispatch })} />);

    const graphId = SUGGESTED_QUESTIONS.symbol.find((p) => p.requiresGraph)!.id;
    await user.click(screen.getByTestId(`suggestion-pill-${graphId}`));
    expect(onDispatch).not.toHaveBeenCalled();
  });

  it("uses the right kind's prompts for non-symbol kinds", () => {
    // Sanity check: rendering with `file` produces the file-kind prompts,
    // not the symbol-kind ones. (Verifies the strip reads from the map
    // and does not hard-code the symbol kind.)
    render(<SuggestionStrip {...makeProps({ objectType: FILE_TYPE })} />);
    const labels = SUGGESTED_QUESTIONS.file.map((p) => p.label);
    for (const label of labels) {
      expect(screen.getByText(label)).toBeInTheDocument();
    }
    // The symbol-only "Who calls this?" must not be rendered.
    expect(screen.queryByText("Who calls this?")).not.toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// ARIA contract
// ---------------------------------------------------------------------------

describe("SuggestionStrip — ARIA", () => {
  it("exposes the strip as an `<aside>` landmark", () => {
    render(<SuggestionStrip {...makeProps()} />);
    const strip = screen.getByTestId("suggestion-strip");
    expect(strip.tagName.toLowerCase()).toBe("aside");
  });
});
