/**
 * Vitest global setup — runs before every test file.
 *
 * - jest-dom matchers
 * - MSW node server (intercepts all SWR fetches)
 * - jsdom matchMedia polyfill (cmdk + others)
 * - RTL auto-cleanup
 */
import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterAll, afterEach, beforeAll } from "vitest";

import { server } from "../mocks/node";

// jsdom does not implement matchMedia — stub it so cmdk/components that
// probe for prefers-* don't crash in unit tests.
if (typeof window !== "undefined" && !window.matchMedia) {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }),
  });
}

// cmdk uses ResizeObserver internally (Radix Dialog measures the
// content). jsdom does not implement it — provide a no-op polyfill
// so unit tests that mount the Spotter don't blow up.
if (typeof window !== "undefined" && !window.ResizeObserver) {
  class ResizeObserverPolyfill {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  Object.defineProperty(window, "ResizeObserver", {
    writable: true,
    value: ResizeObserverPolyfill,
  });
  // Also expose on globalThis in case the module captured `globalThis` directly.
  if (typeof globalThis !== "undefined") {
    (globalThis as { ResizeObserver?: unknown }).ResizeObserver =
      ResizeObserverPolyfill;
  }
}

// MSW lifecycle — start once per test file, reset between tests so
// runtime handlers from one test do not leak to the next.
beforeAll(() => {
  server.listen({ onUnhandledRequest: "bypass" });
});

afterEach(() => {
  cleanup();
  server.resetHandlers();
});

afterAll(() => {
  server.close();
});
