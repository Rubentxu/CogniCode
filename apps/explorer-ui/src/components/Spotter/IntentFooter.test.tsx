import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { IntentFooter, chipsFromResult } from "./IntentFooter";
import type { SpotterResult, SpotterSearchResult } from "../../api/schemas";

function makeSearchResult(viewIds: string[]): SpotterSearchResult {
  return {
    kind: "symbol",
    result: {
      object: {
        id: "test:1",
        label: "test-label",
        object_type: "symbol",
        available_views: viewIds.map((id, i) => ({
          id,
          title: `View ${id}`,
          applies_to: "symbol",
          view_kind: id,
        })) as never,
      },
      score: 1.0,
      matched_field: "label",
    } as SpotterResult,
  };
}

describe("chipsFromResult", () => {
  it("returns empty array for null", () => {
    expect(chipsFromResult(null)).toEqual([]);
  });

  it("dedupes by viewId", () => {
    const result = makeSearchResult(["call-graph", "call-graph", "source"]);
    const chips = chipsFromResult(result);
    expect(chips).toHaveLength(4); // 2 unique + 2 placeholders
    const ids = chips.map((c) => c.viewId);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("always includes the two placeholders", () => {
    const result = makeSearchResult(["overview"]);
    const chips = chipsFromResult(result);
    expect(chips.find((c) => c.viewId === "c4-context")).toBeDefined();
    expect(chips.find((c) => c.viewId === "add-to-investigation")).toBeDefined();
    expect(chips.find((c) => c.viewId === "c4-context")?.disabled).toBe(true);
    expect(chips.find((c) => c.viewId === "add-to-investigation")?.disabled).toBe(true);
  });
});

describe("IntentFooter", () => {
  it("shows hint when no result", () => {
    render(<IntentFooter result={null} onPick={() => {}} />);
    expect(screen.getByText(/pick a result/i)).toBeInTheDocument();
  });

  it("renders one chip per available view plus placeholders", () => {
    const result = makeSearchResult(["overview", "call-graph", "source"]);
    render(<IntentFooter result={result} onPick={() => {}} />);
    expect(screen.getByTestId("spotter-intent-overview")).toBeInTheDocument();
    expect(screen.getByTestId("spotter-intent-call-graph")).toBeInTheDocument();
    expect(screen.getByTestId("spotter-intent-source")).toBeInTheDocument();
    expect(screen.getByTestId("spotter-intent-c4-context")).toBeInTheDocument();
    expect(screen.getByTestId("spotter-intent-add-to-investigation")).toBeInTheDocument();
  });

  it("calls onPick with viewId when chip clicked", async () => {
    const user = userEvent.setup();
    const onPick = vi.fn();
    const result = makeSearchResult(["call-graph"]);
    render(<IntentFooter result={result} onPick={onPick} />);
    await user.click(screen.getByTestId("spotter-intent-call-graph"));
    expect(onPick).toHaveBeenCalledWith("call-graph");
  });

  it("does not call onPick when disabled placeholder clicked", async () => {
    const user = userEvent.setup();
    const onPick = vi.fn();
    const result = makeSearchResult(["overview"]);
    render(<IntentFooter result={result} onPick={onPick} />);
    await user.click(screen.getByTestId("spotter-intent-c4-context"));
    expect(onPick).not.toHaveBeenCalled();
  });

  it("shows Cmd+1 shortcut on first enabled chip", () => {
    const result = makeSearchResult(["call-graph", "source"]);
    render(<IntentFooter result={result} onPick={() => {}} />);
    const chip = screen.getByTestId("spotter-intent-call-graph");
    expect(chip.textContent).toContain("Cmd+1");
  });
});
