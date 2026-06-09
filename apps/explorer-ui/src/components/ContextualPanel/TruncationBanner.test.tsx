/**
 * Tests for the `TruncationBanner` component.
 */
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { TruncationBanner } from "./TruncationBanner";

describe("TruncationBanner", () => {
  it("renders the warning text", () => {
    render(<TruncationBanner reason="max_nodes_exceeded" />);
    const el = screen.getByTestId("truncation-banner");
    expect(el).toBeInTheDocument();
    expect(el.textContent).toMatch(/refine with max_nodes/);
  });
});
