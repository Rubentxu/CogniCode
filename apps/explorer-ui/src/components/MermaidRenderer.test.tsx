/**
 * `MermaidRenderer` tests — Phase 4 acceptance criteria (E20-2 slice 4).
 *
 * Tests:
 * 1. Renders valid mermaid text as a styled code block.
 * 2. Handles empty text gracefully.
 * 3. Handles special characters without crashing.
 * 4. Falls back on invalid/unrecognized mermaid.
 * 5. Renders with viewKind badge when provided.
 */
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import React from "react";

import { MermaidRenderer } from "./MermaidRenderer";

describe("MermaidRenderer", () => {
  describe("valid mermaid text", () => {
    it("renders flowchart diagram text", () => {
      const mermaidText = "flowchart TD\n  A[Start] --> B[End]";
      render(<MermaidRenderer mermaidText={mermaidText} />);
      expect(screen.getByTestId("renderer-mermaid")).toBeInTheDocument();
      expect(screen.getByText(/flowchart TD/)).toBeInTheDocument();
    });

    it("renders call_graph mermaid text", () => {
      const mermaidText = "graph TD\n  A --> B";
      render(<MermaidRenderer mermaidText={mermaidText} />);
      expect(screen.getByTestId("renderer-mermaid")).toBeInTheDocument();
    });

    it("renders sequenceDiagram", () => {
      const mermaidText =
        "sequenceDiagram\n  Alice->>John: Hello John\n  John-->>Alice: Hi Alice";
      render(<MermaidRenderer mermaidText={mermaidText} />);
      expect(screen.getByTestId("renderer-mermaid")).toBeInTheDocument();
    });

    it("renders with viewKind badge", () => {
      const mermaidText = "flowchart TD\n  A --> B";
      render(<MermaidRenderer mermaidText={mermaidText} viewKind="call_graph" />);
      expect(screen.getByTestId("renderer-mermaid")).toBeInTheDocument();
      expect(screen.getByText("call_graph")).toBeInTheDocument();
    });
  });

  describe("empty text handling", () => {
    it("renders empty state for empty string", () => {
      render(<MermaidRenderer mermaidText="" />);
      expect(screen.getByTestId("renderer-mermaid-empty")).toBeInTheDocument();
      expect(screen.getByText("No mermaid text provided.")).toBeInTheDocument();
    });

    it("renders empty state for whitespace-only string", () => {
      render(<MermaidRenderer mermaidText="   " />);
      expect(screen.getByTestId("renderer-mermaid-empty")).toBeInTheDocument();
    });
  });

  describe("special characters", () => {
    it("handles unicode characters", () => {
      const mermaidText = "flowchart TD\n  A[你好] --> B[Hello 世界]";
      render(<MermaidRenderer mermaidText={mermaidText} />);
      expect(screen.getByTestId("renderer-mermaid")).toBeInTheDocument();
    });

    it("handles HTML entities", () => {
      const mermaidText = "flowchart TD\n  A[&amp;] --> B[&lt;&gt;]";
      render(<MermaidRenderer mermaidText={mermaidText} />);
      expect(screen.getByTestId("renderer-mermaid")).toBeInTheDocument();
    });

    it("handles backticks and special markdown chars", () => {
      const mermaidText = "flowchart TD\n  A[`code`] --> B[```]";
      render(<MermaidRenderer mermaidText={mermaidText} />);
      expect(screen.getByTestId("renderer-mermaid")).toBeInTheDocument();
    });

    it("handles multiline text with various whitespace", () => {
      const mermaidText = `flowchart TD
    A --> B
      C --> D
  `;
      render(<MermaidRenderer mermaidText={mermaidText} />);
      expect(screen.getByTestId("renderer-mermaid")).toBeInTheDocument();
    });
  });

  describe("invalid mermaid handling", () => {
    it("shows fallback for unrecognized text", () => {
      const invalidText = "this is not a mermaid diagram at all";
      render(<MermaidRenderer mermaidText={invalidText} />);
      expect(screen.getByTestId("renderer-mermaid-invalid")).toBeInTheDocument();
      expect(screen.getByText("Invalid or unrecognized mermaid diagram.")).toBeInTheDocument();
    });

    it("shows fallback and original text for partial mermaid", () => {
      const partialText = "just some random text that looks like graph but isn't";
      render(<MermaidRenderer mermaidText={partialText} />);
      expect(screen.getByTestId("renderer-mermaid-invalid")).toBeInTheDocument();
      expect(screen.getByText(partialText)).toBeInTheDocument();
    });
  });

  describe("props", () => {
    it("accepts mermaidText prop", () => {
      const mermaidText = "flowchart TD\n  A --> B";
      render(<MermaidRenderer mermaidText={mermaidText} />);
      expect(screen.getByTestId("renderer-mermaid").textContent).toContain(
        "flowchart TD",
      );
    });

    it("accepts viewKind prop", () => {
      render(<MermaidRenderer mermaidText="flowchart TD\n  A --> B" viewKind="vertical_slice" />);
      expect(screen.getByTestId("renderer-mermaid")).toHaveAttribute(
        "data-view-kind",
        "vertical_slice",
      );
    });

    it("renders without viewKind prop", () => {
      render(<MermaidRenderer mermaidText="flowchart TD\n  A --> B" />);
      expect(screen.getByTestId("renderer-mermaid")).toBeInTheDocument();
      expect(
        screen.getByTestId("renderer-mermaid").getAttribute("data-view-kind"),
      ).toBeNull();
    });
  });
});
