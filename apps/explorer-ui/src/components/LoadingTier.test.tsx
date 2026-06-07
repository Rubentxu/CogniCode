import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { LoadingTier } from "./LoadingTier";

describe("LoadingTier", () => {
  it("renders the skeleton tier while loading with no data", () => {
    render(
      <LoadingTier label="Miller Columns" isLoading={true} data={undefined}>
        <span>real content</span>
      </LoadingTier>,
    );
    // aria-busy + role=status is the skeleton tier
    const status = screen.getByRole("status");
    expect(status).toHaveAttribute("aria-busy", "true");
    expect(status).toHaveAttribute("aria-label", "Loading Miller Columns");
    expect(screen.queryByText("real content")).not.toBeInTheDocument();
  });

  it("renders the ready tier when data is present", () => {
    render(
      <LoadingTier label="Inspector" isLoading={false} data={"x"}>
        <span>real content</span>
      </LoadingTier>,
    );
    expect(screen.getByText("real content")).toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("renders the empty tier when data resolves to null", () => {
    render(
      <LoadingTier
        label="Symbols"
        isLoading={false}
        data={null}
        emptyMessage="No symbols found."
      >
        <span>real content</span>
      </LoadingTier>,
    );
    expect(screen.getByText("No symbols found.")).toBeInTheDocument();
  });

  it("renders the error tier when error is provided", () => {
    render(
      <LoadingTier
        label="Lens"
        isLoading={false}
        data={undefined}
        error={new Error("network down")}
      >
        <span>real content</span>
      </LoadingTier>,
    );
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("Failed to load Lens");
    expect(alert).toHaveTextContent("network down");
  });

  it("shows a revalidation dot when isValidating is true and data is ready", () => {
    const { container } = render(
      <LoadingTier
        label="Spotter"
        isLoading={false}
        data={"hit"}
        isValidating={true}
      >
        <span>results</span>
      </LoadingTier>,
    );
    expect(container.querySelector('[aria-busy="true"]')).toBeInTheDocument();
  });

  it("skips the skeleton tier when cached data exists (cache-instant path)", () => {
    // This is the "Tier 1 → Tier 4 jump" from Q007-P2: data is defined and
    // isLoading becomes true during a revalidation, but we should NOT
    // regress to a skeleton — we keep the ready view and pulse the dot.
    render(
      <LoadingTier
        label="Object Inspector"
        isLoading={true}
        data={"cached"}
        isValidating={true}
      >
        <span>cached content</span>
      </LoadingTier>,
    );
    expect(screen.getByText("cached content")).toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("accepts a custom skeleton", () => {
    render(
      <LoadingTier
        label="Graph"
        isLoading={true}
        data={undefined}
        skeleton={<div data-testid="custom-skel" />}
      >
        <span>real</span>
      </LoadingTier>,
    );
    expect(screen.getByTestId("custom-skel")).toBeInTheDocument();
  });
});
