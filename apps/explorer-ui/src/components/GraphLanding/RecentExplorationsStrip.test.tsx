/**
 * `RecentExplorationsStrip` tests — Sprint E4.5.
 *
 * Tests verify:
 * 1. Renders cards when explorations exist
 * 2. Returns null when empty array
 * 3. Sorts by created_at descending
 * 4. Caps display at 5 most recent
 * 5. Loading state returns null
 * 6. Click dispatches onExplorationClick
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { RecentExplorationsStrip } from "./RecentExplorationsStrip";
import type { ExplorationPath } from "../../api/types";
import { explorationPathFixture } from "../../mocks/fixtures";

// Mock useExplorations at the module level
vi.mock("../../hooks/useExplorations", () => ({
  useExplorations: vi.fn(),
}));

function makeExploration(overrides: Partial<ExplorationPath>): ExplorationPath {
  return {
    ...explorationPathFixture,
    id: `exploration-${Math.random().toString(36).slice(2)}`,
    created_at: new Date().toISOString(),
    ...overrides,
  };
}

describe("RecentExplorationsStrip", () => {
  const mockOnExplorationClick = vi.fn();

  afterEach(() => {
    mockOnExplorationClick.mockClear();
  });

  it("renders cards when explorations exist", async () => {
    const { useExplorations } = await import("../../hooks/useExplorations");
    vi.mocked(useExplorations).mockReturnValue({
      data: [makeExploration({ id: "exp-1", columns: [{ object_id: "obj-1", active_view: "overview", kind: "symbol" }] })],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useExplorations>);

    render(
      <RecentExplorationsStrip workspaceId="ws-001" onExplorationClick={mockOnExplorationClick} />
    );

    expect(screen.queryByTestId("recent-explorations-strip")).toBeTruthy();
    expect(screen.queryByTestId("recent-exploration-exp-1")).toBeTruthy();
  });

  it("renders null when explorations is empty array", async () => {
    const { useExplorations } = await import("../../hooks/useExplorations");
    vi.mocked(useExplorations).mockReturnValue({
      data: [],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useExplorations>);

    render(
      <RecentExplorationsStrip workspaceId="ws-001" onExplorationClick={mockOnExplorationClick} />
    );

    expect(screen.queryByTestId("recent-explorations-strip")).toBeNull();
  });

  it("renders null when data is null", async () => {
    const { useExplorations } = await import("../../hooks/useExplorations");
    vi.mocked(useExplorations).mockReturnValue({
      data: null,
      isLoading: false,
      error: null,
    } as ReturnType<typeof useExplorations>);

    render(
      <RecentExplorationsStrip workspaceId="ws-001" onExplorationClick={mockOnExplorationClick} />
    );

    expect(screen.queryByTestId("recent-explorations-strip")).toBeNull();
  });

  it("sorts explorations by created_at descending", async () => {
    const older = makeExploration({ id: "exp-older", created_at: "2026-06-01T12:00:00Z" });
    const newer = makeExploration({ id: "exp-newer", created_at: "2026-06-23T12:00:00Z" });

    const { useExplorations } = await import("../../hooks/useExplorations");
    vi.mocked(useExplorations).mockReturnValue({
      data: [older, newer],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useExplorations>);

    render(
      <RecentExplorationsStrip workspaceId="ws-001" onExplorationClick={mockOnExplorationClick} />
    );

    const cards = screen.queryAllByRole("button");
    // Newer exploration should appear first
    expect(cards[0]).toHaveAttribute("data-testid", "recent-exploration-exp-newer");
    expect(cards[1]).toHaveAttribute("data-testid", "recent-exploration-exp-older");
  });

  it("caps display at 5 most recent", async () => {
    const explorations = Array.from({ length: 8 }, (_, i) =>
      makeExploration({ id: `exp-${i}`, created_at: new Date(Date.now() - i * 1000).toISOString() })
    );

    const { useExplorations } = await import("../../hooks/useExplorations");
    vi.mocked(useExplorations).mockReturnValue({
      data: explorations,
      isLoading: false,
      error: null,
    } as ReturnType<typeof useExplorations>);

    render(
      <RecentExplorationsStrip workspaceId="ws-001" onExplorationClick={mockOnExplorationClick} />
    );

    const cards = screen.queryAllByRole("button");
    expect(cards.length).toBe(5);
  });

  it("returns null when isLoading is true", async () => {
    const { useExplorations } = await import("../../hooks/useExplorations");
    vi.mocked(useExplorations).mockReturnValue({
      data: undefined,
      isLoading: true,
      error: null,
    } as ReturnType<typeof useExplorations>);

    render(
      <RecentExplorationsStrip workspaceId="ws-001" onExplorationClick={mockOnExplorationClick} />
    );

    expect(screen.queryByTestId("recent-explorations-strip")).toBeNull();
  });

  it("clicking a card dispatches onExplorationClick with the exploration", async () => {
    const exploration = makeExploration({
      id: "exp-click-test",
      columns: [{ object_id: "obj-click", active_view: "overview", kind: "symbol" }],
    });

    const { useExplorations } = await import("../../hooks/useExplorations");
    vi.mocked(useExplorations).mockReturnValue({
      data: [exploration],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useExplorations>);

    render(
      <RecentExplorationsStrip workspaceId="ws-001" onExplorationClick={mockOnExplorationClick} />
    );

    const card = screen.getByTestId("recent-exploration-exp-click-test");
    fireEvent.click(card);

    expect(mockOnExplorationClick).toHaveBeenCalledTimes(1);
    expect(mockOnExplorationClick).toHaveBeenCalledWith(exploration);
  });
});
