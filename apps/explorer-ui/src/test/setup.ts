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

// jsdom does not implement the canvas API: `getContext("2d")` returns null.
// cytoscape refuses to build a renderer without a context and throws
// "Could not create canvas of type 2d" from inside an effect, which React
// surfaces as an unhandled error and which can tear a component down
// mid-test. The stub answers with a context whose methods do nothing: the
// tests assert on the DOM, never on the pixels.
if (typeof window !== "undefined" && window.HTMLCanvasElement) {
  const proto = window.HTMLCanvasElement.prototype as HTMLCanvasElement & {
    __cognicodeCanvasStubbed?: boolean;
  };
  if (!proto.__cognicodeCanvasStubbed) {
    Object.defineProperty(proto, "__cognicodeCanvasStubbed", {
      value: true,
    });
    Object.defineProperty(proto, "getContext", {
      writable: true,
      value: function getContext(this: HTMLCanvasElement, kind: string) {
        if (kind !== "2d") return null;
        const canvas = this;
        const noop = () => undefined;
        return new Proxy(
          { canvas },
          {
            get(target, prop) {
              const bag = target as unknown as Record<string | symbol, unknown>;
              if (prop in bag) return bag[prop];
              if (prop === "measureText") return () => ({ width: 0 });
              if (
                prop === "createLinearGradient" ||
                prop === "createRadialGradient"
              ) {
                return () => ({ addColorStop: noop });
              }
              if (prop === "getImageData") {
                return () => ({
                  data: new Uint8ClampedArray(4),
                  width: 1,
                  height: 1,
                });
              }
              return noop;
            },
            set(target, prop, value) {
              (target as unknown as Record<string | symbol, unknown>)[prop] =
                value;
              return true;
            },
          },
        );
      },
    });
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
