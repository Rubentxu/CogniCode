/**
 * Tests for the `ChildrenList` component.
 *
 * TDD: RED before the component lands; GREEN after.
 */
import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { ChildrenList } from "./ChildrenList";
import type { GraphNode } from "../../api/types";

function sib(id: string, label: string): GraphNode {
  return {
    id,
    label,
    kind: "function",
    file: "src/foo.rs",
    line: 10,
    style_class: "function",
  };
}

describe("ChildrenList", () => {
  it("renders all siblings", () => {
    render(
      <ChildrenList
        children={[sib("a", "alpha"), sib("b", "beta"), sib("c", "gamma")]}
        onFocus={() => {}}
      />,
    );
    const rows = screen.getAllByTestId("children-list-row");
    expect(rows).toHaveLength(3);
    expect(rows[0]).toHaveTextContent("alpha");
    expect(rows[1]).toHaveTextContent("beta");
    expect(rows[2]).toHaveTextContent("gamma");
  });

  it("shows the empty state when there are no siblings", () => {
    render(<ChildrenList children={[]} onFocus={() => {}} />);
    expect(screen.getByTestId("children-list-empty")).toBeInTheDocument();
    expect(screen.queryByTestId("children-list")).not.toBeInTheDocument();
  });

  it("calls onFocus on row click", () => {
    const onFocus = vi.fn();
    render(
      <ChildrenList
        children={[sib("a", "alpha"), sib("b", "beta")]}
        onFocus={onFocus}
      />,
    );
    fireEvent.click(screen.getAllByTestId("children-list-row")[1]!);
    expect(onFocus).toHaveBeenCalledWith("b");
  });

  it("is scrollable when overflowing", () => {
    // Render a list longer than the default maxHeight. The container
    // has `overflowY: "auto"` and a fixed `maxHeight` — assert both
    // are present.
    const many = Array.from({ length: 30 }, (_, i) => sib(`s${i}`, `s${i}`));
    const { container } = render(
      <ChildrenList children={many} onFocus={() => {}} />,
    );
    const list = screen.getByTestId("children-list");
    expect(list).toBeInTheDocument();
    // Inline style sets maxHeight + overflowY — read the actual DOM
    // attribute to assert.
    const style = (list as HTMLElement).style;
    expect(style.overflowY).toBe("auto");
    expect(style.maxHeight).toBe("240px");
    // Sanity: the container has 30 children rendered.
    expect(container.querySelectorAll("[data-testid=children-list-row]")).toHaveLength(30);
  });
});
