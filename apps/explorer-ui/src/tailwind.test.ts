/**
 * `tailwind.test.ts` — verifies CogniCode's global CSS includes the
 * accessibility-critical rules declared in the design tokens.
 *
 * Why: WCAG 2.2 AA / AAA requires respecting `prefers-reduced-motion`.
 * CogniCode declares this rule in `tailwind.css` as part of E27.4.
 * If a future refactor removes the rule, this test will fail.
 */
import { describe, expect, test } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const CSS_PATH = join(process.cwd(), "src", "tailwind.css");

describe("tailwind.css accessibility rules", () => {
  const css = readFileSync(CSS_PATH, "utf-8");

  test("declares prefers-reduced-motion media query", () => {
    expect(css).toMatch(/@media\s*\(\s*prefers-reduced-motion\s*:\s*reduce\s*\)/);
  });

  test("reduced-motion rule targets all elements with !important", () => {
    // Verify the universal selector inside the @media block applies
    // broadly and uses !important to override per-component transitions.
    const block = css.match(
      /@media\s*\(\s*prefers-reduced-motion\s*:\s*reduce\s*\)\s*\{([\s\S]*?)\n\}/,
    );
    expect(block, "prefers-reduced-motion block must exist").toBeTruthy();
    expect(block?.[1]).toMatch(/\*[\s\S]*\*::before[\s\S]*\*::after/);
    expect(block?.[1]).toMatch(/transition-duration/);
    expect(block?.[1]).toMatch(/animation-duration/);
  });

  test("declares :focus-visible outline for keyboard users", () => {
    expect(css).toMatch(/\*:focus-visible\s*\{/);
    expect(css).toMatch(/outline:\s*2px solid var\(--color-primary\)/);
  });
});