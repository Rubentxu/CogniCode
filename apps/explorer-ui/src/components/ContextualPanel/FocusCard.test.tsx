/**
 * Tests for the `FocusCard` component.
 *
 * TDD: these tests are RED before the component lands; GREEN after.
 */
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { FocusCard } from "./FocusCard";
import type { GraphNode } from "../../api/types";

function makeFocus(): GraphNode {
  return {
    id: "sym:foo::alpha:1",
    label: "alpha",
    kind: "function",
    file: "src/foo.rs",
    line: 1,
    style_class: "function",
  };
}

describe("FocusCard", () => {
  it("renders the symbol id and kind", () => {
    render(<FocusCard focus={makeFocus()} />);
    const idEl = screen.getByTestId("focus-card-id");
    expect(idEl).toHaveTextContent("sym:foo::alpha:1");
    const kindEl = screen.getByTestId("focus-card-kind");
    expect(kindEl).toHaveTextContent("function");
  });

  it("renders the file path", () => {
    render(<FocusCard focus={makeFocus()} />);
    const fileEl = screen.getByTestId("focus-card-file");
    expect(fileEl).toHaveTextContent("src/foo.rs");
  });
});
