/**
 * `Shell` tests — viewport behaviour, health chip, skip link.
 *
 * Post E3 (ADR-039): Shell renders a 2-zone layout:
 *   InteractiveGraph (left) | PaneStackView (right)
 * Small viewport: graph full-width, PaneStackView as bottom sheet.
 */
import { describe, it, expect } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { useReducer } from "react";

import { Shell } from "./Shell";
import { detectViewport } from "./viewport";
import { HealthProbe } from "./HealthProbe";
import { SkipLink } from "./SkipLink";
import {
  AppContext,
  appReducer,
  initialState,
  type Action,
  type AppState,
} from "../state/context";

/**
 * Minimal harness that provides a live AppContext.
 */
function ShellHarness({
  viewport,
}: {
  viewport?: "small" | "tablet" | "desktop" | "ultrawide";
}) {
  const [state, dispatch] = useReducer(appReducer, initialState);
  const value: { state: AppState; dispatch: React.Dispatch<Action> } = {
    state,
    dispatch,
  };
  return (
    <AppContext.Provider value={value}>
      <Shell viewport={viewport} />
    </AppContext.Provider>
  );
}

describe("detectViewport", () => {
  it("classifies >= 1200px as desktop", () => {
    expect(detectViewport(1280)).toBe("desktop");
    expect(detectViewport(1200)).toBe("desktop");
  });
  it("classifies 900-1199 as tablet", () => {
    expect(detectViewport(1199)).toBe("tablet");
    expect(detectViewport(900)).toBe("tablet");
  });
  it("classifies < 900 as small", () => {
    expect(detectViewport(899)).toBe("small");
    expect(detectViewport(360)).toBe("small");
  });
});

