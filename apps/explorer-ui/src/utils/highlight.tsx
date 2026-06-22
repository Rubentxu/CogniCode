/**
 * Syntax highlighting primitives using Prism.
 *
 * Public surface:
 * - tokenizePrism(code, language?)    — pure, no React (in highlight-core.ts)
 * - splitTokensByNewline(tokens, n)   — pure, no React (in highlight-core.ts)
 * - renderTokens(tokens, keyPrefix?)   — React elements, NO dangerouslySetInnerHTML
 * - highlightCode(code, language?)    — convenience wrapper (React)
 *
 * Rules:
 * - highlight-core.ts MUST NOT import React
 * - highlight.tsx (this file) imports React for rendering only
 */
import type { ReactNode } from "react";

import {
  type TokenNode,
  type PrismLanguage,
  tokenizePrism,
  splitTokensByNewline,
  isPrismLanguage,
} from "./highlight-core";

export { type PrismLanguage };

// ============================================================================
// React rendering
// ============================================================================

/**
 * Render a token tree as React span elements.
 *
 * Emits span.token.token-{type} for each token node.
 * Nested children are rendered recursively.
 * NO dangerouslySetInnerHTML — all content is set via React props.
 */
export function renderTokens(
  tokens: TokenNode[],
  keyPrefix = "",
): ReactNode {
  return tokens.map((token, idx) => {
    const key = `${keyPrefix}${idx}`;
    const className =
      token.type === "plaintext"
        ? "token"
        : `token token-${token.type}`;

    if (token.children && token.children.length > 0) {
      return (
        <span key={key} className={className}>
          {renderTokens(token.children, `${key}-`)}
        </span>
      );
    }

    return (
      <span key={key} className={className}>
        {token.content}
      </span>
    );
  });
}

/**
 * Convenience wrapper: tokenize + render as React nodes.
 * Use this when you don't need the intermediate token tree.
 *
 * Returns null for empty code; renders as plaintext for unknown language.
 */
export function highlightCode(
  code: string,
  language?: string,
): ReactNode {
  if (!code) return null;

  // Cast to PrismLanguage if valid
  const lang = isPrismLanguage(language) ? language : undefined;
  const { tokens } = tokenizePrism(code, lang);

  if (tokens.length === 0) {
    return <span>{code}</span>;
  }

  return renderTokens(tokens);
}
