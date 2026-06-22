/**
 * Core syntax highlighting — pure functions, NO React.
 *
 * API surface:
 * - tokenizePrism(code, language?)   — pure, no React
 * - splitTokensByNewline(tokens, n)  — pure, splits token tree at \n boundaries
 *
 * Rules:
 * - This file MUST NOT import React
 * - renderTokens and highlightCode are in highlight.tsx
 */
import type { PrismLanguage } from "./languageDetect";
export { type PrismLanguage };

import Prism from "prismjs";

// Correct dependency order per Prism components.json:
// clike (c-like) → javascript → markup → jsx → typescript → tsx
import "prismjs/components/prism-clike";
import "prismjs/components/prism-markup";
import "prismjs/components/prism-javascript";
import "prismjs/components/prism-jsx";
import "prismjs/components/prism-typescript"; // tsx depends on typescript
import "prismjs/components/prism-tsx";

// Remaining languages (self-contained, no dependencies)
import "prismjs/components/prism-rust";
import "prismjs/components/prism-python";
import "prismjs/components/prism-go";
import "prismjs/components/prism-java";
import "prismjs/components/prism-c";
import "prismjs/components/prism-cpp";
import "prismjs/components/prism-hcl";
import "prismjs/components/prism-yaml";
import "prismjs/components/prism-json";
import "prismjs/components/prism-toml";
import "prismjs/components/prism-bash";
import "prismjs/components/prism-ruby";

// ============================================================================
// Types
// ============================================================================

/**
 * A node in the flattened Prism token tree.
 * `content` is the string text of this token.
 * `children` are the nested tokens (e.g., JSX attributes inside a tag).
 */
export interface TokenNode {
  type: string;
  content: string;
  children?: TokenNode[];
}

/** Result of tokenizing a code string. */
export interface HighlightResult {
  language: PrismLanguage | undefined;
  tokens: TokenNode[];
}

// ============================================================================
// Pure tokenization
// ============================================================================

/**
 * Tokenize `code` using the Prism grammar for `language`.
 *
 * Returns a flat(ish) token tree where nested tokens (e.g., JSX attributes,
 * Rust lifetimes, template string expressions) are preserved in `children`.
 *
 * Returns `{ language, tokens: [] }` when `language` is undefined or unknown.
 */
export function tokenizePrism(
  code: string,
  language?: PrismLanguage,
): HighlightResult {
  if (!code) {
    return { language: language ?? undefined, tokens: [] };
  }

  const grammar = language ? Prism.languages[language] : undefined;
  if (!grammar) {
    return { language: undefined, tokens: [] };
  }

  const rawTokens = Prism.tokenize(code, grammar) as (
    | string
    | { type: string; content: string | unknown[]; alias?: string }
  )[];

  const tokens = rawTokens.map((t) => parseToken(t));

  return { language, tokens };
}

/**
 * Recursively parse a Prism token into our TokenNode format.
 */
function parseToken(
  token: string | { type: string; content: string | unknown[]; alias?: string },
): TokenNode {
  if (typeof token === "string") {
    return { type: "plaintext", content: token };
  }

  const { type, content } = token;
  const node: TokenNode = { type, content: "" };

  if (typeof content === "string") {
    node.content = content;
  } else if (Array.isArray(content)) {
    // Nested token list (e.g., JSX children, Rust attributes)
    node.content = "";
    node.children = content.map((c) => parseToken(c as typeof token));
  }

  return node;
}

// ============================================================================
// Split by newline
// ============================================================================

/**
 * Split the token tree into `lineCount` line buckets.
 *
 * Tokens containing literal \n in their content are SPLIT across lines
 * so that each \n becomes a line boundary. This correctly handles
 * multiline block comments and multiline string literals without
 * fragmenting the token structure.
 *
 * @param tokens  Flattened token tree from tokenizePrism
 * @param lineCount  Number of lines to produce
 */
export function splitTokensByNewline(
  tokens: TokenNode[],
  lineCount: number,
): TokenNode[][] {
  if (lineCount <= 0) return [];
  if (tokens.length === 0) return Array.from({ length: lineCount }, () => []);

  const lines: TokenNode[][] = Array.from({ length: lineCount }, () => []);
  let lineIdx = 0;

  function push(token: TokenNode) {
    if (lineIdx < lines.length) {
      lines[lineIdx].push(token);
    }
  }

  for (const token of tokens) {
    const parts = token.content.split("\n");
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      const isLastPart = i === parts.length - 1;

      if (part) {
        // Clone token for this line segment with updated content
        push({ ...token, content: part });
      }

      // If not the last part, we have hit a \n → advance to next line
      if (!isLastPart) {
        lineIdx++;
        // Clamp to valid range (guard against more \n than lineCount)
        if (lineIdx >= lines.length) lineIdx = lines.length - 1;
      }
    }
  }

  return lines;
}

// ============================================================================
// Internal helpers
// ============================================================================

/** Narrow string to PrismLanguage when it matches our supported set. */
export function isPrismLanguage(s: string | undefined): s is PrismLanguage {
  if (!s) return false;
  return [
    "rust",
    "typescript",
    "tsx",
    "javascript",
    "jsx",
    "python",
    "go",
    "java",
    "c",
    "cpp",
    "hcl",
    "yaml",
    "json",
    "toml",
    "bash",
    "ruby",
  ].includes(s);
}
