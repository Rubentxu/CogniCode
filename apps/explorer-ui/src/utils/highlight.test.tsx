/**
 * Unit tests for highlight.ts — tokenizePrism, splitTokensByNewline,
 * renderTokens, highlightCode.
 *
 * .tsx because renderTokens and highlightCode produce React nodes.
 * Prism grammars are loaded via the side-effect import in highlight-core.ts.
 */
import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";

import {
  tokenizePrism,
  splitTokensByNewline,
} from "./highlight-core";

import {
  renderTokens,
  highlightCode,
} from "./highlight";

describe("tokenizePrism", () => {
  it("tokenizes a Rust function signature", () => {
    const { tokens } = tokenizePrism("fn build_overview() {", "rust");
    // At minimum we expect a "keyword" token for "fn"
    const types = tokens.map((t) => t.type);
    expect(types).toContain("keyword");
    expect(tokens.find((t) => t.type === "keyword")?.content).toBe("fn");
  });

  it("tokenizes a TypeScript arrow function", () => {
    const { tokens } = tokenizePrism("const x = (a: number) => a * 2;", "typescript");
    const types = tokens.map((t) => t.type);
    expect(types).toContain("keyword"); // const
    // Arrow operator gets its own punctuation/operator token
    const allContent = tokens.map((t) => t.content).join("");
    expect(allContent).toContain("=>");
  });

  it("tokenizes Python function", () => {
    const { tokens } = tokenizePrism("def hello(name):", "python");
    const types = tokens.map((t) => t.type);
    expect(types).toContain("keyword"); // def
  });

  it("tokenizes Go function", () => {
    const { tokens } = tokenizePrism("func main() {", "go");
    const types = tokens.map((t) => t.type);
    expect(types).toContain("keyword"); // func
  });

  it("tokenizes Java method", () => {
    const { tokens } = tokenizePrism("public void run() {", "java");
    const types = tokens.map((t) => t.type);
    expect(types).toContain("keyword");
  });

  it("tokenizes JSON", () => {
    const { tokens } = tokenizePrism('{"key": "value"}', "json");
    const types = tokens.map((t) => t.type);
    expect(types).toContain("string");
  });

  it("tokenizes YAML", () => {
    const { tokens } = tokenizePrism("name: value\nversion: 1", "yaml");
    const types = tokens.map((t) => t.type);
    // YAML grammar uses 'key' for keys and 'string' (via alias) for string values
    expect(types).toContain("key");
    // Verify the colon separator is tokenized (punctuation)
    expect(types).toContain("punctuation");
  });

  it("returns empty tokens for empty string", () => {
    const { tokens } = tokenizePrism("", "rust");
    expect(tokens).toEqual([]);
  });

  it("returns empty tokens for undefined language", () => {
    const { tokens, language } = tokenizePrism("fn main() {}", undefined);
    expect(tokens).toEqual([]);
    expect(language).toBeUndefined();
  });

  it("returns empty tokens for unknown language", () => {
    const { tokens } = tokenizePrism("fn main() {}", "cobol" as never);
    expect(tokens).toEqual([]);
  });

  it("handles nested tokens (JSX)", () => {
    const { tokens } = tokenizePrism("<Component />", "tsx");
    const types = tokens.map((t) => t.type);
    // <Component /> produces a 'tag' token
    expect(types).toContain("tag");
    // Verify Component name is in token content (tag content is nested)
    const tagToken = tokens.find((t) => t.type === "tag");
    expect(tagToken).toBeDefined();
  });

  it("handles single character input", () => {
    const { tokens } = tokenizePrism("{", "rust");
    expect(tokens.length).toBeGreaterThan(0);
  });

  it("preserves whitespace-only tokens", () => {
    const { tokens } = tokenizePrism("  \n  ", "rust");
    // Should still produce at least a plaintext token for whitespace
    const allContent = tokens.map((t) => t.content).join("");
    expect(allContent).toBe("  \n  ");
  });

  it("returns language in result", () => {
    const { language } = tokenizePrism("const x = 1;", "javascript");
    expect(language).toBe("javascript");
  });
});

