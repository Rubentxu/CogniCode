/**
 * Tests for the `ParentBreadcrumb` component.
 *
 * TDD: RED before the component lands; GREEN after.
 */
import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { ParentBreadcrumb } from "./ParentBreadcrumb";
import type { GraphNode } from "../../api/types";

function makeParent(): GraphNode {
  return {
    id: "file:src/foo.rs",
    label: "src/foo.rs",
    kind: "file",
    file: "src/foo.rs",
    line: undefined,
    style_class: "module",
  };
}

describe("ParentBreadcrumb", () => {
  it("renders the file path", () => {
    render(<ParentBreadcrumb parent={makeParent()} onFocus={() => {}} />);
    expect(screen.getByTestId("parent-breadcrumb")).toHaveTextContent("src/foo.rs");
  });

  it("calls onFocus on click", () => {
    const onFocus = vi.fn();
    render(<ParentBreadcrumb parent={makeParent()} onFocus={onFocus} />);
    fireEvent.click(screen.getByTestId("parent-breadcrumb-button"));
    expect(onFocus).toHaveBeenCalledTimes(1);
    expect(onFocus).toHaveBeenCalledWith("file:src/foo.rs");
  });
});