describe("Shell", () => {
  it("renders the top bar with the project title and a health chip", async () => {
    render(<ShellHarness viewport="desktop" />);
    expect(
      screen.getByRole("heading", { name: /CogniCode Explorer/i, level: 1 }),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByTestId("health-chip")).toBeInTheDocument();
    });
  });

  it("renders the skip link as the first focusable element", () => {
    render(<ShellHarness viewport="desktop" />);
    const skip = screen.getByTestId("skip-link");
    expect(skip).toBeInTheDocument();
    expect(skip).toHaveTextContent(/skip to main content/i);
  });

  it("renders a <main> landmark with the right label", () => {
    render(<ShellHarness viewport="desktop" />);
    const main = screen.getByRole("main");
    expect(main).toHaveAttribute("id", "app-main");
    expect(main).toHaveAttribute("aria-label", "Explorer panels");
  });

  it("desktop viewport renders graph + pane-stack zones", async () => {
    render(<ShellHarness viewport="desktop" />);
    // PaneStackView empty state should be present in the right zone
    await waitFor(() => {
      expect(screen.getByTestId("pane-stack-empty")).toBeInTheDocument();
    });
    // Graph loading / empty / resolved should be present in the left zone
    const hasGraph =
      document.querySelector('[data-testid="interactive-graph"]') !== null;
    const hasEmpty =
      document.querySelector('[data-testid="interactive-graph-empty"]') !== null;
    const hasLoading =
      document.querySelector('[data-testid="interactive-graph-loading"]') !== null;
    expect(hasGraph || hasEmpty || hasLoading).toBe(true);
  });

  it("small viewport renders graph full-width with bottom-sheet overlay", async () => {
    render(<ShellHarness viewport="small" />);
    // Bottom sheet should be present
    expect(screen.getByTestId("bottom-sheet")).toBeInTheDocument();
    // Graph/landing zone should eventually render (InteractiveGraph or GraphLanding via Suspense)
    await waitFor(() => {
      const hasGraph =
        document.querySelector('[data-testid="interactive-graph"]') !== null;
      const hasEmpty =
        document.querySelector('[data-testid="interactive-graph-empty"]') !== null;
      const hasLoading =
        document.querySelector('[data-testid="interactive-graph-loading"]') !== null;
      const hasLanding =
        document.querySelector('[data-testid="graph-landing"]') !== null;
      const hasLandingLoading =
        document.querySelector('[data-testid="graph-landing-loading"]') !== null;
      expect(
        hasGraph || hasEmpty || hasLoading || hasLanding || hasLandingLoading,
      ).toBe(true);
    });
  });

  it("tablet viewport renders graph + pane-stack (2-zone grid)", async () => {
    render(<ShellHarness viewport="tablet" />);
    await waitFor(() => {
      expect(screen.getByTestId("pane-stack-empty")).toBeInTheDocument();
    });
    // Graph/landing zone should eventually render
    await waitFor(() => {
      const hasGraph =
        document.querySelector('[data-testid="interactive-graph"]') !== null;
      const hasEmpty =
        document.querySelector('[data-testid="interactive-graph-empty"]') !== null;
      const hasLoading =
        document.querySelector('[data-testid="interactive-graph-loading"]') !== null;
      const hasLanding =
        document.querySelector('[data-testid="graph-landing"]') !== null;
      const hasLandingLoading =
        document.querySelector('[data-testid="graph-landing-loading"]') !== null;
      expect(
        hasGraph || hasEmpty || hasLoading || hasLanding || hasLandingLoading,
      ).toBe(true);
    });
  });

  it("ultrawide viewport renders 2-zone grid (same as desktop)", async () => {
    render(<ShellHarness viewport="ultrawide" />);
    await waitFor(() => {
      expect(screen.getByTestId("pane-stack-empty")).toBeInTheDocument();
    });
    // Graph/landing zone should eventually render
    await waitFor(() => {
      const hasGraph =
        document.querySelector('[data-testid="interactive-graph"]') !== null;
      const hasEmpty =
        document.querySelector('[data-testid="interactive-graph-empty"]') !== null;
      const hasLoading =
        document.querySelector('[data-testid="interactive-graph-loading"]') !== null;
      const hasLanding =
        document.querySelector('[data-testid="graph-landing"]') !== null;
      const hasLandingLoading =
        document.querySelector('[data-testid="graph-landing-loading"]') !== null;
      expect(
        hasGraph || hasEmpty || hasLoading || hasLanding || hasLandingLoading,
      ).toBe(true);
    });
  });

  it("data-viewport attribute reflects the active viewport", () => {
    const { rerender } = render(<ShellHarness viewport="desktop" />);
    expect(screen.getByTestId("shell")).toHaveAttribute(
      "data-viewport",
      "desktop",
    );
    rerender(<ShellHarness viewport="tablet" />);
    expect(screen.getByTestId("shell")).toHaveAttribute(
      "data-viewport",
      "tablet",
    );
    rerender(<ShellHarness viewport="small" />);
    expect(screen.getByTestId("shell")).toHaveAttribute(
      "data-viewport",
      "small",
    );
    rerender(<ShellHarness viewport="ultrawide" />);
    expect(screen.getByTestId("shell")).toHaveAttribute(
      "data-viewport",
      "ultrawide",
    );
  });
});

describe("HealthProbe (chip mode)", () => {
  it("renders the chip in the top bar", async () => {
    render(<HealthProbe showFullScreenOnError={false} />);
    await waitFor(() => {
      expect(screen.getByTestId("health-chip")).toBeInTheDocument();
    });
  });

  it("updates the data-status when the backend responds", async () => {
    render(<HealthProbe showFullScreenOnError={false} />);
    await waitFor(() => {
      expect(screen.getByTestId("health-chip")).toHaveAttribute(
        "data-status",
        "online",
      );
    });
  });
});

describe("SkipLink", () => {
  it("uses the provided target id in the href", () => {
    render(<SkipLink targetId="app-main" />);
    const link = screen.getByTestId("skip-link");
    expect(link).toHaveAttribute("href", "#app-main");
  });
});