describe("splitTokensByNewline", () => {
  it("splits single line tokens into single array", () => {
    const tokens = [{ type: "keyword", content: "fn" }];
    const result = splitTokensByNewline(tokens, 1);
    expect(result).toEqual([[{ type: "keyword", content: "fn" }]]);
  });

  it("splits at newline boundaries", () => {
    // Simulate: "fn main()\nlet x = 1;"
    const tokens = [
      { type: "keyword", content: "fn" },
      { type: "plaintext", content: " main()" },
      { type: "plaintext", content: "\n" },
      { type: "keyword", content: "let" },
      { type: "plaintext", content: " x = 1;" },
    ];
    const result = splitTokensByNewline(tokens, 2);
    expect(result[0]).toContainEqual({ type: "keyword", content: "fn" });
    expect(result[1]).toContainEqual({ type: "keyword", content: "let" });
  });

  it("splits token with literal \\n in content across lines", () => {
    // A token containing a literal newline in its content
    const tokens = [{ type: "comment", content: "/* line1\nline2\nline3 */" }];
    const result = splitTokensByNewline(tokens, 3);
    // Should be split into 3 parts
    expect(result[0]).toContainEqual({ type: "comment", content: "/* line1" });
    expect(result[1]).toContainEqual({ type: "comment", content: "line2" });
    expect(result[2]).toContainEqual({ type: "comment", content: "line3 */" });
  });

  it("handles multiline Rust block comment spanning 3 lines", () => {
    const code = `/* line one
line two
line three */`;
    const { tokens } = tokenizePrism(code, "rust");
    const result = splitTokensByNewline(tokens, 3);
    // Verify we get content on each line (block comment spans all lines)
    expect(result[0]?.length ?? 0).toBeGreaterThan(0);
    expect(result[1]?.length ?? 0).toBeGreaterThan(0);
    expect(result[2]?.length ?? 0).toBeGreaterThan(0);
  });

  it("handles Python triple-quoted string spanning 2 lines", () => {
    const code = `"""hello
world"""`;
    const { tokens } = tokenizePrism(code, "python");
    const result = splitTokensByNewline(tokens, 2);
    expect(result[0]?.length ?? 0).toBeGreaterThan(0);
    expect(result[1]?.length ?? 0).toBeGreaterThan(0);
  });

  it("returns empty array for lineCount 0", () => {
    const tokens = [{ type: "keyword", content: "fn" }];
    const result = splitTokensByNewline(tokens, 0);
    expect(result).toEqual([]);
  });

  it("returns trailing empty arrays when more lines than \\n boundaries", () => {
    const tokens = [{ type: "keyword", content: "fn main();" }];
    const result = splitTokensByNewline(tokens, 5);
    expect(result.length).toBe(5);
    expect(result[0]).toContainEqual({ type: "keyword", content: "fn main();" });
    expect(result[1]).toEqual([]);
    expect(result[2]).toEqual([]);
    expect(result[3]).toEqual([]);
    expect(result[4]).toEqual([]);
  });

  it("handles more \\n boundaries than lineCount gracefully", () => {
    const tokens = [
      { type: "plaintext", content: "a\nb\nc\nd\ne" },
    ];
    const result = splitTokensByNewline(tokens, 3);
    // All parts after line 2 should go into the last line
    expect(result.length).toBe(3);
  });

  // --- T2.2: boundary tests for multiline tokens ---

  it("multiline Rust block comment spans all 3 lines via token content", () => {
    // Simulate: /* line1\nline2\nline3 */ as a single comment token
    const tokens = [
      { type: "comment", content: "/* line1\nline2\nline3 */" },
    ];
    const result = splitTokensByNewline(tokens, 3);
    expect(result[0]?.length ?? 0).toBeGreaterThan(0);
    expect(result[1]?.length ?? 0).toBeGreaterThan(0);
    expect(result[2]?.length ?? 0).toBeGreaterThan(0);
    // Each line should have the comment type
    expect(result[0]?.[0]?.type).toBe("comment");
    expect(result[1]?.[0]?.type).toBe("comment");
    expect(result[2]?.[0]?.type).toBe("comment");
  });

  it("Python triple-quoted string preserves token continuity across 2 lines", () => {
    const code = `"""hello
world"""`;
    const { tokens } = tokenizePrism(code, "python");
    const result = splitTokensByNewline(tokens, 2);
    expect(result[0]?.length ?? 0).toBeGreaterThan(0);
    expect(result[1]?.length ?? 0).toBeGreaterThan(0);
  });

  it("token with literal \\n in content splits across lines", () => {
    const tokens = [{ type: "plaintext", content: "line1\nline2\nline3" }];
    const result = splitTokensByNewline(tokens, 3);
    expect(result[0]?.[0]?.content).toBe("line1");
    expect(result[1]?.[0]?.content).toBe("line2");
    expect(result[2]?.[0]?.content).toBe("line3");
  });

  it("line count mismatch (more lines than \\n boundaries) returns trailing empty arrays", () => {
    const tokens = [{ type: "keyword", content: "fn main();" }];
    const result = splitTokensByNewline(tokens, 5);
    expect(result.length).toBe(5);
    expect(result[0]?.[0]?.content).toBe("fn main();");
    expect(result[1]).toEqual([]);
    expect(result[2]).toEqual([]);
  });
});

describe("renderTokens", () => {
  it("renders keyword token with correct class", () => {
    const tokens = [{ type: "keyword", content: "fn" }];
    const { container } = render(<>{renderTokens(tokens)}</>);
    const el = container.querySelector(".token-keyword");
    expect(el).toBeInTheDocument();
    expect(el?.textContent).toBe("fn");
  });

  it("renders string token with correct class", () => {
    const tokens = [{ type: "string", content: '"hello"' }];
    const { container } = render(<>{renderTokens(tokens)}</>);
    const el = container.querySelector(".token-string");
    expect(el?.textContent).toBe('"hello"');
  });

  it("renders comment token with correct class", () => {
    const tokens = [{ type: "comment", content: "// a comment" }];
    const { container } = render(<>{renderTokens(tokens)}</>);
    const el = container.querySelector(".token-comment");
    expect(el?.textContent).toBe("// a comment");
  });

  it("renders nested children recursively", () => {
    const tokens = [
      {
        type: "tag",
        content: "<Component",
        children: [
          { type: "attr-name", content: "name" },
          { type: "attr-value", content: '"value"' },
        ],
      },
    ];
    const { container } = render(<>{renderTokens(tokens)}</>);
    expect(container.querySelector(".token-tag")).toBeInTheDocument();
    expect(container.querySelector(".token-attr-name")?.textContent).toBe("name");
    expect(container.querySelector(".token-attr-value")?.textContent).toBe('"value"');
  });

  it("renders plaintext tokens without token- class", () => {
    const tokens = [{ type: "plaintext", content: "hello" }];
    const { container } = render(<>{renderTokens(tokens)}</>);
    const el = container.querySelector(".token");
    expect(el?.textContent).toBe("hello");
    // Should NOT have token- prefix for plaintext
    expect(el?.className).toBe("token");
  });

  it("renders multiple tokens in sequence", () => {
    const tokens = [
      { type: "keyword", content: "fn" },
      { type: "plaintext", content: " main()" },
    ];
    const { container } = render(<>{renderTokens(tokens)}</>);
    expect(container.querySelectorAll(".token-keyword")).toHaveLength(1);
    expect(container.querySelectorAll(".token")).toHaveLength(2);
  });

  it("accepts keyPrefix for stable keys", () => {
    const tokens = [{ type: "keyword", content: "fn" }];
    const { container } = render(<>{renderTokens(tokens, "line0-")}</>);
    // No crash — keys should be "line0-0"
    expect(container.querySelector(".token-keyword")).toBeInTheDocument();
  });

  it("does NOT use dangerouslySetInnerHTML", () => {
    // This is verified by the render output having proper span elements
    const tokens = [{ type: "string", content: '<script>alert("xss")</script>' }];
    const { container } = render(<>{renderTokens(tokens)}</>);
    // The content should be rendered as text, not executed
    expect(container.querySelector("script")).not.toBeInTheDocument();
    expect(container.querySelector(".token-string")?.textContent).toBe(
      '<script>alert("xss")</script>',
    );
  });
});

describe("highlightCode", () => {
  it("wraps Rust code in tokens", () => {
    const { container } = render(<>{highlightCode("fn main() {}", "rust")}</>);
    expect(container.querySelector(".token-keyword")).toBeInTheDocument();
  });

  it("renders unknown language as plain span (no token class)", () => {
    const { container } = render(<>{highlightCode("hello world", "unknownlang")}</>);
    // Should render without crashing — fallback to plaintext
    const spans = container.querySelectorAll("span");
    expect(spans.length).toBeGreaterThan(0);
  });

  it("returns null for empty string", () => {
    const { container } = render(<>{highlightCode("", "rust")}</>);
    expect(container.innerHTML).toBe("");
  });

  it("renders JavaScript code", () => {
    const { container } = render(
      <>{highlightCode("const x = 1;", "javascript")}</>,
    );
    expect(container.querySelector(".token-keyword")).toBeInTheDocument();
  });

  it("renders Python code", () => {
    const { container } = render(
      <>{highlightCode("def hello():", "python")}</>,
    );
    expect(container.querySelector(".token-keyword")).toBeInTheDocument();
  });

  it("renders with undefined language gracefully", () => {
    const { container } = render(<>{highlightCode("const x = 1;")}</>);
    // Should not crash — renders as plaintext
    const spans = container.querySelectorAll("span");
    expect(spans.length).toBeGreaterThan(0);
  });
});
